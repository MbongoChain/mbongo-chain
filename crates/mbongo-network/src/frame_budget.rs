//! A global budget for inbound frame bytes (S3 A2.2b-lite).
//!
//! Every inbound frame body is read into memory only after its bytes are
//! reserved here, piece by piece, before each piece is allocated. A frame that
//! cannot get its next piece is refused: the read fails with
//! [`std::io::ErrorKind::OutOfMemory`] and the stream is closed. That is a
//! local resource refusal, not a validity rule; a frame refused here is valid
//! and may be accepted later, and the frame format is unchanged.
//!
//! **Categories.** The budget has three pools, so that one kind of traffic
//! cannot take another's room:
//!
//! - **solicited**: answers to requests this node sent (`read_response`). It
//!   protects this node's own catch-up.
//! - **unsolicited**: requests and announcements peers send (`read_request`).
//! - **small**: any frame of at most [`SMALL_FRAME`] bytes, either direction.
//!   Honest requests are this small.
//!
//! **Ownership.** A [`Reservation`] has exactly one owner, the frame being
//! read, and is not `Clone`. It is released when dropped: when the frame has
//! been decoded, or on a decode error, a short read, a timeout, a cancelled
//! stream or a closed connection, all of which drop the reading future. No
//! reservation ever waits: it is granted at once or refused at once.
//!
//! **What this does not bound.** Decoded messages once the frame is dropped,
//! outbound responses, queues, yamux and other transport buffers, and the rest
//! of the process. This is not a process memory bound.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

/// Frames of at most this many bytes use the small pool.
pub const SMALL_FRAME: usize = 1024;

/// Default solicited capacity: two maximum frames, room for this node's one
/// block request in flight and its retry. Experimental, not a protocol value.
pub const SOLICITED_CAPACITY: usize = 32 * 1024 * 1024;

/// Default unsolicited capacity: three maximum frames. Experimental.
pub const UNSOLICITED_CAPACITY: usize = 48 * 1024 * 1024;

/// Default small-frame capacity. A1 allows at most (64 + 16) connections × 6
/// request/response streams, so at most 480 KiB of small frames at once; this
/// leaves twice that.
pub const SMALL_CAPACITY: usize = 1024 * 1024;

/// Which pool a frame draws from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// An answer to a request this node sent.
    Solicited,
    /// A request or announcement a peer sent.
    Unsolicited,
    /// Any frame of at most [`SMALL_FRAME`] bytes.
    Small,
}

#[derive(Debug)]
struct Pool {
    capacity: usize,
    used: AtomicUsize,
    peak: AtomicUsize,
    refusals: AtomicU64,
}

impl Pool {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            refusals: AtomicU64::new(0),
        }
    }

    /// Adds `bytes` if the pool can hold them, atomically; never overshoots.
    fn take(&self, bytes: usize) -> bool {
        let granted = self.used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
            used.checked_add(bytes).filter(|&next| next <= self.capacity)
        });
        if let Ok(before) = granted {
            self.peak.fetch_max(before + bytes, Ordering::Relaxed);
            true
        } else {
            self.refusals.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    fn give_back(&self, bytes: usize) {
        // Only what was taken is given back, once, so this cannot go below 0.
        self.used.fetch_sub(bytes, Ordering::AcqRel);
    }
}

/// The three pools, shared by every codec of a node.
#[derive(Debug)]
pub struct FrameBudget {
    solicited: Pool,
    unsolicited: Pool,
    small: Pool,
}

/// One pool's reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolReading {
    /// Bytes reserved now.
    pub used: usize,
    /// The most bytes ever reserved at once.
    pub peak: usize,
    /// The pool's capacity.
    pub capacity: usize,
    /// Reservations refused since start.
    pub refusals: u64,
}

impl Default for FrameBudget {
    fn default() -> Self {
        Self::new(SOLICITED_CAPACITY, UNSOLICITED_CAPACITY, SMALL_CAPACITY)
    }
}

impl FrameBudget {
    /// A budget with the given capacities, in bytes.
    #[must_use]
    pub fn new(solicited: usize, unsolicited: usize, small: usize) -> Self {
        Self {
            solicited: Pool::new(solicited),
            unsolicited: Pool::new(unsolicited),
            small: Pool::new(small),
        }
    }

    fn pool(&self, category: Category) -> &Pool {
        match category {
            Category::Solicited => &self.solicited,
            Category::Unsolicited => &self.unsolicited,
            Category::Small => &self.small,
        }
    }

    /// An empty reservation in `category`, to be grown piece by piece.
    #[must_use]
    pub fn reservation(self: &Arc<Self>, category: Category) -> Reservation {
        Reservation {
            budget: Arc::clone(self),
            category,
            bytes: 0,
        }
    }

    /// The reading of one pool.
    #[must_use]
    pub fn reading(&self, category: Category) -> PoolReading {
        let pool = self.pool(category);
        PoolReading {
            used: pool.used.load(Ordering::Acquire),
            peak: pool.peak.load(Ordering::Relaxed),
            capacity: pool.capacity,
            refusals: pool.refusals.load(Ordering::Relaxed),
        }
    }
}

impl std::fmt::Display for FrameBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for (name, category) in [
            ("solicited", Category::Solicited),
            ("unsolicited", Category::Unsolicited),
            ("small", Category::Small),
        ] {
            let r = self.reading(category);
            if !first {
                f.write_str(" ")?;
            }
            first = false;
            write!(
                f,
                "frame_budget_{name}_used={} frame_budget_{name}_peak={} \
                 frame_budget_{name}_capacity={} frame_budget_{name}_refusals={}",
                r.used, r.peak, r.capacity, r.refusals
            )?;
        }
        Ok(())
    }
}

/// Bytes reserved for one frame. Not `Clone`: it has one owner, and dropping
/// it gives every byte back exactly once.
#[derive(Debug)]
pub struct Reservation {
    budget: Arc<FrameBudget>,
    category: Category,
    bytes: usize,
}

impl Reservation {
    /// Reserves `bytes` more, or refuses without changing anything.
    ///
    /// # Errors
    ///
    /// Returns an [`std::io::ErrorKind::OutOfMemory`] error when the pool
    /// cannot hold them.
    pub fn grow(&mut self, bytes: usize) -> std::io::Result<()> {
        if self.budget.pool(self.category).take(bytes) {
            self.bytes += bytes;
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::OutOfMemory,
                format!("inbound frame budget exhausted ({:?})", self.category),
            ))
        }
    }

    /// Bytes this reservation holds.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        if self.bytes > 0 {
            self.budget.pool(self.category).give_back(self.bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(capacity: usize) -> Arc<FrameBudget> {
        Arc::new(FrameBudget::new(capacity, capacity, capacity))
    }

    #[test]
    fn a_reservation_grows_to_the_capacity_and_no_further() {
        let budget = budget(100);
        let mut r = budget.reservation(Category::Unsolicited);
        r.grow(60).unwrap();
        r.grow(40).unwrap();
        let err = r.grow(1).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::OutOfMemory);
        assert_eq!(r.bytes(), 100, "a refusal changes nothing");
        let reading = budget.reading(Category::Unsolicited);
        assert_eq!(
            (reading.used, reading.peak, reading.refusals),
            (100, 100, 1)
        );
        drop(r);
        assert_eq!(budget.reading(Category::Unsolicited).used, 0);
    }

    #[test]
    fn pools_are_separate() {
        let budget = budget(10);
        let mut a = budget.reservation(Category::Unsolicited);
        a.grow(10).unwrap();
        let mut b = budget.reservation(Category::Solicited);
        b.grow(10).unwrap();
        let mut c = budget.reservation(Category::Small);
        c.grow(10).unwrap();
        assert!(a.grow(1).is_err());
        assert_eq!(budget.reading(Category::Solicited).refusals, 0);
    }

    #[test]
    fn an_overflowing_request_is_refused_not_wrapped() {
        let budget = budget(usize::MAX);
        let mut a = budget.reservation(Category::Unsolicited);
        a.grow(usize::MAX - 1).unwrap();
        assert!(a.grow(2).is_err());
        assert_eq!(budget.reading(Category::Unsolicited).used, usize::MAX - 1);
    }

    #[test]
    fn concurrent_reservations_never_overcommit_and_all_return() {
        const CAPACITY: usize = 64 * 1024;
        let budget = budget(CAPACITY);
        let granted = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..16 {
                scope.spawn(|| {
                    for _ in 0..2_000 {
                        let mut r = budget.reservation(Category::Unsolicited);
                        for _ in 0..8 {
                            if r.grow(1024).is_ok() {
                                granted.fetch_add(1, Ordering::Relaxed);
                            }
                            let used = budget.reading(Category::Unsolicited).used;
                            assert!(used <= CAPACITY, "overcommitted: {used}");
                        }
                    }
                });
            }
        });
        let reading = budget.reading(Category::Unsolicited);
        assert_eq!(reading.used, 0, "every reservation returned");
        assert!(reading.peak <= CAPACITY);
        assert!(granted.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn the_reading_shows_every_pool() {
        let budget = budget(8);
        let text = budget.to_string();
        for name in ["solicited", "unsolicited", "small"] {
            assert!(
                text.contains(&format!("frame_budget_{name}_capacity=8")),
                "{text}"
            );
        }
    }
}
