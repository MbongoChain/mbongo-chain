//! Storage-backed implementation of [`RpcBackend`] and [`ApiBackend`].

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use log::{info, warn};
use mbongo_api::rest::{
    Account as RestAccount, ApiBackend, ApiError, BlockDetail, BlockSummary,
    Transaction as RestTransaction, Validator,
};
use mbongo_core::{
    compute_transactions_root, Account, Address, Block, BlockBody, BlockHeader, ComputeTask, Hash,
    Receipt, Transaction, TransactionPayload, TransactionType, COMPUTE_TASK_VERSION,
    MAX_EXECUTION_SPEC_BYTES,
};
use mbongo_network::rpc::{BackendError, RpcBackend};
use mbongo_network::BlockBroadcaster;
use mbongo_storage::{BatchOp, Storage, StorageError};
use mbongo_verification::{verify_receipt_signature, ReceiptError, ReceiptIndex, RECEIPT_VERSION};
use parity_scale_codec::{Decode, Encode};
use tokio::sync::RwLock;

use crate::mempool::{Mempool, MempoolError};

/// Maximum transactions per block.
const MAX_TX_PER_BLOCK: usize = 1000;

/// Maximum transactions one sender may hold pending in this node's mempool.
///
/// Node-local admission policy, not a consensus rule and not a protocol
/// surface: nothing in `apply_block` or the anchoring spec knows about it,
/// and two nodes may disagree on it without disagreeing on any block.
///
/// It exists because issue #100 removed an incidental bound. While a sender
/// could hold only one pending transaction, per-sender memory was bounded at
/// one; a pending chain has no such limit, and the balance check bounds
/// nothing for `AnchorReceipt` transactions, whose amount is always zero.
///
/// Kept well below [`MAX_TX_PER_BLOCK`] so one sender's full chain always
/// fits in a single block. It carries no economic meaning.
const MAX_PENDING_PER_SENDER: usize = 64;

/// Maximum `receipt.metadata` length in bytes (RFC 0002 §3, maintainer-
/// approved value). This is a consensus validity rule of the anchoring
/// protocol, not intrinsic receipt validity: raising it is a protocol
/// version bump, and it is never lowered retroactively.
const MAX_RECEIPT_METADATA_BYTES: usize = 4096;

/// Where a duplicate `task_id` was found (RFC 0002 §2 rules i/j).
enum DuplicateSource {
    /// Anchored in persistent chain state as of the parent block (rule i).
    PriorState,
    /// Anchored by an earlier transaction in the current block (rule j).
    CurrentBlock,
}

/// Composite read-only receipt index (RFC 0002 §4): the union of prior
/// persistent state and receipts anchored earlier in the current block.
/// Never mutated by validation; `pending` is the transient per-block set.
struct CompositeReceiptIndex<'a, S: Storage> {
    storage: &'a S,
    pending: &'a std::collections::HashSet<[u8; 32]>,
}

impl<S: Storage> CompositeReceiptIndex<'_, S> {
    /// Locates a duplicate, prior state first (normative rule i before j).
    fn locate(&self, task_id: &[u8; 32]) -> Result<Option<DuplicateSource>, StorageError> {
        if self.storage.has_receipt(task_id)? {
            return Ok(Some(DuplicateSource::PriorState));
        }
        if self.pending.contains(task_id) {
            return Ok(Some(DuplicateSource::CurrentBlock));
        }
        Ok(None)
    }
}

impl<S: Storage> ReceiptIndex for CompositeReceiptIndex<'_, S> {
    fn contains_task_id(&self, task_id: &[u8; 32]) -> Result<bool, ReceiptError> {
        self.locate(task_id)
            .map(|source| source.is_some())
            .map_err(|e| ReceiptError::Index(e.to_string()))
    }
}

/// The consensus-relevant payload a transaction carries once rule (a) of
/// RFC 0002 §2 and rule (k) of RFC 0005 §3 hold: the payload variant
/// matches the transaction type.
#[derive(Clone, Copy)]
enum TypedPayload<'a> {
    /// `Transfer` or `Stake` with no payload: the v0.2 transfer path.
    Plain,
    /// `AnchorReceipt` carrying its receipt.
    Anchor(&'a Receipt),
    /// `ComputeTask` carrying its task envelope.
    Task(&'a ComputeTask),
}

/// Rule (a) of RFC 0002 §2 and rule (k) of RFC 0005 §3: the payload
/// variant must match the transaction type. `AnchorReceipt` ⟺ anchor
/// payload, `ComputeTask` ⟺ task payload, `Transfer` and `Stake` ⟺ `None`.
///
/// The legacy `(ComputeTask, None)` form, which v0.3 executed as a plain
/// transfer, is rejected here (RFC 0005 §5.1). Every pairing is listed so
/// a future variant cannot slip through a wildcard.
fn check_type_payload(tx: &Transaction) -> Result<TypedPayload<'_>, ()> {
    match (tx.tx_type, &tx.payload) {
        (TransactionType::AnchorReceipt, TransactionPayload::AnchorReceipt(receipt)) => {
            Ok(TypedPayload::Anchor(receipt))
        }
        (TransactionType::ComputeTask, TransactionPayload::ComputeTask(task)) => {
            Ok(TypedPayload::Task(task))
        }
        (TransactionType::Transfer | TransactionType::Stake, TransactionPayload::None) => {
            Ok(TypedPayload::Plain)
        }
        (
            TransactionType::AnchorReceipt | TransactionType::ComputeTask,
            TransactionPayload::None,
        )
        | (
            TransactionType::Transfer | TransactionType::Stake | TransactionType::ComputeTask,
            TransactionPayload::AnchorReceipt(_),
        )
        | (
            TransactionType::Transfer | TransactionType::Stake | TransactionType::AnchorReceipt,
            TransactionPayload::ComputeTask(_),
        ) => Err(()),
    }
}

/// Why a `ComputeTask` envelope failed one of rules (m)–(o) of RFC 0005 §3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskRuleViolation {
    /// `task.version != 1` (rule m).
    Version,
    /// `task.execution_spec.len() > MAX_EXECUTION_SPEC_BYTES` (rule n).
    SpecTooLarge,
    /// `task.submitter != tx.sender` (rule o).
    SubmitterMismatch,
}

/// Rules (m)–(o) of RFC 0005 §3 on a `ComputeTask` envelope, in normative
/// order. On success returns the derived `task_id`, computed only once
/// rule (n) has held, so the hashed preimage is bounded by
/// `MAX_TASK_ID_PREIMAGE_BYTES`. Shared by consensus and admission so the
/// two cannot diverge.
///
/// Consensus never inspects `input_commitment` or `execution_spec` beyond
/// this: the commitment is compared for equality later (rule r, not part
/// of this gate) and the specification is opaque bytes (§2.12).
fn check_task_envelope(
    tx: &Transaction,
    task: &ComputeTask,
) -> Result<[u8; 32], TaskRuleViolation> {
    if task.version != COMPUTE_TASK_VERSION {
        return Err(TaskRuleViolation::Version);
    }
    if task.execution_spec.len() > MAX_EXECUTION_SPEC_BYTES {
        return Err(TaskRuleViolation::SpecTooLarge);
    }
    if task.submitter != tx.sender {
        return Err(TaskRuleViolation::SubmitterMismatch);
    }
    Ok(task.task_id())
}

/// Why an anchored receipt failed one of rules (q)–(s) of RFC 0005 §3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BindingViolation {
    /// No task with `receipt.task_id` in prior state or earlier in the
    /// block (rule q).
    TaskNotRegistered,
    /// `receipt.input_commitment != task.input_commitment` (rule r).
    InputCommitmentMismatch,
    /// `receipt.executor != task.executor` (rule s).
    ExecutorNotAuthorised,
}

/// Rules (q)–(s) of RFC 0005 §3, in normative order: the receipt answers a
/// registered task, over the input the submitter committed to, from the
/// executor the submitter named. `task` is the registered envelope the
/// caller resolved from prior chain state or from earlier in the same
/// block; `None` fails rule (q).
///
/// Three field equalities and nothing else. Consensus never learns how
/// either commitment was derived (§2.4), reads no input or output, and
/// forms no opinion about the answer (§9.2): an anchored receipt is a
/// bound claim, not a proven result. Shared by consensus and admission so
/// the two cannot diverge.
fn check_receipt_binding(
    receipt: &Receipt,
    task: Option<&ComputeTask>,
) -> Result<(), BindingViolation> {
    let Some(task) = task else {
        return Err(BindingViolation::TaskNotRegistered);
    };
    if receipt.input_commitment != task.input_commitment {
        return Err(BindingViolation::InputCommitmentMismatch);
    }
    if receipt.executor != task.executor {
        return Err(BindingViolation::ExecutorNotAuthorised);
    }
    Ok(())
}

/// Loads the registered task under `task_id` from prior chain state and
/// decodes its canonical bytes. Absence is `Ok(None)`; bytes that do not
/// decode are a storage failure, never an "unknown task": consensus wrote
/// them as the canonical encoding and must not reinterpret corruption as
/// a protocol verdict.
fn load_task<S: Storage>(storage: &S, task_id: &[u8; 32]) -> Result<Option<ComputeTask>, String> {
    let Some(bytes) = storage.get_task(task_id).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    ComputeTask::decode(&mut &bytes[..])
        .map(Some)
        .map_err(|e| format!("corrupt task bytes under task_id {}: {e}", Hash(*task_id)))
}

/// Node backend backed by a [`Storage`] implementation.
///
/// Wraps `S` in an [`Arc`] so the backend is cheaply cloneable as
/// required by both [`RpcBackend`] and [`ApiBackend`].
pub struct NodeBackend<S: Storage> {
    /// Storage backend. `pub(crate)` for tests.
    pub(crate) storage: Arc<S>,
    mempool: Arc<RwLock<Mempool>>,
    /// Optional block broadcaster for pushing blocks to peers.
    broadcaster: Option<Arc<dyn BlockBroadcaster>>,
    /// Whether this node is configured as a block producer.
    is_producer: bool,
}

impl<S: Storage> Clone for NodeBackend<S> {
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            mempool: Arc::clone(&self.mempool),
            broadcaster: self.broadcaster.clone(),
            is_producer: self.is_producer,
        }
    }
}

impl<S: Storage> NodeBackend<S> {
    /// Creates a new backend wrapping the given storage.
    ///
    /// `is_producer` controls whether this node is allowed to produce blocks.
    /// When `false`, calls to [`RpcBackend::produce_block`] will return an error.
    pub fn new(storage: S, is_producer: bool) -> Self {
        Self {
            storage: Arc::new(storage),
            mempool: Arc::new(RwLock::new(Mempool::new())),
            broadcaster: None,
            is_producer,
        }
    }

    /// Sets the block broadcaster used to push new blocks to peers.
    pub fn set_broadcaster(&mut self, b: Arc<dyn BlockBroadcaster>) {
        self.broadcaster = Some(b);
    }

    /// Returns the current chain tip height.
    ///
    /// Convenience wrapper for use by the sync orchestrator without
    /// going through the async `RpcBackend` trait.
    pub fn latest_height(&self) -> Result<u64, BackendError> {
        self.storage
            .get_latest_height()
            .map_err(|e| BackendError::Internal(format!("storage error: {e}")))
    }

    /// Writes the genesis block (height 0) if it does not already exist.
    ///
    /// The genesis block has an all-zero parent hash, empty body, and
    /// timestamp 0. This method is idempotent.
    pub fn ensure_genesis(&self) -> Result<(), BackendError> {
        // If height 0 already exists, nothing to do.
        if self
            .storage
            .get_block_by_height(0)
            .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?
            .is_some()
        {
            return Ok(());
        }

        let txs: Vec<Transaction> = Vec::new();
        let block = Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: compute_transactions_root(&txs),
                timestamp: 0,
                height: 0,
            },
            body: BlockBody { transactions: txs },
        };

        let block_hash = compute_block_hash(&block);

        self.storage
            .put_block(&block_hash, &block)
            .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?;
        self.storage
            .put_block_height_index(0, block_hash)
            .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?;

        // DEV ONLY: Pre-funded account for testing.
        // Deterministic dev key (must match wallet example).
        use ed25519_dalek::SigningKey;
        let signing_key = SigningKey::from_bytes(&[0xAAu8; 32]);
        let verifying_key = signing_key.verifying_key();
        let dev_addr = Address(verifying_key.to_bytes());
        let existing = self
            .storage
            .get_account(&dev_addr)
            .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?;
        if existing.is_none() {
            let mut dev_account = Account::new(dev_addr);
            dev_account.balance = 1_000_000_000;
            self.storage
                .put_account(&dev_addr, &dev_account)
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?;
        }

        Ok(())
    }

    /// Validate and atomically apply a block to storage.
    ///
    /// Checks:
    /// 1. `block.header.parent_hash` matches the current chain tip hash.
    /// 2. `block.header.height == current_height + 1`.
    /// 3. `transactions_root` matches re-computed commitment.
    /// 4. Every transaction has a valid signature.
    /// 5. Nonce and balance rules pass for every transaction (re-executed).
    ///
    /// On success the block, its transactions, and all account updates are
    /// committed atomically via [`Storage::write_batch`].
    ///
    /// Used by both `produce_block` (after building the block locally) and
    /// the follower sync path (applying blocks received from peers).
    ///
    /// # Errors
    ///
    /// Returns [`ApplyBlockError`] if validation or storage fails.
    pub fn apply_block(&self, block: &Block) -> Result<Hash, ApplyBlockError> {
        let storage = &self.storage;

        // ── Parent linkage ─────────────────────────────────────────────
        let current_height = storage
            .get_latest_height()
            .map_err(|e| ApplyBlockError::Storage(e.to_string()))?;

        let expected_height = current_height + 1;
        if block.header.height != expected_height {
            return Err(ApplyBlockError::BadHeight {
                expected: expected_height,
                got: block.header.height,
            });
        }

        let parent_block = storage
            .get_block_by_height(current_height)
            .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
            .ok_or_else(|| ApplyBlockError::Storage("parent block not found".to_string()))?;

        let expected_parent_hash = compute_block_hash(&parent_block);
        if block.header.parent_hash != expected_parent_hash {
            return Err(ApplyBlockError::BadParent {
                expected: expected_parent_hash,
                got: block.header.parent_hash,
            });
        }

        // ── Transactions root ──────────────────────────────────────────
        let recomputed_root = compute_transactions_root(&block.body.transactions);
        if block.header.transactions_root != recomputed_root {
            return Err(ApplyBlockError::TransactionsRootMismatch);
        }

        // ── Re-execute transactions ────────────────────────────────────
        let mut ops: Vec<BatchOp> = Vec::new();
        let mut account_cache: std::collections::HashMap<Address, Account> =
            std::collections::HashMap::new();

        // Transaction-sequence baseline: read ONCE before the loop and
        // advanced locally. The persistent counter is committed only in
        // the final atomic batch (SetTxSeq + SetLastIncludedTxSeq below).
        let mut last_seq = storage
            .get_last_included_tx_seq()
            .map_err(|e| ApplyBlockError::Storage(e.to_string()))?;

        // Transient set of task_ids anchored earlier in THIS block, in body
        // order (RFC 0002 §4). Discarded when validation ends — success or
        // failure; rule (j) consults it, and it never touches storage.
        let mut pending_task_ids: std::collections::HashSet<[u8; 32]> =
            std::collections::HashSet::new();

        // Transient map of the tasks committed by ComputeTask transactions
        // earlier in THIS block, keyed by task_id (RFC 0005 rule p,
        // same-block half; rules q–s, same-block half). Separate from the
        // receipt set above: a task and its receipt may legitimately share
        // one id within a block, in that order.
        let mut pending_tasks: std::collections::HashMap<[u8; 32], ComputeTask> =
            std::collections::HashMap::new();

        for (i, tx) in block.body.transactions.iter().enumerate() {
            // (a)/(k) Type/form (RFC 0002 §2, RFC 0005 §3): AnchorReceipt
            // requires an AnchorReceipt payload, ComputeTask a ComputeTask
            // payload; every other type requires None.
            let Ok(typed) = check_type_payload(tx) else {
                return Err(ApplyBlockError::TypePayloadMismatch(i));
            };

            // (b)/(l) Field constraints: unused fields are pinned to
            // canonical values so two encodings of the same anchoring or
            // task commitment cannot differ, and neither can move money.
            let canonical_fields = tx.amount == 0 && tx.receiver == Address::zero();
            match typed {
                TypedPayload::Anchor(_) if !canonical_fields => {
                    return Err(ApplyBlockError::AnchorFieldConstraint(i));
                }
                TypedPayload::Task(_) if !canonical_fields => {
                    return Err(ApplyBlockError::TaskFieldConstraint(i));
                }
                TypedPayload::Plain | TypedPayload::Anchor(_) | TypedPayload::Task(_) => {}
            }

            // (c) Transaction signature.
            if !tx.verify_signature() {
                return Err(ApplyBlockError::InvalidSignature(i));
            }

            let tx_hash = compute_tx_hash(tx);

            // Already-stored transaction handling. For plain types this is
            // the v0.2 idempotent skip (unchanged). An AnchorReceipt must
            // NOT take the skip path: a stored anchor transaction implies
            // its receipt is anchored (both commit in one atomic batch), so
            // re-including it — even byte-identically — violates global
            // task_id uniqueness and is rejected with the same verdict rule
            // (i) would produce. A stored ComputeTask transaction likewise
            // implies its task is registered, and rule (p) gives the same
            // verdict.
            let already_stored = storage
                .get_transaction(&tx_hash)
                .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                .is_some();
            if already_stored {
                match typed {
                    TypedPayload::Anchor(_) => {
                        return Err(ApplyBlockError::TaskIdAlreadyAnchored(i));
                    }
                    TypedPayload::Task(_) => {
                        return Err(ApplyBlockError::TaskAlreadyRegistered(i));
                    }
                    TypedPayload::Plain => continue,
                }
            }

            if let TypedPayload::Task(task) = typed {
                // ── ComputeTask path (RFC 0005 §3 rules m–p, with the
                // generic nonce rule d in its v0.3 position) ────────────
                // (d) Nonce: the sender account must exist and the nonce
                // must match; the nonce is consumed. No balance movement
                // (§2.8: a task commitment is not a transfer).
                let sender_addr = tx.sender;
                let mut sender = match account_cache.get(&sender_addr) {
                    Some(acc) => acc.clone(),
                    None => storage
                        .get_account(&sender_addr)
                        .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                        .ok_or(ApplyBlockError::SenderAccountMissing(i))?,
                };
                sender
                    .validate_and_increment_nonce(tx.nonce)
                    .map_err(|_| ApplyBlockError::InvalidNonce(i))?;

                // (m) version, (n) bound, (o) submitter identity, in that
                // order; the task_id is derived only once the bound holds.
                let task_id = check_task_envelope(tx, task).map_err(|why| match why {
                    TaskRuleViolation::Version => ApplyBlockError::TaskVersionUnsupported(i),
                    TaskRuleViolation::SpecTooLarge => ApplyBlockError::ExecutionSpecTooLarge(i),
                    TaskRuleViolation::SubmitterMismatch => {
                        ApplyBlockError::SenderSubmitterMismatch(i)
                    }
                })?;

                // (p) Uniqueness: prior chain state first, then tasks
                // committed earlier in this block. First-registered-wins;
                // the task_id is content-derived, so a byte-identical
                // envelope under a fresh nonce is the same task (§2.6).
                if storage
                    .has_task(&task_id)
                    .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                {
                    return Err(ApplyBlockError::TaskAlreadyRegistered(i));
                }
                if pending_tasks.contains_key(&task_id) {
                    return Err(ApplyBlockError::TaskRepeatedInBlock(i));
                }
                pending_tasks.insert(task_id, task.clone());

                // Effects: accumulated only; committed in the final atomic
                // batch. The stored task is its canonical SCALE encoding,
                // keyed by task_id, and storage never decodes it (§4). No
                // status is written: "submitted" is the presence of this
                // key (§4.1).
                last_seq += 1;
                ops.push(BatchOp::PutTransaction(tx_hash, tx.clone()));
                ops.push(BatchOp::PutTxSeqIndex(last_seq, tx_hash));
                ops.push(BatchOp::PutTask(task_id, task.encode()));
                account_cache.insert(sender_addr, sender);
            } else if let TypedPayload::Anchor(receipt) = typed {
                // ── AnchorReceipt path (RFC 0002 §2 rules d–j) ──────────
                // (d) Nonce: the sender account must exist and the nonce
                // must match; the nonce is consumed. No balance movement.
                let sender_addr = tx.sender;
                let mut sender = match account_cache.get(&sender_addr) {
                    Some(acc) => acc.clone(),
                    None => storage
                        .get_account(&sender_addr)
                        .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                        .ok_or(ApplyBlockError::SenderAccountMissing(i))?,
                };
                sender
                    .validate_and_increment_nonce(tx.nonce)
                    .map_err(|_| ApplyBlockError::InvalidNonce(i))?;

                // (e) Metadata size cap (consensus parameter, RFC 0002 §3).
                if receipt.metadata.len() > MAX_RECEIPT_METADATA_BYTES {
                    return Err(ApplyBlockError::ReceiptMetadataTooLarge(i));
                }
                // (f) Receipt version.
                if receipt.version != RECEIPT_VERSION {
                    return Err(ApplyBlockError::ReceiptVersionUnsupported(i));
                }
                // (g) Submitter identity: transaction-level anchoring rule
                // orchestrated here, not intrinsic receipt validity.
                if tx.sender != receipt.executor {
                    return Err(ApplyBlockError::SenderExecutorMismatch(i));
                }
                // (h) Receipt signature over the raw 32-byte receipt hash.
                if !verify_receipt_signature(receipt) {
                    return Err(ApplyBlockError::InvalidReceiptSignature(i));
                }
                // (i)+(j) Duplicates via the composite index: prior chain
                // state first, then earlier receipts in this block.
                let index = CompositeReceiptIndex {
                    storage: storage.as_ref(),
                    pending: &pending_task_ids,
                };
                match index
                    .locate(&receipt.task_id)
                    .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                {
                    Some(DuplicateSource::PriorState) => {
                        return Err(ApplyBlockError::TaskIdAlreadyAnchored(i));
                    }
                    Some(DuplicateSource::CurrentBlock) => {
                        return Err(ApplyBlockError::TaskIdRepeatedInBlock(i));
                    }
                    None => {}
                }

                // (q)+(r)+(s) Binding to the registered task (RFC 0005
                // §3): the task exists in prior chain state or earlier in
                // this block, the receipt carries its input_commitment,
                // and the receipt's executor is the one the task named.
                // Rule (g) above already proved the anchoring sender IS
                // receipt.executor, so (s) makes the sender the authorised
                // party. Corrupt stored task bytes are a storage failure,
                // not an unknown task.
                let stored_task = load_task(storage.as_ref(), &receipt.task_id)
                    .map_err(ApplyBlockError::Storage)?;
                let registered =
                    stored_task.as_ref().or_else(|| pending_tasks.get(&receipt.task_id));
                check_receipt_binding(receipt, registered).map_err(|why| match why {
                    BindingViolation::TaskNotRegistered => ApplyBlockError::TaskNotRegistered(i),
                    BindingViolation::InputCommitmentMismatch => {
                        ApplyBlockError::ReceiptInputCommitmentMismatch(i)
                    }
                    BindingViolation::ExecutorNotAuthorised => {
                        ApplyBlockError::ReceiptExecutorNotAuthorised(i)
                    }
                })?;
                pending_task_ids.insert(receipt.task_id);

                // Effects: accumulated only; committed in the final atomic
                // batch. Stored bytes are the canonical SCALE encoding;
                // storage never decodes them. The sequence number comes
                // from the local counter — no persistent mutation here.
                last_seq += 1;
                ops.push(BatchOp::PutTransaction(tx_hash, tx.clone()));
                ops.push(BatchOp::PutTxSeqIndex(last_seq, tx_hash));
                ops.push(BatchOp::PutReceipt(receipt.task_id, receipt.encode()));
                account_cache.insert(sender_addr, sender);
            } else {
                // ── Transfer path: unchanged v0.2 semantics. Stake
                // deliberately still falls through here (RFC 0002
                // Non-Goals); the ComputeTask fall-through ended with
                // RFC 0005 §5.1 and is rejected by rule (k) above. ──────

                // Load sender (from cache or storage).
                let sender_addr = tx.sender;
                let mut sender = match account_cache.get(&sender_addr) {
                    Some(acc) => acc.clone(),
                    None => storage
                        .get_account(&sender_addr)
                        .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                        .ok_or(ApplyBlockError::InsufficientBalance(i))?,
                };

                sender
                    .validate_and_increment_nonce(tx.nonce)
                    .map_err(|_| ApplyBlockError::InvalidNonce(i))?;

                // Load receiver (from cache or storage).
                let receiver_addr = tx.receiver;
                let mut receiver = match account_cache.get(&receiver_addr) {
                    Some(acc) => acc.clone(),
                    None => storage
                        .get_account(&receiver_addr)
                        .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                        .unwrap_or_else(|| Account::new(receiver_addr)),
                };

                Account::transfer(&mut sender, &mut receiver, tx.amount)
                    .map_err(|_| ApplyBlockError::InsufficientBalance(i))?;

                // Allocate the sequence number from the local counter —
                // no persistent mutation during validation.
                last_seq += 1;

                ops.push(BatchOp::PutTransaction(tx_hash, tx.clone()));
                ops.push(BatchOp::PutTxSeqIndex(last_seq, tx_hash));

                account_cache.insert(sender_addr, sender);
                account_cache.insert(receiver_addr, receiver);
            }
        }

        // Flush modified accounts.
        for (addr, account) in &account_cache {
            ops.push(BatchOp::PutAccount(*addr, account.clone()));
        }

        if !block.body.transactions.is_empty() {
            // Persist the sequence state only now, in the same atomic
            // batch as everything else: the counter baseline was read once
            // before the loop and advanced locally, so a rejected block
            // leaves both meta keys byte-for-byte unchanged, and sequence
            // values are a pure function of accepted chain history.
            ops.push(BatchOp::SetTxSeq(last_seq));
            ops.push(BatchOp::SetLastIncludedTxSeq(last_seq));
        }

        let block_hash = compute_block_hash(block);
        ops.push(BatchOp::PutBlock(block_hash, block.clone()));
        ops.push(BatchOp::PutBlockHeightIndex(
            block.header.height,
            block_hash,
        ));

        // Atomic commit.
        storage.write_batch(ops).map_err(|e| ApplyBlockError::Storage(e.to_string()))?;

        Ok(block_hash)
    }

    /// Handle a block received from a peer via block announcement.
    ///
    /// Validates and applies the block if it extends our chain by exactly
    /// one height. Blocks at unexpected heights are silently ignored.
    /// Invalid blocks are logged and discarded.
    ///
    /// Does NOT return errors to the network layer.
    pub fn handle_incoming_block(&self, block: Block) {
        let local_height = match self.storage.get_latest_height() {
            Ok(h) => h,
            Err(e) => {
                warn!("Failed to read local height: {e}");
                return;
            }
        };

        if block.header.height != local_height + 1 {
            info!(
                "Ignoring block at height {} (local height is {local_height})",
                block.header.height
            );
            return;
        }

        match self.apply_block(&block) {
            Ok(hash) => {
                info!(
                    "Applied incoming block: height={}, hash={hash}",
                    block.header.height
                );
            }
            Err(e) => {
                warn!(
                    "Rejected incoming block at height {}: {e}",
                    block.header.height
                );
            }
        }
    }
}

/// Errors from [`NodeBackend::apply_block`].
#[derive(Debug, thiserror::Error)]
pub enum ApplyBlockError {
    /// Parent hash does not match the current chain tip.
    #[error("bad parent: expected {expected}, got {got}")]
    BadParent {
        /// Expected parent hash (current tip).
        expected: Hash,
        /// Parent hash in the block header.
        got: Hash,
    },
    /// Block height does not follow the current chain tip.
    #[error("bad height: expected {expected}, got {got}")]
    BadHeight {
        /// Expected next height.
        expected: u64,
        /// Height in the block header.
        got: u64,
    },
    /// The transactions_root commitment does not match the body.
    #[error("transactions_root mismatch")]
    TransactionsRootMismatch,
    /// The payload variant does not match the transaction type (rule a).
    #[error("payload does not match transaction type at index {0}")]
    TypePayloadMismatch(usize),
    /// An `AnchorReceipt` transaction has a non-zero amount or a non-zero
    /// receiver (rule b).
    #[error("anchor receipt requires amount 0 and zero receiver at index {0}")]
    AnchorFieldConstraint(usize),
    /// A transaction in the block has an invalid signature (rule c).
    #[error("invalid transaction signature at index {0}")]
    InvalidSignature(usize),
    /// The sender account of an `AnchorReceipt` or `ComputeTask`
    /// transaction does not exist (rule d: the nonce rule requires an
    /// existing account).
    #[error("sender account missing at index {0}")]
    SenderAccountMissing(usize),
    /// A transaction has an invalid nonce (rule d).
    #[error("invalid nonce at index {0}")]
    InvalidNonce(usize),
    /// A transaction has insufficient balance.
    #[error("insufficient balance at index {0}")]
    InsufficientBalance(usize),
    /// The receipt metadata exceeds `MAX_RECEIPT_METADATA_BYTES` (rule e).
    #[error("receipt metadata too large at index {0}")]
    ReceiptMetadataTooLarge(usize),
    /// The receipt version is not supported (rule f).
    #[error("unsupported receipt version at index {0}")]
    ReceiptVersionUnsupported(usize),
    /// The transaction sender is not the receipt executor (rule g).
    #[error("sender must equal receipt executor at index {0}")]
    SenderExecutorMismatch(usize),
    /// The receipt signature does not verify (rule h).
    #[error("invalid receipt signature at index {0}")]
    InvalidReceiptSignature(usize),
    /// The task id is already anchored in prior chain state (rule i).
    #[error("task_id already anchored at index {0}")]
    TaskIdAlreadyAnchored(usize),
    /// The task id was anchored by an earlier transaction in the same
    /// block (rule j).
    #[error("task_id repeated within block at index {0}")]
    TaskIdRepeatedInBlock(usize),
    /// A `ComputeTask` transaction has a non-zero amount or a non-zero
    /// receiver (RFC 0005 rule l).
    #[error("compute task requires amount 0 and zero receiver at index {0}")]
    TaskFieldConstraint(usize),
    /// The task envelope version is not supported (RFC 0005 rule m).
    #[error("unsupported compute task version at index {0}")]
    TaskVersionUnsupported(usize),
    /// `task.execution_spec` exceeds `MAX_EXECUTION_SPEC_BYTES` (RFC 0005
    /// rule n).
    #[error("compute task execution_spec too large at index {0}")]
    ExecutionSpecTooLarge(usize),
    /// The transaction sender is not the task submitter (RFC 0005 rule o).
    #[error("sender must equal task submitter at index {0}")]
    SenderSubmitterMismatch(usize),
    /// A task with this task_id exists in prior chain state (RFC 0005
    /// rule p, prior-state half).
    #[error("task_id already registered at index {0}")]
    TaskAlreadyRegistered(usize),
    /// A task with this task_id was committed by an earlier transaction in
    /// the same block (RFC 0005 rule p, same-block half).
    #[error("compute task repeated within block at index {0}")]
    TaskRepeatedInBlock(usize),
    /// The receipt's task_id names no task in prior chain state or earlier
    /// in the same block (RFC 0005 rule q).
    #[error("receipt task_id is not a registered task at index {0}")]
    TaskNotRegistered(usize),
    /// The receipt's input_commitment is not the registered task's
    /// (RFC 0005 rule r).
    #[error("receipt input_commitment does not match the task at index {0}")]
    ReceiptInputCommitmentMismatch(usize),
    /// The receipt's executor is not the one the registered task named
    /// (RFC 0005 rule s).
    #[error("receipt executor is not authorised by the task at index {0}")]
    ReceiptExecutorNotAuthorised(usize),
    /// Storage error.
    #[error("storage error: {0}")]
    Storage(String),
}

/// Computes a deterministic blake3 hash over the SCALE-encoded transaction.
pub(crate) fn compute_tx_hash(tx: &Transaction) -> Hash {
    let encoded = tx.encode();
    let digest = blake3::hash(&encoded);
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_bytes());
    Hash(out)
}

/// Computes a deterministic blake3 hash over the SCALE-encoded block.
pub(crate) fn compute_block_hash(block: &Block) -> Hash {
    let encoded = block.encode();
    let digest = blake3::hash(&encoded);
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_bytes());
    Hash(out)
}

/// Returns the current Unix timestamp in seconds.
fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs()
}

// ── RpcBackend ──────────────────────────────────────────────────────────

impl<S: Storage + Send + Sync + 'static> RpcBackend for NodeBackend<S> {
    fn get_block_height(
        &self,
    ) -> impl std::future::Future<Output = Result<u64, BackendError>> + Send {
        let result = self
            .storage
            .get_latest_height()
            .map_err(|e| BackendError::Internal(format!("storage error: {e}")));
        std::future::ready(result)
    }

    // ping: use the default implementation ("pong").

    fn submit_transaction(
        &self,
        tx: Transaction,
    ) -> impl std::future::Future<Output = Result<String, BackendError>> + Send {
        let storage = Arc::clone(&self.storage);
        let mempool = Arc::clone(&self.mempool);
        async move {
            // ── Admission checks mirroring apply_block's normative order
            // (RFC 0002 §2, RFC 0005 §3). Admission is best-effort:
            // consensus in apply_block stays authoritative, and nothing
            // here mutates storage. ────────────────────────────────────

            // (a)/(k) Type/form.
            let Ok(typed) = check_type_payload(&tx) else {
                return Err(BackendError::Internal(
                    "payload does not match transaction type".to_string(),
                ));
            };
            // (b)/(l) Field constraints. Copy what the checks below need
            // so the borrow of `tx` ends before it is moved into the
            // mempool. The task envelope verdict is computed here (a pure
            // function of the transaction) but acted on later, in the
            // position apply_block gives rules (m)–(o).
            let canonical_fields = tx.amount == 0 && tx.receiver == Address::zero();
            let (anchor, task_check) = match typed {
                TypedPayload::Anchor(receipt) => {
                    if !canonical_fields {
                        return Err(BackendError::Internal(
                            "anchor receipt requires amount 0 and zero receiver".to_string(),
                        ));
                    }
                    (Some((receipt.clone(), receipt.task_id)), None)
                }
                TypedPayload::Task(task) => {
                    if !canonical_fields {
                        return Err(BackendError::Internal(
                            "compute task requires amount 0 and zero receiver".to_string(),
                        ));
                    }
                    (None, Some(check_task_envelope(&tx, task)))
                }
                TypedPayload::Plain => (None, None),
            };

            let tx_hash = compute_tx_hash(&tx);

            // Already-included transaction handling. Plain types keep the
            // idempotent success (return the hash). Re-submitting an
            // anchored AnchorReceipt or a registered ComputeTask — even
            // byte-identically — is a duplicate attempt and is rejected,
            // mirroring apply_block's stored-transaction rules.
            if storage
                .get_transaction(&tx_hash)
                .map_err(|_| BackendError::Internal("storage error".to_string()))?
                .is_some()
            {
                if anchor.is_some() {
                    return Err(BackendError::Internal(
                        "task_id already anchored".to_string(),
                    ));
                }
                if task_check.is_some() {
                    return Err(BackendError::Internal(
                        "task_id already registered".to_string(),
                    ));
                }
                return Ok(tx_hash.to_string());
            }

            // Verify signature.
            if !tx.verify_signature() {
                return Err(BackendError::Internal("invalid signature".to_string()));
            }

            // ── Pending-aware admission (issue #100) ──────────────────
            // The mempool write guard is taken here rather than just
            // before insertion, and held to the end. The expected nonce is
            // now a function of pending state, so computing it outside the
            // guard would let two concurrent same-node submissions each
            // conclude they own the same nonce slot. Everything from here
            // to insertion is one critical section.
            let mut pool = mempool.write().await;

            // Idempotent re-submission of an already pending transaction.
            // This moved ahead of the nonce rules: the transaction
            // occupying this nonce slot is this very transaction, so a
            // pending-aware check would otherwise reject a byte-identical
            // retry that previously succeeded.
            if pool.contains_hash(&tx_hash) {
                return Ok(tx_hash.to_string());
            }

            // Load sender account for validation (nonce, balance).
            let sender_addr = tx.sender;
            let sender = storage
                .get_account(&sender_addr)
                .map_err(|_| BackendError::Internal("storage error".to_string()))?
                .ok_or_else(|| BackendError::Internal("insufficient balance".to_string()))?;

            let pending = pool.sender_pending(&sender_addr, sender.nonce);

            // Validate nonce against committed state *and* what is already
            // pending. Consensus is untouched: apply_block still runs
            // `validate_and_increment_nonce` against its own advancing
            // account view (RFC 0002 rule d). This only stops admission
            // from rejecting a correct successor while its predecessor is
            // still waiting for a block.
            let Some(expected_nonce) = pending.expected_nonce else {
                return Err(BackendError::Internal(
                    "invalid nonce: sender nonce space exhausted".to_string(),
                ));
            };
            if tx.nonce != expected_nonce {
                // On a gap this reports the *missing* nonce, which is what
                // the client has to submit to make progress again.
                return Err(BackendError::Internal(format!(
                    "invalid nonce: expected {expected_nonce}"
                )));
            }

            // Validate balance against committed balance less what the
            // pending chain already spends. (Vacuous for AnchorReceipt:
            // amount is 0.) Pending *incoming* credits are not counted, so
            // admission stays conservative and block application remains
            // the authority on validity.
            match pending.pending_debit.checked_add(tx.amount) {
                Some(total) if sender.balance >= total => {}
                _ => {
                    return Err(BackendError::Internal("insufficient balance".to_string()));
                }
            }

            // Bound per-sender pending growth (node-local resource policy).
            if pending.len >= MAX_PENDING_PER_SENDER {
                return Err(BackendError::Internal(format!(
                    "too many pending transactions for sender (max {MAX_PENDING_PER_SENDER})"
                )));
            }

            // Anchor-specific admission (rules e–i; RFC 0002 §2).
            if let Some((receipt, task_id)) = &anchor {
                // (e) Metadata size cap.
                if receipt.metadata.len() > MAX_RECEIPT_METADATA_BYTES {
                    return Err(BackendError::Internal(
                        "receipt metadata too large".to_string(),
                    ));
                }
                // (f) Receipt version.
                if receipt.version != RECEIPT_VERSION {
                    return Err(BackendError::Internal(
                        "unsupported receipt version".to_string(),
                    ));
                }
                // (g) Submitter identity.
                if tx.sender != receipt.executor {
                    return Err(BackendError::Internal(
                        "sender must equal receipt executor".to_string(),
                    ));
                }
                // (h) Receipt signature.
                if !verify_receipt_signature(receipt) {
                    return Err(BackendError::Internal(
                        "invalid receipt signature".to_string(),
                    ));
                }
                // (i) Already anchored in persistent state (read-only).
                if storage
                    .has_receipt(task_id)
                    .map_err(|_| BackendError::Internal("storage error".to_string()))?
                {
                    return Err(BackendError::Internal(
                        "task_id already anchored".to_string(),
                    ));
                }
                // (q)–(s) Binding to the registered task (RFC 0005 §3):
                // prior chain state, else a task pending in this mempool,
                // which precedes the receipt in the block this node would
                // produce (insertion order) — the same-block half.
                let stored_task =
                    load_task(storage.as_ref(), task_id).map_err(BackendError::Internal)?;
                let registered = stored_task.or_else(|| pool.pending_compute_task(task_id));
                check_receipt_binding(receipt, registered.as_ref()).map_err(|why| {
                    BackendError::Internal(
                        match why {
                            BindingViolation::TaskNotRegistered => {
                                "receipt task_id is not a registered task"
                            }
                            BindingViolation::InputCommitmentMismatch => {
                                "receipt input_commitment does not match the task"
                            }
                            BindingViolation::ExecutorNotAuthorised => {
                                "receipt executor is not authorised by the task"
                            }
                        }
                        .to_string(),
                    )
                })?;
            }

            // Task-specific admission (rules m–p; RFC 0005 §3).
            if let Some(check) = task_check {
                let task_id = match check {
                    Ok(task_id) => task_id,
                    Err(TaskRuleViolation::Version) => {
                        return Err(BackendError::Internal(
                            "unsupported compute task version".to_string(),
                        ));
                    }
                    Err(TaskRuleViolation::SpecTooLarge) => {
                        return Err(BackendError::Internal(
                            "compute task execution_spec too large".to_string(),
                        ));
                    }
                    Err(TaskRuleViolation::SubmitterMismatch) => {
                        return Err(BackendError::Internal(
                            "sender must equal task submitter".to_string(),
                        ));
                    }
                };
                // (p) Already registered in persistent state (read-only).
                if storage
                    .has_task(&task_id)
                    .map_err(|_| BackendError::Internal("storage error".to_string()))?
                {
                    return Err(BackendError::Internal(
                        "task_id already registered".to_string(),
                    ));
                }
                // Mempool-pending duplicate guard, the task counterpart of
                // the receipt guard below: two pending commitments of one
                // task would be drained into the same block and rule (p)
                // would reject the whole block.
                if pool.contains_compute_task_id(&task_id) {
                    return Err(BackendError::Internal(
                        "compute task already pending".to_string(),
                    ));
                }
            }

            // Mempool-pending duplicate task_id guard: without it, two
            // pending receipts for one task_id would be drained into the
            // same block and rule (j) would reject the whole block.
            if let Some((_, task_id)) = &anchor {
                if pool.contains_task_id(task_id) {
                    return Err(BackendError::Internal(
                        "task_id already pending".to_string(),
                    ));
                }
            }
            pool.insert(tx_hash, tx).map_err(|e| match e {
                MempoolError::DuplicateHash => {
                    BackendError::Internal("duplicate transaction".to_string())
                }
                MempoolError::DuplicateSenderNonce => {
                    BackendError::Internal("duplicate sender nonce".to_string())
                }
                MempoolError::DuplicateTaskId => {
                    BackendError::Internal("task_id already pending".to_string())
                }
                MempoolError::DuplicateComputeTask => {
                    BackendError::Internal("compute task already pending".to_string())
                }
            })?;

            Ok(tx_hash.to_string())
        }
    }

    fn produce_block(
        &self,
    ) -> impl std::future::Future<Output = Result<String, BackendError>> + Send {
        let storage = Arc::clone(&self.storage);
        let mempool = Arc::clone(&self.mempool);
        let backend = self.clone();
        async move {
            if !backend.is_producer {
                return Err(BackendError::Internal(
                    "node is not configured as producer".to_string(),
                ));
            }

            // Ensure genesis exists.
            if storage
                .get_block_by_height(0)
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?
                .is_none()
            {
                return Err(BackendError::Internal("genesis block required".to_string()));
            }

            let current_height = storage
                .get_latest_height()
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?;

            let parent_block = storage
                .get_block_by_height(current_height)
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?
                .ok_or_else(|| BackendError::Internal("parent block not found".to_string()))?;

            let parent_hash = compute_block_hash(&parent_block);
            let new_height = current_height + 1;

            // Select transactions from the mempool (insertion order),
            // WITHOUT removing them. They are removed further down, only
            // once the block has actually been applied.
            //
            // Issue #100: a destructive drain here would lose the whole
            // batch whenever application failed, and a pending chain turns
            // that from one transaction into a sender's entire chain.
            // Peeking makes the failure path mutate nothing at all, which
            // is a stronger guarantee than restoring state afterwards and
            // needs no restore logic to get right.
            //
            // The guard is released before application rather than held
            // across it. It is not needed for correctness: `sender_pending`
            // walks upward from the *committed* nonce, so an entry that
            // this block commits is simply below the walk's starting point
            // and cannot be counted twice. Holding it would block every
            // submission for the duration of a storage write for no gain.
            let selected = {
                let pool = mempool.read().await;
                pool.peek_for_block(MAX_TX_PER_BLOCK)
            };
            let txs: Vec<Transaction> = selected.iter().map(|(_, tx)| tx.clone()).collect();

            // Build the block.
            let block = Block {
                header: BlockHeader {
                    parent_hash,
                    state_root: Hash::zero(),
                    transactions_root: compute_transactions_root(&txs),
                    timestamp: now_secs(),
                    height: new_height,
                },
                body: BlockBody { transactions: txs },
            };

            // Delegate to apply_block (shared validation + atomic commit).
            // On failure this returns early and the mempool is untouched:
            // every selected transaction is still pending.
            let block_hash =
                backend.apply_block(&block).map_err(|e| BackendError::Internal(e.to_string()))?;

            // Applied and committed — only now do the included transactions
            // leave the mempool.
            {
                let hashes: Vec<Hash> = selected.into_iter().map(|(h, _)| h).collect();
                mempool.write().await.remove_included(&hashes);
            }

            // Broadcast the newly produced block to connected peers.
            if let Some(ref broadcaster) = backend.broadcaster {
                broadcaster.broadcast(block);
            }

            Ok(block_hash.to_string())
        }
    }

    fn get_latest_block_hash(
        &self,
    ) -> impl std::future::Future<Output = Result<String, BackendError>> + Send {
        let storage = Arc::clone(&self.storage);
        async move {
            let height = storage
                .get_latest_height()
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?;

            let block = storage
                .get_block_by_height(height)
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?
                .ok_or_else(|| {
                    BackendError::Internal(format!("block not found at height {height}"))
                })?;

            Ok(compute_block_hash(&block).to_string())
        }
    }

    fn get_block_by_height(
        &self,
        height: u64,
    ) -> impl std::future::Future<Output = Result<serde_json::Value, BackendError>> + Send {
        let storage = Arc::clone(&self.storage);
        async move {
            let block = storage
                .get_block_by_height(height)
                .map_err(|e| BackendError::Internal(format!("storage error: {e}")))?
                .ok_or_else(|| {
                    BackendError::Internal(format!("block not found at height {height}"))
                })?;

            serde_json::to_value(&block)
                .map_err(|e| BackendError::Internal(format!("serialization error: {e}")))
        }
    }
}

// ── ApiBackend ──────────────────────────────────────────────────────────

#[async_trait]
impl<S: Storage + Send + Sync + 'static> ApiBackend for NodeBackend<S> {
    async fn list_blocks(&self, limit: u32) -> Result<Vec<BlockSummary>, ApiError> {
        let latest = self
            .storage
            .get_latest_height()
            .map_err(|e| ApiError::Internal(e.to_string()))?;

        let mut blocks = Vec::new();
        let count = std::cmp::min(limit as u64, latest + 1);
        for i in 0..count {
            let height = latest - i;
            if let Some(block) = self
                .storage
                .get_block_by_height(height)
                .map_err(|e| ApiError::Internal(e.to_string()))?
            {
                let hash = compute_block_hash(&block);
                blocks.push(BlockSummary {
                    hash: hash.to_string(),
                    height: block.header.height,
                    timestamp: block.header.timestamp,
                });
            }
        }
        Ok(blocks)
    }

    async fn get_block(&self, hash: String) -> Result<BlockDetail, ApiError> {
        let parsed: Hash = hash.parse().map_err(|e: String| ApiError::Invalid(e))?;

        let block = self
            .storage
            .get_block(&parsed)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or(ApiError::NotFound)?;

        Ok(BlockDetail {
            hash: parsed.to_string(),
            height: block.header.height,
            timestamp: block.header.timestamp,
            parent_hash: block.header.parent_hash.to_string(),
            tx_count: block.body.transactions.len() as u32,
        })
    }

    async fn get_transaction(&self, hash: String) -> Result<RestTransaction, ApiError> {
        let parsed: Hash = hash.parse().map_err(|e: String| ApiError::Invalid(e))?;

        let tx = self
            .storage
            .get_transaction(&parsed)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or(ApiError::NotFound)?;

        Ok(RestTransaction {
            hash: parsed.to_string(),
            from: tx.sender.to_string(),
            to: Some(tx.receiver.to_string()),
            value: tx.amount.to_string(),
            block_hash: None,
            block_height: None,
        })
    }

    async fn get_account(&self, address: String) -> Result<RestAccount, ApiError> {
        let parsed: Address = address.parse().map_err(|e: String| ApiError::Invalid(e))?;

        let account = self
            .storage
            .get_account(&parsed)
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .ok_or(ApiError::NotFound)?;

        Ok(RestAccount {
            address: parsed.to_string(),
            balance: account.balance.to_string(),
            nonce: account.nonce,
        })
    }

    async fn list_validators(&self) -> Result<Vec<Validator>, ApiError> {
        // Phase 1 minimal: no validator tracking yet.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use mbongo_core::{
        Account, Address, Block, BlockBody, BlockHeader, Hash, Receipt, Transaction,
        TransactionPayload, TransactionType,
    };
    use mbongo_storage::InMemoryStorage;

    /// Creates a backend with producer role enabled (default for most tests).
    fn make_backend() -> NodeBackend<InMemoryStorage> {
        NodeBackend::new(InMemoryStorage::new(), true)
    }

    fn sample_block() -> (Hash, Block) {
        let hash = Hash([5u8; 32]);
        let block = Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: Hash::zero(),
                timestamp: 1_700_000_000,
                height: 1,
            },
            body: BlockBody {
                transactions: vec![Transaction {
                    tx_type: TransactionType::Transfer,
                    sender: Address::zero(),
                    receiver: Address([6u8; 32]),
                    amount: 50,
                    nonce: 0,
                    payload: TransactionPayload::None,
                    signature: [0u8; 64],
                }],
            },
        };
        (hash, block)
    }

    fn sample_tx() -> (Hash, Transaction) {
        let hash = Hash([2u8; 32]);
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address([3u8; 32]),
            receiver: Address([4u8; 32]),
            amount: 100,
            nonce: 0,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        (hash, tx)
    }

    fn sample_account() -> (Address, Account) {
        let addr = Address([1u8; 32]);
        let mut account = Account::new(addr);
        account.balance = 42_000;
        account.nonce = 3;
        (addr, account)
    }

    /// Creates a properly signed transfer transaction from `sender_sk` to `receiver_addr`.
    fn signed_transfer(
        sender_sk: &SigningKey,
        receiver_addr: Address,
        amount: u128,
        nonce: u64,
    ) -> Transaction {
        let vk: VerifyingKey = sender_sk.verifying_key();
        let sender = Address(vk.to_bytes());
        let mut tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender,
            receiver: receiver_addr,
            amount,
            nonce,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        let sig = sender_sk.sign(&tx.signing_payload());
        tx.signature = sig.to_bytes();
        tx
    }

    // ── RpcBackend tests ────────────────────────────────────────────

    #[tokio::test]
    async fn rpc_ping_returns_pong() {
        let backend = make_backend();
        let result = backend.ping().await;
        assert_eq!(result.unwrap(), "pong");
    }

    #[tokio::test]
    async fn rpc_block_height_returns_zero() {
        let backend = make_backend();
        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 0);
    }

    // ── submit_transaction tests ────────────────────────────────────

    #[tokio::test]
    async fn submit_tx_success_updates_balances() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[1u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([9u8; 32]);

        // Fund sender.
        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 1000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        let tx = signed_transfer(&sender_sk, receiver_addr, 300, 0);
        let hash = backend.submit_transaction(tx).await.unwrap();
        assert!(hash.starts_with("0x"));

        // submit_transaction inserts into mempool only; produce_block persists.
        backend.produce_block().await.unwrap();

        // Verify sender balance decreased.
        let s = backend.storage.get_account(&sender_addr).unwrap().unwrap();
        assert_eq!(s.balance, 700);
        assert_eq!(s.nonce, 1);

        // Verify receiver balance increased.
        let r = backend.storage.get_account(&receiver_addr).unwrap().unwrap();
        assert_eq!(r.balance, 300);
        assert_eq!(r.nonce, 0);
    }

    #[tokio::test]
    async fn submit_tx_nonce_mismatch_fails() {
        let backend = make_backend();
        let sender_sk = SigningKey::from_bytes(&[2u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([10u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 1000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        // nonce=5 but account nonce is 0.
        let tx = signed_transfer(&sender_sk, receiver_addr, 100, 5);
        let result = backend.submit_transaction(tx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid nonce"), "got: {err}");
    }

    #[tokio::test]
    async fn submit_tx_insufficient_balance_fails() {
        let backend = make_backend();
        let sender_sk = SigningKey::from_bytes(&[3u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([11u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 50;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        let tx = signed_transfer(&sender_sk, receiver_addr, 100, 0);
        let result = backend.submit_transaction(tx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("insufficient balance"), "got: {err}");
    }

    #[tokio::test]
    async fn submit_tx_duplicate_returns_same_hash() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[4u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([12u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 1000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        let tx = signed_transfer(&sender_sk, receiver_addr, 200, 0);
        let hash1 = backend.submit_transaction(tx.clone()).await.unwrap();

        // Submit the same transaction again (idempotent).
        let hash2 = backend.submit_transaction(tx).await.unwrap();
        assert_eq!(hash1, hash2);

        // Produce block — only one tx in mempool (duplicate was rejected from re-insert).
        backend.produce_block().await.unwrap();

        // Balance must only be debited once.
        let s = backend.storage.get_account(&sender_addr).unwrap().unwrap();
        assert_eq!(s.balance, 800);
    }

    #[tokio::test]
    async fn submit_tx_invalid_signature_fails() {
        let backend = make_backend();
        let sender_sk = SigningKey::from_bytes(&[5u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([13u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 1000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        // Create a transaction but tamper with it after signing.
        let mut tx = signed_transfer(&sender_sk, receiver_addr, 100, 0);
        tx.amount = 999; // tamper

        let result = backend.submit_transaction(tx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid signature"), "got: {err}");
    }

    // ── ApiBackend tests ────────────────────────────────────────────

    #[tokio::test]
    async fn api_list_blocks_returns_empty() {
        let backend = make_backend();
        let blocks = backend.list_blocks(10).await.unwrap();
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn api_list_validators_returns_empty() {
        let backend = make_backend();
        let validators = backend.list_validators().await.unwrap();
        assert!(validators.is_empty());
    }

    #[tokio::test]
    async fn api_get_block_not_found() {
        let backend = make_backend();
        let result = backend.get_block(Hash::zero().to_string()).await;
        assert!(matches!(result, Err(ApiError::NotFound)));
    }

    #[tokio::test]
    async fn api_get_block_roundtrip() {
        let backend = make_backend();
        let (hash, block) = sample_block();
        backend.storage.put_block(&hash, &block).unwrap();

        let detail = backend.get_block(hash.to_string()).await.unwrap();
        assert_eq!(detail.height, 1);
        assert_eq!(detail.timestamp, 1_700_000_000);
        assert_eq!(detail.tx_count, 1);
        assert_eq!(detail.hash, hash.to_string());
        assert_eq!(detail.parent_hash, Hash::zero().to_string());
    }

    #[tokio::test]
    async fn api_get_transaction_not_found() {
        let backend = make_backend();
        let result = backend.get_transaction(Hash::zero().to_string()).await;
        assert!(matches!(result, Err(ApiError::NotFound)));
    }

    #[tokio::test]
    async fn api_get_transaction_roundtrip() {
        let backend = make_backend();
        let (hash, tx) = sample_tx();
        backend.storage.put_transaction(&hash, &tx).unwrap();

        let rest_tx = backend.get_transaction(hash.to_string()).await.unwrap();
        assert_eq!(rest_tx.hash, hash.to_string());
        assert_eq!(rest_tx.from, Address([3u8; 32]).to_string());
        assert_eq!(rest_tx.to, Some(Address([4u8; 32]).to_string()));
        assert_eq!(rest_tx.value, "100");
    }

    #[tokio::test]
    async fn api_get_account_not_found() {
        let backend = make_backend();
        let result = backend.get_account(Address::zero().to_string()).await;
        assert!(matches!(result, Err(ApiError::NotFound)));
    }

    #[tokio::test]
    async fn api_get_account_roundtrip() {
        let backend = make_backend();
        let (addr, account) = sample_account();
        backend.storage.put_account(&addr, &account).unwrap();

        let rest_acc = backend.get_account(addr.to_string()).await.unwrap();
        assert_eq!(rest_acc.address, addr.to_string());
        assert_eq!(rest_acc.balance, "42000");
        assert_eq!(rest_acc.nonce, 3);
    }

    #[tokio::test]
    async fn api_get_block_invalid_hash() {
        let backend = make_backend();
        let result = backend.get_block("not-a-hash".to_string()).await;
        assert!(matches!(result, Err(ApiError::Invalid(_))));
    }

    #[tokio::test]
    async fn api_get_account_invalid_address() {
        let backend = make_backend();
        let result = backend.get_account("bad".to_string()).await;
        assert!(matches!(result, Err(ApiError::Invalid(_))));
    }

    // ── Genesis tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn ensure_genesis_creates_block_at_height_zero() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        // Height should still be 0 (genesis).
        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 0);

        // Block at height 0 should exist.
        let block = backend.storage.get_block_by_height(0).unwrap().expect("genesis block");
        assert_eq!(block.header.height, 0);
        assert_eq!(block.header.parent_hash, Hash::zero());
        assert_eq!(block.header.timestamp, 0);
        assert!(block.body.transactions.is_empty());
    }

    #[tokio::test]
    async fn ensure_genesis_is_idempotent() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        backend.ensure_genesis().unwrap(); // second call should be a no-op

        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 0);
    }

    #[tokio::test]
    async fn ensure_genesis_creates_dev_account() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        // Deterministic dev key (must match ensure_genesis and wallet example).
        let sk = SigningKey::from_bytes(&[0xAAu8; 32]);
        let dev_addr = Address(sk.verifying_key().to_bytes());
        let account = backend
            .storage
            .get_account(&dev_addr)
            .unwrap()
            .expect("dev account should exist after genesis");
        assert_eq!(account.balance, 1_000_000_000);
        assert_eq!(account.nonce, 0);
    }

    // ── Block production tests ──────────────────────────────────────

    #[tokio::test]
    async fn produce_block_creates_height_one() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let hash = backend.produce_block().await.unwrap();
        assert!(hash.starts_with("0x"));

        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 1);

        let block = backend.storage.get_block_by_height(1).unwrap().expect("block at height 1");
        assert_eq!(block.header.height, 1);
        assert!(block.body.transactions.is_empty());

        // Parent hash should be the genesis block hash.
        let genesis = backend.storage.get_block_by_height(0).unwrap().unwrap();
        let genesis_hash = compute_block_hash(&genesis);
        assert_eq!(block.header.parent_hash, genesis_hash);
    }

    #[tokio::test]
    async fn produce_block_increments_height() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        backend.produce_block().await.unwrap();
        backend.produce_block().await.unwrap();
        backend.produce_block().await.unwrap();

        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 3);

        // Verify chain linkage: block 3's parent should be block 2's hash.
        let block2 = backend.storage.get_block_by_height(2).unwrap().unwrap();
        let block3 = backend.storage.get_block_by_height(3).unwrap().unwrap();
        let block2_hash = compute_block_hash(&block2);
        assert_eq!(block3.header.parent_hash, block2_hash);
    }

    #[tokio::test]
    async fn produce_block_fails_without_genesis() {
        let backend = make_backend();
        // No genesis → must fail.
        let result = backend.produce_block().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("genesis") || err.contains("parent block not found"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn api_list_blocks_after_genesis_and_produce() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        backend.produce_block().await.unwrap();

        let blocks = backend.list_blocks(10).await.unwrap();
        assert_eq!(blocks.len(), 2);
        // Most recent first.
        assert_eq!(blocks[0].height, 1);
        assert_eq!(blocks[1].height, 0);
    }

    // ── Transaction inclusion tests ─────────────────────────────────

    #[tokio::test]
    async fn produce_block_includes_submitted_transactions() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[10u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([20u8; 32]);

        // Fund sender with enough for 3 transfers.
        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 10_000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        // With mempool: only one tx per sender at a time (nonce must match account).
        // Submit, produce, submit, produce, submit, produce.
        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 100, 0))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 200, 1))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 300, 2))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        // Verify all 3 transactions in blocks 1, 2, 3.
        let block1 = backend.storage.get_block_by_height(1).unwrap().expect("block 1");
        let block2 = backend.storage.get_block_by_height(2).unwrap().expect("block 2");
        let block3 = backend.storage.get_block_by_height(3).unwrap().expect("block 3");
        assert_eq!(block1.body.transactions.len(), 1);
        assert_eq!(block1.body.transactions[0].amount, 100);
        assert_eq!(block2.body.transactions.len(), 1);
        assert_eq!(block2.body.transactions[0].amount, 200);
        assert_eq!(block3.body.transactions.len(), 1);
        assert_eq!(block3.body.transactions[0].amount, 300);
    }

    #[tokio::test]
    async fn produce_block_second_block_has_no_duplicates() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[11u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([21u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 10_000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        // Submit 1, produce; submit 2, produce; produce again (empty block).
        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 100, 0))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 200, 1))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        // Third block: no new transactions.
        backend.produce_block().await.unwrap();

        let block1 = backend.storage.get_block_by_height(1).unwrap().unwrap();
        let block2 = backend.storage.get_block_by_height(2).unwrap().unwrap();
        let block3 = backend.storage.get_block_by_height(3).unwrap().unwrap();
        assert_eq!(block1.body.transactions.len(), 1);
        assert_eq!(block2.body.transactions.len(), 1);
        assert_eq!(block3.body.transactions.len(), 0);
    }

    #[tokio::test]
    async fn produce_block_only_includes_new_transactions() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[12u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([22u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 50_000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        // Submit 1, produce; submit 2, produce; submit 3, produce.
        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 100, 0))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 200, 1))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 300, 2))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        let block2 = backend.storage.get_block_by_height(2).unwrap().unwrap();
        let block3 = backend.storage.get_block_by_height(3).unwrap().unwrap();
        assert_eq!(block2.body.transactions.len(), 1);
        assert_eq!(block2.body.transactions[0].amount, 200);
        assert_eq!(block3.body.transactions.len(), 1);
        assert_eq!(block3.body.transactions[0].amount, 300);
    }

    // ── Mempool integration tests ────────────────────────────────────────

    #[tokio::test]
    async fn submit_tx_inserts_into_mempool_not_storage() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[30u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([31u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 5000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        let tx = signed_transfer(&sender_sk, receiver_addr, 100, 0);
        let tx_hash = compute_tx_hash(&tx);
        backend.submit_transaction(tx).await.unwrap();

        // Transaction must NOT be in storage before produce_block.
        assert!(backend.storage.get_transaction(&tx_hash).unwrap().is_none());

        // After produce_block, it must be in storage.
        backend.produce_block().await.unwrap();
        assert!(backend.storage.get_transaction(&tx_hash).unwrap().is_some());
    }

    #[tokio::test]
    async fn submit_tx_duplicate_hash_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[32u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([33u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 1000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        let tx = signed_transfer(&sender_sk, receiver_addr, 100, 0);
        backend.submit_transaction(tx.clone()).await.unwrap();
        // Second submit with same tx returns same hash (idempotent).
        let hash2 = backend.submit_transaction(tx).await.unwrap();
        assert!(hash2.starts_with("0x"));

        backend.produce_block().await.unwrap();
        // Only one tx in block.
        let block = backend.storage.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(block.body.transactions.len(), 1);
    }

    #[tokio::test]
    async fn submit_tx_duplicate_sender_nonce_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[34u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([35u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 2000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        let tx1 = signed_transfer(&sender_sk, receiver_addr, 100, 0);
        backend.submit_transaction(tx1).await.unwrap();

        // Same (sender, nonce) but different content (different receiver) → valid sig, different hash.
        let tx2 = signed_transfer(&sender_sk, Address([36u8; 32]), 200, 0);
        let result = backend.submit_transaction(tx2).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("duplicate") || err.contains("nonce"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn mempool_empty_after_produce() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sender_sk = SigningKey::from_bytes(&[37u8; 32]);
        let sender_addr = Address(sender_sk.verifying_key().to_bytes());
        let receiver_addr = Address([38u8; 32]);

        let mut sender_acc = Account::new(sender_addr);
        sender_acc.balance = 1000;
        backend.storage.put_account(&sender_addr, &sender_acc).unwrap();

        backend
            .submit_transaction(signed_transfer(&sender_sk, receiver_addr, 100, 0))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        backend.produce_block().await.unwrap();
        let block2 = backend.storage.get_block_by_height(2).unwrap().unwrap();
        assert_eq!(block2.body.transactions.len(), 0);
    }

    // ── Atomic write_batch tests ────────────────────────────────────────

    #[tokio::test]
    async fn produce_block_applies_all_state_atomically() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        // Two distinct senders to avoid nonce contention.
        let sk_a = SigningKey::from_bytes(&[40u8; 32]);
        let addr_a = Address(sk_a.verifying_key().to_bytes());
        let sk_b = SigningKey::from_bytes(&[41u8; 32]);
        let addr_b = Address(sk_b.verifying_key().to_bytes());
        let receiver = Address([42u8; 32]);

        // Fund both senders.
        let mut acc_a = Account::new(addr_a);
        acc_a.balance = 5000;
        backend.storage.put_account(&addr_a, &acc_a).unwrap();
        let mut acc_b = Account::new(addr_b);
        acc_b.balance = 3000;
        backend.storage.put_account(&addr_b, &acc_b).unwrap();

        // Submit two transactions from different senders.
        backend
            .submit_transaction(signed_transfer(&sk_a, receiver, 100, 0))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk_b, receiver, 200, 0))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        // Verify all state was applied consistently.
        let block = backend.storage.get_block_by_height(1).unwrap().expect("block 1");
        assert_eq!(block.body.transactions.len(), 2);

        let final_a = backend.storage.get_account(&addr_a).unwrap().unwrap();
        assert_eq!(final_a.balance, 4900);
        assert_eq!(final_a.nonce, 1);

        let final_b = backend.storage.get_account(&addr_b).unwrap().unwrap();
        assert_eq!(final_b.balance, 2800);
        assert_eq!(final_b.nonce, 1);

        let final_r = backend.storage.get_account(&receiver).unwrap().unwrap();
        assert_eq!(final_r.balance, 300);

        // Block, height index, and latest height all consistent.
        assert_eq!(backend.get_block_height().await.unwrap(), 1);
        let block_hash = compute_block_hash(&block);
        assert!(backend.storage.get_block(&block_hash).unwrap().is_some());
    }

    // ── apply_block tests ──────────────────────────────────────────────

    /// Build a valid block on top of the current chain tip.
    fn build_valid_block<S: Storage>(backend: &NodeBackend<S>, txs: Vec<Transaction>) -> Block {
        let current_height = backend.storage.get_latest_height().unwrap();
        let parent = backend.storage.get_block_by_height(current_height).unwrap().unwrap();
        let parent_hash = compute_block_hash(&parent);
        Block {
            header: BlockHeader {
                parent_hash,
                state_root: Hash::zero(),
                transactions_root: compute_transactions_root(&txs),
                timestamp: now_secs(),
                height: current_height + 1,
            },
            body: BlockBody { transactions: txs },
        }
    }

    /// Builds a receipt with the given fields, signed by `executor_sk`
    /// over the raw receipt hash (valid receipt signature).
    fn signed_receipt_for(
        executor_sk: &SigningKey,
        task_id: [u8; 32],
        metadata: Vec<u8>,
        version: u8,
    ) -> Receipt {
        let executor = Address(executor_sk.verifying_key().to_bytes());
        let mut receipt = Receipt {
            version,
            task_id,
            input_commitment: [1u8; 32],
            output_commitment: [2u8; 32],
            executor,
            metadata,
            signature: [0u8; 64],
        };
        receipt.signature = executor_sk.sign(&receipt.receipt_hash().0).to_bytes();
        receipt
    }

    /// Wraps a receipt in a canonically formed `AnchorReceipt` transaction
    /// (amount 0, zero receiver) signed by `sk`.
    fn signed_anchor_tx(sk: &SigningKey, nonce: u64, receipt: Receipt) -> Transaction {
        let sender = Address(sk.verifying_key().to_bytes());
        let mut tx = Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender,
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
            signature: [0u8; 64],
        };
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        tx
    }

    /// Fully valid anchor transaction: sender == executor, both signatures
    /// valid, canonical fields.
    fn valid_anchor_tx(sk: &SigningKey, nonce: u64, task_id: [u8; 32]) -> Transaction {
        signed_anchor_tx(sk, nonce, signed_receipt_for(sk, task_id, vec![1, 2, 3], 1))
    }

    /// Registers, directly in storage, a task that `executor_sk` is
    /// authorised to answer, carrying the input commitment
    /// `signed_receipt_for` uses. Stands in for the block a client would
    /// have committed it in, exactly as `fund` stands in for a funding
    /// transfer, so the v0.3 anchoring tests keep their nonce and height
    /// arithmetic. Returns the task_id an anchor must carry (RFC 0005
    /// rules q–s).
    fn seed_task(
        backend: &NodeBackend<InMemoryStorage>,
        executor_sk: &SigningKey,
        salt: [u8; 32],
    ) -> [u8; 32] {
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: Address([0xC1u8; 32]),
            executor: Address(executor_sk.verifying_key().to_bytes()),
            salt,
            input_commitment: [1u8; 32],
            execution_spec: Vec::new(),
        };
        let task_id = task.task_id();
        backend
            .storage
            .write_batch(vec![BatchOp::PutTask(task_id, task.encode())])
            .unwrap();
        task_id
    }

    /// Funds an account for `sk` with the given balance.
    fn fund(backend: &NodeBackend<InMemoryStorage>, sk: &SigningKey, balance: u128) -> Address {
        let addr = Address(sk.verifying_key().to_bytes());
        let mut acc = Account::new(addr);
        acc.balance = balance;
        backend.storage.put_account(&addr, &acc).unwrap();
        addr
    }

    // ── RFC 0002 Phase 3: validation order and typed errors ──────────

    #[test]
    fn wrong_type_payload_pairing_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        // AnchorReceipt type with None payload.
        let mut tx = valid_anchor_tx(&sk, 0, [0x70u8; 32]);
        tx.payload = TransactionPayload::None;
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TypePayloadMismatch(0))
        ));

        // Transfer type carrying an AnchorReceipt payload.
        let receipt = signed_receipt_for(&sk, [0x71u8; 32], vec![], 1);
        let mut tx = signed_transfer(&sk, Address([9u8; 32]), 1, 0);
        tx.payload = TransactionPayload::AnchorReceipt(Box::new(receipt));
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TypePayloadMismatch(0))
        ));
    }

    #[test]
    fn anchor_field_constraints_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        // Non-zero amount.
        let mut tx = valid_anchor_tx(&sk, 0, [0x72u8; 32]);
        tx.amount = 1;
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::AnchorFieldConstraint(0))
        ));

        // Non-zero receiver.
        let mut tx = valid_anchor_tx(&sk, 0, [0x73u8; 32]);
        tx.receiver = Address([9u8; 32]);
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::AnchorFieldConstraint(0))
        ));
    }

    #[test]
    fn tx_signature_precedes_receipt_signature() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        // Both signatures invalid: rule (c) fires before rule (h).
        let receipt = signed_receipt_for(&sk, [0x74u8; 32], vec![], 1);
        let mut tx = signed_anchor_tx(
            &sk,
            0,
            Receipt {
                signature: [0xEEu8; 64], // invalid receipt signature
                ..receipt
            },
        );
        tx.signature = [0xEEu8; 64]; // invalid transaction signature
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidSignature(0))
        ));
    }

    #[test]
    fn nonce_precedes_receipt_signature() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        // Wrong nonce and invalid receipt signature: rule (d) fires first.
        let receipt = signed_receipt_for(&sk, [0x75u8; 32], vec![], 1);
        let tx = signed_anchor_tx(
            &sk,
            5,
            Receipt {
                signature: [0xEEu8; 64],
                ..receipt
            },
        );
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidNonce(0))
        ));
    }

    #[test]
    fn metadata_cap_enforced() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        let task_id = seed_task(&backend, &sk, [0x76u8; 32]);

        // 4097 bytes: rejected at rule (e).
        let receipt = signed_receipt_for(&sk, task_id, vec![0u8; 4097], 1);
        let tx = signed_anchor_tx(&sk, 0, receipt);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ReceiptMetadataTooLarge(0))
        ));

        // Exactly 4096 bytes: accepted.
        let receipt = signed_receipt_for(&sk, task_id, vec![0u8; 4096], 1);
        let tx = signed_anchor_tx(&sk, 0, receipt);
        let block = build_valid_block(&backend, vec![tx]);
        backend.apply_block(&block).unwrap();
        assert!(backend.storage.has_receipt(&task_id).unwrap());
    }

    #[test]
    fn wrong_receipt_version_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        // Version 2, correctly signed over the version-2 hash: rule (f)
        // fires, not the signature rule.
        let receipt = signed_receipt_for(&sk, [0x77u8; 32], vec![], 2);
        let tx = signed_anchor_tx(&sk, 0, receipt);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ReceiptVersionUnsupported(0))
        ));
    }

    #[test]
    fn sender_executor_mismatch_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        // Receipt validly signed by a DIFFERENT executor: rule (g) fires.
        let other = SigningKey::from_bytes(&[61u8; 32]);
        let receipt = signed_receipt_for(&other, [0x78u8; 32], vec![], 1);
        let tx = signed_anchor_tx(&sk, 0, receipt);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::SenderExecutorMismatch(0))
        ));
    }

    #[test]
    fn invalid_receipt_signature_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        let receipt = signed_receipt_for(&sk, [0x79u8; 32], vec![], 1);
        let tx = signed_anchor_tx(
            &sk,
            0,
            Receipt {
                signature: [0xEEu8; 64],
                ..receipt
            },
        );
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidReceiptSignature(0))
        ));
        assert!(!backend.storage.has_receipt(&[0x79u8; 32]).unwrap());
    }

    #[test]
    fn stored_anchor_tx_in_later_block_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        let sender_addr = fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x96u8; 32]);

        // Anchor once at height 1.
        let anchor = valid_anchor_tx(&sk, 0, task_id);
        let anchor_bytes = match &anchor.payload {
            TransactionPayload::AnchorReceipt(r) => r.encode(),
            TransactionPayload::None | TransactionPayload::ComputeTask(_) => unreachable!(),
        };
        let block = build_valid_block(&backend, vec![anchor.clone()]);
        backend.apply_block(&block).unwrap();
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 1);

        // The EXACT same transaction (identical hash, identical receipt
        // bytes) in a later block must be rejected — the stored-tx skip
        // path must not bypass global task_id uniqueness.
        let block = build_valid_block(&backend, vec![anchor]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskIdAlreadyAnchored(0))
        ));

        // The failed duplicate block changed nothing (read-only checks):
        // height, nonce, balance, sequence state, and receipt bytes are
        // all exactly the post-height-1 state.
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        let sender = backend.storage.get_account(&sender_addr).unwrap().unwrap();
        assert_eq!(sender.balance, 5000);
        assert_eq!(sender.nonce, 1);
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 1);
        assert!(backend.storage.get_tx_hash_by_seq(2).unwrap().is_none());
        assert_eq!(
            backend.storage.get_receipt(&task_id).unwrap(),
            Some(anchor_bytes)
        );
    }

    #[test]
    fn prior_state_duplicate_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x7Au8; 32]);

        // Anchor once.
        let block = build_valid_block(&backend, vec![valid_anchor_tx(&sk, 0, task_id)]);
        backend.apply_block(&block).unwrap();
        assert!(backend.storage.has_receipt(&task_id).unwrap());

        // Anchor the same task_id again in the next block: rule (i).
        let block = build_valid_block(&backend, vec![valid_anchor_tx(&sk, 1, task_id)]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskIdAlreadyAnchored(0))
        ));
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
    }

    #[test]
    fn in_block_duplicate_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x7Bu8; 32]);

        // Two receipts for one task_id in the same block: rule (j) fires
        // on the second, and the whole block — including the first — is
        // rejected with nothing written.
        let block = build_valid_block(
            &backend,
            vec![
                valid_anchor_tx(&sk, 0, task_id),
                valid_anchor_tx(&sk, 1, task_id),
            ],
        );
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskIdRepeatedInBlock(1))
        ));
        assert!(!backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
        let sender = backend.storage.get_account(&Address(sk.verifying_key().to_bytes()));
        assert_eq!(sender.unwrap().unwrap().nonce, 0);
        // No sequence state either.
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert!(backend.storage.get_tx_hash_by_seq(1).unwrap().is_none());
        assert!(backend.storage.get_tx_hash_by_seq(2).unwrap().is_none());
    }

    #[test]
    fn first_failure_precedence_pinned() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let other = SigningKey::from_bytes(&[61u8; 32]);

        // Oversized metadata AND wrong version: (e) before (f).
        let receipt = signed_receipt_for(&sk, [0x7Cu8; 32], vec![0u8; 4097], 2);
        let block = build_valid_block(&backend, vec![signed_anchor_tx(&sk, 0, receipt)]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ReceiptMetadataTooLarge(0))
        ));

        // Wrong version AND wrong executor: (f) before (g).
        let receipt = signed_receipt_for(&other, [0x7Cu8; 32], vec![], 2);
        let block = build_valid_block(&backend, vec![signed_anchor_tx(&sk, 0, receipt)]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ReceiptVersionUnsupported(0))
        ));

        // Wrong executor AND invalid receipt signature: (g) before (h).
        let receipt = signed_receipt_for(&other, [0x7Cu8; 32], vec![], 1);
        let block = build_valid_block(
            &backend,
            vec![signed_anchor_tx(
                &sk,
                0,
                Receipt {
                    signature: [0xEEu8; 64],
                    ..receipt
                },
            )],
        );
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::SenderExecutorMismatch(0))
        ));
    }

    // ── RFC 0002 Phase 3: atomicity ──────────────────────────────────

    #[test]
    fn transfer_before_invalid_receipt_leaves_no_state() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let transfer_sk = SigningKey::from_bytes(&[62u8; 32]);
        let transfer_sender = fund(&backend, &transfer_sk, 5000);
        let anchor_sk = SigningKey::from_bytes(&[63u8; 32]);
        fund(&backend, &anchor_sk, 100);
        let receiver_addr = Address([64u8; 32]);

        let transfer = signed_transfer(&transfer_sk, receiver_addr, 200, 0);
        let receipt = signed_receipt_for(&anchor_sk, [0x7Du8; 32], vec![], 1);
        let bad_anchor = signed_anchor_tx(
            &anchor_sk,
            0,
            Receipt {
                signature: [0xEEu8; 64],
                ..receipt
            },
        );
        let block = build_valid_block(&backend, vec![transfer.clone(), bad_anchor]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidReceiptSignature(1))
        ));

        // The valid transfer left no trace.
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
        let sender = backend.storage.get_account(&transfer_sender).unwrap().unwrap();
        assert_eq!(sender.balance, 5000);
        assert_eq!(sender.nonce, 0);
        assert!(backend.storage.get_account(&receiver_addr).unwrap().is_none());
        assert!(!backend.storage.has_receipt(&[0x7Du8; 32]).unwrap());
        // No sequence state either: last-included unchanged, no index rows.
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert!(backend.storage.get_tx_hash_by_seq(1).unwrap().is_none());
        assert!(backend.storage.get_tx_hash_by_seq(2).unwrap().is_none());

        // Ordinary transfer behavior unchanged: same transfer alone applies.
        let block = build_valid_block(&backend, vec![transfer]);
        backend.apply_block(&block).unwrap();
        let sender = backend.storage.get_account(&transfer_sender).unwrap().unwrap();
        assert_eq!(sender.balance, 4800);
        assert_eq!(sender.nonce, 1);
    }

    #[test]
    fn valid_receipt_before_invalid_tx_leaves_no_receipt() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let anchor_sk = SigningKey::from_bytes(&[63u8; 32]);
        let anchor_sender = fund(&backend, &anchor_sk, 100);
        let poor_sk = SigningKey::from_bytes(&[65u8; 32]);
        fund(&backend, &poor_sk, 10);
        let task_id = seed_task(&backend, &anchor_sk, [0x7Eu8; 32]);

        // Valid receipt first, then a transfer exceeding its balance.
        let anchor = valid_anchor_tx(&anchor_sk, 0, task_id);
        let bad_transfer = signed_transfer(&poor_sk, Address([66u8; 32]), 1000, 0);
        let block = build_valid_block(&backend, vec![anchor, bad_transfer]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InsufficientBalance(1))
        ));

        // The valid receipt was not written.
        assert!(!backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
        let sender = backend.storage.get_account(&anchor_sender).unwrap().unwrap();
        assert_eq!(sender.nonce, 0);
        // No sequence state either.
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert!(backend.storage.get_tx_hash_by_seq(1).unwrap().is_none());
        assert!(backend.storage.get_tx_hash_by_seq(2).unwrap().is_none());
    }

    #[test]
    fn mixed_block_commits_atomically() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let transfer_sk = SigningKey::from_bytes(&[62u8; 32]);
        let transfer_sender = fund(&backend, &transfer_sk, 5000);
        let anchor_sk = SigningKey::from_bytes(&[63u8; 32]);
        let anchor_sender = fund(&backend, &anchor_sk, 100);
        let receiver_addr = Address([64u8; 32]);
        let task_id = seed_task(&backend, &anchor_sk, [0x7Fu8; 32]);

        let receipt = signed_receipt_for(&anchor_sk, task_id, vec![1, 2, 3], 1);
        let expected_bytes = receipt.encode();
        let transfer = signed_transfer(&transfer_sk, receiver_addr, 200, 0);
        let transfer_hash = compute_tx_hash(&transfer);
        let anchor = signed_anchor_tx(&anchor_sk, 0, receipt);
        let anchor_hash = compute_tx_hash(&anchor);

        let block = build_valid_block(&backend, vec![transfer, anchor]);
        backend.apply_block(&block).unwrap();

        // All effects committed atomically.
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        let sender = backend.storage.get_account(&transfer_sender).unwrap().unwrap();
        assert_eq!(sender.balance, 4800);
        assert_eq!(sender.nonce, 1);
        let receiver = backend.storage.get_account(&receiver_addr).unwrap().unwrap();
        assert_eq!(receiver.balance, 200);
        let anchor_acc = backend.storage.get_account(&anchor_sender).unwrap().unwrap();
        assert_eq!(anchor_acc.balance, 100, "anchoring moves no balance");
        assert_eq!(anchor_acc.nonce, 1, "anchoring consumes the nonce");

        // Receipt anchored with canonical SCALE bytes; tx indexed.
        assert!(backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(
            backend.storage.get_receipt(&task_id).unwrap(),
            Some(expected_bytes)
        );
        assert!(backend.storage.get_transaction(&anchor_hash).unwrap().is_some());

        // Sequence state committed in the same batch, in body order.
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 2);
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(1).unwrap(),
            Some(transfer_hash)
        );
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(2).unwrap(),
            Some(anchor_hash)
        );
    }

    // ── Transaction-sequence atomicity (no leak on rejected blocks) ──

    #[test]
    fn failed_block_does_not_advance_persistent_seq_counter() {
        // Scenario 1: valid transfer before an invalid receipt.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let transfer_sk = SigningKey::from_bytes(&[62u8; 32]);
        fund(&backend, &transfer_sk, 5000);
        let anchor_sk = SigningKey::from_bytes(&[63u8; 32]);
        fund(&backend, &anchor_sk, 100);
        let receipt = signed_receipt_for(&anchor_sk, [0x90u8; 32], vec![], 1);
        let bad_anchor = signed_anchor_tx(
            &anchor_sk,
            0,
            Receipt {
                signature: [0xEEu8; 64],
                ..receipt
            },
        );
        let transfer = signed_transfer(&transfer_sk, Address([64u8; 32]), 200, 0);
        let block = build_valid_block(&backend, vec![transfer, bad_anchor]);
        assert!(backend.apply_block(&block).is_err());
        // Read-only observation first: both persistent sequence keys are
        // written atomically from the same local value (SetTxSeq +
        // SetLastIncludedTxSeq in one batch), so the read-only
        // last-included getter and index rows observe the counter state.
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert!(backend.storage.get_tx_hash_by_seq(1).unwrap().is_none());
        // Terminal cross-check of the tx_seq key itself: next_tx_seq
        // returning 1 proves it was 0. It mutates, so it is the LAST
        // assertion against this fresh backend.
        assert_eq!(backend.storage.next_tx_seq().unwrap(), 1);

        // Scenario 2: valid receipt before an over-spending transfer.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let anchor_sk = SigningKey::from_bytes(&[63u8; 32]);
        fund(&backend, &anchor_sk, 100);
        let poor_sk = SigningKey::from_bytes(&[65u8; 32]);
        fund(&backend, &poor_sk, 10);
        let block = build_valid_block(
            &backend,
            vec![
                valid_anchor_tx(&anchor_sk, 0, [0x91u8; 32]),
                signed_transfer(&poor_sk, Address([66u8; 32]), 1000, 0),
            ],
        );
        assert!(backend.apply_block(&block).is_err());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert!(backend.storage.get_tx_hash_by_seq(1).unwrap().is_none());
        assert_eq!(backend.storage.next_tx_seq().unwrap(), 1);

        // Scenario 3: two receipts for the same task_id.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let block = build_valid_block(
            &backend,
            vec![
                valid_anchor_tx(&sk, 0, [0x92u8; 32]),
                valid_anchor_tx(&sk, 1, [0x92u8; 32]),
            ],
        );
        assert!(backend.apply_block(&block).is_err());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert!(backend.storage.get_tx_hash_by_seq(1).unwrap().is_none());
        assert_eq!(backend.storage.next_tx_seq().unwrap(), 1);
    }

    #[test]
    fn retry_after_failed_block_assigns_same_sequences() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let transfer_sk = SigningKey::from_bytes(&[62u8; 32]);
        fund(&backend, &transfer_sk, 5000);
        let anchor_sk = SigningKey::from_bytes(&[63u8; 32]);
        fund(&backend, &anchor_sk, 100);

        // A failing attempt: valid transfer plus a bad receipt.
        let transfer = signed_transfer(&transfer_sk, Address([64u8; 32]), 200, 0);
        let receipt = signed_receipt_for(&anchor_sk, [0x93u8; 32], vec![], 1);
        let bad_anchor = signed_anchor_tx(
            &anchor_sk,
            0,
            Receipt {
                signature: [0xEEu8; 64],
                ..receipt
            },
        );
        let block = build_valid_block(&backend, vec![transfer.clone(), bad_anchor]);
        assert!(backend.apply_block(&block).is_err());

        // Retry with a valid block: sequence values start at 1, exactly as
        // if the failed attempt had never happened.
        let good_anchor =
            valid_anchor_tx(&anchor_sk, 0, seed_task(&backend, &anchor_sk, [0x94u8; 32]));
        let transfer_hash = compute_tx_hash(&transfer);
        let anchor_hash = compute_tx_hash(&good_anchor);
        let block = build_valid_block(&backend, vec![transfer, good_anchor]);
        backend.apply_block(&block).unwrap();

        assert_eq!(
            backend.storage.get_tx_hash_by_seq(1).unwrap(),
            Some(transfer_hash)
        );
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(2).unwrap(),
            Some(anchor_hash)
        );
        assert!(backend.storage.get_tx_hash_by_seq(3).unwrap().is_none());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 2);
    }

    #[test]
    fn tx_seq_deterministic_across_backends() {
        // Backend A experiences a failed block; backend B never does.
        // Both then apply the SAME accepted block (identical genesis, so
        // identical parent hash) and must end with identical sequence
        // state — including the persistent counter.
        let backend_a = make_backend();
        backend_a.ensure_genesis().unwrap();
        let backend_b = make_backend();
        backend_b.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend_a, &sk, 5000);
        fund(&backend_b, &sk, 5000);

        // Failed attempt on A only.
        let block = build_valid_block(
            &backend_a,
            vec![
                valid_anchor_tx(&sk, 0, [0x95u8; 32]),
                valid_anchor_tx(&sk, 1, [0x95u8; 32]), // in-block duplicate
            ],
        );
        assert!(backend_a.apply_block(&block).is_err());

        // Same accepted block applied to both.
        let transfer = signed_transfer(&sk, Address([64u8; 32]), 200, 0);
        let block = build_valid_block(&backend_a, vec![transfer]);
        backend_a.apply_block(&block).unwrap();
        backend_b.apply_block(&block).unwrap();

        assert_eq!(
            backend_a.storage.get_tx_hash_by_seq(1).unwrap(),
            backend_b.storage.get_tx_hash_by_seq(1).unwrap()
        );
        assert_eq!(
            backend_a.storage.get_last_included_tx_seq().unwrap(),
            backend_b.storage.get_last_included_tx_seq().unwrap()
        );
        // Persistent counters agree too (probe mutates, so it is last).
        assert_eq!(
            backend_a.storage.next_tx_seq().unwrap(),
            backend_b.storage.next_tx_seq().unwrap()
        );
    }

    #[test]
    fn idempotent_reapply_allocates_no_new_seq() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let other_sk = SigningKey::from_bytes(&[61u8; 32]);
        fund(&backend, &other_sk, 5000);

        // Block 1 includes txA (seq 1).
        let tx_a = signed_transfer(&sk, Address([64u8; 32]), 100, 0);
        let tx_a_hash = compute_tx_hash(&tx_a);
        let block = build_valid_block(&backend, vec![tx_a.clone()]);
        backend.apply_block(&block).unwrap();
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(1).unwrap(),
            Some(tx_a_hash)
        );

        // Block 2 carries txA again (skipped by the idempotent guard, no
        // new sequence) plus txB (seq 2).
        let tx_b = signed_transfer(&other_sk, Address([65u8; 32]), 100, 0);
        let tx_b_hash = compute_tx_hash(&tx_b);
        let block = build_valid_block(&backend, vec![tx_a, tx_b]);
        backend.apply_block(&block).unwrap();

        assert_eq!(
            backend.storage.get_tx_hash_by_seq(1).unwrap(),
            Some(tx_a_hash)
        );
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(2).unwrap(),
            Some(tx_b_hash)
        );
        assert!(backend.storage.get_tx_hash_by_seq(3).unwrap().is_none());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 2);
    }

    // ── RFC 0002 Phase 3: mempool admission ──────────────────────────

    #[tokio::test]
    async fn submit_anchor_valid_accepted() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        let tx = valid_anchor_tx(&sk, 0, seed_task(&backend, &sk, [0x80u8; 32]));
        backend.submit_transaction(tx).await.unwrap();
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    #[tokio::test]
    async fn submit_anchor_wrong_pairing_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        let mut tx = valid_anchor_tx(&sk, 0, [0x81u8; 32]);
        tx.payload = TransactionPayload::None;
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(err.to_string().contains("payload does not match"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    #[tokio::test]
    async fn submit_anchor_sender_executor_mismatch_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let other = SigningKey::from_bytes(&[61u8; 32]);

        let receipt = signed_receipt_for(&other, [0x82u8; 32], vec![], 1);
        let tx = signed_anchor_tx(&sk, 0, receipt);
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(err.to_string().contains("executor"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    #[tokio::test]
    async fn submit_anchor_bad_receipt_signature_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);

        let receipt = signed_receipt_for(&sk, [0x83u8; 32], vec![], 1);
        let tx = signed_anchor_tx(
            &sk,
            0,
            Receipt {
                signature: [0xEEu8; 64],
                ..receipt
            },
        );
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(err.to_string().contains("receipt signature"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    #[tokio::test]
    async fn submit_anchor_persistent_duplicate_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x84u8; 32]);

        // Anchor via a block first.
        let block = build_valid_block(&backend, vec![valid_anchor_tx(&sk, 0, task_id)]);
        backend.apply_block(&block).unwrap();

        // Submitting the same task_id again is rejected at admission.
        let tx = valid_anchor_tx(&sk, 1, task_id);
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(err.to_string().contains("already anchored"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    #[tokio::test]
    async fn resubmit_identical_anchored_tx_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x97u8; 32]);

        // Anchor via a block.
        let anchor = valid_anchor_tx(&sk, 0, task_id);
        let block = build_valid_block(&backend, vec![anchor.clone()]);
        backend.apply_block(&block).unwrap();

        // Re-submitting the byte-identical anchored transaction must be
        // rejected as already anchored — NOT returned as an idempotent
        // success — and must not enter the pool.
        let err = backend.submit_transaction(anchor).await.unwrap_err();
        assert!(err.to_string().contains("already anchored"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    #[tokio::test]
    async fn submit_anchor_pending_duplicate_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x85u8; 32]);

        backend.submit_transaction(valid_anchor_tx(&sk, 0, task_id)).await.unwrap();
        // The named executor anchoring a second, different receipt for the
        // same task while the first is pending: rejected by the
        // mempool-local task_id guard.
        let second = signed_receipt_for(&sk, task_id, vec![9, 9], 1);
        let err = backend.submit_transaction(signed_anchor_tx(&sk, 1, second)).await.unwrap_err();
        assert!(err.to_string().contains("already pending"), "{err}");
        // A different executor claiming the same task_id never reaches
        // that guard: RFC 0005 rule (s) rejects it first.
        let other = SigningKey::from_bytes(&[61u8; 32]);
        fund(&backend, &other, 100);
        let err = backend
            .submit_transaction(valid_anchor_tx(&other, 0, task_id))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorised"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    #[tokio::test]
    async fn produce_block_anchors_submitted_receipt() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[60u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = seed_task(&backend, &sk, [0x86u8; 32]);

        backend.submit_transaction(valid_anchor_tx(&sk, 0, task_id)).await.unwrap();
        backend.produce_block().await.unwrap();

        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        assert!(backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    #[test]
    fn apply_block_empty_block_succeeds() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let block = build_valid_block(&backend, vec![]);
        let hash = backend.apply_block(&block).unwrap();

        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        assert!(backend.storage.get_block(&hash).unwrap().is_some());
        assert_eq!(
            backend.storage.get_block_by_height(1).unwrap().unwrap().header.height,
            1
        );
    }

    #[test]
    fn apply_block_with_valid_tx() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[50u8; 32]);
        let sender_addr = Address(sk.verifying_key().to_bytes());
        let receiver_addr = Address([51u8; 32]);

        let mut acc = Account::new(sender_addr);
        acc.balance = 5000;
        backend.storage.put_account(&sender_addr, &acc).unwrap();

        let tx = signed_transfer(&sk, receiver_addr, 200, 0);
        let block = build_valid_block(&backend, vec![tx]);

        let hash = backend.apply_block(&block).unwrap();

        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        assert!(backend.storage.get_block(&hash).unwrap().is_some());

        let sender = backend.storage.get_account(&sender_addr).unwrap().unwrap();
        assert_eq!(sender.balance, 4800);
        assert_eq!(sender.nonce, 1);

        let receiver = backend.storage.get_account(&receiver_addr).unwrap().unwrap();
        assert_eq!(receiver.balance, 200);
    }

    #[test]
    fn apply_block_rejects_bad_parent() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let block = Block {
            header: BlockHeader {
                parent_hash: Hash([0xFFu8; 32]), // wrong parent
                state_root: Hash::zero(),
                transactions_root: compute_transactions_root(&[]),
                timestamp: now_secs(),
                height: 1,
            },
            body: BlockBody {
                transactions: vec![],
            },
        };

        let err = backend.apply_block(&block).unwrap_err();
        assert!(
            matches!(err, ApplyBlockError::BadParent { .. }),
            "expected BadParent, got: {err}"
        );
    }

    #[test]
    fn apply_block_rejects_bad_height() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let genesis = backend.storage.get_block_by_height(0).unwrap().unwrap();
        let parent_hash = compute_block_hash(&genesis);

        let block = Block {
            header: BlockHeader {
                parent_hash,
                state_root: Hash::zero(),
                transactions_root: compute_transactions_root(&[]),
                timestamp: now_secs(),
                height: 5, // wrong height (expected 1)
            },
            body: BlockBody {
                transactions: vec![],
            },
        };

        let err = backend.apply_block(&block).unwrap_err();
        assert!(
            matches!(
                err,
                ApplyBlockError::BadHeight {
                    expected: 1,
                    got: 5
                }
            ),
            "expected BadHeight, got: {err}"
        );
    }

    #[test]
    fn apply_block_rejects_invalid_tx_signature() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[52u8; 32]);
        let sender_addr = Address(sk.verifying_key().to_bytes());
        let receiver_addr = Address([53u8; 32]);

        let mut acc = Account::new(sender_addr);
        acc.balance = 5000;
        backend.storage.put_account(&sender_addr, &acc).unwrap();

        let mut tx = signed_transfer(&sk, receiver_addr, 100, 0);
        tx.amount = 999; // tamper ⇒ signature invalid

        let block = build_valid_block(&backend, vec![tx]);
        let err = backend.apply_block(&block).unwrap_err();
        assert!(
            matches!(err, ApplyBlockError::InvalidSignature(0)),
            "expected InvalidSignature(0), got: {err}"
        );
    }

    #[test]
    fn apply_block_rejects_invalid_nonce() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[54u8; 32]);
        let sender_addr = Address(sk.verifying_key().to_bytes());
        let receiver_addr = Address([55u8; 32]);

        let mut acc = Account::new(sender_addr);
        acc.balance = 5000;
        backend.storage.put_account(&sender_addr, &acc).unwrap();

        // Account nonce is 0 but tx nonce is 5.
        let tx = signed_transfer(&sk, receiver_addr, 100, 5);
        let block = build_valid_block(&backend, vec![tx]);
        let err = backend.apply_block(&block).unwrap_err();
        assert!(
            matches!(err, ApplyBlockError::InvalidNonce(0)),
            "expected InvalidNonce(0), got: {err}"
        );
    }

    #[test]
    fn apply_block_rejects_insufficient_balance() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[56u8; 32]);
        let sender_addr = Address(sk.verifying_key().to_bytes());
        let receiver_addr = Address([57u8; 32]);

        let mut acc = Account::new(sender_addr);
        acc.balance = 50; // too low
        backend.storage.put_account(&sender_addr, &acc).unwrap();

        let tx = signed_transfer(&sk, receiver_addr, 200, 0);
        let block = build_valid_block(&backend, vec![tx]);
        let err = backend.apply_block(&block).unwrap_err();
        assert!(
            matches!(err, ApplyBlockError::InsufficientBalance(0)),
            "expected InsufficientBalance(0), got: {err}"
        );
    }

    #[test]
    fn apply_block_rejects_transactions_root_mismatch() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let genesis = backend.storage.get_block_by_height(0).unwrap().unwrap();
        let parent_hash = compute_block_hash(&genesis);

        let block = Block {
            header: BlockHeader {
                parent_hash,
                state_root: Hash::zero(),
                transactions_root: Hash([0xBBu8; 32]), // wrong root
                timestamp: now_secs(),
                height: 1,
            },
            body: BlockBody {
                transactions: vec![],
            },
        };

        let err = backend.apply_block(&block).unwrap_err();
        assert!(
            matches!(err, ApplyBlockError::TransactionsRootMismatch),
            "expected TransactionsRootMismatch, got: {err}"
        );
    }

    #[test]
    fn apply_block_chain_of_three_blocks() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[58u8; 32]);
        let sender_addr = Address(sk.verifying_key().to_bytes());
        let receiver_addr = Address([59u8; 32]);

        let mut acc = Account::new(sender_addr);
        acc.balance = 10_000;
        backend.storage.put_account(&sender_addr, &acc).unwrap();

        // Block 1: transfer 100
        let tx1 = signed_transfer(&sk, receiver_addr, 100, 0);
        let block1 = build_valid_block(&backend, vec![tx1]);
        backend.apply_block(&block1).unwrap();

        // Block 2: transfer 200
        let tx2 = signed_transfer(&sk, receiver_addr, 200, 1);
        let block2 = build_valid_block(&backend, vec![tx2]);
        backend.apply_block(&block2).unwrap();

        // Block 3: empty
        let block3 = build_valid_block(&backend, vec![]);
        backend.apply_block(&block3).unwrap();

        assert_eq!(backend.storage.get_latest_height().unwrap(), 3);

        let final_sender = backend.storage.get_account(&sender_addr).unwrap().unwrap();
        assert_eq!(final_sender.balance, 9700);
        assert_eq!(final_sender.nonce, 2);

        let final_receiver = backend.storage.get_account(&receiver_addr).unwrap().unwrap();
        assert_eq!(final_receiver.balance, 300);
    }

    // ── Issue #100: consensus characterization ─────────────────────────
    //
    // These two tests describe behaviour that already exists. `apply_block`
    // validates each transaction against an advancing in-memory account
    // view (`account_cache`), so a sender's nonce is consumed and visible
    // to the next transaction of the same block. Consecutive same-sender
    // nonces in one block are therefore consensus-valid today.
    //
    // Nothing in the anchoring protocol had to change for issue #100:
    // PROTOCOL_LOCK_v0.3 §3 rule (d) and RFC 0002 rule (d) govern
    // `apply_block`, which is untouched. The limitation reported in #100
    // lives entirely in `submit_transaction`'s admission pre-check.
    //
    // Both tests must pass on unmodified consensus code. If either ever
    // fails, the premise of the #100 fix is false and pending-aware
    // admission must not ship.

    #[test]
    fn apply_block_accepts_consecutive_nonces_from_one_sender() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let sender_addr = fund(&backend, &sk, 5000);
        let receiver_addr = Address([71u8; 32]);

        // Two transfers from ONE sender, consecutive nonces, ONE block.
        let tx1 = signed_transfer(&sk, receiver_addr, 100, 0);
        let tx2 = signed_transfer(&sk, receiver_addr, 200, 1);
        let tx1_hash = compute_tx_hash(&tx1);
        let tx2_hash = compute_tx_hash(&tx2);

        let block = build_valid_block(&backend, vec![tx1, tx2]);
        backend.apply_block(&block).unwrap();

        // Height advanced exactly once: both transactions are in one block.
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);

        // The nonce was consumed twice within that single block.
        let sender = backend.storage.get_account(&sender_addr).unwrap().unwrap();
        assert_eq!(sender.nonce, 2, "both nonces consumed in one block");
        assert_eq!(sender.balance, 4700);

        let receiver = backend.storage.get_account(&receiver_addr).unwrap().unwrap();
        assert_eq!(receiver.balance, 300);

        // Both transactions stored and sequenced in body order.
        assert!(backend.storage.get_transaction(&tx1_hash).unwrap().is_some());
        assert!(backend.storage.get_transaction(&tx2_hash).unwrap().is_some());
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(1).unwrap(),
            Some(tx1_hash)
        );
        assert_eq!(
            backend.storage.get_tx_hash_by_seq(2).unwrap(),
            Some(tx2_hash)
        );
        assert!(backend.storage.get_tx_hash_by_seq(3).unwrap().is_none());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 2);
    }

    #[test]
    fn apply_block_accepts_consecutive_anchors_from_one_executor() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let sk = SigningKey::from_bytes(&[72u8; 32]);
        let executor_addr = fund(&backend, &sk, 100);
        let task_a = seed_task(&backend, &sk, [0xA1u8; 32]);
        let task_b = seed_task(&backend, &sk, [0xA2u8; 32]);

        // Two anchors from ONE executor, consecutive nonces, ONE block.
        // Distinct task ids, so rules (i)/(j) are not the subject here.
        let tx1 = valid_anchor_tx(&sk, 0, task_a);
        let tx2 = valid_anchor_tx(&sk, 1, task_b);

        let block = build_valid_block(&backend, vec![tx1, tx2]);
        backend.apply_block(&block).unwrap();

        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);

        // Both receipts anchored from one block.
        assert!(backend.storage.has_receipt(&task_a).unwrap());
        assert!(backend.storage.has_receipt(&task_b).unwrap());

        // Anchoring consumes the nonce and moves no balance.
        let executor = backend.storage.get_account(&executor_addr).unwrap().unwrap();
        assert_eq!(executor.nonce, 2, "both nonces consumed in one block");
        assert_eq!(executor.balance, 100, "anchoring moves no balance");

        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 2);
    }

    // ── Issue #100: pending-aware admission ────────────────────────────
    //
    // Admission accepts a contiguous pending chain from one sender on one
    // node. Scope is deliberately same-node: transactions are not gossiped,
    // only blocks are, so nothing here claims anything about a sender
    // submitting to two different nodes.

    /// Funds an account and sets its committed nonce.
    fn fund_with_nonce(
        backend: &NodeBackend<InMemoryStorage>,
        sk: &SigningKey,
        balance: u128,
        nonce: u64,
    ) -> Address {
        let addr = Address(sk.verifying_key().to_bytes());
        let mut acc = Account::new(addr);
        acc.balance = balance;
        acc.nonce = nonce;
        backend.storage.put_account(&addr, &acc).unwrap();
        addr
    }

    // Case 1: committed C, pending empty, submit C → ACCEPT.
    #[tokio::test]
    async fn submit_accepts_first_nonce_with_empty_pending() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[80u8; 32]);
        fund(&backend, &sk, 10_000);

        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    // Case 2: pending C, submit C+1 → ACCEPT. This is issue #100 itself.
    #[tokio::test]
    async fn submit_accepts_successor_while_predecessor_pending() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[81u8; 32]);
        fund(&backend, &sk, 10_000);

        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        // No block produced in between: this is what #100 reported failing.
        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 20, 1))
            .await
            .unwrap();

        assert_eq!(backend.mempool.read().await.len(), 2);
    }

    // Case 3: pending C, C+1, submit C+2 → ACCEPT.
    #[tokio::test]
    async fn submit_accepts_three_deep_pending_chain() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[82u8; 32]);
        let addr = fund(&backend, &sk, 10_000);

        for nonce in 0..3u64 {
            backend
                .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, nonce))
                .await
                .unwrap();
        }

        assert_eq!(backend.mempool.read().await.len(), 3);
        let pending = backend.mempool.read().await.sender_pending(&addr, 0);
        assert_eq!(pending.expected_nonce, Some(3));
        assert_eq!(pending.len, 3);
    }

    // Case 4: pending empty, submit C+1 → REJECT (gap).
    #[tokio::test]
    async fn submit_rejects_gap_with_empty_pending() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[83u8; 32]);
        fund(&backend, &sk, 10_000);

        let err = backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 1))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid nonce"), "got: {err}");
        assert!(
            err.contains("expected 0"),
            "must name the missing nonce: {err}"
        );
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    // Case 5: pending C, submit C+2 → REJECT (gap). The error must report
    // the hole (C+1), not one past the highest pending nonce.
    #[tokio::test]
    async fn submit_rejects_gap_beyond_pending_chain() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[84u8; 32]);
        fund(&backend, &sk, 10_000);

        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        let err = backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 2))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid nonce"), "got: {err}");
        assert!(err.contains("expected 1"), "must name the hole: {err}");
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    // Case 6: stale nonce below committed → REJECT.
    #[tokio::test]
    async fn submit_rejects_stale_nonce_below_committed() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[85u8; 32]);
        fund(&backend, &sk, 10_000);

        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();

        // Committed nonce is now 1. A different transaction reusing nonce 0
        // is stale.
        let err = backend
            .submit_transaction(signed_transfer(&sk, Address([2u8; 32]), 10, 0))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid nonce"), "got: {err}");
        assert!(err.contains("expected 1"), "got: {err}");
    }

    // Case 7: exact re-submission stays idempotent. The pending-aware nonce
    // check would otherwise reject a byte-identical retry, because the
    // transaction already occupies its own nonce slot.
    #[tokio::test]
    async fn submit_exact_resubmission_stays_idempotent() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[86u8; 32]);
        fund(&backend, &sk, 10_000);

        let tx = signed_transfer(&sk, Address([1u8; 32]), 10, 0);
        let first = backend.submit_transaction(tx.clone()).await.unwrap();
        let second = backend.submit_transaction(tx).await.unwrap();

        assert_eq!(first, second, "same hash returned");
        assert_eq!(
            backend.mempool.read().await.len(),
            1,
            "no duplicate entry created"
        );
    }

    // Case 8: same sender, same nonce, different transaction → REJECT.
    #[tokio::test]
    async fn submit_same_nonce_different_transaction_rejected() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[87u8; 32]);
        fund(&backend, &sk, 10_000);

        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        // Different receiver → different hash, same (sender, nonce).
        let err = backend
            .submit_transaction(signed_transfer(&sk, Address([2u8; 32]), 10, 0))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid nonce"), "got: {err}");
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    // Case 9: different senders, same nonce → independently ACCEPT.
    #[tokio::test]
    async fn submit_different_senders_same_nonce_independent() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk_a = SigningKey::from_bytes(&[88u8; 32]);
        let sk_b = SigningKey::from_bytes(&[89u8; 32]);
        fund(&backend, &sk_a, 10_000);
        fund(&backend, &sk_b, 10_000);

        backend
            .submit_transaction(signed_transfer(&sk_a, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk_b, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        assert_eq!(backend.mempool.read().await.len(), 2);
    }

    // Case 10: a same-sender pending chain is drained into ONE block.
    #[tokio::test]
    async fn produce_block_includes_whole_pending_chain() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[90u8; 32]);
        let addr = fund(&backend, &sk, 10_000);

        for nonce in 0..3u64 {
            backend
                .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 100, nonce))
                .await
                .unwrap();
        }
        backend.produce_block().await.unwrap();

        // One block, three transactions, nonces consumed three times.
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        let block = backend.storage.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(block.body.transactions.len(), 3);
        let account = backend.storage.get_account(&addr).unwrap().unwrap();
        assert_eq!(account.nonce, 3);
        assert_eq!(account.balance, 9700);
        assert_eq!(backend.mempool.read().await.len(), 0, "mempool drained");
    }

    // Case 11: nonce exhaustion is rejected, never wrapped.
    #[tokio::test]
    async fn submit_rejects_when_nonce_space_exhausted() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[91u8; 32]);
        fund_with_nonce(&backend, &sk, 10_000, u64::MAX);

        // The last allocatable nonce is still admissible.
        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 10, u64::MAX))
            .await
            .unwrap();

        // With it pending there is no successor to allocate.
        let err = backend
            .submit_transaction(signed_transfer(&sk, Address([2u8; 32]), 10, u64::MAX))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid nonce"), "got: {err}");
        assert!(err.contains("exhausted"), "got: {err}");
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    // Case 12: concurrent same-node submissions competing for one nonce.
    // Both transactions are valid in isolation and differ only by receiver,
    // so exactly one may occupy the nonce slot.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_submissions_claim_one_nonce_slot() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[92u8; 32]);
        let addr = fund(&backend, &sk, 10_000);

        let tx_a = signed_transfer(&sk, Address([1u8; 32]), 10, 0);
        let tx_b = signed_transfer(&sk, Address([2u8; 32]), 10, 0);
        assert_ne!(
            compute_tx_hash(&tx_a),
            compute_tx_hash(&tx_b),
            "the two submissions must be genuinely different transactions"
        );

        let b1 = backend.clone();
        let b2 = backend.clone();
        let h1 = tokio::spawn(async move { b1.submit_transaction(tx_a).await });
        let h2 = tokio::spawn(async move { b2.submit_transaction(tx_b).await });
        let (r1, r2) = (h1.await.unwrap(), h2.await.unwrap());

        let accepted = usize::from(r1.is_ok()) + usize::from(r2.is_ok());
        assert_eq!(accepted, 1, "exactly one submission may take nonce 0");

        // One logical nonce slot, and the mempool agrees.
        let pool = backend.mempool.read().await;
        assert_eq!(pool.len(), 1);
        let pending = pool.sender_pending(&addr, 0);
        assert_eq!(pending.expected_nonce, Some(1));
        assert_eq!(pending.len, 1);
    }

    // ── Issue #100: cumulative pending balance ─────────────────────────

    #[tokio::test]
    async fn submit_rejects_chain_exceeding_committed_balance() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[93u8; 32]);
        fund(&backend, &sk, 10);

        // 7 is affordable on its own; 7 + 7 is not.
        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 7, 0))
            .await
            .unwrap();
        let err = backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 7, 1))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("insufficient balance"), "got: {err}");
        assert_eq!(backend.mempool.read().await.len(), 1);
    }

    #[tokio::test]
    async fn submit_accepts_chain_within_committed_balance() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[94u8; 32]);
        let addr = fund(&backend, &sk, 10);

        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 4, 0))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 6, 1))
            .await
            .unwrap();
        assert_eq!(backend.mempool.read().await.len(), 2);

        // And the block that consumes exactly the balance applies.
        backend.produce_block().await.unwrap();
        let account = backend.storage.get_account(&addr).unwrap().unwrap();
        assert_eq!(account.balance, 0);
        assert_eq!(account.nonce, 2);
    }

    // ── Issue #100: the motivating anchoring case ──────────────────────

    /// The exact capability issue #100 reported missing: an executor
    /// anchoring two receipts back to back, with no block in between.
    #[tokio::test]
    async fn submit_two_anchors_from_one_executor_without_a_block_between() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[95u8; 32]);
        let executor = fund(&backend, &sk, 100);
        let task_a = seed_task(&backend, &sk, [0xB1u8; 32]);
        let task_b = seed_task(&backend, &sk, [0xB2u8; 32]);

        // Both submitted before any block is produced.
        backend.submit_transaction(valid_anchor_tx(&sk, 0, task_a)).await.unwrap();
        backend.submit_transaction(valid_anchor_tx(&sk, 1, task_b)).await.unwrap();
        assert_eq!(
            backend.mempool.read().await.len(),
            2,
            "both pending at once"
        );

        // One block anchors both.
        backend.produce_block().await.unwrap();
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        assert!(backend.storage.has_receipt(&task_a).unwrap());
        assert!(backend.storage.has_receipt(&task_b).unwrap());

        let account = backend.storage.get_account(&executor).unwrap().unwrap();
        assert_eq!(account.nonce, 2, "anchoring consumed both nonces");
        assert_eq!(account.balance, 100, "anchoring moves no balance");
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    // ── Issue #100: per-sender pending bound ───────────────────────────

    #[tokio::test]
    async fn submit_rejects_beyond_max_pending_per_sender() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[96u8; 32]);
        let other_sk = SigningKey::from_bytes(&[97u8; 32]);
        fund(&backend, &sk, 10_000);
        fund(&backend, &other_sk, 10_000);

        // Exactly the cap is allowed.
        for nonce in 0..MAX_PENDING_PER_SENDER as u64 {
            backend
                .submit_transaction(signed_transfer(&sk, Address([1u8; 32]), 1, nonce))
                .await
                .unwrap_or_else(|e| panic!("nonce {nonce} should be admitted: {e}"));
        }
        assert_eq!(backend.mempool.read().await.len(), MAX_PENDING_PER_SENDER);

        // One more from the same sender is refused.
        let err = backend
            .submit_transaction(signed_transfer(
                &sk,
                Address([1u8; 32]),
                1,
                MAX_PENDING_PER_SENDER as u64,
            ))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("too many pending"), "got: {err}");

        // A different sender is unaffected: the bound is per sender.
        backend
            .submit_transaction(signed_transfer(&other_sk, Address([1u8; 32]), 1, 0))
            .await
            .unwrap();
        assert_eq!(
            backend.mempool.read().await.len(),
            MAX_PENDING_PER_SENDER + 1
        );
    }

    // ── Issue #100: drain/apply safety ─────────────────────────────────

    /// Block production selects transactions without removing them, so a
    /// failed application loses nothing. Without this, a rejected block
    /// would silently discard the whole selected batch — one transaction
    /// before #100, a sender's entire pending chain after it.
    #[tokio::test]
    async fn failed_block_application_keeps_transactions_pending() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[98u8; 32]);
        let addr = fund(&backend, &sk, 10_000);

        let pending_tx = signed_transfer(&sk, Address([1u8; 32]), 10, 0);
        let pending_hash = compute_tx_hash(&pending_tx);
        backend.submit_transaction(pending_tx).await.unwrap();

        // Consume nonce 0 through a different transaction applied outside
        // the mempool, exactly as an incoming block would. The pending
        // entry is now stale and cannot be applied.
        let external = signed_transfer(&sk, Address([2u8; 32]), 10, 0);
        let block = build_valid_block(&backend, vec![external]);
        backend.apply_block(&block).unwrap();
        assert_eq!(
            backend.storage.get_account(&addr).unwrap().unwrap().nonce,
            1
        );

        // Production now fails, because the selected transaction is stale.
        let err = backend.produce_block().await.unwrap_err().to_string();
        assert!(
            err.contains("nonce"),
            "expected a nonce failure, got: {err}"
        );

        // Nothing was lost, and every index is still consistent.
        let pool = backend.mempool.read().await;
        assert_eq!(pool.len(), 1, "the transaction is still pending");
        assert!(pool.contains_hash(&pending_hash));
        assert_eq!(
            pool.peek_for_block(10).len(),
            1,
            "order index still holds it"
        );
        // Committed nonce is 1, so the stale entry at 0 is below the walk.
        let pending = pool.sender_pending(&addr, 1);
        assert_eq!(pending.expected_nonce, Some(1));
        assert_eq!(pending.len, 1, "still counted as a held resource");
        assert_eq!(pending.pending_debit, 0, "a stale entry is not a debit");
    }

    // ── Issue #100: ordering ───────────────────────────────────────────

    /// A sender's chain reaches the block in ascending nonce order, and
    /// other senders keep their global FIFO position: admission only ever
    /// accepts a sender's next nonce, so insertion order *is* nonce order
    /// and no sender grouping is needed.
    #[tokio::test]
    async fn pending_chain_reaches_the_block_in_nonce_order() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk_a = SigningKey::from_bytes(&[99u8; 32]);
        let sk_b = SigningKey::from_bytes(&[101u8; 32]);
        let addr_a = Address(sk_a.verifying_key().to_bytes());
        let addr_b = Address(sk_b.verifying_key().to_bytes());
        fund(&backend, &sk_a, 10_000);
        fund(&backend, &sk_b, 10_000);

        // Interleaved: A0, B0, A1, B1, A2.
        backend
            .submit_transaction(signed_transfer(&sk_a, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk_b, Address([1u8; 32]), 10, 0))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk_a, Address([1u8; 32]), 10, 1))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk_b, Address([1u8; 32]), 10, 1))
            .await
            .unwrap();
        backend
            .submit_transaction(signed_transfer(&sk_a, Address([1u8; 32]), 10, 2))
            .await
            .unwrap();

        backend.produce_block().await.unwrap();
        let block = backend.storage.get_block_by_height(1).unwrap().unwrap();
        assert_eq!(block.body.transactions.len(), 5);

        // Global FIFO preserved.
        let senders: Vec<Address> = block.body.transactions.iter().map(|t| t.sender).collect();
        assert_eq!(senders, vec![addr_a, addr_b, addr_a, addr_b, addr_a]);

        // Each sender's own nonces are ascending and contiguous.
        let a_nonces: Vec<u64> = block
            .body
            .transactions
            .iter()
            .filter(|t| t.sender == addr_a)
            .map(|t| t.nonce)
            .collect();
        assert_eq!(a_nonces, vec![0, 1, 2]);
        let b_nonces: Vec<u64> = block
            .body
            .transactions
            .iter()
            .filter(|t| t.sender == addr_b)
            .map(|t| t.nonce)
            .collect();
        assert_eq!(b_nonces, vec![0, 1]);

        assert_eq!(
            backend.storage.get_account(&addr_a).unwrap().unwrap().nonce,
            3
        );
        assert_eq!(
            backend.storage.get_account(&addr_b).unwrap().unwrap().nonce,
            2
        );
    }

    // ── Block announcement tests ────────────────────────────────────────

    #[test]
    fn broadcast_block_updates_follower_height() {
        // Simulate: producer produces a block, follower applies it via handle_incoming_block.
        let producer = make_backend();
        producer.ensure_genesis().unwrap();

        let follower = make_backend();
        follower.ensure_genesis().unwrap();

        // Producer: build and apply a block.
        let block = build_valid_block(&producer, vec![]);
        producer.apply_block(&block).unwrap();
        assert_eq!(producer.storage.get_latest_height().unwrap(), 1);

        // Simulate broadcast: follower receives the block.
        follower.handle_incoming_block(block);

        // Follower must now be at the same height as the producer.
        assert_eq!(follower.storage.get_latest_height().unwrap(), 1);
    }

    #[test]
    fn follower_ignores_future_height_block() {
        let follower = make_backend();
        follower.ensure_genesis().unwrap();

        // Build a chain of 5 blocks on a separate backend.
        let producer = make_backend();
        producer.ensure_genesis().unwrap();
        for _ in 0..5 {
            let b = build_valid_block(&producer, vec![]);
            producer.apply_block(&b).unwrap();
        }
        assert_eq!(producer.storage.get_latest_height().unwrap(), 5);

        // Grab block at height 5 (follower is at height 0 → expects height 1).
        let future_block = producer.storage.get_block_by_height(5).unwrap().unwrap();

        // Follower should ignore this block (height 5 when local height is 0).
        follower.handle_incoming_block(future_block);
        assert_eq!(follower.storage.get_latest_height().unwrap(), 0);
    }

    #[test]
    fn follower_rejects_invalid_block() {
        let producer = make_backend();
        producer.ensure_genesis().unwrap();

        let follower = make_backend();
        follower.ensure_genesis().unwrap();

        // Build a valid block but tamper with the transactions_root.
        let mut block = build_valid_block(&producer, vec![]);
        block.header.transactions_root = Hash([0xDDu8; 32]); // wrong root

        // Follower should reject but not panic.
        follower.handle_incoming_block(block);

        // Height must remain at 0 (block was rejected).
        assert_eq!(follower.storage.get_latest_height().unwrap(), 0);
    }

    // ── Timed block production tests ────────────────────────────────────

    use std::sync::atomic::{AtomicU64, Ordering};

    /// Mock broadcaster that counts how many blocks were broadcast.
    struct MockBroadcaster {
        count: AtomicU64,
    }

    impl MockBroadcaster {
        fn new() -> Self {
            Self {
                count: AtomicU64::new(0),
            }
        }

        fn broadcast_count(&self) -> u64 {
            self.count.load(Ordering::SeqCst)
        }
    }

    impl mbongo_network::BlockBroadcaster for MockBroadcaster {
        fn broadcast(&self, _block: Block) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[tokio::test]
    async fn producer_timer_produces_blocks() {
        tokio::time::pause();

        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let producer_backend = backend.clone();
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            // First tick fires immediately; produce 3 blocks total.
            for _ in 0..3 {
                interval.tick().await;
                producer_backend.produce_block().await.unwrap();
            }
        });

        // Advance time to let 3 ticks complete (0, 5, 10 seconds).
        tokio::time::advance(std::time::Duration::from_secs(11)).await;
        handle.await.unwrap();

        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 3);
    }

    #[tokio::test]
    async fn non_producer_does_not_auto_produce() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        // Non-producer: no timed loop spawned.
        // Simply assert that after genesis, height remains 0.
        let height = backend.get_block_height().await.unwrap();
        assert_eq!(height, 0);
    }

    #[tokio::test]
    async fn producer_broadcasts_after_produce() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let mock = Arc::new(MockBroadcaster::new());
        let mut backend = backend;
        backend.set_broadcaster(Arc::clone(&mock) as Arc<dyn mbongo_network::BlockBroadcaster>);

        // Produce two blocks. Each should trigger broadcast.
        backend.produce_block().await.unwrap();
        backend.produce_block().await.unwrap();

        assert_eq!(mock.broadcast_count(), 2);
        assert_eq!(backend.get_block_height().await.unwrap(), 2);
    }

    // ── Producer role enforcement tests ─────────────────────────────────

    #[tokio::test]
    async fn non_producer_cannot_produce_block() {
        let backend = NodeBackend::new(InMemoryStorage::new(), false);
        backend.ensure_genesis().unwrap();

        let result = backend.produce_block().await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not configured as producer"),
            "expected producer error, got: {err}"
        );
    }

    #[tokio::test]
    async fn producer_can_produce_block() {
        let backend = NodeBackend::new(InMemoryStorage::new(), true);
        backend.ensure_genesis().unwrap();

        let result = backend.produce_block().await;
        assert!(result.is_ok());
        assert_eq!(backend.get_block_height().await.unwrap(), 1);
    }

    // ── get_latest_block_hash tests ─────────────────────────────────────

    #[tokio::test]
    async fn get_latest_block_hash_returns_genesis_hash() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let hash = backend.get_latest_block_hash().await.unwrap();
        assert!(hash.starts_with("0x"), "expected hex hash, got: {hash}");

        // Computing expected genesis hash.
        let genesis = backend.storage.get_block_by_height(0).unwrap().unwrap();
        let expected = compute_block_hash(&genesis).to_string();
        assert_eq!(hash, expected);
    }

    #[tokio::test]
    async fn get_latest_block_hash_changes_after_produce() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let hash0 = backend.get_latest_block_hash().await.unwrap();
        backend.produce_block().await.unwrap();
        let hash1 = backend.get_latest_block_hash().await.unwrap();

        assert_ne!(hash0, hash1, "tip hash should change after produce_block");
        assert!(hash1.starts_with("0x"));
    }

    // ── Deterministic replay tests ──────────────────────────────────────

    #[tokio::test]
    async fn replay_reproduces_tip_hash() {
        // Build a chain of 5 blocks on backend A.
        let backend_a = make_backend();
        backend_a.ensure_genesis().unwrap();
        for _ in 0..5 {
            backend_a.produce_block().await.unwrap();
        }
        let original_height = backend_a.get_block_height().await.unwrap();
        let original_hash = backend_a.get_latest_block_hash().await.unwrap();
        assert_eq!(original_height, 5);

        // Export all blocks from backend A.
        let mut blocks = Vec::new();
        for h in 0..=original_height {
            let block = backend_a.storage.get_block_by_height(h).unwrap().unwrap();
            blocks.push(block);
        }

        // Replay on fresh backend B (producer=true so apply_block works, but
        // we use apply_block directly, not produce_block).
        let backend_b = make_backend();
        backend_b.ensure_genesis().unwrap();

        // Apply blocks 1..N (genesis already applied via ensure_genesis).
        for block in &blocks[1..] {
            backend_b.apply_block(block).unwrap();
        }

        let replay_height = backend_b.get_block_height().await.unwrap();
        let replay_hash = backend_b.get_latest_block_hash().await.unwrap();

        assert_eq!(replay_height, original_height);
        assert_eq!(replay_hash, original_hash);
    }

    #[tokio::test]
    async fn replay_height_matches() {
        // Produce 3 blocks, export, replay, verify height.
        let backend_a = make_backend();
        backend_a.ensure_genesis().unwrap();
        for _ in 0..3 {
            backend_a.produce_block().await.unwrap();
        }

        let mut blocks = Vec::new();
        for h in 0..=3 {
            blocks.push(backend_a.storage.get_block_by_height(h).unwrap().unwrap());
        }

        let backend_b = make_backend();
        backend_b.ensure_genesis().unwrap();
        for block in &blocks[1..] {
            backend_b.apply_block(block).unwrap();
        }

        assert_eq!(backend_b.get_block_height().await.unwrap(), 3);
    }

    #[test]
    fn replay_fails_on_invalid_block() {
        // Build a valid chain of 2 blocks.
        let backend_a = make_backend();
        backend_a.ensure_genesis().unwrap();
        let block1 = build_valid_block(&backend_a, vec![]);
        backend_a.apply_block(&block1).unwrap();

        // Tamper with block 1's transactions root before replaying.
        let mut tampered = block1.clone();
        tampered.header.transactions_root = Hash([0xFFu8; 32]);

        let backend_b = make_backend();
        backend_b.ensure_genesis().unwrap();
        let err = backend_b.apply_block(&tampered).unwrap_err();
        assert!(
            matches!(err, ApplyBlockError::TransactionsRootMismatch),
            "expected TransactionsRootMismatch, got: {err}"
        );
    }

    // ── get_block_by_height RPC test ────────────────────────────────────

    #[tokio::test]
    async fn get_block_by_height_returns_block() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let result = backend.get_block_by_height(0).await.unwrap();
        // Result is a serde_json::Value representing the genesis block.
        assert_eq!(result["header"]["height"], serde_json::json!(0));
        assert!(result["body"]["transactions"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_block_by_height_not_found() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();

        let result = backend.get_block_by_height(999).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("block not found"), "got: {err}");
    }

    // ── Auto-sync simulation tests ───────────────────────────────────

    /// Simulates follower catching up from genesis to height N after
    /// connecting to a producer that has N blocks.
    ///
    /// This tests the same flow the sync orchestrator uses:
    /// 1. Producer produces N blocks.
    /// 2. Follower at genesis receives blocks [1..N] via simulated
    ///    sync response and applies them sequentially.
    /// 3. Follower reaches height N with matching tip hash.
    #[tokio::test]
    async fn follower_catches_up_after_connect() {
        let target_height: u64 = 10;

        // ── Producer side: build a chain up to target_height ──────────
        let producer = make_backend();
        producer.ensure_genesis().unwrap();

        for _ in 0..target_height {
            producer.produce_block().await.unwrap();
        }

        let producer_height = producer.latest_height().unwrap();
        assert_eq!(producer_height, target_height);
        let producer_hash = producer.get_latest_block_hash().await.unwrap();

        // ── Simulate sync response: collect all blocks ────────────────
        // This is what the sync service would return in
        // SyncResponse::Blocks for GetBlocks { start: 1, end: N+1 }.
        let mut synced_blocks = Vec::new();
        for h in 1..=target_height {
            let block = producer.storage.get_block_by_height(h).unwrap().unwrap();
            let hash = compute_block_hash(&block);
            synced_blocks.push((hash, block));
        }

        // ── Follower side: at genesis, apply synced blocks ────────────
        let follower = make_backend();
        follower.ensure_genesis().unwrap();
        assert_eq!(follower.latest_height().unwrap(), 0);

        for (_hash, block) in &synced_blocks {
            follower.apply_block(block).unwrap();
        }

        // ── Verify convergence ─────────────────────────────────────────
        let follower_height = follower.latest_height().unwrap();
        let follower_hash = follower.get_latest_block_hash().await.unwrap();

        assert_eq!(follower_height, target_height);
        assert_eq!(follower_hash, producer_hash);
    }

    /// Simulates gap recovery when a follower at height 0 receives a
    /// pushed NewBlock at height 5.
    ///
    /// The orchestrator detects incoming_height > local+1, triggers a
    /// sync for the missing range, then applies the pushed block.
    /// This test exercises the same `apply_block` chain used by
    /// the orchestrator's gap recovery path.
    #[tokio::test]
    async fn follower_gap_recovery_on_new_block() {
        // ── Producer: build chain of 7 blocks ─────────────────────────
        let producer = make_backend();
        producer.ensure_genesis().unwrap();

        for _ in 0..7 {
            producer.produce_block().await.unwrap();
        }
        assert_eq!(producer.latest_height().unwrap(), 7);

        // ── Follower: starts at genesis ───────────────────────────────
        let follower = make_backend();
        follower.ensure_genesis().unwrap();
        assert_eq!(follower.latest_height().unwrap(), 0);

        // The pushed block arrives at height 5 (simulates NewBlock push).
        let pushed_block = producer.storage.get_block_by_height(5).unwrap().unwrap();
        let pushed_height = pushed_block.header.height;
        assert_eq!(pushed_height, 5);

        let local_height = follower.latest_height().unwrap();
        assert!(pushed_height > local_height + 1, "gap condition must hold");

        // ── Gap recovery: fetch missing blocks [1..5) ─────────────────
        // In the real orchestrator, this is a GetBlocks request.
        // Here we simulate the sync response.
        for h in 1..pushed_height {
            let block = producer.storage.get_block_by_height(h).unwrap().unwrap();
            follower.apply_block(&block).unwrap();
        }

        // Now the follower is at height 4. Apply the pushed block.
        assert_eq!(follower.latest_height().unwrap(), pushed_height - 1);
        follower.apply_block(&pushed_block).unwrap();

        // ── Verify ────────────────────────────────────────────────────
        assert_eq!(follower.latest_height().unwrap(), 5);

        // Tip hashes must match at height 5.
        let producer_tip_at_5 =
            compute_block_hash(&producer.storage.get_block_by_height(5).unwrap().unwrap());
        let follower_tip =
            compute_block_hash(&follower.storage.get_block_by_height(5).unwrap().unwrap());
        assert_eq!(follower_tip, producer_tip_at_5);
    }

    // ── RFC 0005: ComputeTask commitment, rules (k)–(p) ───────────────

    use mbongo_core::{ComputeTask, COMPUTE_TASK_VERSION, MAX_EXECUTION_SPEC_BYTES};
    use parity_scale_codec::Decode;

    /// A well-formed envelope naming `executor`, committed by `submitter`.
    fn task_for(
        submitter: Address,
        executor: Address,
        salt: [u8; 32],
        execution_spec: Vec<u8>,
    ) -> ComputeTask {
        ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter,
            executor,
            salt,
            input_commitment: [0x1Cu8; 32],
            execution_spec,
        }
    }

    /// Wraps a task in a canonically formed `ComputeTask` transaction
    /// (amount 0, zero receiver) signed by `sk`.
    fn signed_task_tx(sk: &SigningKey, nonce: u64, task: ComputeTask) -> Transaction {
        let sender = Address(sk.verifying_key().to_bytes());
        let mut tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender,
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::ComputeTask(Box::new(task)),
            signature: [0u8; 64],
        };
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        tx
    }

    /// The executor every valid test task names: a second key, so that
    /// submitter and executor are visibly different parties.
    fn executor_key() -> SigningKey {
        SigningKey::from_bytes(&[0xE1u8; 32])
    }

    fn executor_addr() -> Address {
        Address(executor_key().verifying_key().to_bytes())
    }

    /// Fully valid task transaction: submitter == sender, canonical
    /// fields, valid signature, three-byte specification.
    fn valid_task_tx(sk: &SigningKey, nonce: u64, salt: [u8; 32]) -> Transaction {
        let submitter = Address(sk.verifying_key().to_bytes());
        signed_task_tx(
            sk,
            nonce,
            task_for(submitter, executor_addr(), salt, vec![1, 2, 3]),
        )
    }

    fn task_of(tx: &Transaction) -> &ComputeTask {
        match &tx.payload {
            TransactionPayload::ComputeTask(task) => task,
            TransactionPayload::None | TransactionPayload::AnchorReceipt(_) => {
                panic!("expected a ComputeTask payload")
            }
        }
    }

    /// Re-signs a transaction after a field was changed, so that the only
    /// rule under test is the one the change targets.
    fn resign(sk: &SigningKey, tx: &mut Transaction) {
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
    }

    /// Asserts the chain is exactly the post-genesis state for `sk`:
    /// height 0, nonce 0, balance untouched, no task, no sequence.
    fn assert_untouched(
        backend: &NodeBackend<InMemoryStorage>,
        sk: &SigningKey,
        task_id: &[u8; 32],
    ) {
        let addr = Address(sk.verifying_key().to_bytes());
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
        let acc = backend.storage.get_account(&addr).unwrap().unwrap();
        assert_eq!(acc.nonce, 0);
        assert_eq!(acc.balance, 5000);
        assert!(!backend.storage.has_task(task_id).unwrap());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
    }

    #[test]
    fn compute_task_valid_block_registers_task() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let sender = fund(&backend, &sk, 5000);

        let tx = valid_task_tx(&sk, 0, [0x01u8; 32]);
        let task = task_of(&tx).clone();
        let task_id = task.task_id();
        let tx_hash = compute_tx_hash(&tx);

        let block = build_valid_block(&backend, vec![tx]);
        backend.apply_block(&block).unwrap();

        // Submitted state is the presence of the key (RFC 0005 §4.1); the
        // stored bytes are the canonical SCALE encoding and nothing else.
        assert!(backend.storage.has_task(&task_id).unwrap());
        assert_eq!(
            backend.storage.get_task(&task_id).unwrap(),
            Some(task.encode())
        );
        // No receipt appeared: the two derived states are separate.
        assert!(!backend.storage.has_receipt(&task_id).unwrap());
        // Nonce consumed, no money moved, transaction and sequence stored.
        let acc = backend.storage.get_account(&sender).unwrap().unwrap();
        assert_eq!(acc.nonce, 1);
        assert_eq!(acc.balance, 5000);
        assert!(backend.storage.get_transaction(&tx_hash).unwrap().is_some());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 1);
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
    }

    #[test]
    fn compute_task_repeated_work_needs_a_new_salt() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);

        // Same submitter, executor, input and specification: only the
        // salt differs, and that is exactly how a client repeats work.
        let a = valid_task_tx(&sk, 0, [0x01u8; 32]);
        let b = valid_task_tx(&sk, 1, [0x02u8; 32]);
        let (id_a, id_b) = (task_of(&a).task_id(), task_of(&b).task_id());
        assert_ne!(id_a, id_b);

        let block = build_valid_block(&backend, vec![a, b]);
        backend.apply_block(&block).unwrap();
        assert!(backend.storage.has_task(&id_a).unwrap());
        assert!(backend.storage.has_task(&id_b).unwrap());
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 2);
    }

    #[test]
    fn compute_task_different_executor_is_a_different_task() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let submitter = fund(&backend, &sk, 5000);

        // Identical envelope except the executor: a client may ask two
        // executors for the same work (RFC 0005 §2.6), and neither
        // registration collides with the other.
        let for_e1 = signed_task_tx(
            &sk,
            0,
            task_for(submitter, executor_addr(), [0x03u8; 32], vec![9]),
        );
        let for_e2 = signed_task_tx(
            &sk,
            1,
            task_for(submitter, Address([0xE2u8; 32]), [0x03u8; 32], vec![9]),
        );
        let (id_1, id_2) = (task_of(&for_e1).task_id(), task_of(&for_e2).task_id());
        assert_ne!(id_1, id_2, "changing the executor must change task_id");

        let block = build_valid_block(&backend, vec![for_e1, for_e2]);
        backend.apply_block(&block).unwrap();
        assert!(backend.storage.has_task(&id_1).unwrap());
        assert!(backend.storage.has_task(&id_2).unwrap());

        // The stored envelopes name their executors: nothing on chain can
        // later change who may answer either task.
        let stored =
            ComputeTask::decode(&mut backend.storage.get_task(&id_1).unwrap().unwrap().as_slice())
                .unwrap();
        assert_eq!(stored.executor, executor_addr());
    }

    #[test]
    fn compute_task_type_payload_pairings_rejected() {
        // Rule (k): ComputeTask ⟺ ComputeTask payload, and no other type
        // carries that payload.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let sender = fund(&backend, &sk, 5000);
        let task = task_of(&valid_task_tx(&sk, 0, [0x04u8; 32])).clone();

        // The legacy v0.3 fall-through: ComputeTask type, None payload.
        // Before RFC 0005 this executed as a transfer of `amount`.
        let mut legacy = signed_transfer(&sk, Address([9u8; 32]), 100, 0);
        legacy.tx_type = TransactionType::ComputeTask;
        resign(&sk, &mut legacy);
        let block = build_valid_block(&backend, vec![legacy]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TypePayloadMismatch(0))
        ));
        // ...and it no longer moves money.
        assert_eq!(
            backend.storage.get_account(&sender).unwrap().unwrap().balance,
            5000
        );

        // Every other type carrying a task payload.
        for tx_type in [
            TransactionType::Transfer,
            TransactionType::Stake,
            TransactionType::AnchorReceipt,
        ] {
            let mut tx = signed_task_tx(&sk, 0, task.clone());
            tx.tx_type = tx_type;
            resign(&sk, &mut tx);
            let block = build_valid_block(&backend, vec![tx]);
            assert!(
                matches!(
                    backend.apply_block(&block),
                    Err(ApplyBlockError::TypePayloadMismatch(0))
                ),
                "{tx_type:?} must not carry a ComputeTask payload"
            );
        }

        // ComputeTask type carrying a receipt payload.
        let receipt = signed_receipt_for(&sk, [0x05u8; 32], vec![], 1);
        let mut tx = signed_task_tx(&sk, 0, task.clone());
        tx.payload = TransactionPayload::AnchorReceipt(Box::new(receipt));
        resign(&sk, &mut tx);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TypePayloadMismatch(0))
        ));
        assert_untouched(&backend, &sk, &task.task_id());
    }

    #[test]
    fn compute_task_field_constraints_rejected() {
        // Rule (l): amount == 0 and receiver == zero are consensus rules,
        // not conventions (RFC 0005 §2.8).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);
        let task_id = task_of(&valid_task_tx(&sk, 0, [0x06u8; 32])).task_id();

        let mut tx = valid_task_tx(&sk, 0, [0x06u8; 32]);
        tx.amount = 1;
        resign(&sk, &mut tx);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskFieldConstraint(0))
        ));

        let mut tx = valid_task_tx(&sk, 0, [0x06u8; 32]);
        tx.receiver = Address([1u8; 32]);
        resign(&sk, &mut tx);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskFieldConstraint(0))
        ));
        assert_untouched(&backend, &sk, &task_id);
    }

    #[test]
    fn compute_task_invalid_signature_rejected() {
        // Rule (c): the transaction signature authenticates the
        // submission; the envelope carries no second signature, so
        // tampering with any envelope byte after signing is caught here.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);

        let mut tx = valid_task_tx(&sk, 0, [0x07u8; 32]);
        tx.signature[0] ^= 0x01;
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidSignature(0))
        ));

        let mut tx = valid_task_tx(&sk, 0, [0x07u8; 32]);
        let mut tampered = task_of(&tx).clone();
        tampered.execution_spec.push(0xFF);
        tx.payload = TransactionPayload::ComputeTask(Box::new(tampered));
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidSignature(0))
        ));

        // Signed by a key other than the sender.
        let other = SigningKey::from_bytes(&[71u8; 32]);
        let mut tx = valid_task_tx(&sk, 0, [0x07u8; 32]);
        resign(&other, &mut tx);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidSignature(0))
        ));
    }

    #[test]
    fn compute_task_nonce_rules() {
        // Rule (d): the sender account must exist and the nonce must
        // match. A task never creates an account and never moves balance.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let unfunded = SigningKey::from_bytes(&[72u8; 32]);
        let tx = valid_task_tx(&unfunded, 0, [0x08u8; 32]);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::SenderAccountMissing(0))
        ));

        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);
        let tx = valid_task_tx(&sk, 5, [0x08u8; 32]);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidNonce(0))
        ));
    }

    #[test]
    fn compute_task_version_rejected() {
        // Rule (m): task.version == 1.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);
        for version in [0u8, 2, 255] {
            let mut task = task_of(&valid_task_tx(&sk, 0, [0x09u8; 32])).clone();
            task.version = version;
            let tx = signed_task_tx(&sk, 0, task);
            let block = build_valid_block(&backend, vec![tx]);
            assert!(
                matches!(
                    backend.apply_block(&block),
                    Err(ApplyBlockError::TaskVersionUnsupported(0))
                ),
                "version {version} must be rejected"
            );
        }
    }

    #[test]
    fn compute_task_execution_spec_bound() {
        // Rule (n): 0 and 1024 bytes accepted, 1025 rejected; the maximal
        // stored envelope is exactly 1155 bytes (RFC 0005 §2.10).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let submitter = fund(&backend, &sk, 5000);

        let too_large = task_for(
            submitter,
            executor_addr(),
            [0x0Bu8; 32],
            vec![0x5A; MAX_EXECUTION_SPEC_BYTES + 1],
        );
        let too_large_id = too_large.task_id();
        let tx = signed_task_tx(&sk, 0, too_large);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ExecutionSpecTooLarge(0))
        ));
        assert_untouched(&backend, &sk, &too_large_id);

        let empty = task_for(submitter, executor_addr(), [0x0Au8; 32], Vec::new());
        let maximal = task_for(
            submitter,
            executor_addr(),
            [0x0Bu8; 32],
            vec![0x5A; MAX_EXECUTION_SPEC_BYTES],
        );
        let (empty_id, maximal_id) = (empty.task_id(), maximal.task_id());
        let block = build_valid_block(
            &backend,
            vec![
                signed_task_tx(&sk, 0, empty),
                signed_task_tx(&sk, 1, maximal),
            ],
        );
        backend.apply_block(&block).unwrap();
        assert_eq!(
            backend.storage.get_task(&empty_id).unwrap().unwrap().len(),
            1 + 32 * 4 + 1
        );
        assert_eq!(
            backend.storage.get_task(&maximal_id).unwrap().unwrap().len(),
            mbongo_core::MAX_COMPUTE_TASK_BYTES
        );
    }

    #[test]
    fn compute_task_submitter_mismatch_rejected() {
        // Rule (o): the envelope cannot claim a submitter other than the
        // signer, however valid the signature is.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);
        let victim = Address([0x77u8; 32]);

        let task = task_for(victim, executor_addr(), [0x0Cu8; 32], vec![1]);
        let task_id = task.task_id();
        let tx = signed_task_tx(&sk, 0, task);
        assert!(tx.verify_signature(), "the signature itself is sound");
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::SenderSubmitterMismatch(0))
        ));
        assert_untouched(&backend, &sk, &task_id);
    }

    #[test]
    fn compute_task_duplicate_in_prior_state_rejected() {
        // Rule (p), prior-state half: the task_id is content-derived, so
        // the same envelope under a fresh nonce is the same task and the
        // second registration loses (first-registered-wins).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let sender = fund(&backend, &sk, 5000);

        let first = valid_task_tx(&sk, 0, [0x0Du8; 32]);
        let task_id = task_of(&first).task_id();
        let block = build_valid_block(&backend, vec![first]);
        backend.apply_block(&block).unwrap();

        let again = valid_task_tx(&sk, 1, [0x0Du8; 32]);
        assert_eq!(
            task_of(&again).task_id(),
            task_id,
            "the nonce is not in the envelope"
        );
        let block = build_valid_block(&backend, vec![again]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskAlreadyRegistered(0))
        ));
        // The rejected block changed nothing.
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
        assert_eq!(
            backend.storage.get_account(&sender).unwrap().unwrap().nonce,
            1
        );
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 1);
    }

    #[test]
    fn compute_task_duplicate_in_same_block_rejected() {
        // Rule (p), same-block half: the whole block is invalid.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);

        let a = valid_task_tx(&sk, 0, [0x0Eu8; 32]);
        let b = valid_task_tx(&sk, 1, [0x0Eu8; 32]);
        let task_id = task_of(&a).task_id();
        let block = build_valid_block(&backend, vec![a, b]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskRepeatedInBlock(1))
        ));
        // Atomic: the first task was not persisted either.
        assert_untouched(&backend, &sk, &task_id);
    }

    #[test]
    fn stored_compute_task_tx_in_later_block_rejected() {
        // The byte-identical transaction must not take the idempotent
        // skip path: a stored task transaction implies a registered task.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&backend, &sk, 5000);

        let tx = valid_task_tx(&sk, 0, [0x0Fu8; 32]);
        let block = build_valid_block(&backend, vec![tx.clone()]);
        backend.apply_block(&block).unwrap();

        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskAlreadyRegistered(0))
        ));
        assert_eq!(backend.storage.get_latest_height().unwrap(), 1);
    }

    #[test]
    fn compute_task_rule_precedence_is_deterministic() {
        // When several rules fail at once, the first in normative order
        // names the error: (l) before (m) before (n) before (o).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let submitter = fund(&backend, &sk, 5000);
        let big = vec![0x5A; MAX_EXECUTION_SPEC_BYTES + 1];

        // (l) + (m)
        let mut task = task_for(submitter, executor_addr(), [0x10u8; 32], vec![]);
        task.version = 2;
        let mut tx = signed_task_tx(&sk, 0, task);
        tx.amount = 1;
        resign(&sk, &mut tx);
        let block = build_valid_block(&backend, vec![tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskFieldConstraint(0))
        ));

        // (m) + (n)
        let mut task = task_for(submitter, executor_addr(), [0x10u8; 32], big.clone());
        task.version = 2;
        let block = build_valid_block(&backend, vec![signed_task_tx(&sk, 0, task)]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskVersionUnsupported(0))
        ));

        // (n) + (o)
        let task = task_for(Address([0x77u8; 32]), executor_addr(), [0x10u8; 32], big);
        let block = build_valid_block(&backend, vec![signed_task_tx(&sk, 0, task)]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ExecutionSpecTooLarge(0))
        ));
    }

    #[test]
    fn compute_task_then_matching_receipt_in_one_block() {
        // A task and the receipt that answers it may share one block, in
        // that order, and land in their two separate indexes: the
        // same-block half of RFC 0005 rule (q), with (r) and (s) holding —
        // same task_id, same input_commitment, the named executor.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let submitter = fund(&backend, &sk, 5000);
        let ek = executor_key();
        fund(&backend, &ek, 10);

        let task = task_for(submitter, executor_addr(), [0x11u8; 32], vec![4, 2]);
        let task_id = task.task_id();
        let task_tx = signed_task_tx(&sk, 0, task.clone());

        let mut receipt = signed_receipt_for(&ek, task_id, vec![], 1);
        receipt.input_commitment = task.input_commitment;
        receipt.signature = ek.sign(&receipt.receipt_hash().0).to_bytes();
        let anchor_tx = signed_anchor_tx(&ek, 0, receipt.clone());

        let block = build_valid_block(&backend, vec![task_tx, anchor_tx]);
        backend.apply_block(&block).unwrap();
        assert_eq!(
            backend.storage.get_task(&task_id).unwrap(),
            Some(task.encode())
        );
        assert_eq!(
            backend.storage.get_receipt(&task_id).unwrap(),
            Some(receipt.encode())
        );
    }

    #[test]
    fn compute_task_block_revalidates_on_a_follower() {
        // The same block, re-decoded from storage bytes and re-applied by
        // an independent node, passes the same rules and yields the same
        // tip: block validation and block re-validation are one path.
        let producer = make_backend();
        producer.ensure_genesis().unwrap();
        let follower = make_backend();
        follower.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        fund(&producer, &sk, 5000);
        fund(&follower, &sk, 5000);

        let tx = valid_task_tx(&sk, 0, [0x12u8; 32]);
        let task_id = task_of(&tx).task_id();
        let block = build_valid_block(&producer, vec![tx]);
        let producer_tip = producer.apply_block(&block).unwrap();

        // Round-trip through the canonical block encoding, as sync does.
        let stored = producer.storage.get_block_by_height(1).unwrap().unwrap();
        let bytes = stored.encode();
        let decoded = Block::decode(&mut &bytes[..]).unwrap();
        assert_eq!(decoded, block);

        let follower_tip = follower.apply_block(&decoded).unwrap();
        assert_eq!(follower_tip, producer_tip);
        assert_eq!(
            follower.storage.get_task(&task_id).unwrap(),
            producer.storage.get_task(&task_id).unwrap()
        );
        // And the JSON form a block RPC returns carries the task intact.
        let json = serde_json::to_value(&block).unwrap();
        let back: Block = serde_json::from_value(json).unwrap();
        assert_eq!(back, block);
    }

    // ── RFC 0005: admission mirrors consensus ─────────────────────────

    #[tokio::test]
    async fn submit_compute_task_admitted_and_included() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let sender = fund(&backend, &sk, 5000);

        let tx = valid_task_tx(&sk, 0, [0x20u8; 32]);
        let task_id = task_of(&tx).task_id();
        let hash = backend.submit_transaction(tx.clone()).await.unwrap();
        // Idempotent re-submission while pending.
        assert_eq!(backend.submit_transaction(tx.clone()).await.unwrap(), hash);
        // Same envelope, fresh nonce: the same task, already pending.
        let err = backend
            .submit_transaction(valid_task_tx(&sk, 1, [0x20u8; 32]))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("compute task already pending"),
            "{err}"
        );
        // A different salt is admitted behind it.
        backend.submit_transaction(valid_task_tx(&sk, 1, [0x21u8; 32])).await.unwrap();

        backend.produce_block().await.unwrap();
        assert!(backend.storage.has_task(&task_id).unwrap());
        assert_eq!(
            backend.storage.get_account(&sender).unwrap().unwrap().nonce,
            2
        );
        assert_eq!(
            backend.storage.get_account(&sender).unwrap().unwrap().balance,
            5000
        );

        // After inclusion: the byte-identical transaction and the same
        // envelope under a new nonce are both duplicate registrations.
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(
            err.to_string().contains("task_id already registered"),
            "{err}"
        );
        let err = backend
            .submit_transaction(valid_task_tx(&sk, 2, [0x20u8; 32]))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("task_id already registered"),
            "{err}"
        );
    }

    #[tokio::test]
    async fn submit_compute_task_rejections_mirror_apply_block() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let sk = SigningKey::from_bytes(&[70u8; 32]);
        let submitter = fund(&backend, &sk, 5000);
        let base = || task_of(&valid_task_tx(&sk, 0, [0x22u8; 32])).clone();

        // (k) legacy None payload.
        let mut legacy = signed_transfer(&sk, Address([9u8; 32]), 1, 0);
        legacy.tx_type = TransactionType::ComputeTask;
        resign(&sk, &mut legacy);
        let err = backend.submit_transaction(legacy).await.unwrap_err();
        assert!(err.to_string().contains("payload does not match"), "{err}");

        // (l)
        let mut tx = signed_task_tx(&sk, 0, base());
        tx.amount = 1;
        resign(&sk, &mut tx);
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(
            err.to_string().contains("compute task requires amount 0"),
            "{err}"
        );

        // (c)
        let mut tx = signed_task_tx(&sk, 0, base());
        tx.signature[1] ^= 0x01;
        let err = backend.submit_transaction(tx).await.unwrap_err();
        assert!(err.to_string().contains("invalid signature"), "{err}");

        // (m)
        let mut task = base();
        task.version = 2;
        let err = backend.submit_transaction(signed_task_tx(&sk, 0, task)).await.unwrap_err();
        assert!(
            err.to_string().contains("unsupported compute task version"),
            "{err}"
        );

        // (n)
        let mut task = base();
        task.execution_spec = vec![0; MAX_EXECUTION_SPEC_BYTES + 1];
        let err = backend.submit_transaction(signed_task_tx(&sk, 0, task)).await.unwrap_err();
        assert!(
            err.to_string().contains("execution_spec too large"),
            "{err}"
        );

        // (o)
        let mut task = base();
        task.submitter = Address([0x77u8; 32]);
        let err = backend.submit_transaction(signed_task_tx(&sk, 0, task)).await.unwrap_err();
        assert!(
            err.to_string().contains("sender must equal task submitter"),
            "{err}"
        );

        // Nothing above reached the mempool.
        assert_eq!(backend.mempool.read().await.len(), 0);
        let _ = submitter;
    }

    // ── RFC 0005: receipt binding, rules (q)–(s) ──────────────────────

    /// A client key that commits tasks and never anchors: keeps the
    /// executor's nonces untouched.
    fn client_key() -> SigningKey {
        SigningKey::from_bytes(&[0xC0u8; 32])
    }

    /// Commits a task naming `executor` in its own block at the current
    /// tip, from the funded client key. Returns the registered envelope.
    fn register_task(
        backend: &NodeBackend<InMemoryStorage>,
        executor: Address,
        salt: [u8; 32],
        input_commitment: [u8; 32],
    ) -> ComputeTask {
        let ck = client_key();
        let client = Address(ck.verifying_key().to_bytes());
        let nonce = backend.storage.get_account(&client).unwrap().map_or(0, |a| a.nonce);
        if nonce == 0 {
            fund(backend, &ck, 1);
        }
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client,
            executor,
            salt,
            input_commitment,
            execution_spec: vec![0x51],
        };
        let block = build_valid_block(backend, vec![signed_task_tx(&ck, nonce, task.clone())]);
        backend.apply_block(&block).unwrap();
        assert!(backend.storage.has_task(&task.task_id()).unwrap());
        task
    }

    /// A receipt answering `task`, signed by `executor_sk`: the task's
    /// task_id and input_commitment, the signer as executor.
    fn receipt_answering(executor_sk: &SigningKey, task: &ComputeTask) -> Receipt {
        let mut receipt = signed_receipt_for(executor_sk, task.task_id(), vec![0xAB], 1);
        receipt.input_commitment = task.input_commitment;
        receipt.signature = executor_sk.sign(&receipt.receipt_hash().0).to_bytes();
        receipt
    }

    /// Anchors `receipt` from `sk` in a fresh block at the tip.
    fn anchor_block(
        backend: &NodeBackend<InMemoryStorage>,
        sk: &SigningKey,
        nonce: u64,
        receipt: Receipt,
    ) -> Result<Hash, ApplyBlockError> {
        let block = build_valid_block(backend, vec![signed_anchor_tx(sk, nonce, receipt)]);
        backend.apply_block(&block)
    }

    #[test]
    fn valid_anchor_for_registered_task() {
        // (q), (r), (s) positive: the named executor answers a registered
        // task over the committed input. Both derived states exist
        // afterwards (RFC 0005 §4.1): submitted and completed.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x30u8; 32], [0x1Cu8; 32]);
        let task_id = task.task_id();

        let receipt = receipt_answering(&ek, &task);
        anchor_block(&backend, &ek, 0, receipt.clone()).unwrap();

        assert!(backend.storage.has_task(&task_id).unwrap());
        assert_eq!(
            backend.storage.get_receipt(&task_id).unwrap(),
            Some(receipt.encode())
        );
        let acc = backend.storage.get_account(&executor).unwrap().unwrap();
        assert_eq!(acc.nonce, 1);
        assert_eq!(acc.balance, 10, "anchoring moves no balance");
        assert_eq!(backend.storage.get_latest_height().unwrap(), 2);
    }

    #[test]
    fn anchor_unknown_task_rejected() {
        // (q) negative, three ways: an arbitrary task_id; a task_id whose
        // registration failed (never stored); the receipt that was valid
        // before RFC 0005 (v0.3 anchored it unbound).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);

        let arbitrary = signed_receipt_for(&ek, [0x7Au8; 32], vec![], 1);
        assert!(matches!(
            anchor_block(&backend, &ek, 0, arbitrary),
            Err(ApplyBlockError::TaskNotRegistered(0))
        ));

        // A task whose registration failed: over the bound, so its block
        // was rejected and nothing under its task_id exists.
        let ck = client_key();
        let client = fund(&backend, &ck, 1);
        let mut failed = task_for(
            client,
            executor,
            [0x31u8; 32],
            vec![0x5A; MAX_EXECUTION_SPEC_BYTES + 1],
        );
        failed.submitter = client;
        let block = build_valid_block(&backend, vec![signed_task_tx(&ck, 0, failed.clone())]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ExecutionSpecTooLarge(0))
        ));
        assert!(!backend.storage.has_task(&failed.task_id()).unwrap());
        let for_failed = receipt_answering(&ek, &failed);
        assert!(matches!(
            anchor_block(&backend, &ek, 0, for_failed),
            Err(ApplyBlockError::TaskNotRegistered(0))
        ));

        // Nothing was anchored, and the executor's nonce is untouched.
        assert!(!backend.storage.has_receipt(&[0x7Au8; 32]).unwrap());
        assert_eq!(
            backend.storage.get_account(&executor).unwrap().unwrap().nonce,
            0
        );
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
    }

    #[test]
    fn anchor_wrong_task_binding_rejected() {
        // A receipt built for task A but carrying task B's task_id: the
        // registered task under that id is the authority, and the receipt
        // is judged against B. Where B committed a different input the
        // verdict is (r); where B named a different executor it is (s).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task_a = register_task(&backend, executor, [0x32u8; 32], [0x1Cu8; 32]);
        let task_b = register_task(&backend, executor, [0x33u8; 32], [0x2Du8; 32]);
        let task_c = register_task(&backend, Address([0xE2u8; 32]), [0x34u8; 32], [0x1Cu8; 32]);

        // A's receipt, B's id: same executor, different input → (r).
        let mut receipt = receipt_answering(&ek, &task_a);
        receipt.task_id = task_b.task_id();
        receipt.signature = ek.sign(&receipt.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &ek, 0, receipt),
            Err(ApplyBlockError::ReceiptInputCommitmentMismatch(0))
        ));

        // A's receipt, C's id: same input, C named another executor → (s).
        let mut receipt = receipt_answering(&ek, &task_a);
        receipt.task_id = task_c.task_id();
        receipt.signature = ek.sign(&receipt.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &ek, 0, receipt),
            Err(ApplyBlockError::ReceiptExecutorNotAuthorised(0))
        ));

        // The correct binding still works afterwards.
        anchor_block(&backend, &ek, 0, receipt_answering(&ek, &task_a)).unwrap();
        assert!(backend.storage.has_receipt(&task_a.task_id()).unwrap());
        assert!(!backend.storage.has_receipt(&task_b.task_id()).unwrap());
        assert!(!backend.storage.has_receipt(&task_c.task_id()).unwrap());
    }

    #[test]
    fn anchor_wrong_input_commitment_rejected() {
        // (r) negative: one flipped bit, and a commitment that is valid
        // for another registered task. Consensus compares the 32 bytes and
        // learns nothing about how either was derived.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x35u8; 32], [0x1Cu8; 32]);
        let other = register_task(&backend, executor, [0x36u8; 32], [0x2Du8; 32]);

        let mut flipped = receipt_answering(&ek, &task);
        flipped.input_commitment[31] ^= 0x01;
        flipped.signature = ek.sign(&flipped.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &ek, 0, flipped),
            Err(ApplyBlockError::ReceiptInputCommitmentMismatch(0))
        ));

        let mut borrowed = receipt_answering(&ek, &task);
        borrowed.input_commitment = other.input_commitment;
        borrowed.signature = ek.sign(&borrowed.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &ek, 0, borrowed),
            Err(ApplyBlockError::ReceiptInputCommitmentMismatch(0))
        ));
        assert!(!backend.storage.has_receipt(&task.task_id()).unwrap());
    }

    #[test]
    fn anchor_wrong_executor_rejected() {
        // (s) negative: a different, fully funded executor produces a
        // well-formed receipt for the task — its own executor field, its
        // own valid receipt signature, its own valid anchoring signature,
        // so rules (c), (g) and (h) all pass — and is rejected because the
        // task named someone else. This is the squatting receipt of RFC
        // 0005 §9.1, and it never consumes the task.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x37u8; 32], [0x1Cu8; 32]);
        let squatter = SigningKey::from_bytes(&[0xE9u8; 32]);
        fund(&backend, &squatter, 10);

        let receipt = receipt_answering(&squatter, &task);
        assert!(
            verify_receipt_signature(&receipt),
            "the receipt itself is sound"
        );
        assert!(matches!(
            anchor_block(&backend, &squatter, 0, receipt),
            Err(ApplyBlockError::ReceiptExecutorNotAuthorised(0))
        ));
        assert!(!backend.storage.has_receipt(&task.task_id()).unwrap());

        // The named executor can still answer: the task was not consumed.
        anchor_block(&backend, &ek, 0, receipt_answering(&ek, &task)).unwrap();
        assert!(backend.storage.has_receipt(&task.task_id()).unwrap());
    }

    #[test]
    fn task_submitter_not_executor_cannot_anchor() {
        // The client who committed the task, anchoring its own task with
        // itself as receipt executor: authenticated (g), unauthorised (s).
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let task = register_task(&backend, executor_addr(), [0x38u8; 32], [0x1Cu8; 32]);
        let ck = client_key();
        let receipt = receipt_answering(&ck, &task);
        assert!(matches!(
            anchor_block(&backend, &ck, 1, receipt),
            Err(ApplyBlockError::ReceiptExecutorNotAuthorised(0))
        ));
        assert!(!backend.storage.has_receipt(&task.task_id()).unwrap());
    }

    #[test]
    fn unrelated_signer_cannot_anchor_the_executors_receipt() {
        // A control-plane-like relayer holding the executor's genuine
        // signed receipt bytes but not the executor's key: the anchoring
        // sender is not the receipt executor, so rule (g) rejects it
        // before (s) is reached. Forging the anchoring signature instead
        // fails rule (c). Either way, no key but the executor's anchors.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x39u8; 32], [0x1Cu8; 32]);
        let relayer = SigningKey::from_bytes(&[0xCCu8; 32]);
        fund(&backend, &relayer, 10);

        let genuine = receipt_answering(&ek, &task);
        assert!(matches!(
            anchor_block(&backend, &relayer, 0, genuine.clone()),
            Err(ApplyBlockError::SenderExecutorMismatch(0))
        ));

        let mut forged = signed_anchor_tx(&ek, 0, genuine);
        forged.signature = relayer.sign(&forged.signing_payload()).to_bytes();
        let block = build_valid_block(&backend, vec![forged]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::InvalidSignature(0))
        ));
        assert!(!backend.storage.has_receipt(&task.task_id()).unwrap());
    }

    #[test]
    fn same_block_anchor_before_task_rejected() {
        // (q) is "prior chain state or EARLIER in the same block": the
        // receipt ahead of its task sees no task, and the block fails.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ck = client_key();
        let client = fund(&backend, &ck, 1);
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);

        let task = task_for(client, executor, [0x3Au8; 32], vec![1]);
        let task_id = task.task_id();
        let anchor_tx = signed_anchor_tx(&ek, 0, receipt_answering(&ek, &task));
        let task_tx = signed_task_tx(&ck, 0, task);
        let block = build_valid_block(&backend, vec![anchor_tx, task_tx]);
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::TaskNotRegistered(0))
        ));
        // Neither the task nor the receipt was persisted.
        assert!(!backend.storage.has_task(&task_id).unwrap());
        assert!(!backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
    }

    #[test]
    fn invalid_anchor_block_is_atomic() {
        // A valid task at index 0 and a receipt violating (r) at index 1:
        // the whole block fails through the existing atomic batch, so the
        // task is not persisted either, and no nonce or sequence moves.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ck = client_key();
        let client = fund(&backend, &ck, 1);
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);

        let task = task_for(client, executor, [0x3Bu8; 32], vec![1]);
        let task_id = task.task_id();
        let mut receipt = receipt_answering(&ek, &task);
        receipt.input_commitment[0] ^= 0x80;
        receipt.signature = ek.sign(&receipt.receipt_hash().0).to_bytes();
        let block = build_valid_block(
            &backend,
            vec![
                signed_task_tx(&ck, 0, task),
                signed_anchor_tx(&ek, 0, receipt),
            ],
        );
        assert!(matches!(
            backend.apply_block(&block),
            Err(ApplyBlockError::ReceiptInputCommitmentMismatch(1))
        ));
        assert!(!backend.storage.has_task(&task_id).unwrap());
        assert!(!backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(
            backend.storage.get_account(&client).unwrap().unwrap().nonce,
            0
        );
        assert_eq!(
            backend.storage.get_account(&executor).unwrap().unwrap().nonce,
            0
        );
        assert_eq!(backend.storage.get_last_included_tx_seq().unwrap(), 0);
        assert_eq!(backend.storage.get_latest_height().unwrap(), 0);
    }

    #[test]
    fn binding_precedence_is_deterministic() {
        // Existing rules keep their positions: a duplicate (i) is reported
        // before any binding question; a bad receipt signature (h) before
        // an unknown task; and within the binding, (q) before (r) before
        // (s) when several fail at once.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x3Cu8; 32], [0x1Cu8; 32]);
        let other_exec = SigningKey::from_bytes(&[0xE9u8; 32]);
        fund(&backend, &other_exec, 10);

        // (h) before (q): unknown task AND bad signature → signature.
        let mut bad = signed_receipt_for(&ek, [0x7Bu8; 32], vec![], 1);
        bad.signature = [0xEEu8; 64];
        assert!(matches!(
            anchor_block(&backend, &ek, 0, bad),
            Err(ApplyBlockError::InvalidReceiptSignature(0))
        ));

        // (r) before (s): wrong input AND wrong executor → input.
        let mut both = receipt_answering(&other_exec, &task);
        both.input_commitment[0] ^= 0x01;
        both.signature = other_exec.sign(&both.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &other_exec, 0, both),
            Err(ApplyBlockError::ReceiptInputCommitmentMismatch(0))
        ));

        // (i) before (q)–(s): once anchored, a second receipt for the task
        // — even from an unauthorised executor with the wrong input — is a
        // duplicate, not a binding failure. First-anchored-wins is the
        // v0.3 rule; nothing here re-decides it.
        anchor_block(&backend, &ek, 0, receipt_answering(&ek, &task)).unwrap();
        let mut second = receipt_answering(&other_exec, &task);
        second.input_commitment[0] ^= 0x01;
        second.signature = other_exec.sign(&second.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &other_exec, 0, second),
            Err(ApplyBlockError::TaskIdAlreadyAnchored(0))
        ));
        // And a second, different but fully bound receipt from the named
        // executor is likewise a duplicate.
        let mut distinct = receipt_answering(&ek, &task);
        distinct.output_commitment = [0x99u8; 32];
        distinct.signature = ek.sign(&distinct.receipt_hash().0).to_bytes();
        assert!(matches!(
            anchor_block(&backend, &ek, 1, distinct),
            Err(ApplyBlockError::TaskIdAlreadyAnchored(0))
        ));
    }

    #[test]
    fn corrupt_stored_task_is_a_storage_failure_not_an_unknown_task() {
        // Bytes under a task_id that do not decode are consensus's own
        // canonical encoding gone wrong: reported as a storage error, so a
        // node never turns corruption into a protocol verdict.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x3Du8; 32], [0x1Cu8; 32]);
        let task_id = task.task_id();
        backend
            .storage
            .write_batch(vec![BatchOp::PutTask(task_id, vec![0xFF, 0x00])])
            .unwrap();
        let err = anchor_block(&backend, &ek, 0, receipt_answering(&ek, &task)).unwrap_err();
        assert!(
            matches!(&err, ApplyBlockError::Storage(msg) if msg.contains("corrupt task bytes")),
            "{err}"
        );
    }

    // ── RFC 0005 (q)–(s): admission mirrors consensus ─────────────────

    #[tokio::test]
    async fn admission_and_block_validation_agree_on_binding() {
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = register_task(&backend, executor, [0x40u8; 32], [0x1Cu8; 32]);
        let squatter = SigningKey::from_bytes(&[0xE9u8; 32]);
        fund(&backend, &squatter, 10);

        // (q)
        let unknown = signed_receipt_for(&ek, [0x7Cu8; 32], vec![], 1);
        let err = backend.submit_transaction(signed_anchor_tx(&ek, 0, unknown)).await.unwrap_err();
        assert!(err.to_string().contains("not a registered task"), "{err}");
        // (r)
        let mut wrong_input = receipt_answering(&ek, &task);
        wrong_input.input_commitment[5] ^= 0x10;
        wrong_input.signature = ek.sign(&wrong_input.receipt_hash().0).to_bytes();
        let err = backend
            .submit_transaction(signed_anchor_tx(&ek, 0, wrong_input))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("input_commitment does not match"),
            "{err}"
        );
        // (s)
        let err = backend
            .submit_transaction(signed_anchor_tx(
                &squatter,
                0,
                receipt_answering(&squatter, &task),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not authorised"), "{err}");
        assert_eq!(backend.mempool.read().await.len(), 0);

        // The bound receipt is admitted and included.
        backend
            .submit_transaction(signed_anchor_tx(&ek, 0, receipt_answering(&ek, &task)))
            .await
            .unwrap();
        backend.produce_block().await.unwrap();
        assert!(backend.storage.has_receipt(&task.task_id()).unwrap());

        // Identical-bytes resubmission after inclusion: rejected as already
        // anchored (RFC 0002 first-anchored-wins), so operational retry is
        // lookup first, never blind resubmission (E §11.2).
        let err = backend
            .submit_transaction(signed_anchor_tx(&ek, 0, receipt_answering(&ek, &task)))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("already anchored"), "{err}");
    }

    #[tokio::test]
    async fn admission_accepts_receipt_behind_pending_task() {
        // The same-block half at admission: a task pending in the mempool
        // precedes its receipt in the produced block, so the receipt is
        // admitted; a receipt whose task is neither stored nor pending is
        // not.
        let backend = make_backend();
        backend.ensure_genesis().unwrap();
        let ck = client_key();
        let client = fund(&backend, &ck, 1);
        let ek = executor_key();
        let executor = fund(&backend, &ek, 10);
        let task = task_for(client, executor, [0x41u8; 32], vec![7]);
        let task_id = task.task_id();

        let err = backend
            .submit_transaction(signed_anchor_tx(&ek, 0, receipt_answering(&ek, &task)))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not a registered task"), "{err}");

        backend.submit_transaction(signed_task_tx(&ck, 0, task.clone())).await.unwrap();
        backend
            .submit_transaction(signed_anchor_tx(&ek, 0, receipt_answering(&ek, &task)))
            .await
            .unwrap();
        assert_eq!(backend.mempool.read().await.len(), 2);
        backend.produce_block().await.unwrap();
        assert!(backend.storage.has_task(&task_id).unwrap());
        assert!(backend.storage.has_receipt(&task_id).unwrap());
        assert_eq!(backend.mempool.read().await.len(), 0);
    }

    // ── RFC 0005 §12: the neutral binding vectors drive consensus ─────

    const BINDING_FIXTURE: &str =
        include_str!("../../../test-vectors/compute-task/anchor-binding-v1.json");
    const TASK_FIXTURE: &str =
        include_str!("../../../test-vectors/compute-task/compute-task-v1.json");
    const RECEIPT_FIXTURE: &str = include_str!("../../../test-vectors/receipt/receipt-v1.json");

    fn fixture_hex(v: &serde_json::Value) -> Vec<u8> {
        let s = v.as_str().expect("hex string");
        assert!(!s.starts_with("0x"));
        hex_decode(s)
    }

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    fn fixture_32(v: &serde_json::Value) -> [u8; 32] {
        fixture_hex(v).try_into().expect("32 bytes")
    }

    /// The receipt executor of a binding vector, which rule (g) makes the
    /// anchoring sender.
    fn r_executor(v: &serde_json::Value) -> &serde_json::Value {
        &v["receipt"]["executor"]
    }

    fn fixture_task(tdoc: &serde_json::Value, name: &str) -> ComputeTask {
        let entry = tdoc["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"].as_str() == Some(name))
            .unwrap_or_else(|| panic!("task vector {name}"));
        let t = &entry["task"];
        let spec = &t["execution_spec"];
        let execution_spec = match spec["pattern"].as_str() {
            Some("repeat") => {
                let byte = fixture_hex(&spec["byte"]);
                vec![byte[0]; spec["length"].as_u64().unwrap() as usize]
            }
            Some("literal") => fixture_hex(&spec["hex"]),
            other => panic!("pattern {other:?}"),
        };
        let task = ComputeTask {
            version: t["version"].as_u64().unwrap() as u8,
            submitter: Address(fixture_32(&t["submitter"])),
            executor: Address(fixture_32(&t["executor"])),
            salt: fixture_32(&t["salt"]),
            input_commitment: fixture_32(&t["input_commitment"]),
            execution_spec,
        };
        assert_eq!(task.task_id(), fixture_32(&entry["expected"]["task_id"]));
        task
    }

    #[test]
    fn anchor_binding_vectors_drive_consensus() {
        let bdoc: serde_json::Value = serde_json::from_str(BINDING_FIXTURE).unwrap();
        let tdoc: serde_json::Value = serde_json::from_str(TASK_FIXTURE).unwrap();
        let rdoc: serde_json::Value = serde_json::from_str(RECEIPT_FIXTURE).unwrap();
        assert_eq!(bdoc["fixture_version"].as_u64(), Some(1));
        let vectors = bdoc["vectors"].as_array().unwrap();
        assert_eq!(vectors.len(), 5, "vector cardinality");

        // Keys are resolved from the referenced fixtures, never restated.
        let submitter_sk = SigningKey::from_bytes(&fixture_32(&rdoc["test_key"]["ed25519_seed"]));
        let executor_sk =
            SigningKey::from_bytes(&fixture_32(&tdoc["test_keys"]["executor"]["ed25519_seed"]));
        let executor = Address(executor_sk.verifying_key().to_bytes());
        let submitter = Address(submitter_sk.verifying_key().to_bytes());

        let mut outcomes = std::collections::BTreeMap::new();
        for v in vectors {
            let name = v["name"].as_str().unwrap();
            let backend = make_backend();
            backend.ensure_genesis().unwrap();
            fund(&backend, &submitter_sk, 10);

            // Register the referenced task exactly as the fixture states it,
            // committed by its submitter in a block of its own.
            if let Some(task_name) = v["task_vector"].as_str() {
                let task = fixture_task(&tdoc, task_name);
                assert_eq!(task.submitter, submitter);
                assert_eq!(task.executor, executor);
                let block =
                    build_valid_block(&backend, vec![signed_task_tx(&submitter_sk, 0, task)]);
                backend.apply_block(&block).unwrap();
            }

            // The anchoring account sits at the nonce the vector pins (test
            // seeding, as `fund` is), so the vector's bytes are exactly
            // what enters consensus.
            let mut anchoring = Account::new(Address(fixture_32(r_executor(v))));
            anchoring.balance = 10;
            anchoring.nonce = v["transaction"]["nonce"].as_u64().unwrap();
            backend.storage.put_account(&anchoring.address, &anchoring).unwrap();

            // Rebuild the receipt and transaction from the fixture fields;
            // the pinned bytes, hash and signatures must be what production
            // produces, and the transaction must verify.
            let r = &v["receipt"];
            let receipt = Receipt {
                version: r["version"].as_u64().unwrap() as u8,
                task_id: fixture_32(&r["task_id"]),
                input_commitment: fixture_32(&r["input_commitment"]),
                output_commitment: fixture_32(&r["output_commitment"]),
                executor: Address(fixture_32(&r["executor"])),
                metadata: vec![],
                signature: fixture_hex(&v["expected"]["executor_signature"]).try_into().unwrap(),
            };
            assert_eq!(
                receipt.receipt_hash().0,
                fixture_32(&v["expected"]["receipt_hash"]),
                "{name}: receipt hash"
            );
            assert!(
                verify_receipt_signature(&receipt),
                "{name}: receipt signature"
            );
            assert_eq!(
                receipt.encode(),
                fixture_hex(&v["expected"]["receipt_full_encoding"]),
                "{name}: receipt encoding"
            );
            let tx = Transaction {
                tx_type: TransactionType::AnchorReceipt,
                sender: receipt.executor,
                receiver: Address::zero(),
                amount: 0,
                nonce: v["transaction"]["nonce"].as_u64().unwrap(),
                payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
                signature: fixture_hex(&v["expected"]["transaction_signature"]).try_into().unwrap(),
            };
            assert!(tx.verify_signature(), "{name}: transaction signature");
            assert_eq!(
                tx.signing_payload(),
                fixture_hex(&v["expected"]["signing_payload"]),
                "{name}: signing payload"
            );
            assert_eq!(
                tx.encode(),
                fixture_hex(&v["expected"]["full_transaction"]),
                "{name}: full encoding"
            );
            assert_eq!(
                compute_tx_hash(&tx).0,
                fixture_32(&v["expected"]["transaction_hash"]),
                "{name}: transaction hash"
            );

            // The consensus verdict is the one the fixture states.
            let block = build_valid_block(&backend, vec![tx]);
            let result = backend.apply_block(&block);
            let expected_valid = v["consensus"]["valid"].as_bool().unwrap();
            let rule = v["consensus"]["rule"].as_str();
            match (&result, rule) {
                (Ok(_), None) => assert!(expected_valid, "{name}"),
                (Err(ApplyBlockError::TaskNotRegistered(0)), Some("q"))
                | (Err(ApplyBlockError::ReceiptInputCommitmentMismatch(0)), Some("r"))
                | (Err(ApplyBlockError::ReceiptExecutorNotAuthorised(0)), Some("s")) => {
                    assert!(!expected_valid, "{name}");
                }
                (other, rule) => panic!("{name}: got {other:?}, fixture expects rule {rule:?}"),
            }
            outcomes.insert(name.to_string(), result.is_ok());
        }
        // Exactly the outcomes RFC 0005 §12 asks for: one bound receipt per
        // task shape, and one rejection per rule.
        assert_eq!(outcomes.values().filter(|ok| **ok).count(), 2);
        assert_eq!(outcomes.values().filter(|ok| !**ok).count(), 3);
    }
}
