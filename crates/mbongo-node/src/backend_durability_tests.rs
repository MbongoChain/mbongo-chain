//! Durability of commits (RFC 0006 §3; storage schema 4, plan S1-8).
//!
//! What these tests hold the node to:
//! - the genesis, with the chain identity, is one synced batch;
//! - a block this node produces is committed synced, and only after that
//!   commit returns is it broadcast;
//! - a failed synced commit is neither broadcast nor reported as produced,
//!   and leaves the height where it was;
//! - a block received from a peer is committed buffered, as before.
//!
//! The store below records each write and each broadcast, in order, in one
//! log, and can be told to fail synced writes.

use super::tests::dev_producer;
use super::*;
use mbongo_storage::{ChainIdentity, Durability, InMemoryStorage, StorageError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

type Log = Arc<Mutex<Vec<String>>>;

/// An in-memory store that logs every batch with its durability and the
/// heights it indexes, and fails synced batches while `fail_synced` is set.
struct Recording {
    inner: InMemoryStorage,
    log: Log,
    fail_synced: Arc<AtomicBool>,
}

impl Recording {
    fn new(log: &Log) -> (Self, Arc<AtomicBool>) {
        let fail = Arc::new(AtomicBool::new(false));
        let store = Self {
            inner: InMemoryStorage::new(),
            log: Arc::clone(log),
            fail_synced: Arc::clone(&fail),
        };
        (store, fail)
    }
}

impl Storage for Recording {
    fn get_account(&self, a: &Address) -> Result<Option<Account>, StorageError> {
        self.inner.get_account(a)
    }
    fn put_account(&self, a: &Address, account: &Account) -> Result<(), StorageError> {
        self.inner.put_account(a, account)
    }
    fn get_block(&self, h: &Hash) -> Result<Option<Block>, StorageError> {
        self.inner.get_block(h)
    }
    fn put_block(&self, h: &Hash, b: &Block) -> Result<(), StorageError> {
        self.inner.put_block(h, b)
    }
    fn get_transaction(&self, h: &Hash) -> Result<Option<Transaction>, StorageError> {
        self.inner.get_transaction(h)
    }
    fn put_transaction(&self, h: &Hash, tx: &Transaction) -> Result<(), StorageError> {
        self.inner.put_transaction(h, tx)
    }
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        self.inner.get_block_by_height(height)
    }
    fn put_block_height_index(&self, height: u64, h: Hash) -> Result<(), StorageError> {
        self.inner.put_block_height_index(height, h)
    }
    fn get_latest_height(&self) -> Result<u64, StorageError> {
        self.inner.get_latest_height()
    }
    fn next_tx_seq(&self) -> Result<u64, StorageError> {
        self.inner.next_tx_seq()
    }
    fn put_tx_seq_index(&self, seq: u64, h: &Hash) -> Result<(), StorageError> {
        self.inner.put_tx_seq_index(seq, h)
    }
    fn get_tx_hash_by_seq(&self, seq: u64) -> Result<Option<Hash>, StorageError> {
        self.inner.get_tx_hash_by_seq(seq)
    }
    fn get_last_included_tx_seq(&self) -> Result<u64, StorageError> {
        self.inner.get_last_included_tx_seq()
    }
    fn set_last_included_tx_seq(&self, seq: u64) -> Result<(), StorageError> {
        self.inner.set_last_included_tx_seq(seq)
    }
    fn has_receipt(&self, id: &[u8; 32]) -> Result<bool, StorageError> {
        self.inner.has_receipt(id)
    }
    fn get_receipt(&self, id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError> {
        self.inner.get_receipt(id)
    }
    fn has_task(&self, id: &[u8; 32]) -> Result<bool, StorageError> {
        self.inner.has_task(id)
    }
    fn get_task(&self, id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError> {
        self.inner.get_task(id)
    }
    fn chain_identity(&self) -> Result<Option<ChainIdentity>, StorageError> {
        self.inner.chain_identity()
    }
    fn write_batch_with(
        &self,
        ops: Vec<BatchOp>,
        durability: Durability,
    ) -> Result<(), StorageError> {
        let heights: Vec<u64> = ops
            .iter()
            .filter_map(|op| match op {
                BatchOp::PutBlockHeightIndex(h, _) => Some(*h),
                _ => None,
            })
            .collect();
        if durability == Durability::Synced && self.fail_synced.load(Ordering::SeqCst) {
            self.log.lock().unwrap().push(format!("failed {durability:?} {heights:?}"));
            return Err(StorageError::Database);
        }
        self.inner.write_batch_with(ops, durability)?;
        self.log.lock().unwrap().push(format!("write {durability:?} {heights:?}"));
        Ok(())
    }
}

/// Logs every broadcast, in the same log as the writes.
struct Announcer(Log);

impl mbongo_network::BlockBroadcaster for Announcer {
    fn broadcast(&self, block: Block) {
        self.0.lock().unwrap().push(format!("broadcast {}", block.header.height));
    }
}

fn producer(log: &Log) -> (NodeBackend<Recording>, Arc<AtomicBool>) {
    let (store, fail) = Recording::new(log);
    let mut backend = dev_producer(store);
    backend.set_broadcaster(Arc::new(Announcer(Arc::clone(log))));
    (backend, fail)
}

fn entries(log: &Log) -> Vec<String> {
    log.lock().unwrap().clone()
}

#[test]
fn the_genesis_and_its_identity_are_one_synced_batch() {
    let log: Log = Arc::default();
    let (backend, _) = producer(&log);
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    assert_eq!(entries(&log), ["write Synced [0]"]);
    assert_eq!(
        backend.storage.chain_identity().unwrap(),
        Some(chain_identity(&Genesis::dev()))
    );
    // Starting again on the same store writes nothing.
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    assert_eq!(entries(&log), ["write Synced [0]"]);
}

#[test]
fn a_genesis_block_without_an_identity_is_refused() {
    let log: Log = Arc::default();
    let (store, _) = Recording::new(&log);
    let block = genesis_block(&Genesis::dev());
    let hash = compute_block_hash(&block);
    store
        .write_batch(vec![
            BatchOp::PutBlock(hash, block),
            BatchOp::PutBlockHeightIndex(0, hash),
        ])
        .unwrap();
    let backend = NodeBackend::new(store, false);
    let err = backend.ensure_genesis(&Genesis::dev()).unwrap_err().to_string();
    assert!(err.contains("no chain identity"), "{err}");
}

#[test]
fn a_genesis_block_recorded_for_another_chain_is_refused() {
    // The dev genesis block, but an identity naming another chain: the
    // block alone does not make the store this chain's.
    let log: Log = Arc::default();
    let (store, _) = Recording::new(&log);
    let block = genesis_block(&Genesis::dev());
    let hash = compute_block_hash(&block);
    let other = mbongo_core::genesis::chain_id(b"another genesis file");
    assert_ne!(other, Genesis::dev().chain_id);
    store
        .write_batch(vec![
            BatchOp::PutBlock(hash, block),
            BatchOp::PutBlockHeightIndex(0, hash),
            BatchOp::PutChainIdentity(ChainIdentity {
                chain_id: other,
                genesis_hash: hash,
            }),
        ])
        .unwrap();
    let backend = NodeBackend::new(store, false);
    let err = backend.ensure_genesis(&Genesis::dev()).unwrap_err().to_string();
    assert!(err.contains(&format!("records chain {other}")), "{err}");
    // Refused, not rewritten: the store still names the other chain.
    assert_eq!(
        backend.storage.chain_identity().unwrap().unwrap().chain_id,
        other
    );
}

#[tokio::test]
async fn a_produced_block_is_committed_synced_before_it_is_broadcast() {
    let log: Log = Arc::default();
    let (backend, _) = producer(&log);
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    backend.produce_block().await.unwrap();
    backend.produce_block().await.unwrap();
    assert_eq!(
        entries(&log),
        [
            "write Synced [0]",
            "write Synced [1]",
            "broadcast 1",
            "write Synced [2]",
            "broadcast 2"
        ]
    );
}

#[tokio::test]
async fn a_failed_synced_commit_is_neither_broadcast_nor_reported() {
    let log: Log = Arc::default();
    let (backend, fail) = producer(&log);
    backend.ensure_genesis(&Genesis::dev()).unwrap();
    fail.store(true, Ordering::SeqCst);
    assert!(backend.produce_block().await.is_err());
    assert_eq!(entries(&log), ["write Synced [0]", "failed Synced [1]"]);
    assert_eq!(backend.get_block_height().await.unwrap(), 0);
    // Once writes succeed again, the next block is height 1.
    fail.store(false, Ordering::SeqCst);
    backend.produce_block().await.unwrap();
    assert_eq!(
        entries(&log)[2..],
        ["write Synced [1]".to_string(), "broadcast 1".to_string()]
    );
}

#[tokio::test]
async fn a_block_from_a_peer_is_committed_buffered() {
    // The authority produces a block; a follower applies it as a peer's.
    let (authority, _) = producer(&Arc::default());
    authority.ensure_genesis(&Genesis::dev()).unwrap();
    authority.produce_block().await.unwrap();
    let block = authority.storage.get_block_by_height(1).unwrap().unwrap();

    let log: Log = Arc::default();
    let (store, _) = Recording::new(&log);
    let follower = NodeBackend::new(store, false);
    follower.ensure_genesis(&Genesis::dev()).unwrap();
    assert!(follower.handle_incoming_block(block).is_some());
    assert_eq!(entries(&log), ["write Synced [0]", "write Buffered [1]"]);
}
