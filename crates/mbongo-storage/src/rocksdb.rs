//! RocksDB-backed persistent storage for Mbongo Chain.
//!
//! # Storage schema 4 (RFC 0006)
//!
//! Column families: `accounts`, `blocks`, `transactions`, `meta`,
//! `height_index`, `tx_seq_index`, `receipts`, `tasks` (the layout of schema
//! 3). `meta` holds:
//!
//! | Key | Value |
//! |---|---|
//! | `schema_version` | `4`, `u32` big-endian |
//! | `chain_id` | the chain id, 32 raw bytes |
//! | `genesis_hash` | the hash of block 0, 32 raw bytes |
//! | `latest_height`, `tx_seq`, `last_included_tx_seq` | as in schema 3 |
//!
//! The two identity keys are written together, in the genesis batch, with
//! block 0 and the allocations, and never again. They are the only record of
//! the chain a directory belongs to.
//!
//! # Opening a directory
//!
//! Nothing is migrated, and no earlier directory is stamped. Every decision is
//! first taken on a **read-only** view of the database; a directory that is
//! refused is never opened for writing, so its keys and column families stay
//! as they were. RocksDB may still write its own info `LOG` files during a
//! read-only open: the guarantee is logical (column families, keys and
//! values), not byte for byte.
//!
//! | Found | Result |
//! |---|---|
//! | no directory, or an empty one | new: created, stamped 4 |
//! | files that are not a database | refused |
//! | an unknown or a missing column family | refused |
//! | no stamp, every column family present and empty | creation interrupted before its stamp: stamped 4, then as below |
//! | stamp 1, 2 or 3, or no stamp with any data | refused |
//! | stamp above 4, or malformed | refused |
//! | stamp 4, no identity, nothing but the stamp | creation interrupted before the genesis: resumed |
//! | stamp 4, no identity, anything else | refused |
//! | stamp 4, partial or malformed identity | refused |
//! | stamp 4, identity not matching its block 0 | refused |
//! | stamp 4, identity of another chain | refused ([`RocksDbStorage::open_chain`]) |
//! | stamp 4, identity of this chain | reopened |
//!
//! After the read-write open, which takes RocksDB's exclusive `LOCK`, the same
//! classification runs again on the locked database, and the open is refused
//! if the directory changed in between: two starts cannot both initialize it.

use std::path::Path;

use parity_scale_codec::{Decode, Encode};
use rocksdb::{
    ColumnFamilyDescriptor, IteratorMode, Options, WriteBatchWithTransaction, WriteOptions, DB,
};

use mbongo_core::{Account, Address, Block, Hash, Transaction};

use crate::storage::{
    check_identity_write, hex_of, identity_from, BatchOp, ChainIdentity, Durability, Storage,
    StorageError, CHAIN_ID_KEY, GENESIS_HASH_KEY,
};

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
/// Column family name for committed compute tasks (task id → opaque
/// bytes), RFC 0005 §4.
const CF_TASKS: &str = "tasks";

/// Meta key holding the on-disk schema version (`u32`, big-endian).
const SCHEMA_VERSION_KEY: &[u8] = b"schema_version";
/// Current on-disk schema version: 4 (RFC 0006). Versions 1 (v0.2, no
/// `schema_version` key), 2 (v0.3) and 3 (v0.4) are refused, never
/// migrated.
///
/// Crate-private: no production consumer outside this crate needs it.
pub(crate) const SCHEMA_VERSION_CURRENT: u32 = 4;

/// The column families of schema 4, `default` included: exactly these.
const KNOWN_CFS: [&str; 9] = [
    "default",
    CF_ACCOUNTS,
    CF_BLOCKS,
    CF_TRANSACTIONS,
    CF_META,
    CF_HEIGHT_INDEX,
    CF_TX_SEQ_INDEX,
    CF_RECEIPTS,
    CF_TASKS,
];

/// How [`RocksDbStorage::open_chain`] found the directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    /// It was new; the database was created and stamped. The genesis is
    /// still to be written.
    Created,
    /// A creation of it was interrupted before its genesis batch: it holds
    /// only the stamp (written now if it was missing). The genesis is still
    /// to be written.
    Resumed,
    /// It holds this chain.
    Reopened,
}

/// A change to make to a directory once, between the read-only decision and
/// the read-write open, standing for another start that wrote it in that
/// window. Tests only.
#[cfg(test)]
type Change = Box<dyn FnOnce(&Path)>;

#[cfg(test)]
thread_local! {
    pub(crate) static BETWEEN_INSPECT_AND_OPEN: std::cell::RefCell<Option<Change>> =
        const { std::cell::RefCell::new(None) };
}

/// What a directory holds, once every refusal has been ruled out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
    /// No directory, or an empty one.
    Nothing,
    /// Every column family, all empty, no stamp.
    Unstamped,
    /// Stamp 4 and nothing else.
    StampedEmpty,
    /// Stamp 4 and a whole identity, consistent with block 0.
    Chain(ChainIdentity),
}

/// Persistent storage backed by RocksDB, storage schema 4.
pub struct RocksDbStorage {
    db: DB,
    /// The options the database was opened with. Kept so that a test store
    /// opened with statistics can read them; nothing else reads them.
    #[cfg_attr(not(test), allow(dead_code))]
    opts: Options,
}

impl RocksDbStorage {
    /// Opens the database at `path`, creating it if the directory is absent
    /// or empty, under the rules of the module documentation. It does not
    /// check which chain the directory holds: a node uses
    /// [`Self::open_chain`].
    ///
    /// # Errors
    ///
    /// [`StorageError::Schema`] for every refusal in the module
    /// documentation; [`StorageError::Database`] on engine failure, including
    /// a directory another process has open.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        Self::open_with(path.as_ref(), Options::default()).map(|(store, _)| store)
    }

    /// Opens the directory of the chain `expected`: new, an interrupted
    /// creation of a store, or `expected` itself. Another chain's directory
    /// is refused before it is opened for writing.
    ///
    /// # Errors
    ///
    /// As [`Self::open`], and [`StorageError::Schema`] if the directory
    /// belongs to another chain.
    pub fn open_chain<P: AsRef<Path>>(
        path: P,
        expected: &ChainIdentity,
    ) -> Result<(Self, Opened), StorageError> {
        let path = path.as_ref();
        if let Found::Chain(stored) = inspect(path)? {
            refuse_other_chain(&stored, expected)?;
        }
        let (store, found) = Self::open_with(path, Options::default())?;
        let opened = match found {
            Found::Nothing => Opened::Created,
            Found::Unstamped | Found::StampedEmpty => Opened::Resumed,
            Found::Chain(stored) => {
                // Checked again under the lock: the directory may have
                // changed since the read-only view.
                refuse_other_chain(&stored, expected)?;
                Opened::Reopened
            }
        };
        Ok((store, opened))
    }

    /// The read-only decision, then the read-write open, then the same
    /// decision under the lock. Returns what was found before anything was
    /// written.
    ///
    /// Tests can change the directory between the first decision and the
    /// lock through [`BETWEEN_INSPECT_AND_OPEN`].
    fn open_with(path: &Path, opts: Options) -> Result<(Self, Found), StorageError> {
        let found = inspect(path)?;
        #[cfg(test)]
        if let Some(change) = BETWEEN_INSPECT_AND_OPEN.with(|h| h.borrow_mut().take()) {
            change(path);
        }
        let store = if found == Found::Nothing {
            Self::create_fresh(path, opts)?
        } else {
            let names: Vec<&str> = KNOWN_CFS.iter().filter(|n| **n != "default").copied().collect();
            let cfs: Vec<ColumnFamilyDescriptor> = names
                .iter()
                .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
                .collect();
            // No create_if_missing, no create_missing_column_families: the
            // layout was checked, and nothing is added to it.
            let db =
                DB::open_cf_descriptors(&opts, path, cfs).map_err(|_| StorageError::Database)?;
            let now = classify(&db)?;
            if now != found {
                return Err(StorageError::Schema(
                    "the data directory changed while it was being opened; refusing it".to_string(),
                ));
            }
            if now == Found::Unstamped {
                write_schema_version(&db, SCHEMA_VERSION_CURRENT)?;
            }
            Self { db, opts }
        };
        Ok((store, found))
    }

    /// Creates a new database: every column family, then the stamp, synced.
    /// Refused if a database appeared at `path` meanwhile.
    fn create_fresh(path: &Path, mut opts: Options) -> Result<Self, StorageError> {
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_error_if_exists(true);
        let cfs: Vec<ColumnFamilyDescriptor> = KNOWN_CFS
            .iter()
            .filter(|name| **name != "default")
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
            .collect();
        let db = DB::open_cf_descriptors(&opts, path, cfs).map_err(|_| {
            StorageError::Schema(
                "cannot create a new database here: another start may be creating it".to_string(),
            )
        })?;
        write_schema_version(&db, SCHEMA_VERSION_CURRENT)?;
        Ok(Self { db, opts })
    }

    /// A store opened with RocksDB statistics, to observe its write-ahead
    /// log syncs ([`Self::wal_syncs`]). Tests only.
    #[cfg(test)]
    pub(crate) fn open_with_statistics(path: &Path) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.enable_statistics();
        Self::open_with(path, opts).map(|(store, _)| store)
    }

    /// How many times RocksDB has synced this store's write-ahead log (the
    /// `rocksdb.wal.synced` ticker). Zero without statistics.
    #[cfg(test)]
    pub(crate) fn wal_syncs(&self) -> u64 {
        self.opts.get_ticker_count(rocksdb::statistics::Ticker::WalFileSynced)
    }

    /// Returns the on-disk schema version.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] on database failure or a malformed version
    /// value.
    #[cfg(test)]
    pub(crate) fn schema_version(&self) -> Result<Option<u32>, StorageError> {
        read_schema_version(&self.db)
    }
}

fn refuse_other_chain(
    stored: &ChainIdentity,
    expected: &ChainIdentity,
) -> Result<(), StorageError> {
    if stored == expected {
        return Ok(());
    }
    Err(StorageError::Schema(format!(
        "the data directory belongs to chain {} (genesis {}), not chain {} (genesis {}); \
         it was not changed",
        stored.chain_id,
        hex_of(&stored.genesis_hash),
        expected.chain_id,
        hex_of(&expected.genesis_hash)
    )))
}

/// The read-only decision. Every refusal is an error; nothing is written.
fn inspect(path: &Path) -> Result<Found, StorageError> {
    match std::fs::read_dir(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Found::Nothing),
        Err(_) => {
            return Err(StorageError::Schema(
                "the data directory cannot be read as a directory".to_string(),
            ))
        }
        Ok(mut entries) => {
            if entries.next().is_none() {
                return Ok(Found::Nothing);
            }
        }
    }
    let Ok(existing) = DB::list_cf(&Options::default(), path) else {
        return Err(StorageError::Schema(
            "the data directory holds files but no database (a creation interrupted before the \
             database existed, or files of another kind); refusing it unchanged"
                .to_string(),
        ));
    };
    if let Some(unknown) = existing.iter().find(|cf| !KNOWN_CFS.contains(&cf.as_str())) {
        return Err(StorageError::Schema(format!(
            "unknown column family '{unknown}': the database was written by another binary"
        )));
    }
    if !existing.iter().any(|cf| cf == CF_META) {
        return Err(StorageError::Schema(
            "missing column family 'meta': not a Mbongo database".to_string(),
        ));
    }
    let db = DB::open_cf_for_read_only(&Options::default(), path, &existing, false)
        .map_err(|_| StorageError::Database)?;
    let missing = KNOWN_CFS.iter().find(|cf| !existing.iter().any(|e| e == *cf));
    match (read_schema_version(&db)?, missing) {
        // Interrupted before the stamp: the layout of schema 4, all empty.
        (None, None) if only_stamp_or_nothing(&db, None)? => Ok(Found::Unstamped),
        (None, _) => Err(StorageError::Schema(
            "storage schema 1 (no schema_version): written by v0.2; schemas below 4 are \
             refused, not migrated. Start from an empty directory; this one was not changed"
                .to_string(),
        )),
        (Some(v), _) if v < SCHEMA_VERSION_CURRENT => Err(StorageError::Schema(format!(
            "storage schema {v}: written by an earlier version; schemas below \
             {SCHEMA_VERSION_CURRENT} are refused, not migrated. Start from an empty directory; \
             this one was not changed"
        ))),
        (Some(v), _) if v > SCHEMA_VERSION_CURRENT => Err(StorageError::Schema(format!(
            "storage schema {v} is newer than supported version {SCHEMA_VERSION_CURRENT}"
        ))),
        (Some(_), Some(cf)) => Err(StorageError::Schema(format!(
            "missing column family '{cf}' in a schema-{SCHEMA_VERSION_CURRENT} database"
        ))),
        (Some(_), None) => classify(&db),
    }
}

/// The decision on an open database of the schema-4 layout.
fn classify(db: &DB) -> Result<Found, StorageError> {
    match read_schema_version(db)? {
        None if only_stamp_or_nothing(db, None)? => Ok(Found::Unstamped),
        Some(SCHEMA_VERSION_CURRENT) => match chain_identity_of(db)? {
            None if only_stamp_or_nothing(db, Some(SCHEMA_VERSION_CURRENT))? => {
                Ok(Found::StampedEmpty)
            }
            None => Err(StorageError::Schema(
                "the database holds chain data but no chain identity; refusing it".to_string(),
            )),
            Some(identity) => {
                check_block_zero(db, &identity)?;
                Ok(Found::Chain(identity))
            }
        },
        _ => Err(StorageError::Schema(
            "the database is not a schema-4 database".to_string(),
        )),
    }
}

/// The stored identity must name the stored block 0: the height index of
/// height 0 is the genesis hash, and that block exists.
fn check_block_zero(db: &DB, identity: &ChainIdentity) -> Result<(), StorageError> {
    let cf_index = db.cf_handle(CF_HEIGHT_INDEX).ok_or(StorageError::Database)?;
    let cf_blocks = db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
    let indexed = db.get_cf(&cf_index, 0u64.to_be_bytes()).map_err(|_| StorageError::Database)?;
    let consistent = indexed.as_deref() == Some(identity.genesis_hash.0.as_slice())
        && db
            .get_cf(&cf_blocks, identity.genesis_hash.0)
            .map_err(|_| StorageError::Database)?
            .is_some();
    if consistent {
        Ok(())
    } else {
        Err(StorageError::Schema(format!(
            "the chain identity names genesis {}, which is not this database's block 0",
            hex_of(&identity.genesis_hash)
        )))
    }
}

/// Whether every column family is empty, except `meta`, which holds the stamp
/// `stamp` and nothing else (or nothing, for `None`).
fn only_stamp_or_nothing(db: &DB, stamp: Option<u32>) -> Result<bool, StorageError> {
    for name in KNOWN_CFS {
        let Some(mut iter) = (if name == "default" {
            Some(db.iterator(IteratorMode::Start))
        } else {
            db.cf_handle(name).map(|cf| db.iterator_cf(&cf, IteratorMode::Start))
        }) else {
            continue;
        };
        if name == CF_META {
            if let Some(version) = stamp {
                match iter.next() {
                    Some(Ok((key, value)))
                        if &*key == SCHEMA_VERSION_KEY && *value == version.to_be_bytes() => {}
                    Some(Err(_)) => return Err(StorageError::Database),
                    _ => return Ok(false),
                }
            }
        }
        match iter.next() {
            None => {}
            Some(Err(_)) => return Err(StorageError::Database),
            Some(Ok(_)) => return Ok(false),
        }
    }
    Ok(true)
}

/// The identity in `meta`, if any.
fn chain_identity_of(db: &DB) -> Result<Option<ChainIdentity>, StorageError> {
    let cf = db.cf_handle(CF_META).ok_or(StorageError::Database)?;
    let chain_id = db.get_cf(&cf, CHAIN_ID_KEY).map_err(|_| StorageError::Database)?;
    let genesis_hash = db.get_cf(&cf, GENESIS_HASH_KEY).map_err(|_| StorageError::Database)?;
    identity_from(chain_id.as_deref(), genesis_hash.as_deref())
}

/// Reads `schema_version` from the meta column family, `None` if absent.
fn read_schema_version(db: &DB) -> Result<Option<u32>, StorageError> {
    let cf = db.cf_handle(CF_META).ok_or(StorageError::Database)?;
    match db.get_cf(&cf, SCHEMA_VERSION_KEY).map_err(|_| StorageError::Database)? {
        Some(bytes) => {
            let arr: [u8; 4] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::Schema("malformed schema_version value".to_string()))?;
            Ok(Some(u32::from_be_bytes(arr)))
        }
        None => Ok(None),
    }
}

/// Writes `schema_version` to the meta column family, synced.
fn write_schema_version(db: &DB, version: u32) -> Result<(), StorageError> {
    let cf = db.cf_handle(CF_META).ok_or(StorageError::Database)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(&cf, SCHEMA_VERSION_KEY, version.to_be_bytes(), &opts)
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

    fn has_task(&self, task_id: &[u8; 32]) -> Result<bool, StorageError> {
        let cf = self.db.cf_handle(CF_TASKS).ok_or(StorageError::Database)?;
        Ok(self
            .db
            .get_pinned_cf(&cf, task_id)
            .map_err(|_| StorageError::Database)?
            .is_some())
    }

    fn get_task(&self, task_id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError> {
        let cf = self.db.cf_handle(CF_TASKS).ok_or(StorageError::Database)?;
        self.db.get_cf(&cf, task_id).map_err(|_| StorageError::Database)
    }

    fn chain_identity(&self) -> Result<Option<ChainIdentity>, StorageError> {
        chain_identity_of(&self.db)
    }

    fn write_batch_with(
        &self,
        ops: Vec<BatchOp>,
        durability: Durability,
    ) -> Result<(), StorageError> {
        // An identity is checked against the stored one before anything is
        // written, so a refused batch writes nothing.
        if ops.iter().any(|op| matches!(op, BatchOp::PutChainIdentity(_))) {
            let stored = chain_identity_of(&self.db)?;
            for op in &ops {
                if let BatchOp::PutChainIdentity(new) = op {
                    check_identity_write(stored, new)?;
                }
            }
        }
        let cf_accounts = self.db.cf_handle(CF_ACCOUNTS).ok_or(StorageError::Database)?;
        let cf_blocks = self.db.cf_handle(CF_BLOCKS).ok_or(StorageError::Database)?;
        let cf_transactions = self.db.cf_handle(CF_TRANSACTIONS).ok_or(StorageError::Database)?;
        let cf_meta = self.db.cf_handle(CF_META).ok_or(StorageError::Database)?;
        let cf_height_index = self.db.cf_handle(CF_HEIGHT_INDEX).ok_or(StorageError::Database)?;
        let cf_tx_seq_index = self.db.cf_handle(CF_TX_SEQ_INDEX).ok_or(StorageError::Database)?;
        let cf_receipts = self.db.cf_handle(CF_RECEIPTS).ok_or(StorageError::Database)?;
        let cf_tasks = self.db.cf_handle(CF_TASKS).ok_or(StorageError::Database)?;

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
                BatchOp::PutTask(task_id, bytes) => {
                    batch.put_cf(&cf_tasks, task_id, bytes);
                }
                BatchOp::PutChainIdentity(identity) => {
                    batch.put_cf(&cf_meta, CHAIN_ID_KEY, identity.chain_id.0);
                    batch.put_cf(&cf_meta, GENESIS_HASH_KEY, identity.genesis_hash.0);
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

        self.db
            .write_opt(batch, &write_options(durability))
            .map_err(|_| StorageError::Database)
    }
}

/// The RocksDB write options of `durability`: `sync` set for
/// [`Durability::Synced`] only.
fn write_options(durability: Durability) -> WriteOptions {
    let mut opts = WriteOptions::default();
    opts.set_sync(durability == Durability::Synced);
    opts
}
