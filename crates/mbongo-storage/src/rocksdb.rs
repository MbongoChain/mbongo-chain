//! RocksDB-backed persistent storage for Mbongo Chain.

use std::path::Path;

use parity_scale_codec::{Decode, Encode};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};

use mbongo_core::{Account, Address, Block, Hash, Transaction};

use crate::storage::{Storage, StorageError};

/// Column family name for account state.
const CF_ACCOUNTS: &str = "accounts";
/// Column family name for blocks.
const CF_BLOCKS: &str = "blocks";
/// Column family name for transactions.
const CF_TRANSACTIONS: &str = "transactions";

/// Persistent storage backed by RocksDB with three column families:
/// `accounts`, `blocks`, and `transactions`.
pub struct RocksDbStorage {
    db: DB,
}

impl RocksDbStorage {
    /// Opens (or creates) a RocksDB database at the given path.
    ///
    /// Column families `accounts`, `blocks`, and `transactions` are created
    /// automatically if they do not already exist.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Database`] if the database cannot be opened.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let cf_opts = Options::default();

        let cfs = vec![
            ColumnFamilyDescriptor::new(CF_ACCOUNTS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_BLOCKS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_TRANSACTIONS, cf_opts),
        ];

        let db = DB::open_cf_descriptors(&db_opts, path, cfs).map_err(|_| StorageError::Database)?;

        Ok(Self { db })
    }
}

impl Storage for RocksDbStorage {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        let cf = self.db.cf_handle(CF_ACCOUNTS).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, &address.0).map_err(|_| StorageError::Database)? {
            Some(bytes) => {
                let account =
                    Account::decode(&mut &bytes[..]).map_err(|_| StorageError::Serialization)?;
                Ok(Some(account))
            }
            None => Ok(None),
        }
    }

    fn put_account(&self, address: &Address, account: &Account) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(CF_ACCOUNTS).ok_or(StorageError::Database)?;
        self.db
            .put_cf(&cf, &address.0, account.encode())
            .map_err(|_| StorageError::Database)
    }

    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError> {
        let cf = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, &hash.0).map_err(|_| StorageError::Database)? {
            Some(bytes) => {
                let block: Block =
                    serde_json::from_slice(&bytes).map_err(|_| StorageError::Serialization)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    fn put_block(&self, hash: &Hash, block: &Block) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        let bytes = serde_json::to_vec(block).map_err(|_| StorageError::Serialization)?;
        self.db
            .put_cf(&cf, &hash.0, bytes)
            .map_err(|_| StorageError::Database)
    }

    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError> {
        let cf = self
            .db
            .cf_handle(CF_TRANSACTIONS)
            .ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, &hash.0).map_err(|_| StorageError::Database)? {
            Some(bytes) => {
                let tx = Transaction::decode(&mut &bytes[..])
                    .map_err(|_| StorageError::Serialization)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    fn put_transaction(&self, hash: &Hash, tx: &Transaction) -> Result<(), StorageError> {
        let cf = self
            .db
            .cf_handle(CF_TRANSACTIONS)
            .ok_or(StorageError::Database)?;
        self.db
            .put_cf(&cf, &hash.0, tx.encode())
            .map_err(|_| StorageError::Database)
    }
}
