//! In-memory storage backend backed by `HashMap`.
//!
//! Suitable for testing and short-lived node instances.

use std::collections::HashMap;
use std::sync::RwLock;

use parity_scale_codec::{Decode, Encode};

use mbongo_core::{Account, Address, Block, Hash, Transaction};

use crate::storage::{BatchOp, Storage, StorageError};

/// In-memory storage that keeps all data in a `HashMap<Vec<u8>, Vec<u8>>`.
///
/// Thread-safe via interior `RwLock`.
pub struct InMemoryStorage {
    accounts: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    blocks: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    transactions: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    /// Maps height (big-endian u64 bytes) → block hash (32 bytes).
    height_index: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    /// Maps tx sequence number (big-endian u64 bytes) → tx hash (32 bytes).
    tx_seq_index: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    /// Stores metadata values under fixed keys.
    meta: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

impl InMemoryStorage {
    /// Creates a new empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: RwLock::new(HashMap::new()),
            blocks: RwLock::new(HashMap::new()),
            transactions: RwLock::new(HashMap::new()),
            height_index: RwLock::new(HashMap::new()),
            tx_seq_index: RwLock::new(HashMap::new()),
            meta: RwLock::new(HashMap::new()),
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
                let block =
                    Block::decode(&mut &bytes[..]).map_err(|_| StorageError::Serialization)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    fn put_block(&self, hash: &Hash, block: &Block) -> Result<(), StorageError> {
        let mut map = self.blocks.write().map_err(|_| StorageError::Database)?;
        map.insert(hash.0.to_vec(), block.encode());
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

    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        let idx = self.height_index.read().map_err(|_| StorageError::Database)?;
        let key = height.to_be_bytes().to_vec();
        let hash_bytes = match idx.get(&key) {
            Some(b) => b.clone(),
            None => return Ok(None),
        };
        drop(idx);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&hash_bytes);
        let hash = Hash(arr);
        self.get_block(&hash)
    }

    fn put_block_height_index(&self, height: u64, hash: Hash) -> Result<(), StorageError> {
        let mut idx = self.height_index.write().map_err(|_| StorageError::Database)?;
        idx.insert(height.to_be_bytes().to_vec(), hash.0.to_vec());
        drop(idx);

        // Update latest height if this height is greater.
        let mut meta = self.meta.write().map_err(|_| StorageError::Database)?;
        let current = meta
            .get(b"latest_height".as_ref())
            .map(|b| {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(b);
                u64::from_be_bytes(arr)
            })
            .unwrap_or(0);
        if height > current {
            meta.insert(b"latest_height".to_vec(), height.to_be_bytes().to_vec());
        }
        Ok(())
    }

    fn get_latest_height(&self) -> Result<u64, StorageError> {
        let meta = self.meta.read().map_err(|_| StorageError::Database)?;
        match meta.get(b"latest_height".as_ref()) {
            Some(b) => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(b);
                Ok(u64::from_be_bytes(arr))
            }
            None => Ok(0),
        }
    }

    fn next_tx_seq(&self) -> Result<u64, StorageError> {
        let mut meta = self.meta.write().map_err(|_| StorageError::Database)?;
        let current = meta
            .get(b"tx_seq".as_ref())
            .map(|b| {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(b);
                u64::from_be_bytes(arr)
            })
            .unwrap_or(0);
        let next = current + 1;
        meta.insert(b"tx_seq".to_vec(), next.to_be_bytes().to_vec());
        Ok(next)
    }

    fn put_tx_seq_index(&self, seq: u64, hash: &Hash) -> Result<(), StorageError> {
        let mut idx = self.tx_seq_index.write().map_err(|_| StorageError::Database)?;
        idx.insert(seq.to_be_bytes().to_vec(), hash.0.to_vec());
        Ok(())
    }

    fn get_tx_hash_by_seq(&self, seq: u64) -> Result<Option<Hash>, StorageError> {
        let idx = self.tx_seq_index.read().map_err(|_| StorageError::Database)?;
        match idx.get(&seq.to_be_bytes().to_vec()) {
            Some(b) => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(b);
                Ok(Some(Hash(arr)))
            }
            None => Ok(None),
        }
    }

    fn get_last_included_tx_seq(&self) -> Result<u64, StorageError> {
        let meta = self.meta.read().map_err(|_| StorageError::Database)?;
        match meta.get(b"last_included_tx_seq".as_ref()) {
            Some(b) => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(b);
                Ok(u64::from_be_bytes(arr))
            }
            None => Ok(0),
        }
    }

    fn set_last_included_tx_seq(&self, seq: u64) -> Result<(), StorageError> {
        let mut meta = self.meta.write().map_err(|_| StorageError::Database)?;
        meta.insert(b"last_included_tx_seq".to_vec(), seq.to_be_bytes().to_vec());
        Ok(())
    }

    fn write_batch(&self, ops: Vec<BatchOp>) -> Result<(), StorageError> {
        // Acquire all locks up front to guarantee atomicity.
        let mut accounts = self.accounts.write().map_err(|_| StorageError::Database)?;
        let mut blocks = self.blocks.write().map_err(|_| StorageError::Database)?;
        let mut transactions = self.transactions.write().map_err(|_| StorageError::Database)?;
        let mut height_index = self.height_index.write().map_err(|_| StorageError::Database)?;
        let mut tx_seq_index = self.tx_seq_index.write().map_err(|_| StorageError::Database)?;
        let mut meta = self.meta.write().map_err(|_| StorageError::Database)?;

        let mut max_height: Option<u64> = None;

        for op in ops {
            match op {
                BatchOp::PutAccount(address, account) => {
                    accounts.insert(address.0.to_vec(), account.encode());
                }
                BatchOp::PutBlock(hash, block) => {
                    blocks.insert(hash.0.to_vec(), block.encode());
                }
                BatchOp::PutTransaction(hash, tx) => {
                    transactions.insert(hash.0.to_vec(), tx.encode());
                }
                BatchOp::PutBlockHeightIndex(height, hash) => {
                    height_index.insert(height.to_be_bytes().to_vec(), hash.0.to_vec());
                    max_height = Some(match max_height {
                        Some(h) if h >= height => h,
                        _ => height,
                    });
                }
                BatchOp::PutTxSeqIndex(seq, hash) => {
                    tx_seq_index.insert(seq.to_be_bytes().to_vec(), hash.0.to_vec());
                }
                BatchOp::SetTxSeq(seq) => {
                    meta.insert(b"tx_seq".to_vec(), seq.to_be_bytes().to_vec());
                }
                BatchOp::SetLastIncludedTxSeq(seq) => {
                    meta.insert(b"last_included_tx_seq".to_vec(), seq.to_be_bytes().to_vec());
                }
            }
        }

        // Update latest_height if any height index entries were written.
        if let Some(height) = max_height {
            let current = meta
                .get(b"latest_height".as_ref())
                .map(|b| {
                    let mut arr = [0u8; 8];
                    arr.copy_from_slice(b);
                    u64::from_be_bytes(arr)
                })
                .unwrap_or(0);
            if height > current {
                meta.insert(b"latest_height".to_vec(), height.to_be_bytes().to_vec());
            }
        }

        Ok(())
    }
}
