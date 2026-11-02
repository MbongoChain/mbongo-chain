//! Time as an injected dependency.
//!
//! Every expiry in this crate — session, lease, capability, object — is
//! decided against a [`Clock`], so a test can move time deterministically
//! instead of sleeping. Seconds since the Unix epoch, matching the block
//! header timestamp the chain already uses.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// A source of "now", in seconds.
pub trait Clock: Send + Sync {
    /// Seconds since the Unix epoch.
    fn now(&self) -> u64;
}

/// The system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
    }
}

/// A clock that only moves when a test moves it.
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    now: Arc<AtomicU64>,
}

impl ManualClock {
    /// A manual clock starting at `start`.
    pub fn starting_at(start: u64) -> Self {
        Self {
            now: Arc::new(AtomicU64::new(start)),
        }
    }

    /// Moves the clock forward by `secs`.
    pub fn advance(&self, secs: u64) {
        self.now.fetch_add(secs, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }
}
