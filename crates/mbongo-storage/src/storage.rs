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
}
