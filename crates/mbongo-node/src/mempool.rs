//! Minimal deterministic mempool for Phase 2.
//!
//! Everything here is node-local policy: bounds, expiry and eviction decide
//! what this node proposes, never which blocks are valid. Two nodes may differ
//! on all of it without differing on any block.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::ops::Bound;
use std::time::{Duration, Instant};

use mbongo_core::{Address, ComputeTask, Hash, Transaction, TransactionPayload};

/// Default bound on pending transactions held by one node.
pub const DEFAULT_MAX_PENDING: usize = 8192;

/// Largest value accepted for [`MempoolConfig::max_pending`].
pub const MAX_PENDING_LIMIT: usize = 65_536;

/// Default time a transaction may stay pending, in seconds.
pub const DEFAULT_TTL_SECS: u64 = 3600;

/// Bounds of a mempool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MempoolConfig {
    /// Most transactions held at once. A new one beyond this is refused.
    pub max_pending: usize,
    /// How long a transaction may stay pending, from its admission.
    pub ttl: Duration,
}

impl MempoolConfig {
    /// Builds a configuration from operator values.
    ///
    /// # Errors
    ///
    /// Returns a description if `max_pending` is outside
    /// 1..=[`MAX_PENDING_LIMIT`], or `ttl_secs` is zero.
    pub fn new(max_pending: usize, ttl_secs: u64) -> Result<Self, String> {
        if max_pending == 0 || max_pending > MAX_PENDING_LIMIT {
            return Err(format!(
                "mempool max pending must be between 1 and {MAX_PENDING_LIMIT}"
            ));
        }
        if ttl_secs == 0 {
            return Err("mempool ttl must be at least one second".to_string());
        }
        Ok(Self {
            max_pending,
            ttl: Duration::from_secs(ttl_secs),
        })
    }
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_pending: DEFAULT_MAX_PENDING,
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
        }
    }
}

/// Why a transaction left the mempool without this node including it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionReason {
    /// A committed block contains it.
    Included,
    /// Its nonce is below the committed nonce of its sender.
    StaleNonce,
    /// A committed block made it unable to apply: its task was registered or
    /// its receipt anchored through another transaction, the balance it needs
    /// was spent, or its nonce no longer follows the committed one.
    Superseded,
    /// It stayed pending longer than the configured time.
    Expired,
    /// Something it depends on was evicted.
    Dependent,
}

/// One transaction removed from the mempool, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eviction {
    /// Hash of the removed transaction.
    pub hash: Hash,
    /// Why it was removed.
    pub reason: EvictionReason,
}

/// One removal still to perform. `with_suffix` says whether the sender's
/// higher nonces go too: they do when the entry could still have applied, and
/// do not when it is already behind the committed nonce.
struct Removal {
    hash: Hash,
    reason: EvictionReason,
    with_suffix: bool,
}

/// Errors returned by mempool operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(clippy::enum_variant_names)] // nearly every admission failure is a duplicate; the prefix is the semantics
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
    /// A `ComputeTask` transaction committing this `task_id` is already
    /// pending. Two pending commitments of one task would be drained
    /// into the same block, where RFC 0005 rule (p) rejects the block.
    #[error("duplicate pending compute task")]
    DuplicateComputeTask,
    /// The mempool holds its configured maximum. Nothing is displaced: with
    /// no fees there is no fair priority, and displacing the oldest would
    /// let a flood remove honest transactions.
    #[error("mempool full")]
    PoolFull,
}

/// Returns the receipt `task_id` carried by a transaction, if any.
fn task_id_of(tx: &Transaction) -> Option<[u8; 32]> {
    match &tx.payload {
        TransactionPayload::AnchorReceipt(receipt) => Some(receipt.task_id),
        TransactionPayload::None | TransactionPayload::ComputeTask(_) => None,
    }
}

/// Returns the derived `task_id` of the compute task a transaction
/// commits, if any. A pending receipt and a pending task may share an id:
/// they are different indexes on chain (RFC 0005 §4.1) and different
/// indexes here.
fn compute_task_id_of(tx: &Transaction) -> Option<[u8; 32]> {
    match &tx.payload {
        TransactionPayload::ComputeTask(task) => Some(task.task_id()),
        TransactionPayload::None | TransactionPayload::AnchorReceipt(_) => None,
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
/// Maintains indexes by transaction hash, (sender, nonce), receipt
/// `task_id` for `AnchorReceipt` transactions, and derived `task_id` for
/// `ComputeTask` transactions, for deduplication. Order of insertion is
/// preserved for block production.
pub struct Mempool {
    by_hash: HashMap<Hash, Transaction>,
    by_sender_nonce: HashMap<(Address, u64), Hash>,
    by_task_id: HashMap<[u8; 32], Hash>,
    by_compute_task_id: HashMap<[u8; 32], Hash>,
    /// Pending nonces per sender, maintained on every insertion and removal.
    /// `by_sender_nonce` is keyed by (sender, nonce), so finding one sender's
    /// entries from it would mean scanning the whole map; this keeps the
    /// per-sender bound an O(1) question, and gives eviction the entries above
    /// a nonce directly.
    pending_by_sender: HashMap<Address, BTreeSet<u64>>,
    /// When each entry was admitted, on the monotonic clock.
    admitted_at: HashMap<Hash, Instant>,
    order: Vec<Hash>,
    config: MempoolConfig,
}

impl Mempool {
    /// Creates an empty mempool with the default bounds.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(MempoolConfig::default())
    }

    /// Creates an empty mempool with the given bounds.
    #[must_use]
    pub fn with_config(config: MempoolConfig) -> Self {
        Self {
            by_hash: HashMap::new(),
            by_sender_nonce: HashMap::new(),
            by_task_id: HashMap::new(),
            by_compute_task_id: HashMap::new(),
            pending_by_sender: HashMap::new(),
            admitted_at: HashMap::new(),
            order: Vec::new(),
            config,
        }
    }

    /// Returns the bound on pending transactions.
    #[must_use]
    pub fn max_pending(&self) -> usize {
        self.config.max_pending
    }

    /// Returns true if a new transaction would be refused for lack of room.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.by_hash.len() >= self.config.max_pending
    }

    /// Inserts a transaction into the mempool.
    ///
    /// # Errors
    ///
    /// Returns [`MempoolError::DuplicateHash`] if `tx_hash` already exists.
    /// Returns [`MempoolError::DuplicateSenderNonce`] if (sender, nonce) is already pending.
    /// Returns [`MempoolError::DuplicateTaskId`] if an `AnchorReceipt` with
    /// the same `task_id` is already pending.
    /// Returns [`MempoolError::DuplicateComputeTask`] if a `ComputeTask`
    /// deriving the same `task_id` is already pending.
    /// Returns [`MempoolError::PoolFull`] if the mempool is at its bound.
    pub fn insert(&mut self, tx_hash: Hash, tx: Transaction) -> Result<(), MempoolError> {
        self.insert_at(tx_hash, tx, Instant::now())
    }

    /// Inserts a transaction admitted at `now`. See [`Self::insert`].
    ///
    /// # Errors
    ///
    /// As for [`Self::insert`].
    pub fn insert_at(
        &mut self,
        tx_hash: Hash,
        tx: Transaction,
        now: Instant,
    ) -> Result<(), MempoolError> {
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
        let compute_task_id = compute_task_id_of(&tx);
        if let Some(tid) = &compute_task_id {
            if self.by_compute_task_id.contains_key(tid) {
                return Err(MempoolError::DuplicateComputeTask);
            }
        }
        if self.is_full() {
            return Err(MempoolError::PoolFull);
        }

        let sender = tx.sender;
        let nonce = tx.nonce;
        self.by_hash.insert(tx_hash, tx);
        self.by_sender_nonce.insert(key, tx_hash);
        if let Some(tid) = task_id {
            self.by_task_id.insert(tid, tx_hash);
        }
        if let Some(tid) = compute_task_id {
            self.by_compute_task_id.insert(tid, tx_hash);
        }
        self.pending_by_sender.entry(sender).or_default().insert(nonce);
        self.admitted_at.insert(tx_hash, now);
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
        let len = self.pending_by_sender.get(sender).map_or(0, BTreeSet::len);
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
        if let Some(tid) = compute_task_id_of(&tx) {
            self.by_compute_task_id.remove(&tid);
        }
        if let Some(nonces) = self.pending_by_sender.get_mut(&tx.sender) {
            nonces.remove(&tx.nonce);
            if nonces.is_empty() {
                self.pending_by_sender.remove(&tx.sender);
            }
        }
        self.admitted_at.remove(hash);
        Some(tx)
    }

    /// Performs `removals` and everything that depends on them, then compacts
    /// `order` once.
    ///
    /// Blocks follow insertion order, so a removal has to take what would
    /// otherwise fail every block:
    ///
    /// - the sender's higher nonces, or a resubmitted nonce would sit behind
    ///   its successors;
    /// - for a `ComputeTask` that never reached the chain, the pending receipt
    ///   bound to it, whatever its sender, which rule (q) would refuse.
    ///
    /// `task_registered` reads committed state. A receipt stays when its task
    /// is registered, because the task it needs is then on chain.
    fn remove_with_dependents(
        &mut self,
        removals: Vec<Removal>,
        task_registered: &impl Fn(&[u8; 32]) -> bool,
    ) -> Vec<Eviction> {
        let mut queue: VecDeque<Removal> = removals.into();
        let mut evicted = Vec::new();
        while let Some(removal) = queue.pop_front() {
            let Some(tx) = self.unlink(&removal.hash) else {
                continue;
            };
            evicted.push(Eviction {
                hash: removal.hash,
                reason: removal.reason,
            });
            if removal.with_suffix {
                if let Some(nonces) = self.pending_by_sender.get(&tx.sender) {
                    for nonce in nonces.range((Bound::Excluded(tx.nonce), Bound::Unbounded)) {
                        if let Some(hash) = self.by_sender_nonce.get(&(tx.sender, *nonce)) {
                            queue.push_back(Removal {
                                hash: *hash,
                                reason: EvictionReason::Dependent,
                                with_suffix: true,
                            });
                        }
                    }
                }
            }
            if removal.reason != EvictionReason::Included {
                if let Some(task_id) = compute_task_id_of(&tx) {
                    if let Some(hash) = self.by_task_id.get(&task_id) {
                        if !task_registered(&task_id) {
                            queue.push_back(Removal {
                                hash: *hash,
                                reason: EvictionReason::Dependent,
                                with_suffix: true,
                            });
                        }
                    }
                }
            }
        }
        if !evicted.is_empty() {
            let removed: HashSet<Hash> = evicted.iter().map(|e| e.hash).collect();
            self.order.retain(|h| !removed.contains(h));
        }
        evicted
    }

    /// Brings the mempool up to date with a committed block.
    ///
    /// `block_txs` holds the block's transactions and their hashes.
    /// `committed` returns a sender's committed nonce and balance, or `None`
    /// when it cannot answer, which leaves that sender's entries for a later
    /// block. `task_registered` reads committed state too. Entries above an
    /// included or stale one stay: they are still valid.
    ///
    /// Only a block's own senders can lose a pending entry this way: nonces
    /// and debits are the sender's own, and a task or receipt id binds its
    /// sender.
    pub fn after_commit(
        &mut self,
        block_txs: &[(Hash, Transaction)],
        committed: impl Fn(&Address) -> Option<(u64, u128)>,
        task_registered: impl Fn(&[u8; 32]) -> bool,
    ) -> Vec<Eviction> {
        let mut removals = Vec::new();
        let mut views: HashMap<Address, Option<(u64, u128)>> = HashMap::new();
        for (hash, tx) in block_txs {
            views.entry(tx.sender).or_insert_with(|| committed(&tx.sender));
            if self.by_hash.contains_key(hash) {
                removals.push(Removal {
                    hash: *hash,
                    reason: EvictionReason::Included,
                    with_suffix: false,
                });
            }
        }
        for (sender, view) in &views {
            let (Some((nonce_now, _)), Some(nonces)) = (view, self.pending_by_sender.get(sender))
            else {
                continue;
            };
            for nonce in nonces.range(..*nonce_now) {
                if let Some(hash) = self.by_sender_nonce.get(&(*sender, *nonce)) {
                    removals.push(Removal {
                        hash: *hash,
                        reason: EvictionReason::StaleNonce,
                        with_suffix: false,
                    });
                }
            }
        }
        for (hash, tx) in block_txs {
            let rival = task_id_of(tx).and_then(|tid| self.by_task_id.get(&tid)).or_else(|| {
                compute_task_id_of(tx).and_then(|tid| self.by_compute_task_id.get(&tid))
            });
            if let Some(rival) = rival {
                if rival != hash {
                    removals.push(Removal {
                        hash: *rival,
                        reason: EvictionReason::Superseded,
                        with_suffix: true,
                    });
                }
            }
        }
        let mut evicted = self.remove_with_dependents(removals, &task_registered);

        // What is left must start at the committed nonce, run without a gap,
        // and be affordable. The first entry that breaks this goes, with what
        // follows it: it would otherwise fail every block.
        let mut removals = Vec::new();
        for (sender, view) in &views {
            let (Some((nonce_now, balance)), Some(nonces)) =
                (view, self.pending_by_sender.get(sender))
            else {
                continue;
            };
            let mut expected = *nonce_now;
            let mut debit: u128 = 0;
            for nonce in nonces {
                let Some(hash) = self.by_sender_nonce.get(&(*sender, *nonce)) else {
                    continue;
                };
                let amount = self.by_hash.get(hash).map_or(0, |tx| tx.amount);
                debit = debit.saturating_add(amount);
                if *nonce != expected || debit > *balance {
                    removals.push(Removal {
                        hash: *hash,
                        reason: EvictionReason::Superseded,
                        with_suffix: true,
                    });
                    break;
                }
                expected = expected.saturating_add(1);
            }
        }
        evicted.extend(self.remove_with_dependents(removals, &task_registered));
        evicted
    }

    /// Evicts every entry older than the configured time, with its dependents.
    pub fn sweep_expired(
        &mut self,
        now: Instant,
        task_registered: impl Fn(&[u8; 32]) -> bool,
    ) -> Vec<Eviction> {
        let ttl = self.config.ttl;
        let removals = self
            .order
            .iter()
            .filter(|hash| {
                self.admitted_at
                    .get(*hash)
                    .is_some_and(|at| now.saturating_duration_since(*at) >= ttl)
            })
            .map(|hash| Removal {
                hash: *hash,
                reason: EvictionReason::Expired,
                with_suffix: true,
            })
            .collect();
        self.remove_with_dependents(removals, &task_registered)
    }

    /// Returns up to `max` transactions in insertion order **without**
    /// removing them.
    ///
    /// Block production peeks, applies, and only then calls
    /// [`Mempool::after_commit`]. A destructive drain before application
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
    #[cfg(test)]
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

    /// Returns true if a `ComputeTask` deriving this `task_id` is pending.
    #[must_use]
    pub fn contains_compute_task_id(&self, task_id: &[u8; 32]) -> bool {
        self.by_compute_task_id.contains_key(task_id)
    }

    /// Returns the pending `ComputeTask` deriving this `task_id`, if any.
    ///
    /// Admission uses it to mirror the same-block half of RFC 0005 rules
    /// (q)–(s): a task pending ahead of a receipt will precede that receipt
    /// in the block this node produces, since `order` is insertion order.
    #[must_use]
    pub fn pending_compute_task(&self, task_id: &[u8; 32]) -> Option<ComputeTask> {
        let hash = self.by_compute_task_id.get(task_id)?;
        match &self.by_hash.get(hash)?.payload {
            TransactionPayload::ComputeTask(task) => Some((**task).clone()),
            TransactionPayload::None | TransactionPayload::AnchorReceipt(_) => None,
        }
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
impl Mempool {
    /// Moves an entry's admission time, for tests of expiry.
    pub fn set_admitted_at(&mut self, hash: &Hash, at: Instant) {
        self.admitted_at.insert(*hash, at);
    }

    /// Asserts that every index describes the same set of entries.
    pub fn assert_consistent(&self) {
        let len = self.by_hash.len();
        assert_eq!(self.order.len(), len, "order");
        assert_eq!(self.admitted_at.len(), len, "admitted_at");
        assert_eq!(self.by_sender_nonce.len(), len, "by_sender_nonce");
        let per_sender: usize = self.pending_by_sender.values().map(BTreeSet::len).sum();
        assert_eq!(per_sender, len, "pending_by_sender");
        assert!(len <= self.config.max_pending, "bound exceeded");
        let unique: HashSet<&Hash> = self.order.iter().collect();
        assert_eq!(unique.len(), len, "order holds a duplicate");
        for hash in &self.order {
            let tx = self.by_hash.get(hash).expect("order names a missing entry");
            assert_eq!(self.by_sender_nonce.get(&(tx.sender, tx.nonce)), Some(hash));
            assert!(self.pending_by_sender[&tx.sender].contains(&tx.nonce));
            assert!(self.admitted_at.contains_key(hash));
            if let Some(tid) = task_id_of(tx) {
                assert_eq!(self.by_task_id.get(&tid), Some(hash));
            }
            if let Some(tid) = compute_task_id_of(tx) {
                assert_eq!(self.by_compute_task_id.get(&tid), Some(hash));
            }
        }
        let receipts = self.by_hash.values().filter(|tx| task_id_of(tx).is_some()).count();
        assert_eq!(self.by_task_id.len(), receipts, "by_task_id");
        let tasks = self.by_hash.values().filter(|tx| compute_task_id_of(tx).is_some()).count();
        assert_eq!(self.by_compute_task_id.len(), tasks, "by_compute_task_id");
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

    // ── RFC 0005: pending ComputeTask index ───────────────────────────

    /// Builds a `ComputeTask` transaction with the given salt from
    /// `sender`. Signatures are irrelevant to mempool indexing tests.
    fn make_task_tx(
        sender: u8,
        nonce: u64,
        hash_byte: u8,
        salt: [u8; 32],
    ) -> (Hash, Transaction, [u8; 32]) {
        let task = mbongo_core::ComputeTask {
            version: 1,
            submitter: Address([sender; 32]),
            executor: Address([0xE0u8; 32]),
            salt,
            input_commitment: [0x1Cu8; 32],
            execution_spec: vec![1, 2, 3],
        };
        let task_id = task.task_id();
        let tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: Address([sender; 32]),
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::ComputeTask(Box::new(task)),
            signature: [0u8; 64],
        };
        (Hash([hash_byte; 32]), tx, task_id)
    }

    #[test]
    fn mempool_duplicate_compute_task_rejected() {
        let mut pool = Mempool::new();
        let (h1, tx1, task_id) = make_task_tx(1, 0, 30, [0x5Au8; 32]);
        pool.insert(h1, tx1).unwrap();
        assert!(pool.contains_compute_task_id(&task_id));
        // The receipt index is a different keyspace.
        assert!(!pool.contains_task_id(&task_id));

        // Same envelope under a different nonce and hash derives the same
        // task_id (the nonce is not in the envelope) → rejected.
        let (h2, tx2, same_id) = make_task_tx(1, 1, 31, [0x5Au8; 32]);
        assert_eq!(same_id, task_id);
        let err = pool.insert(h2, tx2).unwrap_err();
        assert!(matches!(err, MempoolError::DuplicateComputeTask));
        assert_eq!(pool.len(), 1);

        // A different salt is a different task and is admitted.
        let (h3, tx3, other_id) = make_task_tx(1, 1, 32, [0x5Bu8; 32]);
        assert_ne!(other_id, task_id);
        pool.insert(h3, tx3).unwrap();
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn mempool_task_and_receipt_may_share_an_id() {
        // A task and a receipt for that task may be pending together:
        // RFC 0005 lets the receipt follow the task in the same block.
        let mut pool = Mempool::new();
        let (h1, tx1, task_id) = make_task_tx(1, 0, 33, [0x5Cu8; 32]);
        pool.insert(h1, tx1).unwrap();
        let (h2, tx2) = make_anchor_tx(2, 0, 34, task_id);
        pool.insert(h2, tx2).unwrap();
        assert!(pool.contains_compute_task_id(&task_id));
        assert!(pool.contains_task_id(&task_id));
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn mempool_remove_clears_compute_task_index() {
        let mut pool = Mempool::new();
        let (h1, tx1, task_id) = make_task_tx(1, 0, 35, [0x5Du8; 32]);
        pool.insert(h1, tx1).unwrap();
        pool.remove_included(&[h1]);
        assert!(!pool.contains_compute_task_id(&task_id));
        assert_eq!(pool.len(), 0);
        // The id is free again.
        let (h2, tx2, _) = make_task_tx(1, 0, 36, [0x5Du8; 32]);
        pool.insert(h2, tx2).unwrap();
    }

    // ── Bounds and eviction (S3) ──

    fn bounded(max_pending: usize, ttl_secs: u64) -> Mempool {
        Mempool::with_config(MempoolConfig::new(max_pending, ttl_secs).unwrap())
    }

    fn reasons(evicted: &[Eviction]) -> Vec<(Hash, EvictionReason)> {
        evicted.iter().map(|e| (e.hash, e.reason)).collect()
    }

    fn no_task(_: &[u8; 32]) -> bool {
        false
    }

    #[test]
    fn config_refuses_values_outside_its_range() {
        assert!(MempoolConfig::new(0, 1).is_err());
        assert!(MempoolConfig::new(MAX_PENDING_LIMIT + 1, 1).is_err());
        assert!(MempoolConfig::new(1, 0).is_err());
        assert!(MempoolConfig::new(1, 1).is_ok());
        assert!(MempoolConfig::new(MAX_PENDING_LIMIT, u64::MAX).is_ok());
    }

    #[test]
    fn a_full_pool_refuses_and_stays_as_it_was() {
        let mut pool = bounded(2, 60);
        insert_chain(&mut pool, 1, &[0, 1]);
        let (hash, tx) = make_tx_with_hash(2, 0, 50);
        assert_eq!(pool.insert(hash, tx), Err(MempoolError::PoolFull));
        assert_eq!(pool.len(), 2);
        assert!(!pool.contains_hash(&hash));
        pool.assert_consistent();
    }

    #[test]
    fn a_duplicate_on_a_full_pool_stays_a_duplicate() {
        let mut pool = bounded(1, 60);
        let (hash, tx) = make_tx_with_hash(1, 0, 7);
        pool.insert(hash, tx.clone()).unwrap();
        assert_eq!(pool.insert(hash, tx), Err(MempoolError::DuplicateHash));
    }

    #[test]
    fn room_returns_after_an_eviction() {
        let mut pool = bounded(1, 60);
        let (hash, tx) = make_tx_with_hash(1, 0, 7);
        let start = Instant::now();
        pool.insert_at(hash, tx, start).unwrap();
        pool.sweep_expired(start + Duration::from_secs(60), no_task);
        let (next, tx) = make_tx_with_hash(2, 0, 8);
        pool.insert(next, tx).unwrap();
        pool.assert_consistent();
    }

    #[test]
    fn nothing_expires_before_its_time_or_on_an_earlier_reading() {
        let mut pool = bounded(4, 60);
        let start = Instant::now() + Duration::from_secs(1000);
        let (hash, tx) = make_tx_with_hash(1, 0, 7);
        pool.insert_at(hash, tx, start).unwrap();
        assert!(pool.sweep_expired(start + Duration::from_secs(59), no_task).is_empty());
        assert!(pool.sweep_expired(start - Duration::from_secs(500), no_task).is_empty());
        assert_eq!(
            reasons(&pool.sweep_expired(start + Duration::from_secs(60), no_task)),
            vec![(hash, EvictionReason::Expired)]
        );
        pool.assert_consistent();
    }

    #[test]
    fn expiry_takes_the_higher_nonces_of_that_sender_and_nobody_else() {
        let mut pool = bounded(8, 60);
        let start = Instant::now();
        let (a0, tx) = make_tx_with_hash(1, 5, 10);
        pool.insert_at(a0, tx, start).unwrap();
        let later = start + Duration::from_secs(30);
        let (a1, tx) = make_tx_with_hash(1, 6, 11);
        pool.insert_at(a1, tx, later).unwrap();
        let (a2, tx) = make_tx_with_hash(1, 7, 12);
        pool.insert_at(a2, tx, later).unwrap();
        let (other, tx) = make_tx_with_hash(2, 0, 13);
        pool.insert_at(other, tx, start + Duration::from_secs(1)).unwrap();

        let evicted = pool.sweep_expired(start + Duration::from_secs(60), no_task);
        assert_eq!(
            evicted.len(),
            3,
            "the other sender's entry expires one second later"
        );
        assert_eq!(
            evicted[0],
            Eviction {
                hash: a0,
                reason: EvictionReason::Expired
            }
        );
        assert!(evicted[1..].iter().all(|e| e.reason == EvictionReason::Dependent));
        assert!(pool.contains_hash(&other));
        assert_eq!(
            pool.sender_pending(&Address([1u8; 32]), 5).expected_nonce,
            Some(5)
        );
        pool.assert_consistent();
    }

    #[test]
    fn a_commit_removes_included_and_stale_entries_alone() {
        let mut pool = bounded(8, 60);
        let (stale, tx) = make_tx_with_hash(1, 5, 10);
        pool.insert(stale, tx).unwrap();
        let (included, block_tx) = make_tx_with_hash(1, 6, 11);
        pool.insert(included, block_tx.clone()).unwrap();
        let (valid, tx) = make_tx_with_hash(1, 7, 12);
        pool.insert(valid, tx).unwrap();
        let (also_valid, tx) = make_tx_with_hash(1, 8, 13);
        pool.insert(also_valid, tx).unwrap();

        let evicted = pool.after_commit(&[(included, block_tx)], |_| Some((7, u128::MAX)), no_task);
        assert_eq!(evicted.len(), 2);
        assert!(evicted.contains(&Eviction {
            hash: included,
            reason: EvictionReason::Included
        }));
        assert!(evicted.contains(&Eviction {
            hash: stale,
            reason: EvictionReason::StaleNonce
        }));
        assert!(pool.contains_hash(&valid) && pool.contains_hash(&also_valid));
        pool.assert_consistent();
    }

    #[test]
    fn a_commit_with_no_nonce_answer_leaves_the_sender_alone() {
        let mut pool = bounded(8, 60);
        let (kept, tx) = make_tx_with_hash(1, 5, 10);
        pool.insert(kept, tx).unwrap();
        let (_, block_tx) = make_tx_with_hash(1, 9, 11);
        assert!(pool.after_commit(&[(Hash([11u8; 32]), block_tx)], |_| None, no_task).is_empty());
        assert!(pool.contains_hash(&kept));
    }

    #[test]
    fn peek_keeps_nonce_order_after_an_eviction_and_a_resubmission() {
        let mut pool = bounded(8, 60);
        let start = Instant::now();
        let (a0, tx0) = make_tx_with_hash(1, 0, 10);
        pool.insert_at(a0, tx0.clone(), start).unwrap();
        let (a1, tx1) = make_tx_with_hash(1, 1, 11);
        pool.insert_at(a1, tx1.clone(), start + Duration::from_secs(30)).unwrap();
        pool.sweep_expired(start + Duration::from_secs(60), no_task);
        pool.insert(a0, tx0).unwrap();
        pool.insert(a1, tx1).unwrap();
        let nonces: Vec<u64> = pool.peek_for_block(10).iter().map(|(_, tx)| tx.nonce).collect();
        assert_eq!(nonces, vec![0, 1]);
    }

    #[test]
    fn a_chain_that_no_longer_follows_the_committed_nonce_goes() {
        let mut pool = bounded(8, 60);
        let (ahead, tx) = make_tx_with_hash(1, 6, 10);
        pool.insert(ahead, tx).unwrap();
        let (above, tx) = make_tx_with_hash(1, 7, 11);
        pool.insert(above, tx).unwrap();
        let (block_hash, block_tx) = make_tx_with_hash(1, 5, 12);

        // The block did not move the committed nonce past 5.
        let evicted =
            pool.after_commit(&[(block_hash, block_tx)], |_| Some((5, u128::MAX)), no_task);
        assert_eq!(evicted.len(), 2, "{evicted:?}");
        assert_eq!(evicted[0].reason, EvictionReason::Superseded);
        assert_eq!(evicted[1].reason, EvictionReason::Dependent);
        pool.assert_consistent();
    }

    /// The model of committed state that the generated run drives.
    struct Chain {
        nonces: [u64; 8],
        balances: [u128; 8],
        registered: HashSet<[u8; 32]>,
    }

    impl Chain {
        fn view(&self, a: &Address) -> Option<(u64, u128)> {
            let i = usize::from(a.0[0] - 1);
            Some((self.nonces[i], self.balances[i]))
        }
    }

    fn transfer(sender: Address, nonce: u64, amount: u128) -> Transaction {
        Transaction {
            tx_type: TransactionType::Transfer,
            sender,
            receiver: Address([99u8; 32]),
            amount,
            nonce,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        }
    }

    fn task(sender: Address, nonce: u64, salt: u64) -> Transaction {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&salt.to_le_bytes());
        let payload = ComputeTask {
            version: 1,
            submitter: sender,
            executor: Address([7u8; 32]),
            salt: bytes,
            input_commitment: [1u8; 32],
            execution_spec: Vec::new(),
        };
        Transaction {
            tx_type: TransactionType::ComputeTask,
            sender,
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::ComputeTask(Box::new(payload)),
            signature: [0u8; 64],
        }
    }

    fn receipt(sender: Address, nonce: u64, task_id: [u8; 32]) -> Transaction {
        let payload = mbongo_core::Receipt {
            version: 1,
            task_id,
            input_commitment: [1u8; 32],
            output_commitment: [2u8; 32],
            executor: sender,
            metadata: Vec::new(),
            signature: [0u8; 64],
        };
        Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender,
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::AnchorReceipt(Box::new(payload)),
            signature: [0u8; 64],
        }
    }

    fn hash_of(step: u64, salt: u8) -> Hash {
        let mut bytes = [salt; 32];
        bytes[..8].copy_from_slice(&step.to_le_bytes());
        Hash(bytes)
    }

    /// After a commit or a sweep: indexes agree, every chain starts at its
    /// committed nonce with no gap, is affordable, and every pending receipt
    /// still has its task, pending or registered.
    fn assert_applicable(pool: &Mempool, chain: &Chain) {
        pool.assert_consistent();
        for i in 0..8u8 {
            let who = Address([i + 1; 32]);
            let (nonce, balance) = chain.view(&who).unwrap();
            let walked = pool.sender_pending(&who, nonce);
            let held = u64::try_from(walked.len).unwrap();
            assert_eq!(
                walked.expected_nonce,
                Some(nonce + held),
                "chain of {i} has a gap"
            );
            assert!(
                walked.pending_debit <= balance,
                "chain of {i} is not affordable"
            );
        }
        for task_id in pool.by_task_id.keys() {
            assert!(
                pool.by_compute_task_id.contains_key(task_id) || chain.registered.contains(task_id),
                "a receipt lost its task"
            );
        }
    }

    /// A deterministic run of admissions, own and foreign blocks, and sweeps,
    /// over transfers, tasks and receipts. Every eviction reason has to occur.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn generated_operations_keep_the_pool_consistent_and_applicable() {
        const STEPS: u64 = 30_000;
        let mut pool = bounded(24, 60);
        let start = Instant::now();
        let mut chain = Chain {
            nonces: [0; 8],
            balances: [1_000_000; 8],
            registered: HashSet::new(),
        };
        let mut clock = 0u64;
        let mut seed = 0x5EED_u64;
        let mut next = |bound: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % bound
        };
        let mut seen: HashMap<String, u64> = HashMap::new();
        let mut full = 0u64;
        for step in 0..STEPS {
            let i = usize::try_from(next(8)).unwrap();
            let who = Address([u8::try_from(i).unwrap() + 1; 32]);
            clock += next(2);
            let now = start + Duration::from_secs(clock);
            let walked = pool.sender_pending(&who, chain.nonces[i]);
            let nonce = walked.expected_nonce.unwrap();
            let mut evicted = Vec::new();
            match next(12) {
                0..=3 => {
                    let amount = u128::from(next(1000));
                    if walked.pending_debit + amount <= chain.balances[i] {
                        match pool.insert_at(hash_of(step, 1), transfer(who, nonce, amount), now) {
                            Ok(()) | Err(MempoolError::DuplicateSenderNonce) => {}
                            Err(MempoolError::PoolFull) => full += 1,
                            Err(e) => panic!("{e}"),
                        }
                    }
                }
                4 => {
                    let tx = task(who, nonce, next(64));
                    if !chain.registered.contains(&compute_task_id_of(&tx).unwrap()) {
                        match pool.insert_at(hash_of(step, 2), tx, now) {
                            Ok(()) | Err(MempoolError::DuplicateComputeTask) => {}
                            Err(MempoolError::PoolFull) => full += 1,
                            Err(e) => panic!("{e}"),
                        }
                    }
                }
                5 | 6 => {
                    let pending: Vec<[u8; 32]> = pool.by_compute_task_id.keys().copied().collect();
                    if !pending.is_empty() {
                        let index = usize::try_from(next(pending.len() as u64)).unwrap();
                        let tx = receipt(who, nonce, pending[index]);
                        match pool.insert_at(hash_of(step, 3), tx, now) {
                            Ok(()) | Err(MempoolError::DuplicateTaskId) => {}
                            Err(MempoolError::PoolFull) => full += 1,
                            Err(e) => panic!("{e}"),
                        }
                    }
                }
                7 | 8 => {
                    // A block from this pool: the head of one chain.
                    if let Some(hash) = pool.by_sender_nonce.get(&(who, chain.nonces[i])).copied() {
                        let tx = pool.by_hash[&hash].clone();
                        chain.nonces[i] += 1;
                        chain.balances[i] -= tx.amount;
                        if let Some(tid) = compute_task_id_of(&tx) {
                            chain.registered.insert(tid);
                        }
                        let view = |a: &Address| chain.view(a);
                        let known = |t: &[u8; 32]| chain.registered.contains(t);
                        evicted = pool.after_commit(&[(hash, tx)], view, known);
                    }
                }
                9 | 10 => {
                    // A block from elsewhere, at this sender's committed nonce.
                    let at = chain.nonces[i];
                    let tx = if next(2) == 0 {
                        let spend = u128::from(next(1000)) * 50;
                        transfer(who, at, spend.min(chain.balances[i]))
                    } else {
                        let rival = pool
                            .by_hash
                            .values()
                            .find(|t| t.sender == who && compute_task_id_of(t).is_some());
                        match rival.and_then(compute_task_salt) {
                            Some(salt) => task(who, at, salt),
                            None => transfer(who, at, 0),
                        }
                    };
                    chain.nonces[i] += 1;
                    chain.balances[i] -= tx.amount;
                    if let Some(tid) = compute_task_id_of(&tx) {
                        chain.registered.insert(tid);
                    }
                    let view = |a: &Address| chain.view(a);
                    let known = |t: &[u8; 32]| chain.registered.contains(t);
                    evicted = pool.after_commit(&[(hash_of(step, 4), tx)], view, known);
                }
                _ => {
                    let known = |t: &[u8; 32]| chain.registered.contains(t);
                    evicted = pool.sweep_expired(now, known);
                }
            }
            for eviction in &evicted {
                *seen.entry(format!("{:?}", eviction.reason)).or_insert(0) += 1;
            }
            if !evicted.is_empty() {
                assert_applicable(&pool, &chain);
            } else {
                pool.assert_consistent();
            }
        }
        println!("{seen:?} full={full}");
        for reason in [
            "Included",
            "StaleNonce",
            "Superseded",
            "Expired",
            "Dependent",
        ] {
            assert!(
                seen.get(reason).copied().unwrap_or(0) > 20,
                "{reason}: {seen:?}"
            );
        }
        assert!(full > 20, "{full}");
    }

    fn compute_task_salt(tx: &Transaction) -> Option<u64> {
        match &tx.payload {
            TransactionPayload::ComputeTask(task) => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&task.salt[..8]);
                Some(u64::from_le_bytes(bytes))
            }
            TransactionPayload::None | TransactionPayload::AnchorReceipt(_) => None,
        }
    }
}
