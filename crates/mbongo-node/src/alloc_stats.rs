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

/// The fields of `/proc/self/smaps_rollup` that attribute resident memory,
/// in KiB, as `key=value` pairs; empty where the file is unavailable.
// Used by the Linux GNU diagnostics; elsewhere only by its own test.
#[cfg_attr(not(all(target_os = "linux", target_env = "gnu")), allow(dead_code))]
pub fn smaps_rollup(text: &str) -> String {
    const KEYS: [&str; 6] = [
        "Rss",
        "Pss",
        "Anonymous",
        "Private_Dirty",
        "Shared_Clean",
        "Swap",
    ];
    KEYS.iter()
        .filter_map(|key| {
            text.lines().find_map(|line| {
                let value = line.strip_prefix(key)?.strip_prefix(':')?;
                let kib = value.split_whitespace().next()?;
                Some(format!("smaps_{}_kib={kib}", key.to_lowercase()))
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// glibc's own view of its heap, and a one-off trim (S3 A2.2c attribution).
/// Linux GNU only, and only in `alloc-stats` builds.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub mod glibc {
    /// `mallinfo2()`, in bytes. It sums glibc's arenas.
    #[derive(Debug, Clone, Copy)]
    pub struct MallInfo {
        /// Bytes obtained from the system with `brk`/`sbrk`, all arenas.
        pub arena: usize,
        /// Bytes in mmapped chunks.
        pub hblkhd: usize,
        /// Bytes allocated and in use.
        pub uordblks: usize,
        /// Bytes free in the arenas, held for reuse.
        pub fordblks: usize,
        /// Releasable bytes at the top of the main arena.
        pub keepcost: usize,
    }

    /// Reads glibc's allocator statistics.
    pub fn mallinfo() -> MallInfo {
        // SAFETY: `mallinfo2` takes no arguments, has no preconditions, and
        // returns a plain struct by value (glibc 2.33 and later).
        let m = unsafe { libc::mallinfo2() };
        MallInfo {
            arena: m.arena,
            hblkhd: m.hblkhd,
            uordblks: m.uordblks,
            fordblks: m.fordblks,
            keepcost: m.keepcost,
        }
    }

    /// Asks glibc to give free memory back to the system; true if it did.
    pub fn trim() -> bool {
        // SAFETY: `malloc_trim(0)` has no preconditions; it only releases
        // free memory glibc holds and keeps every live allocation valid.
        unsafe { libc::malloc_trim(0) == 1 }
    }

    impl std::fmt::Display for MallInfo {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "mallinfo_arena={} mallinfo_hblkhd={} mallinfo_uordblks={} \
                 mallinfo_fordblks={} mallinfo_keepcost={}",
                self.arena, self.hblkhd, self.uordblks, self.fordblks, self.keepcost
            )
        }
    }

    /// Logs memory as Rust, glibc and the kernel see it, trims once, and logs
    /// it again. For a single explicit trigger, never a periodic one.
    pub fn trim_and_report() {
        let rollup = || {
            std::fs::read_to_string("/proc/self/smaps_rollup")
                .map(|text| super::smaps_rollup(&text))
                .unwrap_or_default()
        };
        let process = || mbongo_network::proc_memory::ProcessMemory::read().unwrap_or_default();
        log::info!(
            "malloc-diag before: {} {} {} {}",
            super::snapshot(),
            process(),
            mallinfo(),
            rollup()
        );
        let released = trim();
        log::info!(
            "malloc-diag after: {} {} {} {} trim_released={released}",
            super::snapshot(),
            process(),
            mallinfo(),
            rollup()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smaps_rollup_keeps_the_attribution_fields_in_kib() {
        let text = "00400000-7ffd00000000 ---p 00000000 00:00 0  [rollup]\n\
            Rss:              435600 kB\nPss:              430000 kB\n\
            Shared_Clean:       2600 kB\nPrivate_Dirty:    433000 kB\n\
            Anonymous:        433000 kB\nSwap:                  0 kB\nRssx: 1 kB\n";
        assert_eq!(
            smaps_rollup(text),
            "smaps_rss_kib=435600 smaps_pss_kib=430000 smaps_anonymous_kib=433000 \
             smaps_private_dirty_kib=433000 smaps_shared_clean_kib=2600 smaps_swap_kib=0"
        );
        assert_eq!(smaps_rollup(""), "");
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[test]
    fn glibc_reports_and_a_trim_keeps_live_data_intact() {
        let held = vec![7u8; 4 * 1024 * 1024];
        let info = glibc::mallinfo();
        assert!(info.arena + info.hblkhd > 0);
        let _ = glibc::trim();
        assert!(held.iter().all(|&b| b == 7), "a trim frees nothing live");
    }

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
