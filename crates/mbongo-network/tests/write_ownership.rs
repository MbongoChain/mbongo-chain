//! S3 A2.2d-1: what a block costs to broadcast and an answer costs to write,
//! measured with a counting allocator. One test, in its own binary, so that
//! nothing else allocates while it measures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

use futures::AsyncWrite;
use libp2p::request_response::Codec;
use mbongo_core::{
    Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionPayload, TransactionType,
};
use mbongo_network::{Announcement, SyncCodec, SyncNotification, SyncResponse, SYNC_PROTOCOL};
use parity_scale_codec::Encode;

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size(), Ordering::SeqCst);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            LIVE.fetch_add(new_size, Ordering::SeqCst);
            LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn live() -> usize {
    LIVE.load(Ordering::SeqCst)
}

fn block(height: u64, transfers: u64) -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash::zero(),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: 1_700_000_000,
            height,
        },
        body: BlockBody {
            transactions: (0..transfers)
                .map(|nonce| Transaction {
                    tx_type: TransactionType::Transfer,
                    sender: Address([1u8; 32]),
                    receiver: Address([2u8; 32]),
                    amount: 1,
                    nonce,
                    payload: TransactionPayload::None,
                    signature: [0u8; 64],
                })
                .collect(),
        },
    }
}

/// Takes the 4-byte length prefix, then never more: a write in flight.
struct StallsAfterHeader {
    taken: usize,
}

impl AsyncWrite for StallsAfterHeader {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.taken >= 4 {
            return Poll::Pending;
        }
        let n = buf.len().min(4 - self.taken);
        self.taken += n;
        Poll::Ready(Ok(n))
    }
    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

const PEERS: usize = 8;
const SLACK: usize = 64 * 1024;

#[test]
fn broadcasts_share_one_encoding_and_answers_are_freed_before_their_write() {
    // ── A broadcast to 8 peers ───────────────────────────────────────────
    let produced = block(7, 6_000);
    let encoded_len = SyncNotification::NewBlock {
        block: produced.clone(),
    }
    .encode()
    .len();

    let before = live();
    let shared = Announcement::new_block(&produced);
    let requests: Vec<Announcement> = (0..PEERS).map(|_| shared.clone()).collect();
    let shared_cost = live() - before;
    drop((shared, requests));

    let before = live();
    let cloned: Vec<SyncNotification> = (0..PEERS)
        .map(|_| SyncNotification::NewBlock {
            block: produced.clone(),
        })
        .collect();
    let per_peer_cost = live() - before;
    drop(cloned);

    println!(
        "broadcast to {PEERS} peers: shared {shared_cost} B, \
         one clone per peer {per_peer_cost} B, one encoding {encoded_len} B"
    );
    assert!(
        shared_cost <= encoded_len + SLACK,
        "one encoding for every peer: {shared_cost} B for a {encoded_len} B announcement"
    );
    assert!(
        per_peer_cost >= 4 * shared_cost,
        "the clone-per-peer form costs several times more: {per_peer_cost} vs {shared_cost}"
    );

    // ── An answer of about 1 MiB, mid-write ──────────────────────────────
    let start = live();
    let answer = SyncResponse::Blocks(vec![(Hash([5u8; 32]), block(8, 6_000))]);
    let decoded_cost = live() - start;
    let encoded = answer.encode().len();
    let mut codec = SyncCodec::default();
    let mut writer = StallsAfterHeader { taken: 0 };
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut write = codec.write_response(&SYNC_PROTOCOL, &mut writer, answer);
    assert!(
        write.as_mut().poll(&mut cx).is_pending(),
        "the write is in flight"
    );
    let in_flight = live() - start;
    drop(write);
    let after = live() - start;

    println!(
        "answer mid-write: {in_flight} B held (decoded {decoded_cost} B, encoded {encoded} B), \
         {after} B after the write is dropped"
    );
    assert!(
        in_flight <= encoded + SLACK,
        "only the encoding is held while writing: {in_flight} B, encoded {encoded} B"
    );
    assert!(
        after <= SLACK,
        "everything is freed once the write is dropped: {after} B"
    );
}
