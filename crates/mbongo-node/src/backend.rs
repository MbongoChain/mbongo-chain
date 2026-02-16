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
    compute_transactions_root, Account, Address, Block, BlockBody, BlockHeader, Hash, Transaction,
};
use mbongo_network::rpc::{BackendError, RpcBackend};
use mbongo_network::BlockBroadcaster;
use mbongo_storage::{BatchOp, Storage};
use parity_scale_codec::Encode;
use tokio::sync::RwLock;

use crate::mempool::{Mempool, MempoolError};

/// Maximum transactions per block.
const MAX_TX_PER_BLOCK: usize = 1000;

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

        let mut last_seq = storage
            .get_last_included_tx_seq()
            .map_err(|e| ApplyBlockError::Storage(e.to_string()))?;

        for (i, tx) in block.body.transactions.iter().enumerate() {
            // Signature validation.
            if !tx.verify_signature() {
                return Err(ApplyBlockError::InvalidSignature(i));
            }

            let tx_hash = compute_tx_hash(tx);

            // Skip if already persisted (idempotent re-apply guard).
            let already_stored = storage
                .get_transaction(&tx_hash)
                .map_err(|e| ApplyBlockError::Storage(e.to_string()))?
                .is_some();
            if already_stored {
                continue;
            }

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

            // Allocate sequence number (safe to leak on batch failure).
            last_seq =
                storage.next_tx_seq().map_err(|e| ApplyBlockError::Storage(e.to_string()))?;

            ops.push(BatchOp::PutTransaction(tx_hash, tx.clone()));
            ops.push(BatchOp::PutTxSeqIndex(last_seq, tx_hash));

            account_cache.insert(sender_addr, sender);
            account_cache.insert(receiver_addr, receiver);
        }

        // Flush modified accounts.
        for (addr, account) in &account_cache {
            ops.push(BatchOp::PutAccount(*addr, account.clone()));
        }

        if !block.body.transactions.is_empty() {
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
    /// A transaction in the block has an invalid signature.
    #[error("invalid transaction signature at index {0}")]
    InvalidSignature(usize),
    /// A transaction has an invalid nonce.
    #[error("invalid nonce at index {0}")]
    InvalidNonce(usize),
    /// A transaction has insufficient balance.
    #[error("insufficient balance at index {0}")]
    InsufficientBalance(usize),
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
            let tx_hash = compute_tx_hash(&tx);

            // Idempotence: if already in storage (included in a block), return the hash.
            if storage
                .get_transaction(&tx_hash)
                .map_err(|_| BackendError::Internal("storage error".to_string()))?
                .is_some()
            {
                return Ok(tx_hash.to_string());
            }

            // Verify signature.
            if !tx.verify_signature() {
                return Err(BackendError::Internal("invalid signature".to_string()));
            }

            // Load sender account for validation (nonce, balance).
            let sender_addr = tx.sender;
            let sender = storage
                .get_account(&sender_addr)
                .map_err(|_| BackendError::Internal("storage error".to_string()))?
                .ok_or_else(|| BackendError::Internal("insufficient balance".to_string()))?;

            // Validate nonce (do not mutate; we only check).
            if sender.nonce != tx.nonce {
                return Err(BackendError::Internal("invalid nonce".to_string()));
            }

            // Validate balance.
            if sender.balance < tx.amount {
                return Err(BackendError::Internal("insufficient balance".to_string()));
            }

            // Insert into mempool. Idempotent: if already in mempool, return hash.
            let mut pool = mempool.write().await;
            if pool.contains_hash(&tx_hash) {
                return Ok(tx_hash.to_string());
            }
            pool.insert(tx_hash, tx).map_err(|e| match e {
                MempoolError::DuplicateHash => {
                    BackendError::Internal("duplicate transaction".to_string())
                }
                MempoolError::DuplicateSenderNonce => {
                    BackendError::Internal("duplicate sender nonce".to_string())
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

            // Drain transactions from mempool (insertion order).
            let mut pool = mempool.write().await;
            let txs = pool.drain_for_block(MAX_TX_PER_BLOCK);
            drop(pool);

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
            let block_hash =
                backend.apply_block(&block).map_err(|e| BackendError::Internal(e.to_string()))?;

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
        Account, Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionType,
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
}
