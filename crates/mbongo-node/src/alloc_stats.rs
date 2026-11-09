//! Heap allocation counters for diagnostic builds (S3 A2.2a).
//!
//! Compiled only with the `alloc-stats` feature. It wraps the system allocator
//! and counts what passes through it, so that live heap bytes can be compared
//! with what the kernel reports:
//!
//! - **live**: bytes allocated and not yet freed, as the program sees them.
//! - **kernel resident anonymous memory** (from `/proc`): live bytes, plus what
//!   the allocator keeps for reuse after a free, plus allocator overhead.
//!
//! The difference between the two is memory the program no longer uses but
//! the allocator has not returned. That is retention, not a leak; a leak would
//! show as **live** bytes that keep growing.
//!
//! The counting itself costs a few atomic operations per allocation, which is
//! its observer effect; a diagnostic build is never an operated node.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// The system allocator, counted.
pub struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static FREES: AtomicU64 = AtomicU64::new(0);

fn grew(size: usize) {
    let now = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

fn shrank(size: usize) {
    LIVE.fetch_sub(size, Ordering::Relaxed);
}

// SAFETY: every method forwards to `System` with the caller's arguments
// unchanged and returns its result; the counters only observe.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract for `alloc` is passed on unchanged.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            grew(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: as for `alloc`.
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            grew(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract for `dealloc` is passed on unchanged.
        unsafe { System.dealloc(ptr, layout) };
        FREES.fetch_add(1, Ordering::Relaxed);
        shrank(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller's contract for `realloc` is passed on unchanged.
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            if new_size >= layout.size() {
                grew(new_size - layout.size());
            } else {
                shrank(layout.size() - new_size);
            }
        }
        new
    }
}

/// One reading of the counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    /// Bytes allocated and not yet freed.
    pub live_bytes: usize,
    /// The most bytes ever live at once.
    pub peak_live_bytes: usize,
    /// Allocations since start.
    pub allocations: u64,
    /// Frees since start.
    pub frees: u64,
}

/// Reads the counters.
pub fn snapshot() -> Snapshot {
    Snapshot {
        live_bytes: LIVE.load(Ordering::Relaxed),
        peak_live_bytes: PEAK.load(Ordering::Relaxed),
        allocations: ALLOCATIONS.load(Ordering::Relaxed),
        frees: FREES.load(Ordering::Relaxed),
    }
}

impl std::fmt::Display for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "live_bytes={} peak_live_bytes={} allocations={} frees={}",
            self.live_bytes, self.peak_live_bytes, self.allocations, self.frees
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With the feature on, this binary's allocator is `Counting`: a large
    /// allocation is seen live while held and gone once freed.
    #[test]
    fn a_held_allocation_is_live_and_a_freed_one_is_not() {
        const SIZE: usize = 64 * 1024 * 1024;
        let before = snapshot();
        let buffer = vec![1u8; SIZE];
        let held = snapshot();
        assert!(
            held.live_bytes >= before.live_bytes + SIZE / 2,
            "{held} vs {before}"
        );
        assert!(held.peak_live_bytes >= SIZE);
        assert!(held.allocations > before.allocations);
        drop(buffer);
        let after = snapshot();
        assert!(after.frees > held.frees);
        assert!(
            after.live_bytes + SIZE / 2 <= held.live_bytes,
            "{after} vs {held}"
        );
    }
}
