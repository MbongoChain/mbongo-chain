//! Exclusive ownership of a state directory.
//!
//! The operating system holds the lock, not a PID file: it is released when
//! the handle closes, on a normal shutdown and when the process dies, however
//! it dies. The file itself is never deleted. The handle is not inherited, so
//! a runner orphaned by a crash cannot keep the lock.
//!
//! Local filesystems only: advisory locks over a network filesystem are not
//! reliable, and such a directory is not supported.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;

use super::journal::JournalError;

const LOCK_FILE: &str = "provider-state.lock";

/// Held for as long as the state directory is owned.
#[derive(Debug)]
pub struct StateLock {
    _file: File,
}

#[cfg(unix)]
fn acquire(path: &Path) -> io::Result<Option<File>> {
    use std::os::fd::AsRawFd;
    use std::os::raw::c_int;

    const LOCK_EX: c_int = 2;
    const LOCK_NB: c_int = 4;
    extern "C" {
        fn flock(fd: c_int, operation: c_int) -> c_int;
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    // SAFETY: `flock` reads no memory, and the descriptor is open for the
    // whole call. The lock belongs to this open file description.
    if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
        return Ok(Some(file));
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::WouldBlock {
        Ok(None)
    } else {
        Err(error)
    }
}

#[cfg(windows)]
fn acquire(path: &Path) -> io::Result<Option<File>> {
    use std::os::windows::fs::OpenOptionsExt;

    const ERROR_SHARING_VIOLATION: i32 = 32;
    // A share mode of 0: no other handle can open this file while this one
    // is open.
    let opened = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(path);
    match opened {
        Ok(file) => Ok(Some(file)),
        Err(e) if e.raw_os_error() == Some(ERROR_SHARING_VIOLATION) => Ok(None),
        Err(e) => Err(e),
    }
}

impl StateLock {
    /// Takes the lock on `dir`, without waiting. Nothing else in the
    /// directory is touched: on failure the state stays exactly as it was.
    pub fn acquire(dir: &Path) -> Result<Self, JournalError> {
        match acquire(&dir.join(LOCK_FILE)) {
            Ok(Some(file)) => Ok(Self { _file: file }),
            Ok(None) => Err(JournalError::Locked),
            Err(e) => Err(JournalError::Io(format!("{:?}", e.kind()))),
        }
    }
}
