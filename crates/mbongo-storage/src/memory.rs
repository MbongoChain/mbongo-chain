//! In-memory storage backend backed by `HashMap`.
//!
//! Suitable for testing and short-lived node instances.

use std::collections::HashMap;
use std::sync::RwLock;

use parity_scale_codec::{Decode, Encode};

use mbongo_core::{Account, Address, Block, Hash, Transaction};

use crate::storage::{Storage, StorageError};

/// In-memory storage that keeps all data in a `HashMap<Vec<u8>, Vec<u8>>`.
///
/// Thread-safe via interior `RwLock`.
pub struct InMemoryStorage {
    accounts: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    blocks: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    transactions: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl InMemoryStorage {
    /// Creates a new empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            blocks: RwLock::new(HashMap::new()),
            transactions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for InMemoryStorage {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        let map = self.accounts.read().map_err(|_| StorageError::Database)?;
        match map.get(&address.0.to_vec()) {
            Some(bytes) => {
                let account =
                    Account::decode(&mut &bytes[..]).map_err(|_| StorageError::Serialization)?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    fn put_account(&self, address: &Address, account: &Account) -> Result<(), StorageError> {
        let mut map = self.accounts.write().map_err(|_| StorageError::Database)?;
        map.insert(address.0.to_vec(), account.encode());
        Ok(())
    }

    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError> {
        let map = self.blocks.read().map_err(|_| StorageError::Database)?;
        match map.get(&hash.0.to_vec()) {
            Some(bytes) => {
                let block: Block =
                    serde_json::from_slice(bytes).map_err(|_| StorageError::Serialization)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    fn put_block(&self, hash: &Hash, block: &Block) -> Result<(), StorageError> {
        let mut map = self.blocks.write().map_err(|_| StorageError::Database)?;
        let bytes = serde_json::to_vec(block).map_err(|_| StorageError::Serialization)?;
        map.insert(hash.0.to_vec(), bytes);
        Ok(())
    }

    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError> {
        let map = self.transactions.read().map_err(|_| StorageError::Database)?;
        match map.get(&hash.0.to_vec()) {
            Some(bytes) => {
                let tx = Transaction::decode(&mut &bytes[..])
                    .map_err(|_| StorageError::Serialization)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    fn put_transaction(&self, hash: &Hash, tx: &Transaction) -> Result<(), StorageError> {
        let mut map = self.transactions.write().map_err(|_| StorageError::Database)?;
        map.insert(hash.0.to_vec(), tx.encode());
        Ok(())
    }
}
