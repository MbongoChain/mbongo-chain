//! Storage trait and error types for Mbongo Chain persistence.

use mbongo_core::{Account, Address, Block, ChainId, Hash, Transaction};

/// Errors returned by storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// An error occurred in the underlying database engine.
    #[error("Database error")]
    Database,
    /// Serialization or deserialization failed.
    #[error("Serialization error")]
    Serialization,
    /// The on-disk schema is unknown or unsupported (RFC 0002 §5).
    #[error("Schema error: {0}")]
    Schema(String),
}

/// A single atomic operation within a [`Storage::write_batch`] call.
pub enum BatchOp {
    /// Persist an account keyed by address.
    PutAccount(Address, Account),
    /// Persist a block keyed by its hash.
    PutBlock(Hash, Block),
    /// Persist a transaction keyed by its hash.
    PutTransaction(Hash, Transaction),
    /// Store a mapping from block height to block hash and update latest height.
    PutBlockHeightIndex(u64, Hash),
    /// Store a mapping from sequence number to transaction hash.
    PutTxSeqIndex(u64, Hash),
    /// Set the transaction sequence counter to a specific value.
    SetTxSeq(u64),
    /// Set the last transaction sequence number included in a block.
    SetLastIncludedTxSeq(u64),
    /// Persist an anchored receipt keyed by its 32-byte task id.
    /// The value bytes are opaque to the storage layer (RFC 0002 §4).
    PutReceipt([u8; 32], Vec<u8>),
    /// Persist a committed compute task keyed by its 32-byte task id.
    /// The value bytes are opaque to the storage layer (RFC 0005 §4).
    PutTask([u8; 32], Vec<u8>),
    /// Record the chain this store belongs to: its `chain_id` and genesis
    /// hash, both together (RFC 0006, storage schema 4). Written once, in the
    /// genesis batch.
    PutChainIdentity(ChainIdentity),
}

/// The chain a store belongs to (RFC 0006, storage schema 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainIdentity {
    /// `BLAKE3("mbongo:chain-id:v1" || genesis file bytes)`.
    pub chain_id: ChainId,
    /// The hash of block 0, the genesis block that the genesis file derives.
    pub genesis_hash: Hash,
}

/// `meta` key of the chain id: 32 raw bytes (storage schema 4).
pub(crate) const CHAIN_ID_KEY: &[u8] = b"chain_id";
/// `meta` key of the genesis hash: 32 raw bytes (storage schema 4).
pub(crate) const GENESIS_HASH_KEY: &[u8] = b"genesis_hash";

/// The identity stored under [`CHAIN_ID_KEY`] and [`GENESIS_HASH_KEY`]: both
/// absent is none; both present with 32 bytes each is one; anything else is
/// refused.
pub(crate) fn identity_from(
    chain_id: Option<&[u8]>,
    genesis_hash: Option<&[u8]>,
) -> Result<Option<ChainIdentity>, StorageError> {
    match (chain_id, genesis_hash) {
        (None, None) => Ok(None),
        (Some(id), Some(hash)) => {
            let (Ok(id), Ok(hash)) = (<[u8; 32]>::try_from(id), <[u8; 32]>::try_from(hash)) else {
                return Err(StorageError::Schema(
                    "the stored chain identity is malformed: a value is not 32 bytes".to_string(),
                ));
            };
            Ok(Some(ChainIdentity {
                chain_id: ChainId(id),
                genesis_hash: Hash(hash),
            }))
        }
        _ => Err(StorageError::Schema(
            "the stored chain identity is partial: only one of chain_id and genesis_hash"
                .to_string(),
        )),
    }
}

/// Refuses a [`BatchOp::PutChainIdentity`] of `new` when `stored` is another
/// identity.
pub(crate) fn check_identity_write(
    stored: Option<ChainIdentity>,
    new: &ChainIdentity,
) -> Result<(), StorageError> {
    match stored {
        Some(stored) if stored != *new => Err(StorageError::Schema(format!(
            "this store belongs to chain {} (genesis {}); refusing to record chain {}",
            stored.chain_id,
            hex_of(&stored.genesis_hash),
            new.chain_id
        ))),
        _ => Ok(()),
    }
}

pub(crate) fn hex_of(hash: &Hash) -> String {
    use std::fmt::Write;
    hash.0.iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// How durable a [`Storage::write_batch_with`] must be when it returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    /// Written to the database; it may still be lost on a crash of the
    /// machine (in RocksDB, the write-ahead log is not synced). A crash of
    /// the process alone loses nothing.
    Buffered,
    /// On stable storage before the call returns (in RocksDB,
    /// `WriteOptions::set_sync(true)`). For writes that must not be lost once
    /// anyone has seen their effect: the genesis, and a block the authority
    /// produced, before it is broadcast (RFC 0006 §3).
    Synced,
}

/// Domain-oriented storage interface for Mbongo Chain state.
///
/// All implementations must serialize values before persisting and
/// deserialize them on retrieval. Keys are derived from the domain
/// identifiers ([`Address`] for accounts, [`Hash`] for blocks and
/// transactions).
pub trait Storage {
    /// Retrieve an account by address.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or deserialization failure.
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError>;

    /// Persist an account keyed by address.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure.
    fn put_account(&self, address: &Address, account: &Account) -> Result<(), StorageError>;

    /// Retrieve a block by its hash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or deserialization failure.
    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError>;

    /// Persist a block keyed by its hash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure.
    fn put_block(&self, hash: &Hash, block: &Block) -> Result<(), StorageError>;

    /// Retrieve a transaction by its hash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or deserialization failure.
    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError>;

    /// Persist a transaction keyed by its hash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure.
    fn put_transaction(&self, hash: &Hash, tx: &Transaction) -> Result<(), StorageError>;

    /// Retrieve a block by its height via the height→hash index.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or deserialization failure.
    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError>;

    /// Store a mapping from block height to block hash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure.
    fn put_block_height_index(&self, height: u64, hash: Hash) -> Result<(), StorageError>;

    /// Return the latest persisted block height, or 0 if no blocks exist.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn get_latest_height(&self) -> Result<u64, StorageError>;

    /// Atomically increment the transaction sequence counter and return the new value.
    ///
    /// The first call returns 1.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn next_tx_seq(&self) -> Result<u64, StorageError>;

    /// Store a mapping from sequence number to transaction hash.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn put_tx_seq_index(&self, seq: u64, hash: &Hash) -> Result<(), StorageError>;

    /// Retrieve a transaction hash by its sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn get_tx_hash_by_seq(&self, seq: u64) -> Result<Option<Hash>, StorageError>;

    /// Return the last transaction sequence number included in a block, or 0 if none.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn get_last_included_tx_seq(&self) -> Result<u64, StorageError>;

    /// Set the last transaction sequence number included in a block.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn set_last_included_tx_seq(&self, seq: u64) -> Result<(), StorageError>;

    /// Return whether a receipt with the given task id has been anchored.
    ///
    /// The storage layer never decodes or validates receipt bytes; this is
    /// a pure key-existence check (RFC 0002 §4–5).
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn has_receipt(&self, task_id: &[u8; 32]) -> Result<bool, StorageError>;

    /// Retrieve the opaque receipt bytes stored under the given task id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn get_receipt(&self, task_id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError>;

    /// Return whether a compute task with the given task id has been
    /// committed.
    ///
    /// The storage layer never decodes or validates task bytes; this is a
    /// pure key-existence check (RFC 0005 §4).
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn has_task(&self, task_id: &[u8; 32]) -> Result<bool, StorageError>;

    /// Retrieve the opaque task bytes stored under the given task id.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure.
    fn get_task(&self, task_id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError>;

    /// Apply a list of operations atomically, [`Durability::Buffered`].
    ///
    /// All operations succeed or all fail — no partial state is visible.
    /// In RocksDB this uses a `WriteBatch`; in-memory implementations
    /// acquire all locks and apply sequentially.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure.
    fn write_batch(&self, ops: Vec<BatchOp>) -> Result<(), StorageError> {
        self.write_batch_with(ops, Durability::Buffered)
    }

    /// Apply a list of operations atomically, as durable as `durability`
    /// asks: with [`Durability::Synced`] the batch is on stable storage when
    /// this returns `Ok`. Atomicity is the same for both.
    ///
    /// A [`BatchOp::PutChainIdentity`] is refused, and the whole batch with
    /// it, if the store already holds a different identity.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure; on a
    /// failure nothing of the batch is written.
    fn write_batch_with(
        &self,
        ops: Vec<BatchOp>,
        durability: Durability,
    ) -> Result<(), StorageError>;

    /// The chain this store belongs to (RFC 0006, storage schema 4), `None`
    /// before its genesis is written.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Schema`] if only part of an identity is
    /// stored, or a value has the wrong length; [`StorageError::Database`] on
    /// database failure.
    fn chain_identity(&self) -> Result<Option<ChainIdentity>, StorageError>;
}
