//! A crash-safe, encrypted, append-only journal.
//!
//! **Guarantee.** [`Journal::append`] returns only after the record is
//! flushed to the device, so an acknowledged record survives a process crash
//! and a power loss, as far as the device honours the flush.
//!
//! **Atomicity.** One record is one transition. A crash during an append
//! leaves a partial last frame, which was never acknowledged and is dropped
//! on open. Each frame header carries its own check, so a damaged length can
//! never pass for such a tail: any other damage is [`JournalError::Corrupt`],
//! and the caller must refuse to start.
//!
//! **Confidentiality and integrity.** Records are sealed with
//! ChaCha20-Poly1305, bound to the journal's generation and sequence number,
//! so frames cannot be altered, reordered or moved between generations.
//!
//! **Not protected:** a party with write access to the disk can roll the
//! whole file back to an older copy. The key and the file sit on the same
//! provider's disk. Exclusive ownership is [`super::lock::StateLock`].

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

const MAGIC: &[u8; 4] = b"MBJ1";
const FILE_HEADER: usize = 12;
const FRAME_HEADER: usize = 16;
const NONCE: usize = 12;
const TAG: usize = 16;
/// Largest sealed record.
pub const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;

/// Why a journal could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JournalError {
    /// The device or filesystem refused.
    #[error("STATE_IO: {0}")]
    Io(String),
    /// The journal is damaged, or sealed with another key.
    #[error("STATE_CORRUPT: {0}")]
    Corrupt(&'static str),
    /// An earlier write failed: nothing more is accepted until a restart.
    #[error("STATE_UNAVAILABLE")]
    Unavailable,
    /// Another process owns this state directory.
    #[error("STATE_LOCKED")]
    Locked,
    /// A record is larger than [`MAX_RECORD_BYTES`].
    #[error("STATE_RECORD_TOO_LARGE")]
    TooLarge,
}

fn io_error(e: &io::Error) -> JournalError {
    JournalError::Io(format!("{:?}", e.kind()))
}

/// The journal file.
pub struct Journal {
    path: PathBuf,
    file: File,
    cipher: ChaCha20Poly1305,
    generation: u64,
    next_seq: u64,
    bytes: u64,
    poisoned: bool,
}

/// The header's own check: a damaged length is never read as a torn tail.
fn header_check(generation: u64, seq: u64, len: u32) -> [u8; 4] {
    let mut h = blake3::Hasher::new();
    h.update(MAGIC);
    h.update(&generation.to_le_bytes());
    h.update(&seq.to_le_bytes());
    h.update(&len.to_le_bytes());
    let digest = h.finalize();
    let mut out = [0u8; 4];
    out.copy_from_slice(&digest.as_bytes()[..4]);
    out
}

fn aad(generation: u64, seq: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&generation.to_le_bytes());
    out[8..].copy_from_slice(&seq.to_le_bytes());
    out
}

#[cfg_attr(not(unix), allow(clippy::unnecessary_wraps))]
fn sync_dir(path: &Path) -> Result<(), JournalError> {
    // A directory cannot be flushed this way on Windows: a rename there is
    // durable only as far as the filesystem journals its metadata.
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        File::open(dir).and_then(|d| d.sync_all()).map_err(|e| io_error(&e))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn seal(
    cipher: &ChaCha20Poly1305,
    generation: u64,
    seq: u64,
    nonce: [u8; NONCE],
    payload: &[u8],
) -> Result<Vec<u8>, JournalError> {
    if payload.len() > MAX_RECORD_BYTES {
        return Err(JournalError::TooLarge);
    }
    let aad = aad(generation, seq);
    let sealed = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: payload,
                aad: &aad,
            },
        )
        .map_err(|_| JournalError::Io("seal".to_owned()))?;
    let len = u32::try_from(NONCE + sealed.len()).map_err(|_| JournalError::TooLarge)?;
    let mut frame = Vec::with_capacity(FRAME_HEADER + NONCE + sealed.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(&seq.to_le_bytes());
    frame.extend_from_slice(&header_check(generation, seq, len));
    frame.extend_from_slice(&nonce);
    frame.extend_from_slice(&sealed);
    Ok(frame)
}

fn file_header(generation: u64) -> [u8; FILE_HEADER] {
    let mut h = [0u8; FILE_HEADER];
    h[..4].copy_from_slice(MAGIC);
    h[4..].copy_from_slice(&generation.to_le_bytes());
    h
}

impl Journal {
    /// Opens or creates the journal, returning it and every acknowledged
    /// record in order. A partial last frame is removed. Anything else that
    /// does not verify is an error: the caller must not start.
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<(Self, Vec<Vec<u8>>), JournalError> {
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
        // A rewrite that never reached its rename left only a stale copy.
        let _ = fs::remove_file(Self::rewrite_path(path));
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(path)
            .map_err(|e| io_error(&e))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| io_error(&e))?;
        if data.is_empty() {
            let generation = 1;
            file.write_all(&file_header(generation))
                .and_then(|()| file.sync_data())
                .map_err(|e| io_error(&e))?;
            sync_dir(path)?;
            let journal = Self {
                path: path.to_path_buf(),
                file,
                cipher,
                generation,
                next_seq: 0,
                bytes: FILE_HEADER as u64,
                poisoned: false,
            };
            return Ok((journal, Vec::new()));
        }
        if data.len() < FILE_HEADER || &data[..4] != MAGIC {
            return Err(JournalError::Corrupt("file header"));
        }
        let generation = u64::from_le_bytes(
            data[4..FILE_HEADER]
                .try_into()
                .map_err(|_| JournalError::Corrupt("file header"))?,
        );
        let (records, end) = Self::read_frames(&data, &cipher, generation)?;
        if end < data.len() {
            // Through its own handle: an append-only handle cannot truncate
            // on Windows.
            OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| f.set_len(end as u64).and_then(|()| f.sync_all()))
                .map_err(|e| io_error(&e))?;
        }
        file.seek(SeekFrom::End(0)).map_err(|e| io_error(&e))?;
        let journal = Self {
            path: path.to_path_buf(),
            file,
            cipher,
            generation,
            next_seq: records.len() as u64,
            bytes: end as u64,
            poisoned: false,
        };
        Ok((journal, records))
    }

    fn read_frames(
        data: &[u8],
        cipher: &ChaCha20Poly1305,
        generation: u64,
    ) -> Result<(Vec<Vec<u8>>, usize), JournalError> {
        let mut records = Vec::new();
        let mut at = FILE_HEADER;
        let mut seq = 0u64;
        while data.len() - at >= FRAME_HEADER {
            let word = |r: std::ops::Range<usize>| &data[at + r.start..at + r.end];
            let len = u32::from_le_bytes(word(0..4).try_into().unwrap_or_default());
            let frame_seq = u64::from_le_bytes(word(4..12).try_into().unwrap_or_default());
            if word(12..16) != header_check(generation, frame_seq, len) {
                return Err(JournalError::Corrupt("frame header"));
            }
            if frame_seq != seq {
                return Err(JournalError::Corrupt("sequence"));
            }
            let body = len as usize;
            if !(NONCE + TAG..=MAX_RECORD_BYTES + NONCE + TAG).contains(&body) {
                return Err(JournalError::Corrupt("frame length"));
            }
            if data.len() - at - FRAME_HEADER < body {
                // Announced by a valid header, never completed: a torn tail.
                return Ok((records, at));
            }
            let start = at + FRAME_HEADER;
            let aad = aad(generation, seq);
            let payload = cipher
                .decrypt(
                    Nonce::from_slice(&data[start..start + NONCE]),
                    Payload {
                        msg: &data[start + NONCE..start + body],
                        aad: &aad,
                    },
                )
                .map_err(|_| JournalError::Corrupt("record"))?;
            records.push(payload);
            at = start + body;
            seq += 1;
        }
        Ok((records, at))
    }

    fn rewrite_path(path: &Path) -> PathBuf {
        path.with_extension("rewrite")
    }

    /// Appends a record and flushes it. After a failure the journal refuses
    /// everything until a restart: a later success could otherwise follow a
    /// partial frame.
    pub fn append(&mut self, payload: &[u8], nonce: [u8; NONCE]) -> Result<(), JournalError> {
        if self.poisoned {
            return Err(JournalError::Unavailable);
        }
        let frame = seal(&self.cipher, self.generation, self.next_seq, nonce, payload)?;
        let written = self.file.write_all(&frame).and_then(|()| self.file.sync_data());
        if let Err(e) = written {
            self.poisoned = true;
            return Err(io_error(&e));
        }
        self.next_seq += 1;
        self.bytes += frame.len() as u64;
        Ok(())
    }

    /// Replaces the journal with `records`, atomically: a crash leaves either
    /// the old file or the new one, never a mix.
    pub fn rewrite(
        &mut self,
        records: &[Vec<u8>],
        mut nonce: impl FnMut() -> [u8; NONCE],
    ) -> Result<(), JournalError> {
        if self.poisoned {
            return Err(JournalError::Unavailable);
        }
        let generation = self.generation + 1;
        let tmp = Self::rewrite_path(&self.path);
        let mut body = file_header(generation).to_vec();
        for (seq, record) in records.iter().enumerate() {
            let frame = seal(&self.cipher, generation, seq as u64, nonce(), record)?;
            body.extend_from_slice(&frame);
        }
        let attempt = (|| {
            let mut out = File::create(&tmp)?;
            out.write_all(&body)?;
            out.sync_all()
        })();
        if let Err(e) = attempt {
            let _ = fs::remove_file(&tmp);
            return Err(io_error(&e));
        }
        // The open handle is replaced first: Windows refuses to rename over
        // an open file. On failure the journal is poisoned, not half-switched.
        let placeholder = File::open(&tmp).map_err(|e| io_error(&e))?;
        drop(std::mem::replace(&mut self.file, placeholder));
        if let Err(e) = fs::rename(&tmp, &self.path) {
            self.poisoned = true;
            return Err(io_error(&e));
        }
        sync_dir(&self.path).map_err(|e| {
            self.poisoned = true;
            e
        })?;
        self.file = OpenOptions::new().append(true).open(&self.path).map_err(|e| {
            self.poisoned = true;
            io_error(&e)
        })?;
        self.generation = generation;
        self.next_seq = records.len() as u64;
        self.bytes = body.len() as u64;
        Ok(())
    }

    /// Current size of the journal.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Makes every later write fail, as a failed device would.
    #[doc(hidden)]
    pub fn poison(&mut self) {
        self.poisoned = true;
    }
}
