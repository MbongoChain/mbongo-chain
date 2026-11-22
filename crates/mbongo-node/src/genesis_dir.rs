//! Which genesis a data directory belongs to (S1-4, RFC 0006 §1).
//!
//! Storage schema 3 has no place for the chain identity: that arrives with
//! schema 4 in S1-8. Until then the node keeps an exact copy of the genesis
//! file it was created from in a marker file inside the data directory,
//! next to the RocksDB files, and checks it on every start. The RocksDB
//! layout is unchanged.
//!
//! # Marker
//!
//! `MBONGO_GENESIS`, written once and never rewritten:
//!
//! ```text
//! mbongo-genesis-marker:v1\n
//! chain_id <64 lowercase hex>\n
//! <the genesis file bytes, exactly>
//! ```
//!
//! It is trustworthy only if the bytes are a valid genesis file and the
//! `chain_id` line is the `chain_id` of those bytes. It matches the selected
//! genesis only if the bytes are identical.
//!
//! # Start sequence
//!
//! 1. **Before RocksDB opens the directory**, classify what is in it:
//!    - absent, empty, or holding only `MBONGO_GENESIS.tmp`: new;
//!    - holding `MBONGO_GENESIS`: verify it (corrupt → refuse, other genesis
//!      → refuse);
//!    - anything else: refuse. A directory with data and no marker was not
//!      created by this code (a v0.4 directory, another tool) and cannot be
//!      shown to belong to the selected genesis.
//!
//!    Nothing is opened, migrated or written in a refused directory.
//! 2. New: write the marker atomically (`.tmp`, `sync_all`, then a hard
//!    link, which fails rather than replace an existing marker).
//! 3. Open RocksDB.
//! 4. Block 0 present: it must be the genesis block the marker's genesis
//!    derives. Block 0 absent: the database must hold only the schema stamp
//!    that opening writes, which is exactly the state an initialization
//!    interrupted between steps 2 and 5 leaves; anything else is refused.
//! 5. The caller writes the genesis in one `write_batch`
//!    (`NodeBackend::ensure_genesis`).
//!
//! Every crash point between these steps leaves a state that step 1 or 4
//! recognises, and the tests below produce each one with the real steps.
//!
//! # S1-8
//!
//! Schema 4 stores `chain_id` and the genesis hash in the database, in the
//! genesis `write_batch`, and refuses every directory below schema 4. A
//! schema-3 directory with a marker is therefore refused by S1-8 like any
//! other v0.4 or S1-4 directory: there is no migration, and the marker is no
//! longer read.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use mbongo_core::genesis::{DEV_GENESIS_FILE, MAX_GENESIS_FILE_BYTES};
use mbongo_core::{chain_id, ChainId, Genesis};
use mbongo_storage::{RocksDbStorage, Storage};

use crate::backend::{compute_block_hash, genesis_block};

/// The marker file, inside the data directory.
pub(crate) const MARKER_FILE: &str = "MBONGO_GENESIS";
/// The marker while it is written; never trusted.
pub(crate) const MARKER_TMP: &str = "MBONGO_GENESIS.tmp";
/// First line of every marker.
const MARKER_MAGIC: &[u8] = b"mbongo-genesis-marker:v1\n";
/// Second line prefix.
const CHAIN_ID_PREFIX: &[u8] = b"chain_id ";

/// The genesis the operator selected, with its exact bytes.
pub(crate) struct SelectedGenesis {
    /// Where it came from, for the start banner.
    pub source: String,
    /// The file bytes, which the marker records.
    pub bytes: Vec<u8>,
    /// The parsed file.
    pub genesis: Genesis,
}

impl SelectedGenesis {
    /// The built-in dev genesis (`--dev`).
    pub(crate) fn dev() -> Self {
        Self {
            source: "dev (built in)".to_string(),
            bytes: DEV_GENESIS_FILE.to_vec(),
            genesis: Genesis::dev(),
        }
    }

    /// A genesis file (`--genesis <file>`), refused unless it is a valid
    /// `mbongo-genesis:v1` file.
    pub(crate) fn from_file(path: &Path) -> Result<Self, String> {
        let file = File::open(path)
            .map_err(|e| format!("cannot read genesis file {}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        // One byte over the bound, so that an oversized file is refused by
        // the parser as TooLarge instead of being read whole.
        file.take(MAX_GENESIS_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("cannot read genesis file {}: {e}", path.display()))?;
        let genesis = Genesis::from_file_bytes(&bytes)
            .map_err(|e| format!("invalid genesis file {}: {e}", path.display()))?;
        Ok(Self {
            source: path.display().to_string(),
            bytes,
            genesis,
        })
    }
}

/// How the data directory was found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Opened {
    /// It was new: the marker was written now.
    Created,
    /// It held a valid marker for this genesis and only the schema stamp:
    /// an earlier initialization was interrupted before its genesis batch.
    Resumed,
    /// It already holds this chain.
    Reopened,
}

/// Why a data directory was refused. In every case nothing in it changed.
#[derive(Debug)]
pub(crate) enum DirError {
    Io(String),
    /// Data and no marker.
    Unverifiable {
        entries: Vec<String>,
    },
    /// A marker that is not a valid record of a genesis file.
    MarkerCorrupt(String),
    /// A valid marker of another genesis.
    Mismatch {
        stored: ChainId,
        selected: ChainId,
    },
    /// A marker appeared while this start was writing its own.
    MarkerExists,
    /// A valid marker, and a database that is neither this chain nor an
    /// interrupted initialization of it.
    Inconsistent(String),
    Storage(String),
}

impl std::fmt::Display for DirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Unverifiable { entries } => write!(
                f,
                "the data directory holds data but no {MARKER_FILE} marker ({}), so it cannot be \
                 shown to belong to the selected genesis; it was not opened or modified. Use an \
                 empty directory",
                entries.join(", ")
            ),
            Self::MarkerCorrupt(e) => write!(
                f,
                "the {MARKER_FILE} marker of the data directory is not trustworthy ({e}); the \
                 directory was not opened or modified"
            ),
            Self::Mismatch { stored, selected } => write!(
                f,
                "the data directory belongs to chain {stored}, not to the selected genesis \
                 (chain {selected}); it was not opened or modified"
            ),
            Self::MarkerExists => write!(
                f,
                "a {MARKER_FILE} marker appeared while this node was creating one; refusing to \
                 replace it"
            ),
            Self::Inconsistent(e) => write!(f, "the data directory is inconsistent: {e}"),
            Self::Storage(e) => write!(f, "storage error: {e}"),
        }
    }
}

impl std::error::Error for DirError {}

fn io_err(what: &str, path: &Path, e: io::Error) -> DirError {
    DirError::Io(format!("cannot {what} {}: {e}", path.display()))
}

/// What step 1 found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DirState {
    New,
    Marked,
}

/// Step 1: what the directory holds, read without opening it.
pub(crate) fn classify(dir: &Path) -> Result<DirState, DirError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(DirState::New),
        Err(e) => return Err(io_err("read the data directory", dir, e)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| io_err("read the data directory", dir, e))?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    if names.iter().any(|n| n == MARKER_FILE) {
        return Ok(DirState::Marked);
    }
    let data: Vec<String> = names.into_iter().filter(|n| n != MARKER_TMP).collect();
    if data.is_empty() {
        Ok(DirState::New)
    } else {
        let mut entries: Vec<String> = data.iter().take(5).cloned().collect();
        if data.len() > 5 {
            entries.push(format!("and {} more", data.len() - 5));
        }
        Err(DirError::Unverifiable { entries })
    }
}

/// The marker bytes for a genesis file.
pub(crate) fn marker_bytes(genesis_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(MARKER_MAGIC.len() + 74 + genesis_bytes.len());
    out.extend_from_slice(MARKER_MAGIC);
    out.extend_from_slice(CHAIN_ID_PREFIX);
    out.extend_from_slice(chain_id(genesis_bytes).to_string().as_bytes());
    out.push(b'\n');
    out.extend_from_slice(genesis_bytes);
    out
}

/// The genesis bytes a marker records, if it is trustworthy.
pub(crate) fn parse_marker(raw: &[u8]) -> Result<&[u8], String> {
    let rest = raw
        .strip_prefix(MARKER_MAGIC)
        .ok_or("it does not start with the marker header")?;
    let rest = rest.strip_prefix(CHAIN_ID_PREFIX).ok_or("no chain_id line")?;
    if rest.len() < 65 || rest[64] != b'\n' {
        return Err("malformed chain_id line".to_string());
    }
    let (recorded, genesis_bytes) = (&rest[..64], &rest[65..]);
    Genesis::from_file_bytes(genesis_bytes)
        .map_err(|e| format!("the genesis it records is invalid: {e}"))?;
    if recorded != chain_id(genesis_bytes).to_string().as_bytes() {
        return Err("its chain_id is not the chain_id of the genesis it records".to_string());
    }
    Ok(genesis_bytes)
}

/// Step 1, for a marked directory: the marker must be trustworthy and
/// record exactly the selected genesis.
pub(crate) fn verify_marker(dir: &Path, selected: &SelectedGenesis) -> Result<(), DirError> {
    let path = dir.join(MARKER_FILE);
    let limit = (MARKER_MAGIC.len() + CHAIN_ID_PREFIX.len() + 65 + MAX_GENESIS_FILE_BYTES) as u64;
    let mut raw = Vec::new();
    File::open(&path)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut raw))
        .map_err(|e| io_err("read", &path, e))?;
    if raw.len() as u64 > limit {
        return Err(DirError::MarkerCorrupt(
            "it is larger than any marker".to_string(),
        ));
    }
    let stored = parse_marker(&raw).map_err(DirError::MarkerCorrupt)?;
    if stored != selected.bytes.as_slice() {
        return Err(DirError::Mismatch {
            stored: chain_id(stored),
            selected: selected.genesis.chain_id,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> io::Result<()> {
    // Windows has no directory handle to flush; NTFS journals the link.
    Ok(())
}

/// Step 2: write the marker of a new directory, atomically and without
/// replacing one that exists.
pub(crate) fn write_marker(dir: &Path, selected: &SelectedGenesis) -> Result<(), DirError> {
    fs::create_dir_all(dir).map_err(|e| io_err("create the data directory", dir, e))?;
    if let Some(parent) = dir.parent().filter(|p| !p.as_os_str().is_empty()) {
        sync_dir(parent).map_err(|e| io_err("sync", parent, e))?;
    }
    let tmp: PathBuf = dir.join(MARKER_TMP);
    let marker = dir.join(MARKER_FILE);
    {
        let mut file = File::create(&tmp).map_err(|e| io_err("create", &tmp, e))?;
        file.write_all(&marker_bytes(&selected.bytes))
            .and_then(|()| file.sync_all())
            .map_err(|e| io_err("write", &tmp, e))?;
    }
    match fs::hard_link(&tmp, &marker) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Err(DirError::MarkerExists),
        Err(e) => return Err(io_err("create", &marker, e)),
    }
    sync_dir(dir).map_err(|e| io_err("sync", dir, e))?;
    remove_tmp(dir)
}

/// The `.tmp` copy is never read; once a marker exists it is dropped.
fn remove_tmp(dir: &Path) -> Result<(), DirError> {
    let tmp = dir.join(MARKER_TMP);
    match fs::remove_file(&tmp) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_err("remove", &tmp, e)),
    }
}

/// Step 4: with the marker verified and RocksDB open, the database must be
/// this chain, or an initialization of it interrupted before its batch.
pub(crate) fn check_database(
    storage: &RocksDbStorage,
    selected: &SelectedGenesis,
) -> Result<bool, DirError> {
    let storage_err = |e: mbongo_storage::StorageError| DirError::Storage(e.to_string());
    match storage.get_block_by_height(0).map_err(storage_err)? {
        Some(stored) => {
            let expected = compute_block_hash(&genesis_block(&selected.genesis));
            if compute_block_hash(&stored) != expected {
                return Err(DirError::Inconsistent(format!(
                    "its genesis block is not the one chain {} derives",
                    selected.genesis.chain_id
                )));
            }
            Ok(true)
        }
        None => {
            if !storage.holds_only_schema_stamp().map_err(storage_err)? {
                return Err(DirError::Inconsistent(
                    "it has no genesis block but holds chain data, so it is not an interrupted \
                     initialization"
                        .to_string(),
                ));
            }
            Ok(false)
        }
    }
}

/// Steps 1 to 4. On success the caller writes the genesis
/// (`NodeBackend::ensure_genesis`), which is a no-op for
/// [`Opened::Reopened`].
pub(crate) fn open(
    dir: &Path,
    selected: &SelectedGenesis,
) -> Result<(RocksDbStorage, Opened), DirError> {
    let created = match classify(dir)? {
        DirState::New => {
            write_marker(dir, selected)?;
            true
        }
        DirState::Marked => {
            verify_marker(dir, selected)?;
            remove_tmp(dir)?;
            false
        }
    };
    let storage = RocksDbStorage::open(dir).map_err(|e| DirError::Storage(e.to_string()))?;
    let opened = match (check_database(&storage, selected)?, created) {
        (true, _) => Opened::Reopened,
        (false, true) => Opened::Created,
        (false, false) => Opened::Resumed,
    };
    Ok((storage, opened))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NodeBackend;
    use mbongo_core::Address;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A fresh path under the temp dir, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "mbongo-genesis-dir-{name}-{}-{n}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Every file under `dir` with its bytes: what "nothing changed" means.
    fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                out.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    fs::read(&path).unwrap(),
                );
            }
        }
        out
    }

    /// Another valid genesis: the dev file with timestamp 1.
    fn other_genesis() -> SelectedGenesis {
        let mut bytes = DEV_GENESIS_FILE.to_vec();
        let at = bytes.len() - 2;
        bytes[at] = b'1';
        let genesis = Genesis::from_file_bytes(&bytes).unwrap();
        SelectedGenesis {
            source: "other".to_string(),
            bytes,
            genesis,
        }
    }

    /// A full start: steps 1 to 5.
    fn start(dir: &Path, selected: &SelectedGenesis) -> Result<Opened, DirError> {
        let (storage, opened) = open(dir, selected)?;
        let backend = NodeBackend::new(storage, false);
        backend
            .ensure_genesis(&selected.genesis)
            .map_err(|e| DirError::Storage(e.to_string()))?;
        Ok(opened)
    }

    fn dev_account() -> Address {
        Genesis::dev().allocations[1].0
    }

    /// The state of a started chain: height, genesis block, dev accounts.
    fn chain_state(dir: &Path) -> (u64, mbongo_core::Hash, Vec<Option<mbongo_core::Account>>) {
        let storage = RocksDbStorage::open(dir).unwrap();
        let block = storage.get_block_by_height(0).unwrap().unwrap();
        let accounts = Genesis::dev()
            .allocations
            .iter()
            .map(|(a, _)| storage.get_account(a).unwrap())
            .collect();
        (
            storage.get_latest_height().unwrap(),
            compute_block_hash(&block),
            accounts,
        )
    }

    #[test]
    fn the_marker_round_trips_and_binds_the_chain_id() {
        let raw = marker_bytes(DEV_GENESIS_FILE);
        assert!(raw.starts_with(MARKER_MAGIC));
        assert_eq!(parse_marker(&raw).unwrap(), DEV_GENESIS_FILE);
        let line = format!("chain_id {}\n", Genesis::dev().chain_id);
        assert_eq!(
            &raw[MARKER_MAGIC.len()..MARKER_MAGIC.len() + line.len()],
            line.as_bytes()
        );
    }

    #[test]
    fn a_new_directory_is_created_and_initialized() {
        let dir = TempDir::new("new");
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Created
        );
        assert!(dir.path().join(MARKER_FILE).is_file());
        assert!(!dir.path().join(MARKER_TMP).exists());
        let (height, _, accounts) = chain_state(dir.path());
        assert_eq!(height, 0);
        let balances: Vec<u128> = accounts.iter().map(|a| a.as_ref().unwrap().balance).collect();
        assert_eq!(balances, vec![0, 1_000_000_000]);
        assert_eq!(
            fs::read(dir.path().join(MARKER_FILE)).unwrap(),
            marker_bytes(DEV_GENESIS_FILE)
        );
    }

    #[test]
    fn an_existing_empty_directory_is_new() {
        let dir = TempDir::new("empty");
        fs::create_dir_all(dir.path()).unwrap();
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Created
        );
    }

    #[test]
    fn repeated_restarts_with_the_same_genesis_reopen_and_write_nothing() {
        let dir = TempDir::new("restart");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        let first = chain_state(dir.path());
        let marker = fs::read(dir.path().join(MARKER_FILE)).unwrap();
        for _ in 0..3 {
            assert_eq!(
                start(dir.path(), &SelectedGenesis::dev()).unwrap(),
                Opened::Reopened
            );
            assert_eq!(chain_state(dir.path()), first);
            assert_eq!(fs::read(dir.path().join(MARKER_FILE)).unwrap(), marker);
        }
    }

    #[test]
    fn a_chain_with_blocks_reopens_without_its_genesis_written_again() {
        let dir = TempDir::new("live");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        {
            // The live chain spends from the dev account.
            let storage = RocksDbStorage::open(dir.path()).unwrap();
            let mut account = storage.get_account(&dev_account()).unwrap().unwrap();
            account.balance = 5;
            storage.put_account(&dev_account(), &account).unwrap();
        }
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Reopened
        );
        let storage = RocksDbStorage::open(dir.path()).unwrap();
        assert_eq!(
            storage.get_account(&dev_account()).unwrap().unwrap().balance,
            5
        );
    }

    #[test]
    fn another_genesis_is_refused_and_the_directory_is_unchanged() {
        let dir = TempDir::new("mismatch");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        let before = snapshot(dir.path());
        let other = other_genesis();
        match start(dir.path(), &other) {
            Err(DirError::Mismatch { stored, selected }) => {
                assert_eq!(stored, Genesis::dev().chain_id);
                assert_eq!(selected, other.genesis.chain_id);
            }
            r => panic!("expected a mismatch, got {r:?}"),
        }
        assert_eq!(snapshot(dir.path()), before);
        // And the right genesis still opens it.
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Reopened
        );
    }

    #[test]
    fn a_database_without_a_marker_is_refused_and_not_opened() {
        // A v0.4 directory: RocksDB, the code-defined genesis block (the
        // same block as the dev genesis: timestamp 0) and the dev account,
        // but no marker.
        let dir = TempDir::new("v04");
        {
            let backend = NodeBackend::new(RocksDbStorage::open(dir.path()).unwrap(), false);
            backend.ensure_genesis(&Genesis::dev()).unwrap();
        }
        let before = snapshot(dir.path());
        for selected in [SelectedGenesis::dev(), other_genesis()] {
            match start(dir.path(), &selected) {
                Err(DirError::Unverifiable { entries }) => assert!(!entries.is_empty()),
                r => panic!("expected unverifiable, got {r:?}"),
            }
            assert_eq!(
                snapshot(dir.path()),
                before,
                "the refused directory changed"
            );
        }
    }

    #[test]
    fn any_foreign_file_without_a_marker_is_refused() {
        let dir = TempDir::new("foreign");
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        fs::write(dir.path().join(MARKER_TMP), b"partial").unwrap();
        let before = snapshot(dir.path());
        assert!(matches!(
            start(dir.path(), &SelectedGenesis::dev()),
            Err(DirError::Unverifiable { .. })
        ));
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn corrupt_markers_are_refused_and_the_directory_is_unchanged() {
        let good = marker_bytes(DEV_GENESIS_FILE);
        let header = MARKER_MAGIC.len() + CHAIN_ID_PREFIX.len();
        let mut flipped_genesis = good.clone();
        let last = flipped_genesis.len() - 2;
        flipped_genesis[last] = b'1'; // still a valid genesis, other chain_id
        let mut flipped_id = good.clone();
        flipped_id[header] = if good[header] == b'0' { b'1' } else { b'0' };
        let mut no_newline = good.clone();
        no_newline[header + 64] = b' ';
        let mut invalid_genesis = good.clone();
        invalid_genesis.push(b' ');
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("empty", Vec::new()),
            ("truncated header", good[..10].to_vec()),
            ("truncated genesis", good[..good.len() - 1].to_vec()),
            (
                "wrong magic",
                [b"mbongo-genesis-marker:v2\n", &good[MARKER_MAGIC.len()..]].concat(),
            ),
            ("genesis byte changed", flipped_genesis),
            ("chain_id changed", flipped_id),
            ("chain_id line malformed", no_newline),
            ("trailing byte", invalid_genesis),
            ("oversized", vec![b'x'; MAX_GENESIS_FILE_BYTES + 200]),
        ];
        assert_eq!(cases.len(), 9);
        for (name, bytes) in cases {
            let dir = TempDir::new("corrupt");
            start(dir.path(), &SelectedGenesis::dev()).unwrap();
            fs::write(dir.path().join(MARKER_FILE), &bytes).unwrap();
            let before = snapshot(dir.path());
            match start(dir.path(), &SelectedGenesis::dev()) {
                Err(DirError::MarkerCorrupt(_)) => {}
                r => panic!("{name}: expected a corrupt marker, got {r:?}"),
            }
            assert_eq!(
                snapshot(dir.path()),
                before,
                "{name}: the directory changed"
            );
        }
    }

    #[test]
    fn the_marker_is_never_replaced() {
        let dir = TempDir::new("noreplace");
        fs::create_dir_all(dir.path()).unwrap();
        let other = other_genesis();
        fs::write(dir.path().join(MARKER_FILE), marker_bytes(&other.bytes)).unwrap();
        // Another start had won the race: step 2 must not overwrite it.
        assert!(matches!(
            write_marker(dir.path(), &SelectedGenesis::dev()),
            Err(DirError::MarkerExists)
        ));
        assert_eq!(
            fs::read(dir.path().join(MARKER_FILE)).unwrap(),
            marker_bytes(&other.bytes)
        );
    }

    // ── Crash at each initialization boundary ──────────────────────────
    //
    // Each case runs the real steps up to a boundary, stops there as a
    // crash would, then starts normally.

    #[test]
    fn crash_while_writing_the_marker_restarts_as_new() {
        // Boundary 1: only a partial .tmp exists.
        let dir = TempDir::new("crash1");
        fs::create_dir_all(dir.path()).unwrap();
        let partial = &marker_bytes(DEV_GENESIS_FILE)[..20];
        fs::write(dir.path().join(MARKER_TMP), partial).unwrap();
        assert_eq!(classify(dir.path()).unwrap(), DirState::New);
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Created
        );
        assert_eq!(chain_state(dir.path()).0, 0);
        assert!(!dir.path().join(MARKER_TMP).exists());
    }

    #[test]
    fn crash_after_linking_the_marker_resumes() {
        // Boundary 2: marker and .tmp both present, nothing else.
        let dir = TempDir::new("crash2");
        fs::create_dir_all(dir.path()).unwrap();
        let raw = marker_bytes(DEV_GENESIS_FILE);
        fs::write(dir.path().join(MARKER_TMP), &raw).unwrap();
        fs::hard_link(dir.path().join(MARKER_TMP), dir.path().join(MARKER_FILE)).unwrap();
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Resumed
        );
        assert!(!dir.path().join(MARKER_TMP).exists());
        assert_eq!(chain_state(dir.path()).0, 0);
    }

    #[test]
    fn crash_before_opening_the_database_resumes() {
        // Boundary 3: step 2 completed, RocksDB never opened.
        let dir = TempDir::new("crash3");
        write_marker(dir.path(), &SelectedGenesis::dev()).unwrap();
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Resumed
        );
    }

    #[test]
    fn crash_before_the_genesis_batch_resumes() {
        // Boundary 4: steps 1 to 4 completed, the batch never written.
        let dir = TempDir::new("crash4");
        let (storage, opened) = open(dir.path(), &SelectedGenesis::dev()).unwrap();
        assert_eq!(opened, Opened::Created);
        assert!(storage.get_block_by_height(0).unwrap().is_none());
        drop(storage);
        assert_eq!(
            start(dir.path(), &SelectedGenesis::dev()).unwrap(),
            Opened::Resumed
        );
        let (height, block, accounts) = chain_state(dir.path());
        assert_eq!(height, 0);
        assert_eq!(block, compute_block_hash(&genesis_block(&Genesis::dev())));
        assert!(accounts.iter().all(Option::is_some));
    }

    #[test]
    fn an_interrupted_directory_resumes_only_for_its_own_genesis() {
        let dir = TempDir::new("crash4-other");
        drop(open(dir.path(), &SelectedGenesis::dev()).unwrap());
        let before = snapshot(dir.path());
        assert!(matches!(
            start(dir.path(), &other_genesis()),
            Err(DirError::Mismatch { .. })
        ));
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn a_marker_without_a_genesis_block_but_with_data_is_refused() {
        // Not an interrupted initialization: something wrote chain data
        // without the genesis batch. Never resumed.
        let dir = TempDir::new("nogenesis");
        let (storage, _) = open(dir.path(), &SelectedGenesis::dev()).unwrap();
        let mut account = mbongo_core::Account::new(dev_account());
        account.balance = 1;
        storage.put_account(&dev_account(), &account).unwrap();
        drop(storage);
        match start(dir.path(), &SelectedGenesis::dev()) {
            Err(DirError::Inconsistent(e)) => assert!(e.contains("not an interrupted"), "{e}"),
            r => panic!("expected inconsistent, got {r:?}"),
        }
        let storage = RocksDbStorage::open(dir.path()).unwrap();
        assert!(storage.get_block_by_height(0).unwrap().is_none());
        assert_eq!(
            storage.get_account(&dev_account()).unwrap().unwrap().balance,
            1
        );
    }

    #[test]
    fn a_marker_with_another_genesis_block_is_refused() {
        // The marker says dev, the database holds the genesis block of
        // another file. Both are trustworthy, together they are not.
        let dir = TempDir::new("otherblock");
        write_marker(dir.path(), &SelectedGenesis::dev()).unwrap();
        {
            let backend = NodeBackend::new(RocksDbStorage::open(dir.path()).unwrap(), false);
            backend.ensure_genesis(&other_genesis().genesis).unwrap();
        }
        match start(dir.path(), &SelectedGenesis::dev()) {
            Err(DirError::Inconsistent(e)) => assert!(e.contains("genesis block"), "{e}"),
            r => panic!("expected inconsistent, got {r:?}"),
        }
    }

    #[test]
    fn rocksdb_accepts_the_marker_next_to_its_files() {
        // The marker lives in the RocksDB directory: RocksDB must open,
        // write and reopen with it present, and leave it untouched.
        let dir = TempDir::new("coexist");
        start(dir.path(), &SelectedGenesis::dev()).unwrap();
        for _ in 0..2 {
            let storage = RocksDbStorage::open(dir.path()).unwrap();
            storage.put_block_height_index(7, mbongo_core::Hash([7u8; 32])).unwrap();
        }
        assert_eq!(
            fs::read(dir.path().join(MARKER_FILE)).unwrap(),
            marker_bytes(DEV_GENESIS_FILE)
        );
    }

    #[test]
    fn a_genesis_file_is_read_and_validated() {
        let dir = TempDir::new("file");
        fs::create_dir_all(dir.path()).unwrap();
        let good = dir.path().join("g.json");
        fs::write(&good, DEV_GENESIS_FILE).unwrap();
        let selected = SelectedGenesis::from_file(&good).unwrap();
        assert_eq!(selected.genesis.chain_id, Genesis::dev().chain_id);
        assert_eq!(selected.bytes, DEV_GENESIS_FILE);

        let bad = dir.path().join("bad.json");
        fs::write(&bad, [DEV_GENESIS_FILE, b"\n"].concat()).unwrap();
        let err = SelectedGenesis::from_file(&bad).err().unwrap();
        assert!(err.contains("invalid genesis file"), "{err}");

        let missing = dir.path().join("missing.json");
        let err = SelectedGenesis::from_file(&missing).err().unwrap();
        assert!(err.contains("cannot read genesis file"), "{err}");

        let large = dir.path().join("large.json");
        fs::write(&large, vec![b' '; MAX_GENESIS_FILE_BYTES + 10]).unwrap();
        let err = SelectedGenesis::from_file(&large).err().unwrap();
        assert!(err.contains("invalid genesis file"), "{err}");
    }
}
