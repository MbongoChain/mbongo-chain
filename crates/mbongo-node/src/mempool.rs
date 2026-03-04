//! Minimal deterministic mempool for Phase 2.

use std::collections::HashMap;

use mbongo_core::{Address, Hash, Transaction};

/// Errors returned by mempool operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MempoolError {
    /// Transaction with this hash already exists in mempool or storage.
    #[error("duplicate transaction hash")]
    DuplicateHash,
    /// A transaction from this sender with this nonce is already pending.
    #[error("duplicate sender nonce")]
    DuplicateSenderNonce,
}

/// In-memory mempool with deterministic ordering.
///
/// Maintains indexes by transaction hash and (sender, nonce) for deduplication.
/// Order of insertion is preserved for block production.
pub struct Mempool {
    by_hash: HashMap<Hash, Transaction>,
    by_sender_nonce: HashMap<(Address, u64), Hash>,
    order: Vec<Hash>,
}

impl Mempool {
    /// Creates an empty mempool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_hash: HashMap::new(),
            by_sender_nonce: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Inserts a transaction into the mempool.
    ///
    /// # Errors
    ///
    /// Returns [`MempoolError::DuplicateHash`] if `tx_hash` already exists.
    /// Returns [`MempoolError::DuplicateSenderNonce`] if (sender, nonce) is already pending.
    pub fn insert(&mut self, tx_hash: Hash, tx: Transaction) -> Result<(), MempoolError> {
        if self.by_hash.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateHash);
        }
        let key = (tx.sender, tx.nonce);
        if self.by_sender_nonce.contains_key(&key) {
            return Err(MempoolError::DuplicateSenderNonce);
        }

        self.by_hash.insert(tx_hash, tx.clone());
        self.by_sender_nonce.insert(key, tx_hash);
        self.order.push(tx_hash);
        Ok(())
    }

    /// Removes a transaction by hash from all indexes.
    #[allow(dead_code)] // Part of public API; used in tests and future eviction logic.
    pub fn remove(&mut self, hash: &Hash) {
        if let Some(tx) = self.by_hash.remove(hash) {
            self.by_sender_nonce.remove(&(tx.sender, tx.nonce));
            self.order.retain(|h| h != hash);
        }
    }

    /// Drains up to `max` transactions in insertion order for block production.
    ///
    /// Removes the returned transactions from the mempool.
    #[must_use]
    pub fn drain_for_block(&mut self, max: usize) -> Vec<Transaction> {
        let take = max.min(self.order.len());
        let hashes: Vec<Hash> = self.order.drain(..take).collect();
        let mut txs = Vec::with_capacity(hashes.len());
        for h in &hashes {
            if let Some(tx) = self.by_hash.remove(h) {
                self.by_sender_nonce.remove(&(tx.sender, tx.nonce));
                txs.push(tx);
            }
        }
        txs
    }

    /// Returns the number of transactions in the mempool.
    #[must_use]
    #[allow(dead_code)] // Part of public API; used in tests and future metrics.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Returns true if the mempool contains a transaction with the given hash.
    #[must_use]
    pub fn contains_hash(&self, hash: &Hash) -> bool {
        self.by_hash.contains_key(hash)
    }
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbongo_core::{Address, TransactionType};

    fn make_tx(sender: u8, nonce: u64) -> (Hash, Transaction) {
        make_tx_with_hash(sender, nonce, sender)
    }

    /// Same as make_tx but with a distinct hash (for duplicate sender/nonce tests).
    fn make_tx_with_hash(sender: u8, nonce: u64, hash_byte: u8) -> (Hash, Transaction) {
        let addr = Address([sender; 32]);
        let hash = Hash([hash_byte; 32]);
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender: addr,
            receiver: Address([99u8; 32]),
            amount: 100,
            nonce,
            signature: [0u8; 64],
        };
        (hash, tx)
    }

    #[test]
    fn mempool_insert_and_len() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        pool.insert(h1, tx1).unwrap();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn mempool_duplicate_hash_rejected() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        pool.insert(h1, tx1).unwrap();
        let (_, tx2) = make_tx(2, 0);
        let err = pool.insert(h1, tx2).unwrap_err();
        assert!(matches!(err, MempoolError::DuplicateHash));
    }

    #[test]
    fn mempool_duplicate_sender_nonce_rejected() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx_with_hash(1, 0, 10);
        pool.insert(h1, tx1).unwrap();
        // Same (sender, nonce), different hash → DuplicateSenderNonce.
        let (h2, tx2) = make_tx_with_hash(1, 0, 11);
        let err = pool.insert(h2, tx2).unwrap_err();
        assert!(matches!(err, MempoolError::DuplicateSenderNonce));
    }

    #[test]
    fn mempool_drain_removes_and_returns_in_order() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        let (h2, tx2) = make_tx(2, 0);
        let (h3, tx3) = make_tx(3, 0);
        pool.insert(h1, tx1.clone()).unwrap();
        pool.insert(h2, tx2.clone()).unwrap();
        pool.insert(h3, tx3.clone()).unwrap();

        let drained = pool.drain_for_block(2);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].sender.0[0], 1);
        assert_eq!(drained[1].sender.0[0], 2);
        assert_eq!(pool.len(), 1);
        assert!(pool.contains_hash(&h3));
    }

    #[test]
    fn mempool_remove() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        pool.insert(h1, tx1).unwrap();
        pool.remove(&h1);
        assert_eq!(pool.len(), 0);
        assert!(!pool.contains_hash(&h1));
    }
}
