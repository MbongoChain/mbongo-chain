//! RocksDB-backed persistent storage for Mbongo Chain.

use std::path::Path;

use parity_scale_codec::{Decode, Encode};
use rocksdb::{ColumnFamilyDescriptor, Options, WriteBatchWithTransaction, DB};

use mbongo_core::{Account, Address, Block, Hash, Transaction};

use crate::storage::{BatchOp, Storage, StorageError};

/// Column family name for account state.
const CF_ACCOUNTS: &str = "accounts";
/// Column family name for blocks.
const CF_BLOCKS: &str = "blocks";
/// Column family name for transactions.
const CF_TRANSACTIONS: &str = "transactions";
/// Column family name for metadata (latest height, etc.).
const CF_META: &str = "meta";
/// Column family name for height → block-hash index.
const CF_HEIGHT_INDEX: &str = "height_index";
/// Column family name for tx sequence → tx hash index.
const CF_TX_SEQ_INDEX: &str = "tx_seq_index";

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
            ColumnFamilyDescriptor::new(CF_TRANSACTIONS, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_META, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_HEIGHT_INDEX, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_TX_SEQ_INDEX, cf_opts),
        ];

        let db =
            DB::open_cf_descriptors(&db_opts, path, cfs).map_err(|_| StorageError::Database)?;

        Ok(Self { db })
    }
}

impl Storage for RocksDbStorage {
    fn get_account(&self, address: &Address) -> Result<Option<Account>, StorageError> {
        let cf = self.db.cf_handle(CF_ACCOUNTS).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, address.0).map_err(|_| StorageError::Database)? {
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
            .put_cf(&cf, address.0, account.encode())
            .map_err(|_| StorageError::Database)
    }

    fn get_block(&self, hash: &Hash) -> Result<Option<Block>, StorageError> {
        let cf = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, hash.0).map_err(|_| StorageError::Database)? {
            Some(bytes) => {
                let block =
                    Block::decode(&mut &bytes[..]).map_err(|_| StorageError::Serialization)?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    fn put_block(&self, hash: &Hash, block: &Block) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        self.db.put_cf(&cf, hash.0, block.encode()).map_err(|_| StorageError::Database)
    }

    fn get_transaction(&self, hash: &Hash) -> Result<Option<Transaction>, StorageError> {
        let cf = self.db.cf_handle(CF_TRANSACTIONS).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, hash.0).map_err(|_| StorageError::Database)? {
            Some(bytes) => {
                let tx = Transaction::decode(&mut &bytes[..])
                    .map_err(|_| StorageError::Serialization)?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    fn put_transaction(&self, hash: &Hash, tx: &Transaction) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(CF_TRANSACTIONS).ok_or(StorageError::Database)?;
        self.db.put_cf(&cf, hash.0, tx.encode()).map_err(|_| StorageError::Database)
    }

    fn get_block_by_height(&self, height: u64) -> Result<Option<Block>, StorageError> {
        let cf = self.db.cf_handle(CF_HEIGHT_INDEX).ok_or(StorageError::Database)?;
        let key = height.to_be_bytes();
        let hash_bytes = match self.db.get_cf(&cf, key).map_err(|_| StorageError::Database)? {
            Some(b) => b,
            None => return Ok(None),
        };
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&hash_bytes);
        let hash = Hash(arr);
        self.get_block(&hash)
    }

    fn put_block_height_index(&self, height: u64, hash: Hash) -> Result<(), StorageError> {
        let cf_idx = self.db.cf_handle(CF_HEIGHT_INDEX).ok_or(StorageError::Database)?;
        self.db
            .put_cf(&cf_idx, height.to_be_bytes(), hash.0)
            .map_err(|_| StorageError::Database)?;

        // Update latest height if this height is greater.
        let cf_meta = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        let current = self
            .db
            .get_cf(&cf_meta, b"latest_height")
            .map_err(|_| StorageError::Database)?
            .map(|b| {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&b);
                u64::from_be_bytes(arr)
            })
            .unwrap_or(0);
        if height > current {
            self.db
                .put_cf(&cf_meta, b"latest_height", height.to_be_bytes())
                .map_err(|_| StorageError::Database)?;
        }
        Ok(())
    }

    fn get_latest_height(&self) -> Result<u64, StorageError> {
        let cf = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, b"latest_height").map_err(|_| StorageError::Database)? {
            Some(b) => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&b);
                Ok(u64::from_be_bytes(arr))
            }
            None => Ok(0),
        }
    }

    fn next_tx_seq(&self) -> Result<u64, StorageError> {
        let cf = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        let current = self
            .db
            .get_cf(&cf, b"tx_seq")
            .map_err(|_| StorageError::Database)?
            .map(|b| {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&b);
                u64::from_be_bytes(arr)
            })
            .unwrap_or(0);
        let next = current + 1;
        self.db
            .put_cf(&cf, b"tx_seq", next.to_be_bytes())
            .map_err(|_| StorageError::Database)?;
        Ok(next)
    }

    fn put_tx_seq_index(&self, seq: u64, hash: &Hash) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(CF_TX_SEQ_INDEX).ok_or(StorageError::Database)?;
        self.db
            .put_cf(&cf, seq.to_be_bytes(), hash.0)
            .map_err(|_| StorageError::Database)
    }

    fn get_tx_hash_by_seq(&self, seq: u64) -> Result<Option<Hash>, StorageError> {
        let cf = self.db.cf_handle(CF_TX_SEQ_INDEX).ok_or(StorageError::Database)?;
        match self.db.get_cf(&cf, seq.to_be_bytes()).map_err(|_| StorageError::Database)? {
            Some(b) => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&b);
                Ok(Some(Hash(arr)))
            }
            None => Ok(None),
        }
    }

    fn get_last_included_tx_seq(&self) -> Result<u64, StorageError> {
        let cf = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        match self
            .db
            .get_cf(&cf, b"last_included_tx_seq")
            .map_err(|_| StorageError::Database)?
        {
            Some(b) => {
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&b);
                Ok(u64::from_be_bytes(arr))
            }
            None => Ok(0),
        }
    }

    fn set_last_included_tx_seq(&self, seq: u64) -> Result<(), StorageError> {
        let cf = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        self.db
            .put_cf(&cf, b"last_included_tx_seq", seq.to_be_bytes())
            .map_err(|_| StorageError::Database)
    }

    fn write_batch(&self, ops: Vec<BatchOp>) -> Result<(), StorageError> {
        let cf_accounts = self.db.cf_handle(CF_ACCOUNTS).ok_or(StorageError::Database)?;
        let cf_blocks = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        let cf_transactions = self.db.cf_handle(CF_TRANSACTIONS).ok_or(StorageError::Database)?;
        let cf_meta = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        let cf_height_index = self.db.cf_handle(CF_HEIGHT_INDEX).ok_or(StorageError::Database)?;
        let cf_tx_seq_index = self.db.cf_handle(CF_TX_SEQ_INDEX).ok_or(StorageError::Database)?;

        let mut batch = WriteBatchWithTransaction::<false>::default();

        // Track the max height written so we can update latest_height once.
        let mut max_height: Option<u64> = None;

        for op in ops {
            match op {
                BatchOp::PutAccount(address, account) => {
                    batch.put_cf(&cf_accounts, address.0, account.encode());
                }
                BatchOp::PutBlock(hash, block) => {
                    batch.put_cf(&cf_blocks, hash.0, block.encode());
                }
                BatchOp::PutTransaction(hash, tx) => {
                    batch.put_cf(&cf_transactions, hash.0, tx.encode());
                }
                BatchOp::PutBlockHeightIndex(height, hash) => {
                    batch.put_cf(&cf_height_index, height.to_be_bytes(), hash.0);
                    max_height = Some(match max_height {
                        Some(h) if h >= height => h,
                        _ => height,
                    });
                }
                BatchOp::PutTxSeqIndex(seq, hash) => {
                    batch.put_cf(&cf_tx_seq_index, seq.to_be_bytes(), hash.0);
                }
                BatchOp::SetTxSeq(seq) => {
                    batch.put_cf(&cf_meta, b"tx_seq", seq.to_be_bytes());
                }
                BatchOp::SetLastIncludedTxSeq(seq) => {
                    batch.put_cf(&cf_meta, b"last_included_tx_seq", seq.to_be_bytes());
                }
            }
        }

        // Update latest_height if any height index entries were written.
        if let Some(height) = max_height {
            let current = self
                .db
                .get_cf(&cf_meta, b"latest_height")
                .map_err(|_| StorageError::Database)?
                .map(|b| {
                    let mut arr = [0u8; 8];
                    arr.copy_from_slice(&b);
                    u64::from_be_bytes(arr)
                })
                .unwrap_or(0);
            if height > current {
                batch.put_cf(&cf_meta, b"latest_height", height.to_be_bytes());
            }
        }

        self.db.write(batch).map_err(|_| StorageError::Database)
    }
}
