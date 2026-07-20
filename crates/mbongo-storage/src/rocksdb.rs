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
/// Column family name for anchored receipts (task id → opaque bytes).
const CF_RECEIPTS: &str = "receipts";

/// Meta key holding the on-disk schema version (`u32`, big-endian).
const SCHEMA_VERSION_KEY: &[u8] = b"schema_version";
/// Current on-disk schema version. Version 1 (no `schema_version` key,
/// no `receipts` column family) is the v0.2 layout.
///
/// Crate-private: no production consumer outside this crate needs it.
pub(crate) const SCHEMA_VERSION_CURRENT: u32 = 2;

/// Column families a v0.2 (schema v1) database is required to contain.
const REQUIRED_V1_CFS: [&str; 6] = [
    CF_ACCOUNTS,
    CF_BLOCKS,
    CF_TRANSACTIONS,
    CF_META,
    CF_HEIGHT_INDEX,
    CF_TX_SEQ_INDEX,
];

/// All column families known to this binary (schema v2).
const KNOWN_CFS: [&str; 8] = [
    "default",
    CF_ACCOUNTS,
    CF_BLOCKS,
    CF_TRANSACTIONS,
    CF_META,
    CF_HEIGHT_INDEX,
    CF_TX_SEQ_INDEX,
    CF_RECEIPTS,
];

/// Persistent storage backed by RocksDB.
///
/// Schema v2 column families: `accounts`, `blocks`, `transactions`, `meta`,
/// `height_index`, `tx_seq_index`, and `receipts`.
pub struct RocksDbStorage {
    db: DB,
}

impl RocksDbStorage {
    /// Opens a RocksDB database at the given path, following the normative
    /// open/migration sequence of RFC 0002 §5:
    ///
    /// 1. List existing column families.
    /// 2. Reject any column family not known to this binary.
    /// 3. Open exactly the listed column families.
    /// 4. Reject `schema_version` greater than [`SCHEMA_VERSION_CURRENT`].
    /// 5. v1 → v2 migration: create the `receipts` column family if absent.
    /// 6. Stamp `schema_version = 2` only after step 5 succeeded.
    ///
    /// A fresh directory is initialized with all known column families and
    /// stamped [`SCHEMA_VERSION_CURRENT`] immediately. The migration is
    /// idempotent: a crash between steps 5 and 6 is recovered on next open
    /// (creation is skipped, the stamp is applied).
    ///
    /// Note: opening an existing v0.2 (schema v1) directory performs the
    /// v1 → v2 migration as a side effect and crosses the RFC 0002
    /// downgrade boundary — the directory can no longer be opened by v0.2
    /// binaries. Rollback requires wiping the data directory.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Schema`] for unknown column families, a
    /// missing required column family, or an unsupported schema version;
    /// [`StorageError::Database`] on engine failure.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let cf_opts = Options::default();

        // Step 1: list existing column families. Failure means no database
        // exists at this path → fresh initialization.
        let Ok(existing) = DB::list_cf(&Options::default(), path.as_ref()) else {
            return Self::create_fresh(path.as_ref(), &cf_opts);
        };

        // Step 2: reject unknown column families before touching the
        // database read-write.
        if let Some(unknown) = existing.iter().find(|cf| !KNOWN_CFS.contains(&cf.as_str())) {
            return Err(StorageError::Schema(format!(
                "unknown column family '{unknown}': database was written by a newer or \
                 foreign binary"
            )));
        }
        // A valid v1 (v0.2) database always contains all six original
        // column families; a missing one is corruption, not a migration.
        if let Some(missing) = REQUIRED_V1_CFS.iter().find(|cf| !existing.iter().any(|e| e == *cf))
        {
            return Err(StorageError::Schema(format!(
                "missing required column family '{missing}'"
            )));
        }

        // Step 3: open exactly the listed column families. No
        // create_missing_column_families — creation happens only through
        // the explicit migration below or fresh initialization.
        let db_opts = Options::default();
        let cfs: Vec<ColumnFamilyDescriptor> = existing
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(name.clone(), cf_opts.clone()))
            .collect();
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), cfs)
            .map_err(|_| StorageError::Database)?;

        // Step 4: reject newer schemas before any modification.
        let version = read_schema_version(&db)?;
        if version > SCHEMA_VERSION_CURRENT {
            return Err(StorageError::Schema(format!(
                "schema version {version} is newer than supported version \
                 {SCHEMA_VERSION_CURRENT}"
            )));
        }

        // Step 5: v1 → v2 migration — create the receipts column family if
        // absent. Also covers the self-heal case (v2 stamp present but CF
        // missing) permitted by RFC 0002 §5, since the CF is derived state.
        let mut db = db;
        if db.cf_handle(CF_RECEIPTS).is_none() {
            db.create_cf(CF_RECEIPTS, &cf_opts).map_err(|_| StorageError::Database)?;
        }

        // Step 6: stamp the schema version only after the column family
        // exists. Idempotent recovery: if a previous open crashed between
        // steps 5 and 6, creation is skipped above and the stamp lands here.
        if version < SCHEMA_VERSION_CURRENT {
            write_schema_version(&db, SCHEMA_VERSION_CURRENT)?;
        }

        Ok(Self { db })
    }

    /// Initializes a fresh database with all known column families and
    /// stamps the current schema version.
    fn create_fresh(path: &Path, cf_opts: &Options) -> Result<Self, StorageError> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let cfs: Vec<ColumnFamilyDescriptor> = KNOWN_CFS
            .iter()
            .filter(|name| **name != "default")
            .map(|name| ColumnFamilyDescriptor::new(*name, cf_opts.clone()))
            .collect();
        let db =
            DB::open_cf_descriptors(&db_opts, path, cfs).map_err(|_| StorageError::Database)?;
        write_schema_version(&db, SCHEMA_VERSION_CURRENT)?;
        Ok(Self { db })
    }

    /// Returns the on-disk schema version.
    ///
    /// Test-only: exercised by the schema/migration tests; not part of
    /// the production API.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure or a malformed version
    /// value.
    #[cfg(test)]
    pub(crate) fn schema_version(&self) -> Result<u32, StorageError> {
        read_schema_version(&self.db)
    }
}

/// Reads `schema_version` from the meta column family. Absent means 1
/// (the v0.2 layout, which predates the key).
fn read_schema_version(db: &DB) -> Result<u32, StorageError> {
    let cf = db.cf_handle(CF_META).ok_or(StorageError::Database)?;
    match db.get_cf(&cf, SCHEMA_VERSION_KEY).map_err(|_| StorageError::Database)? {
        Some(bytes) => {
            let arr: [u8; 4] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::Schema("malformed schema_version value".to_string()))?;
            Ok(u32::from_be_bytes(arr))
        }
        None => Ok(1),
    }
}

/// Writes `schema_version` to the meta column family.
fn write_schema_version(db: &DB, version: u32) -> Result<(), StorageError> {
    let cf = db.cf_handle(CF_META).ok_or(StorageError::Database)?;
    db.put_cf(&cf, SCHEMA_VERSION_KEY, version.to_be_bytes())
        .map_err(|_| StorageError::Database)
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

    fn has_receipt(&self, task_id: &[u8; 32]) -> Result<bool, StorageError> {
        let cf = self.db.cf_handle(CF_RECEIPTS).ok_or(StorageError::Database)?;
        Ok(self
            .db
            .get_pinned_cf(&cf, task_id)
            .map_err(|_| StorageError::Database)?
            .is_some())
    }

    fn get_receipt(&self, task_id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError> {
        let cf = self.db.cf_handle(CF_RECEIPTS).ok_or(StorageError::Database)?;
        self.db.get_cf(&cf, task_id).map_err(|_| StorageError::Database)
    }

    fn write_batch(&self, ops: Vec<BatchOp>) -> Result<(), StorageError> {
        let cf_accounts = self.db.cf_handle(CF_ACCOUNTS).ok_or(StorageError::Database)?;
        let cf_blocks = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        let cf_transactions = self.db.cf_handle(CF_TRANSACTIONS).ok_or(StorageError::Database)?;
        let cf_meta = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        let cf_height_index = self.db.cf_handle(CF_HEIGHT_INDEX).ok_or(StorageError::Database)?;
        let cf_tx_seq_index = self.db.cf_handle(CF_TX_SEQ_INDEX).ok_or(StorageError::Database)?;
        let cf_receipts = self.db.cf_handle(CF_RECEIPTS).ok_or(StorageError::Database)?;

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
                BatchOp::PutReceipt(task_id, bytes) => {
                    batch.put_cf(&cf_receipts, task_id, bytes);
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
