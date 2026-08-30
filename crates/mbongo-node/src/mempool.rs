//! Minimal deterministic mempool for Phase 2.

use std::collections::{HashMap, HashSet};

use mbongo_core::{Address, Hash, Transaction, TransactionPayload};

/// Errors returned by mempool operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(clippy::enum_variant_names)] // every admission failure IS a duplicate; the prefix is the semantics
pub enum MempoolError {
    /// Transaction with this hash already exists in mempool or storage.
    #[error("duplicate transaction hash")]
    DuplicateHash,
    /// A transaction from this sender with this nonce is already pending.
    #[error("duplicate sender nonce")]
    DuplicateSenderNonce,
    /// An `AnchorReceipt` transaction with this `task_id` is already
    /// pending. Two pending receipts for one task id would be drained
    /// into the same block, where RFC 0002 rule (j) rejects the block.
    #[error("duplicate pending task_id")]
    DuplicateTaskId,
}

/// Returns the receipt `task_id` carried by a transaction, if any.
fn task_id_of(tx: &Transaction) -> Option<[u8; 32]> {
    match &tx.payload {
        TransactionPayload::AnchorReceipt(receipt) => Some(receipt.task_id),
        TransactionPayload::None => None,
    }
}

/// Pending-state facts about one sender, read against one committed
/// account view (issue #100).
///
/// This is a *report*, not a decision: the mempool states what is pending,
/// and [`crate::backend`] applies the admission policy. Nothing in
/// consensus consults it — `apply_block` validates against its own
/// in-memory account view and never reads mempool state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderPending {
    /// The next nonce this sender may submit: the first nonce at or above
    /// `committed_nonce` that is not already pending.
    ///
    /// A gap stops the walk, so this is the *missing* nonce rather than
    /// one past the highest pending entry. With committed nonce 5 and
    /// nonces 5 and 7 pending, this is 6 — not 8. Taking a maximum would
    /// admit 8 and leave 6 permanently missing, which no later block could
    /// ever apply.
    ///
    /// `None` means the u64 nonce space is exhausted for this sender.
    pub expected_nonce: Option<u64>,
    /// Sum of `amount` over the contiguous pending chain walked from
    /// `committed_nonce`, saturating at `u128::MAX`.
    ///
    /// Only the sender's own debits are counted. Amounts this sender may
    /// *receive* from other pending transactions are deliberately ignored:
    /// counting them would admit a chain that block application could
    /// still reject, and admission must stay conservative.
    pub pending_debit: u128,
    /// Total pending entries held for this sender, including any whose
    /// nonce is below `committed_nonce`.
    ///
    /// This is resource accounting, which is a different question from the
    /// nonce walk: an entry the walk ignores still occupies memory.
    pub len: usize,
}

/// In-memory mempool with deterministic ordering.
///
/// Maintains indexes by transaction hash, (sender, nonce), and — for
/// `AnchorReceipt` transactions — receipt `task_id`, for deduplication.
/// Order of insertion is preserved for block production.
pub struct Mempool {
    by_hash: HashMap<Hash, Transaction>,
    by_sender_nonce: HashMap<(Address, u64), Hash>,
    by_task_id: HashMap<[u8; 32], Hash>,
    /// Pending entry count per sender, maintained on every insertion and
    /// removal. `by_sender_nonce` is keyed by (sender, nonce), so counting
    /// one sender's entries from it would mean scanning the whole map;
    /// this keeps the per-sender bound an O(1) question.
    pending_by_sender: HashMap<Address, usize>,
    order: Vec<Hash>,
}

impl Mempool {
    /// Creates an empty mempool.
    #[must_use]
    pub fn new() -> Self {
        Self {
            by_hash: HashMap::new(),
            by_sender_nonce: HashMap::new(),
            by_task_id: HashMap::new(),
            pending_by_sender: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Inserts a transaction into the mempool.
    ///
    /// # Errors
    ///
    /// Returns [`MempoolError::DuplicateHash`] if `tx_hash` already exists.
    /// Returns [`MempoolError::DuplicateSenderNonce`] if (sender, nonce) is already pending.
    /// Returns [`MempoolError::DuplicateTaskId`] if an `AnchorReceipt` with
    /// the same `task_id` is already pending.
    pub fn insert(&mut self, tx_hash: Hash, tx: Transaction) -> Result<(), MempoolError> {
        if self.by_hash.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateHash);
        }
        let key = (tx.sender, tx.nonce);
        if self.by_sender_nonce.contains_key(&key) {
            return Err(MempoolError::DuplicateSenderNonce);
        }
        let task_id = task_id_of(&tx);
        if let Some(tid) = &task_id {
            if self.by_task_id.contains_key(tid) {
                return Err(MempoolError::DuplicateTaskId);
            }
        }

        let sender = tx.sender;
        self.by_hash.insert(tx_hash, tx);
        self.by_sender_nonce.insert(key, tx_hash);
        if let Some(tid) = task_id {
            self.by_task_id.insert(tid, tx_hash);
        }
        *self.pending_by_sender.entry(sender).or_insert(0) += 1;
        self.order.push(tx_hash);
        Ok(())
    }

    /// Reports pending facts for `sender`, read against `committed_nonce`.
    ///
    /// Walks upward from `committed_nonce` while each nonce is pending.
    /// Entries *below* `committed_nonce` are never consulted by the walk —
    /// with committed nonce 6 and only nonce 5 pending, the expected nonce
    /// is 6. Such entries are reported in [`SenderPending::len`] but this
    /// method never removes them: calculating pending state and cleaning it
    /// up are separate concerns, and a read-only report must not mutate.
    ///
    /// Cost is O(k) hash lookups for a chain of length k, plus O(1) for the
    /// count.
    #[must_use]
    pub fn sender_pending(&self, sender: &Address, committed_nonce: u64) -> SenderPending {
        let len = self.pending_by_sender.get(sender).copied().unwrap_or(0);
        let mut expected = committed_nonce;
        let mut pending_debit: u128 = 0;

        while let Some(hash) = self.by_sender_nonce.get(&(*sender, expected)) {
            if let Some(tx) = self.by_hash.get(hash) {
                pending_debit = pending_debit.saturating_add(tx.amount);
            }
            let Some(next) = expected.checked_add(1) else {
                // The chain runs to u64::MAX: no nonce is left to allocate.
                return SenderPending {
                    expected_nonce: None,
                    pending_debit,
                    len,
                };
            };
            expected = next;
        }

        SenderPending {
            expected_nonce: Some(expected),
            pending_debit,
            len,
        }
    }

    /// Removes a transaction by hash from all indexes.
    #[allow(dead_code)] // Part of public API; used in tests and future eviction logic.
    pub fn remove(&mut self, hash: &Hash) {
        if self.unlink(hash).is_some() {
            self.order.retain(|h| h != hash);
        }
    }

    /// Drops a transaction from every index except `order`, returning it.
    ///
    /// `order` is left to the caller so that a bulk removal can compact it
    /// in one pass instead of once per transaction.
    fn unlink(&mut self, hash: &Hash) -> Option<Transaction> {
        let tx = self.by_hash.remove(hash)?;
        self.by_sender_nonce.remove(&(tx.sender, tx.nonce));
        if let Some(tid) = task_id_of(&tx) {
            self.by_task_id.remove(&tid);
        }
        if let Some(count) = self.pending_by_sender.get_mut(&tx.sender) {
            *count -= 1;
            if *count == 0 {
                self.pending_by_sender.remove(&tx.sender);
            }
        }
        Some(tx)
    }

    /// Returns up to `max` transactions in insertion order **without**
    /// removing them.
    ///
    /// Block production peeks, applies, and only then calls
    /// [`Mempool::remove_included`]. A destructive drain before application
    /// would lose every transaction in the batch when application fails,
    /// and issue #100 turns that from one transaction into a sender's whole
    /// pending chain. There is deliberately no destructive drain to reach
    /// for.
    #[must_use]
    pub fn peek_for_block(&self, max: usize) -> Vec<(Hash, Transaction)> {
        self.order
            .iter()
            .take(max)
            .filter_map(|h| self.by_hash.get(h).map(|tx| (*h, tx.clone())))
            .collect()
    }

    /// Removes transactions that a successfully applied block included.
    ///
    /// Unknown hashes are ignored, so this is idempotent. `order` is
    /// compacted once for the whole batch, preserving the relative order of
    /// everything that remains.
    pub fn remove_included(&mut self, hashes: &[Hash]) {
        let mut removed: HashSet<Hash> = HashSet::new();
        for hash in hashes {
            if self.unlink(hash).is_some() {
                removed.insert(*hash);
            }
        }
        if !removed.is_empty() {
            self.order.retain(|h| !removed.contains(h));
        }
    }

    /// Returns true if an `AnchorReceipt` with this `task_id` is pending.
    #[must_use]
    pub fn contains_task_id(&self, task_id: &[u8; 32]) -> bool {
        self.by_task_id.contains_key(task_id)
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
    use mbongo_core::{Address, TransactionPayload, TransactionType};

    fn make_tx(sender: u8, nonce: u64) -> (Hash, Transaction) {
        make_tx_with_hash(sender, nonce, sender)
    }

    /// Same as make_tx but with a distinct hash (for duplicate sender/nonce tests).
    fn make_tx_with_hash(sender: u8, nonce: u64, hash_byte: u8) -> (Hash, Transaction) {
        make_tx_full(sender, nonce, hash_byte, 100)
    }

    /// Same as make_tx_with_hash but with a caller-chosen amount.
    fn make_tx_full(sender: u8, nonce: u64, hash_byte: u8, amount: u128) -> (Hash, Transaction) {
        let addr = Address([sender; 32]);
        let hash = Hash([hash_byte; 32]);
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender: addr,
            receiver: Address([99u8; 32]),
            amount,
            nonce,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        (hash, tx)
    }

    /// Inserts a transaction with a hash derived from `nonce`, so a chain
    /// can be built without hand-picking hash bytes.
    fn insert_chain(pool: &mut Mempool, sender: u8, nonces: &[u64]) {
        for (i, n) in nonces.iter().enumerate() {
            let hash_byte = u8::try_from(i).expect("test chain fits in u8") + 100;
            let (h, tx) = make_tx_with_hash(sender, *n, hash_byte);
            pool.insert(h, tx).unwrap();
        }
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

    // ── Issue #100: sender_pending ─────────────────────────────────────

    #[test]
    fn sender_pending_empty_expects_committed_nonce() {
        let pool = Mempool::new();
        let p = pool.sender_pending(&Address([1u8; 32]), 7);
        assert_eq!(p.expected_nonce, Some(7));
        assert_eq!(p.pending_debit, 0);
        assert_eq!(p.len, 0);
    }

    #[test]
    fn sender_pending_contiguous_one_expects_next() {
        let mut pool = Mempool::new();
        insert_chain(&mut pool, 1, &[7]);
        let p = pool.sender_pending(&Address([1u8; 32]), 7);
        assert_eq!(p.expected_nonce, Some(8));
        assert_eq!(p.len, 1);
    }

    #[test]
    fn sender_pending_contiguous_chain_expects_after_head() {
        let mut pool = Mempool::new();
        insert_chain(&mut pool, 1, &[7, 8, 9]);
        let p = pool.sender_pending(&Address([1u8; 32]), 7);
        assert_eq!(p.expected_nonce, Some(10));
        assert_eq!(p.len, 3);
    }

    #[test]
    fn sender_pending_gap_expects_the_hole_not_the_maximum() {
        let mut pool = Mempool::new();
        // Committed 5, pending {5, 7}: the missing nonce is 6.
        insert_chain(&mut pool, 1, &[5, 7]);
        let p = pool.sender_pending(&Address([1u8; 32]), 5);
        assert_eq!(
            p.expected_nonce,
            Some(6),
            "a gap must stop the walk; max+1 would be 8"
        );
        assert_eq!(p.len, 2, "the stranded entry still occupies memory");
    }

    #[test]
    fn sender_pending_stale_below_committed_is_ignored_by_the_walk() {
        let mut pool = Mempool::new();
        // Committed 6, pending {5}: nothing at or above 6 is pending.
        insert_chain(&mut pool, 1, &[5]);
        let p = pool.sender_pending(&Address([1u8; 32]), 6);
        assert_eq!(p.expected_nonce, Some(6));
        assert_eq!(p.pending_debit, 0, "a stale entry is not a pending debit");
        assert_eq!(p.len, 1, "but it is still counted as a resource");
    }

    #[test]
    fn sender_pending_stale_plus_contiguous_walks_only_the_chain() {
        let mut pool = Mempool::new();
        // Committed 6, pending {5, 6, 7}: walk 6 → 7 → 8, ignoring 5.
        insert_chain(&mut pool, 1, &[5, 6, 7]);
        let p = pool.sender_pending(&Address([1u8; 32]), 6);
        assert_eq!(p.expected_nonce, Some(8));
        assert_eq!(p.pending_debit, 200, "only nonces 6 and 7 are debits");
        assert_eq!(p.len, 3);
    }

    #[test]
    fn sender_pending_nonce_exhaustion_returns_none() {
        let mut pool = Mempool::new();
        insert_chain(&mut pool, 1, &[u64::MAX]);
        let p = pool.sender_pending(&Address([1u8; 32]), u64::MAX);
        assert_eq!(p.expected_nonce, None, "no nonce left to allocate");
    }

    #[test]
    fn sender_pending_at_max_nonce_with_nothing_pending_is_allocatable() {
        let pool = Mempool::new();
        let p = pool.sender_pending(&Address([1u8; 32]), u64::MAX);
        assert_eq!(p.expected_nonce, Some(u64::MAX));
    }

    #[test]
    fn sender_pending_isolates_senders() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx_with_hash(1, 0, 30);
        let (h2, tx2) = make_tx_with_hash(2, 0, 31);
        pool.insert(h1, tx1).unwrap();
        pool.insert(h2, tx2).unwrap();

        let a = pool.sender_pending(&Address([1u8; 32]), 0);
        let b = pool.sender_pending(&Address([3u8; 32]), 0);
        assert_eq!(a.expected_nonce, Some(1));
        assert_eq!(a.len, 1);
        assert_eq!(
            b.expected_nonce,
            Some(0),
            "an untouched sender is unaffected"
        );
        assert_eq!(b.len, 0);
    }

    #[test]
    fn sender_pending_accumulates_debit_over_the_chain() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx_full(1, 0, 40, 4);
        let (h2, tx2) = make_tx_full(1, 1, 41, 6);
        pool.insert(h1, tx1).unwrap();
        pool.insert(h2, tx2).unwrap();
        let p = pool.sender_pending(&Address([1u8; 32]), 0);
        assert_eq!(p.pending_debit, 10);
        assert_eq!(p.expected_nonce, Some(2));
    }

    #[test]
    fn sender_pending_debit_saturates_instead_of_wrapping() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx_full(1, 0, 42, u128::MAX);
        let (h2, tx2) = make_tx_full(1, 1, 43, u128::MAX);
        pool.insert(h1, tx1).unwrap();
        pool.insert(h2, tx2).unwrap();
        let p = pool.sender_pending(&Address([1u8; 32]), 0);
        assert_eq!(
            p.pending_debit,
            u128::MAX,
            "saturation keeps the balance comparison fail-closed"
        );
    }

    #[test]
    fn sender_pending_middle_removal_produces_a_hole() {
        let mut pool = Mempool::new();
        insert_chain(&mut pool, 1, &[0, 1, 2]);
        // Remove the middle entry (nonce 1, second inserted → hash byte 101).
        pool.remove(&Hash([101u8; 32]));

        let p = pool.sender_pending(&Address([1u8; 32]), 0);
        assert_eq!(p.expected_nonce, Some(1), "the walk stops at the hole");
        assert_eq!(p.len, 2);
    }

    #[test]
    fn sender_pending_count_returns_to_zero_after_removal() {
        let mut pool = Mempool::new();
        insert_chain(&mut pool, 1, &[0, 1]);
        pool.remove(&Hash([100u8; 32]));
        pool.remove(&Hash([101u8; 32]));
        let p = pool.sender_pending(&Address([1u8; 32]), 0);
        assert_eq!(p.len, 0);
        assert_eq!(p.expected_nonce, Some(0));
    }

    // ── Issue #100: peek / remove_included ─────────────────────────────

    #[test]
    fn mempool_peek_returns_in_order_without_removing() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        let (h2, tx2) = make_tx(2, 0);
        let (h3, tx3) = make_tx(3, 0);
        pool.insert(h1, tx1).unwrap();
        pool.insert(h2, tx2).unwrap();
        pool.insert(h3, tx3).unwrap();

        let peeked = pool.peek_for_block(2);
        assert_eq!(peeked.len(), 2);
        assert_eq!(peeked[0].0, h1);
        assert_eq!(peeked[1].0, h2);
        assert_eq!(peeked[0].1.sender.0[0], 1);
        assert_eq!(peeked[1].1.sender.0[0], 2);

        // Peeking is non-destructive: everything is still pending.
        assert_eq!(pool.len(), 3);
        assert!(pool.contains_hash(&h1));
        assert!(pool.contains_hash(&h2));
        assert!(pool.contains_hash(&h3));
    }

    #[test]
    fn mempool_peek_is_repeatable() {
        let mut pool = Mempool::new();
        insert_chain(&mut pool, 1, &[0, 1]);
        let first = pool.peek_for_block(10);
        let second = pool.peek_for_block(10);
        assert_eq!(first.len(), 2);
        assert_eq!(first, second, "a failed application must change nothing");
    }

    #[test]
    fn mempool_remove_included_clears_every_index() {
        let mut pool = Mempool::new();
        let task_id = [0xACu8; 32];
        let (h1, tx1) = make_anchor_tx(1, 0, 22, task_id);
        pool.insert(h1, tx1).unwrap();

        let peeked = pool.peek_for_block(10);
        assert_eq!(peeked.len(), 1);
        assert!(
            pool.contains_task_id(&task_id),
            "still pending before removal"
        );

        pool.remove_included(&[h1]);
        assert_eq!(pool.len(), 0);
        assert!(!pool.contains_hash(&h1));
        assert!(!pool.contains_task_id(&task_id));
        assert_eq!(pool.sender_pending(&Address([1u8; 32]), 0).len, 0);

        // After removal the same task_id can be anchored again.
        let (h2, tx2) = make_anchor_tx(1, 1, 23, task_id);
        pool.insert(h2, tx2).unwrap();
    }

    #[test]
    fn mempool_remove_included_preserves_order_of_the_remainder() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        let (h2, tx2) = make_tx(2, 0);
        let (h3, tx3) = make_tx(3, 0);
        pool.insert(h1, tx1).unwrap();
        pool.insert(h2, tx2).unwrap();
        pool.insert(h3, tx3).unwrap();

        // Remove the first and last; the middle keeps its position.
        pool.remove_included(&[h1, h3]);
        let remaining = pool.peek_for_block(10);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, h2);
    }

    #[test]
    fn mempool_remove_included_ignores_unknown_hashes() {
        let mut pool = Mempool::new();
        let (h1, tx1) = make_tx(1, 0);
        pool.insert(h1, tx1).unwrap();
        // Idempotent: removing twice, and removing something never present,
        // must both be harmless.
        pool.remove_included(&[h1]);
        pool.remove_included(&[h1, Hash([0xEEu8; 32])]);
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.sender_pending(&Address([1u8; 32]), 0).len, 0);
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

    /// Builds an `AnchorReceipt` transaction carrying the given task id.
    /// Signatures are irrelevant to mempool indexing tests.
    fn make_anchor_tx(
        sender: u8,
        nonce: u64,
        hash_byte: u8,
        task_id: [u8; 32],
    ) -> (Hash, Transaction) {
        let receipt = mbongo_core::Receipt {
            version: 1,
            task_id,
            input_commitment: [0u8; 32],
            output_commitment: [0u8; 32],
            executor: Address([sender; 32]),
            metadata: vec![],
            signature: [0u8; 64],
        };
        let tx = Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender: Address([sender; 32]),
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
            signature: [0u8; 64],
        };
        (Hash([hash_byte; 32]), tx)
    }

    #[test]
    fn mempool_duplicate_task_id_rejected() {
        let mut pool = Mempool::new();
        let task_id = [0xABu8; 32];
        let (h1, tx1) = make_anchor_tx(1, 0, 20, task_id);
        pool.insert(h1, tx1).unwrap();
        assert!(pool.contains_task_id(&task_id));

        // Different sender/nonce/hash, same task_id → DuplicateTaskId.
        let (h2, tx2) = make_anchor_tx(2, 0, 21, task_id);
        let err = pool.insert(h2, tx2).unwrap_err();
        assert!(matches!(err, MempoolError::DuplicateTaskId));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn mempool_remove_clears_task_id_index() {
        let mut pool = Mempool::new();
        let task_id = [0xADu8; 32];
        let (h1, tx1) = make_anchor_tx(1, 0, 24, task_id);
        pool.insert(h1, tx1).unwrap();
        pool.remove(&h1);
        assert!(!pool.contains_task_id(&task_id));
        assert_eq!(pool.len(), 0);
    }
}
