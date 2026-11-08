//! S3 A1: a frame's buffer follows the bytes that arrive, not the length a
//! peer claims.
//!
//! A peer may send a length prefix of 16 MiB (the frozen maximum, Lock v0.4
//! §9) and then nothing, or a trickle. This test measures, with a counting
//! allocator, how much memory the codec holds while such a frame is pending.
//! It has its own test binary, and a single test, so nothing else allocates
//! while it measures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::AsyncRead;
use libp2p::request_response::Codec;
use mbongo_network::{BlockNotifyCodec, BLOCK_NOTIFY_PROTOCOL};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::SeqCst) + layout.size();
            PEAK.fetch_max(now, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            // The old and new blocks may coexist during the move: count both.
            let now = LIVE.fetch_add(new_size, Ordering::SeqCst) + new_size;
            PEAK.fetch_max(now, Ordering::SeqCst);
            LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MIB: usize = 1024 * 1024;
/// The frozen maximum frame (Lock v0.4 §9).
const MAX_FRAME: usize = 16 * MIB;

/// Bytes the peer has sent so far; a read past them waits.
struct Feed {
    data: Vec<u8>,
    sent: usize,
    read: usize,
}

#[derive(Clone)]
struct Wire(Arc<Mutex<Feed>>);

impl AsyncRead for Wire {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        out: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut feed = self.0.lock().unwrap();
        let n = out.len().min(feed.sent - feed.read);
        if n == 0 {
            return Poll::Pending;
        }
        let start = feed.read;
        out[..n].copy_from_slice(&feed.data[start..start + n]);
        feed.read += n;
        Poll::Ready(Ok(n))
    }
}

#[test]
fn a_pending_frame_holds_memory_in_proportion_to_what_arrived() {
    let claimed = u32::try_from(MAX_FRAME).unwrap();
    let mut data = claimed.to_le_bytes().to_vec();
    data.resize(4 + MAX_FRAME, 0xAB);
    let feed = Arc::new(Mutex::new(Feed {
        data,
        sent: 0,
        read: 0,
    }));
    let mut wire = Wire(Arc::clone(&feed));
    let mut codec = BlockNotifyCodec;
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);

    let base = LIVE.load(Ordering::SeqCst);
    let mut read = codec.read_request(&BLOCK_NOTIFY_PROTOCOL, &mut wire);

    // (bytes of the frame body sent so far, most the codec may hold)
    let steps = [
        (1024, 128 * 1024),
        (MIB, MIB + 128 * 1024),
        (4 * MIB, 4 * MIB + 128 * 1024),
    ];
    let mut measured = 0;
    for (body, bound) in steps {
        feed.lock().unwrap().sent = 4 + body;
        PEAK.store(LIVE.load(Ordering::SeqCst), Ordering::SeqCst);
        assert!(
            read.as_mut().poll(&mut cx).is_pending(),
            "the frame is not complete"
        );
        let peak = PEAK.load(Ordering::SeqCst).saturating_sub(base);
        println!("body received {body:>8} B: peak held {peak:>9} B (bound {bound} B)");
        assert!(
            peak <= bound,
            "{peak} B held after {body} B of a frame claiming {MAX_FRAME} B"
        );
        measured += 1;
    }
    assert_eq!(measured, steps.len());
    drop(read);
    assert!(
        LIVE.load(Ordering::SeqCst) <= base + 64 * 1024,
        "dropping the read releases its buffer"
    );
}
