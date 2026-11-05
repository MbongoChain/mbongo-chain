//! Memory held by a full mempool of the largest admissible transactions.
//!
//! A counting allocator measures live heap bytes on this thread, so the figure
//! covers every index, not only the encoded transactions. Runs in its own test
//! process. The mempool's own unit tests compile in here too, which is harmless:
//! the counter is per thread.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Instant;

use mbongo_core::{Address, Hash, Receipt, Transaction, TransactionPayload, TransactionType};

#[allow(dead_code)]
#[path = "../src/mempool.rs"]
mod mempool;

use mempool::{Mempool, MempoolConfig, DEFAULT_MAX_PENDING, DEFAULT_TTL_SECS, MAX_PENDING_LIMIT};

thread_local! {
    static LIVE: Cell<isize> = const { Cell::new(0) };
}

struct Counting;

fn count(delta: isize) {
    let _ = LIVE.try_with(|live| live.set(live.get() + delta));
}

#[allow(clippy::cast_possible_wrap)]
// SAFETY: every call forwards to the system allocator with the same arguments.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size() as isize);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count(-(layout.size() as isize));
        System.dealloc(ptr, layout);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count(new_size as isize - layout.size() as isize);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// `MAX_RECEIPT_METADATA_BYTES` in `backend.rs`, the largest field admission
/// allows. No other payload comes close: a `ComputeTask` is at most 1155 bytes.
const LARGEST_METADATA: usize = 4096;

/// Stated bound, per entry, for a full pool of the largest transactions.
const BYTES_PER_ENTRY_BOUND: usize = 6 * 1024;

fn largest_transaction(index: usize) -> (Hash, Transaction) {
    let mut id = [0u8; 32];
    id[..8].copy_from_slice(&(index as u64).to_le_bytes());
    let sender = Address(id);
    let receipt = Receipt {
        version: 1,
        task_id: id,
        input_commitment: [1u8; 32],
        output_commitment: [2u8; 32],
        executor: sender,
        metadata: vec![0xAB; LARGEST_METADATA],
        signature: [3u8; 64],
    };
    let tx = Transaction {
        tx_type: TransactionType::AnchorReceipt,
        sender,
        receiver: Address::zero(),
        amount: 0,
        nonce: 0,
        payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
        signature: [4u8; 64],
    };
    (Hash(id), tx)
}

fn live() -> isize {
    LIVE.with(Cell::get)
}

/// Fills a pool to `cap`, checks the bound, empties it, and returns bytes per entry.
fn measure(cap: usize) -> usize {
    let before = live();
    let mut pool = Mempool::with_config(MempoolConfig::new(cap, DEFAULT_TTL_SECS).unwrap());
    let start = Instant::now();
    for index in 0..cap {
        let (hash, tx) = largest_transaction(index);
        pool.insert_at(hash, tx, start).unwrap();
    }
    pool.assert_consistent();
    let (hash, tx) = largest_transaction(cap);
    assert_eq!(pool.insert(hash, tx), Err(mempool::MempoolError::PoolFull));
    assert_eq!(pool.len(), cap);

    let held = usize::try_from(live() - before).unwrap();
    let per_entry = held / cap;
    println!("cap {cap}: {held} bytes held, {per_entry} per entry");
    assert!(per_entry <= BYTES_PER_ENTRY_BOUND, "{per_entry} per entry");

    let evicted = pool.sweep_expired(
        start + std::time::Duration::from_secs(DEFAULT_TTL_SECS),
        |_| false,
    );
    assert_eq!(evicted.len(), cap);
    assert_eq!(pool.len(), 0);
    per_entry
}

#[test]
fn a_full_pool_stays_within_the_stated_memory_at_each_configured_limit() {
    for cap in [1024, DEFAULT_MAX_PENDING, MAX_PENDING_LIMIT] {
        measure(cap);
    }
}
