//! Storage trait and error types for Mbongo Chain persistence.

use mbongo_core::{Account, Address, Block, Hash, Transaction};

/// Errors returned by storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// An error occurred in the underlying database engine.
    #[error("Database error")]
    Database,
    /// Serialization or deserialization failed.
    #[error("Serialization error")]
    Serialization,
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

    /// Apply a list of operations atomically.
    ///
    /// All operations succeed or all fail — no partial state is visible.
    /// In RocksDB this uses a `WriteBatch`; in-memory implementations
    /// acquire all locks and apply sequentially.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database or serialization failure.
    fn write_batch(&self, ops: Vec<BatchOp>) -> Result<(), StorageError>;
}
