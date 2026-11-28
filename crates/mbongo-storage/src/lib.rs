//! Storage backends for Mbongo Chain.
//!
//! Provides the [`Storage`] trait for domain-oriented persistence and two
//! implementations:
//!
//! - [`InMemoryStorage`] — `HashMap`-backed store for tests.
//! - [`RocksDbStorage`] — persistent store using RocksDB column families.

pub mod memory;
pub mod rocksdb;
pub mod storage;

pub use memory::InMemoryStorage;
pub use rocksdb::Opened;
pub use rocksdb::RocksDbStorage;
pub use storage::{BatchOp, ChainIdentity, Durability, Storage, StorageError};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rocksdb::SCHEMA_VERSION_CURRENT;
    use mbongo_core::{
        Account, Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionPayload,
        TransactionType,
    };

    fn sample_account() -> (Address, Account) {
        let addr = Address([1u8; 32]);
        let mut account = Account::new(addr);
        account.balance = 42_000;
        account.nonce = 3;
        (addr, account)
    }

    fn sample_transaction() -> (Hash, Transaction) {
        let hash = Hash([2u8; 32]);
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address([3u8; 32]),
            receiver: Address([4u8; 32]),
            amount: 100,
            nonce: 0,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        (hash, tx)
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
                producer: mbongo_core::Address::zero(),
            },
            body: BlockBody {
                transactions: vec![Transaction {
                    tx_type: TransactionType::Transfer,
                    sender: Address::zero(),
                    receiver: Address([6u8; 32]),
                    amount: 50,
                    nonce: 0,
                    payload: TransactionPayload::None,
                    signature: [0u8; 64],
                }],
            },
            producer_signature: [0u8; 64],
        };
        (hash, block)
    }

    /// Run the height-index suite against any [`Storage`] implementation.
    fn height_index_suite(store: &dyn Storage) {
        // Initially latest height is 0.
        assert_eq!(store.get_latest_height().unwrap(), 0);
        assert!(store.get_block_by_height(0).unwrap().is_none());

        // Store a block and index it at height 0.
        let (hash, block) = sample_block();
        store.put_block(&hash, &block).unwrap();
        store.put_block_height_index(0, hash).unwrap();

        // Latest height should now be 0 (the height we just stored is 0, and
        // it's only written when > current, but current starts at 0).
        // Actually 0 is not > 0, so the meta won't update past the initial 0.
        // Let's verify lookup works:
        let loaded = store.get_block_by_height(0).unwrap().expect("block at height 0");
        assert_eq!(loaded.header.height, block.header.height);

        // Index at height 1 to test latest-height update.
        let hash2 = Hash([7u8; 32]);
        let block2 = Block {
            header: BlockHeader {
                parent_hash: hash,
                state_root: Hash::zero(),
                transactions_root: Hash::zero(),
                timestamp: 1_700_000_001,
                height: 2,
                producer: mbongo_core::Address::zero(),
            },
            body: BlockBody {
                transactions: vec![],
            },
            producer_signature: [0u8; 64],
        };
        store.put_block(&hash2, &block2).unwrap();
        store.put_block_height_index(2, hash2).unwrap();
        assert_eq!(store.get_latest_height().unwrap(), 2);

        // Height 99 should still be None.
        assert!(store.get_block_by_height(99).unwrap().is_none());
    }

    /// Run the tx-seq suite against any [`Storage`] implementation.
    fn tx_seq_suite(store: &dyn Storage) {
        // Initial state.
        assert_eq!(store.get_last_included_tx_seq().unwrap(), 0);
        assert!(store.get_tx_hash_by_seq(1).unwrap().is_none());

        // Allocate sequence numbers.
        assert_eq!(store.next_tx_seq().unwrap(), 1);
        assert_eq!(store.next_tx_seq().unwrap(), 2);
        assert_eq!(store.next_tx_seq().unwrap(), 3);

        // Index two hashes.
        let h1 = Hash([10u8; 32]);
        let h2 = Hash([11u8; 32]);
        store.put_tx_seq_index(1, &h1).unwrap();
        store.put_tx_seq_index(2, &h2).unwrap();

        assert_eq!(store.get_tx_hash_by_seq(1).unwrap(), Some(h1));
        assert_eq!(store.get_tx_hash_by_seq(2).unwrap(), Some(h2));
        assert!(store.get_tx_hash_by_seq(3).unwrap().is_none());

        // last_included tracking.
        store.set_last_included_tx_seq(2).unwrap();
        assert_eq!(store.get_last_included_tx_seq().unwrap(), 2);
    }

    /// Run the full roundtrip suite against any [`Storage`] implementation.
    fn roundtrip_suite(store: &dyn Storage) {
        // Account roundtrip
        let (addr, account) = sample_account();
        assert!(store.get_account(&addr).unwrap().is_none());
        store.put_account(&addr, &account).unwrap();
        let loaded = store.get_account(&addr).unwrap().expect("account missing");
        assert_eq!(loaded, account);

        // Block roundtrip
        let (hash, block) = sample_block();
        assert!(store.get_block(&hash).unwrap().is_none());
        store.put_block(&hash, &block).unwrap();
        let loaded = store.get_block(&hash).unwrap().expect("block missing");
        assert_eq!(loaded, block);

        // Transaction roundtrip
        let (hash, tx) = sample_transaction();
        assert!(store.get_transaction(&hash).unwrap().is_none());
        store.put_transaction(&hash, &tx).unwrap();
        let loaded = store.get_transaction(&hash).unwrap().expect("transaction missing");
        assert_eq!(loaded, tx);
    }

    /// Run the write_batch suite against any [`Storage`] implementation.
    fn write_batch_suite(store: &dyn Storage) {
        let (addr1, account1) = sample_account();
        let (tx_hash, tx) = sample_transaction();
        let (block_hash, block) = sample_block();

        // All state should be empty before batch.
        assert!(store.get_account(&addr1).unwrap().is_none());
        assert!(store.get_transaction(&tx_hash).unwrap().is_none());
        assert!(store.get_block(&block_hash).unwrap().is_none());
        assert!(store.get_block_by_height(1).unwrap().is_none());
        assert!(store.get_tx_hash_by_seq(1).unwrap().is_none());
        assert_eq!(store.get_last_included_tx_seq().unwrap(), 0);

        // Apply all mutations in a single batch.
        store
            .write_batch(vec![
                BatchOp::PutAccount(addr1, account1.clone()),
                BatchOp::PutTransaction(tx_hash, tx.clone()),
                BatchOp::PutBlock(block_hash, block.clone()),
                BatchOp::PutBlockHeightIndex(1, block_hash),
                BatchOp::PutTxSeqIndex(1, tx_hash),
                BatchOp::SetTxSeq(1),
                BatchOp::SetLastIncludedTxSeq(1),
            ])
            .unwrap();

        // All state should now be readable.
        let loaded_acc = store.get_account(&addr1).unwrap().expect("account");
        assert_eq!(loaded_acc, account1);

        let loaded_tx = store.get_transaction(&tx_hash).unwrap().expect("tx");
        assert_eq!(loaded_tx, tx);

        let loaded_block = store.get_block(&block_hash).unwrap().expect("block");
        assert_eq!(loaded_block, block);

        let loaded_by_height = store.get_block_by_height(1).unwrap().expect("block at height 1");
        assert_eq!(loaded_by_height, block);

        assert_eq!(store.get_latest_height().unwrap(), 1);

        let loaded_seq = store.get_tx_hash_by_seq(1).unwrap().expect("tx seq");
        assert_eq!(loaded_seq, tx_hash);

        assert_eq!(store.get_last_included_tx_seq().unwrap(), 1);
    }

    // ── InMemoryStorage tests ────────────────────────────────────────

    #[test]
    fn memory_account_roundtrip() {
        let store = InMemoryStorage::new();
        let (addr, account) = sample_account();
        store.put_account(&addr, &account).unwrap();
        let loaded = store.get_account(&addr).unwrap().expect("account missing");
        assert_eq!(loaded, account);
    }

    #[test]
    fn memory_block_roundtrip() {
        let store = InMemoryStorage::new();
        let (hash, block) = sample_block();
        store.put_block(&hash, &block).unwrap();
        let loaded = store.get_block(&hash).unwrap().expect("block missing");
        assert_eq!(loaded, block);
    }

    #[test]
    fn memory_transaction_roundtrip() {
        let store = InMemoryStorage::new();
        let (hash, tx) = sample_transaction();
        store.put_transaction(&hash, &tx).unwrap();
        let loaded = store.get_transaction(&hash).unwrap().expect("transaction missing");
        assert_eq!(loaded, tx);
    }

    #[test]
    fn memory_full_roundtrip() {
        let store = InMemoryStorage::new();
        roundtrip_suite(&store);
    }

    #[test]
    fn memory_height_index() {
        let store = InMemoryStorage::new();
        height_index_suite(&store);
    }

    #[test]
    fn memory_tx_seq() {
        let store = InMemoryStorage::new();
        tx_seq_suite(&store);
    }

    #[test]
    fn memory_write_batch() {
        let store = InMemoryStorage::new();
        write_batch_suite(&store);
    }

    // ── RocksDbStorage tests ─────────────────────────────────────────

    #[test]
    fn rocksdb_account_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        let (addr, account) = sample_account();
        store.put_account(&addr, &account).unwrap();
        let loaded = store.get_account(&addr).unwrap().expect("account missing");
        assert_eq!(loaded, account);
    }

    #[test]
    fn rocksdb_block_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        let (hash, block) = sample_block();
        store.put_block(&hash, &block).unwrap();
        let loaded = store.get_block(&hash).unwrap().expect("block missing");
        assert_eq!(loaded, block);
    }

    #[test]
    fn rocksdb_transaction_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        let (hash, tx) = sample_transaction();
        store.put_transaction(&hash, &tx).unwrap();
        let loaded = store.get_transaction(&hash).unwrap().expect("transaction missing");
        assert_eq!(loaded, tx);
    }

    #[test]
    fn rocksdb_full_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        roundtrip_suite(&store);
    }

    #[test]
    fn rocksdb_height_index() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        height_index_suite(&store);
    }

    #[test]
    fn rocksdb_tx_seq() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        tx_seq_suite(&store);
    }

    #[test]
    fn rocksdb_write_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        write_batch_suite(&store);
    }

    // ── Receipt store tests (RFC 0002 Phase 1) ───────────────────────

    /// Run the receipt suite against any [`Storage`] implementation.
    /// Storage treats receipt bytes as opaque; these are arbitrary bytes.
    fn receipt_suite(store: &dyn Storage) {
        let task_a = [0xA1u8; 32];
        let task_b = [0xB2u8; 32];
        let bytes_a = vec![1u8, 2, 3, 4, 5];

        // Missing lookup.
        assert!(!store.has_receipt(&task_a).unwrap());
        assert!(store.get_receipt(&task_a).unwrap().is_none());

        // Write via batch, read back.
        store.write_batch(vec![BatchOp::PutReceipt(task_a, bytes_a.clone())]).unwrap();
        assert!(store.has_receipt(&task_a).unwrap());
        assert_eq!(store.get_receipt(&task_a).unwrap(), Some(bytes_a));

        // Unrelated task id still missing.
        assert!(!store.has_receipt(&task_b).unwrap());

        // Receipt write participates in a batch with existing op kinds.
        let (addr, account) = sample_account();
        let bytes_b = vec![9u8; 64];
        store
            .write_batch(vec![
                BatchOp::PutAccount(addr, account.clone()),
                BatchOp::PutReceipt(task_b, bytes_b.clone()),
            ])
            .unwrap();
        assert!(store.has_receipt(&task_b).unwrap());
        assert_eq!(store.get_receipt(&task_b).unwrap(), Some(bytes_b));
        assert_eq!(store.get_account(&addr).unwrap(), Some(account));
    }

    #[test]
    fn memory_receipts() {
        let store = InMemoryStorage::new();
        receipt_suite(&store);
    }

    #[test]
    fn rocksdb_receipts() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        receipt_suite(&store);
    }

    // ── Task store tests (RFC 0005 §4) ───────────────────────────────

    /// Run the task suite against any [`Storage`] implementation.
    /// Storage treats task bytes as opaque; these are arbitrary bytes.
    fn task_suite(store: &dyn Storage) {
        let task_a = [0xC1u8; 32];
        let task_b = [0xD2u8; 32];
        let bytes_a = vec![7u8, 8, 9];

        // Missing lookup.
        assert!(!store.has_task(&task_a).unwrap());
        assert!(store.get_task(&task_a).unwrap().is_none());

        // Write via batch, read back.
        store.write_batch(vec![BatchOp::PutTask(task_a, bytes_a.clone())]).unwrap();
        assert!(store.has_task(&task_a).unwrap());
        assert_eq!(store.get_task(&task_a).unwrap(), Some(bytes_a));

        // Unrelated task id still missing.
        assert!(!store.has_task(&task_b).unwrap());

        // The tasks and receipts indexes are distinct keyspaces: a task
        // under an id says nothing about a receipt under the same id, and
        // vice versa (RFC 0005 §4.1 — two derived states, two indexes).
        assert!(!store.has_receipt(&task_a).unwrap());
        store.write_batch(vec![BatchOp::PutReceipt(task_b, vec![1u8])]).unwrap();
        assert!(store.has_receipt(&task_b).unwrap());
        assert!(!store.has_task(&task_b).unwrap());

        // Task write participates in a batch with existing op kinds.
        let (addr, account) = sample_account();
        let bytes_b = vec![3u8; 1155];
        store
            .write_batch(vec![
                BatchOp::PutAccount(addr, account.clone()),
                BatchOp::PutTask(task_b, bytes_b.clone()),
            ])
            .unwrap();
        assert!(store.has_task(&task_b).unwrap());
        assert_eq!(store.get_task(&task_b).unwrap(), Some(bytes_b));
        assert_eq!(store.get_account(&addr).unwrap(), Some(account));
    }

    #[test]
    fn memory_tasks() {
        let store = InMemoryStorage::new();
        task_suite(&store);
    }

    #[test]
    fn rocksdb_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        task_suite(&store);
    }

    // ── Schema version and migration tests (RFC 0002 §5) ─────────────

    use ::rocksdb::{ColumnFamilyDescriptor, Options, DB};

    const V1_CFS: [&str; 6] = [
        "accounts",
        "blocks",
        "transactions",
        "meta",
        "height_index",
        "tx_seq_index",
    ];

    /// Creates a database with the v0.2 (schema v1) layout the way v0.2
    /// code did, bypassing the new open sequence. Optionally seeds sample
    /// account/block/transaction data through raw handles.
    fn create_v1_database(path: &std::path::Path, seed_data: bool) {
        use parity_scale_codec::Encode;
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        let cfs: Vec<ColumnFamilyDescriptor> = V1_CFS
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
            .collect();
        let db = DB::open_cf_descriptors(&db_opts, path, cfs).unwrap();
        if seed_data {
            let (addr, account) = sample_account();
            let (block_hash, block) = sample_block();
            let (tx_hash, tx) = sample_transaction();
            let cf = db.cf_handle("accounts").unwrap();
            db.put_cf(&cf, addr.0, account.encode()).unwrap();
            let cf = db.cf_handle("blocks").unwrap();
            db.put_cf(&cf, block_hash.0, block.encode()).unwrap();
            let cf = db.cf_handle("transactions").unwrap();
            db.put_cf(&cf, tx_hash.0, tx.encode()).unwrap();
        }
        // Dropped here: database closed with no schema_version key and no
        // receipts column family — exactly the v0.2 on-disk state.
    }

    // ── Storage schema 4 (RFC 0006) ──────────────────────────────────────

    use crate::storage::{ChainIdentity, Durability};
    use crate::Opened;
    use mbongo_core::ChainId;
    use std::collections::BTreeMap;
    use std::path::Path;

    const SCHEMA_4_CFS: [&str; 8] = [
        "accounts",
        "blocks",
        "transactions",
        "meta",
        "height_index",
        "tx_seq_index",
        "receipts",
        "tasks",
    ];

    /// The identity of test chain `n`, whose genesis block hashes to
    /// `[0xC0 + n; 32]` (the storage layer stores hashes, it does not
    /// compute them).
    fn identity(n: u8) -> ChainIdentity {
        ChainIdentity {
            chain_id: ChainId([n; 32]),
            genesis_hash: Hash([0xC0 + n; 32]),
        }
    }

    /// The genesis batch of test chain `n`: block 0, its height index, one
    /// allocation, and the identity, synced.
    fn genesis_ops(n: u8) -> Vec<BatchOp> {
        let id = identity(n);
        let (_, mut block) = sample_block();
        block.header.height = 0;
        let (addr, account) = sample_account();
        vec![
            BatchOp::PutBlock(id.genesis_hash, block),
            BatchOp::PutBlockHeightIndex(0, id.genesis_hash),
            BatchOp::PutAccount(addr, account),
            BatchOp::PutChainIdentity(id),
        ]
    }

    fn write_genesis(store: &dyn Storage, n: u8) {
        store.write_batch_with(genesis_ops(n), Durability::Synced).unwrap();
    }

    /// Opens an existing database with raw RocksDB, every column family it
    /// has, to build the states a test needs.
    fn raw(path: &Path) -> DB {
        let existing = DB::list_cf(&Options::default(), path).unwrap();
        let cfs: Vec<ColumnFamilyDescriptor> = existing
            .iter()
            .map(|n| ColumnFamilyDescriptor::new(n.clone(), Options::default()))
            .collect();
        DB::open_cf_descriptors(&Options::default(), path, cfs).unwrap()
    }

    /// Creates a database of the schema-4 layout with raw RocksDB, `stamp`
    /// written if given, nothing else.
    fn create_layout(path: &Path, names: &[&str], stamp: Option<&[u8]>) {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let cfs: Vec<ColumnFamilyDescriptor> = names
            .iter()
            .map(|n| ColumnFamilyDescriptor::new(*n, Options::default()))
            .collect();
        let db = DB::open_cf_descriptors(&opts, path, cfs).unwrap();
        if let Some(stamp) = stamp {
            let cf = db.cf_handle("meta").unwrap();
            db.put_cf(&cf, b"schema_version", stamp).unwrap();
        }
    }

    /// A schema-3 (v0.4) database: the eight column families, stamp 3, and
    /// a block if `with_block`.
    fn create_v3_database(path: &Path, with_block: bool) {
        create_layout(path, &SCHEMA_4_CFS, Some(&3u32.to_be_bytes()));
        if with_block {
            let db = raw(path);
            let (hash, block) = sample_block();
            let cf = db.cf_handle("blocks").unwrap();
            db.put_cf(&cf, hash.0, parity_scale_codec::Encode::encode(&block)).unwrap();
            let cf = db.cf_handle("height_index").unwrap();
            db.put_cf(&cf, 0u64.to_be_bytes(), hash.0).unwrap();
        }
    }

    /// Creates a database with the v0.3 (schema v2) layout the way v0.3
    /// code did: the six v1 column families plus `receipts`, stamped 2.
    /// Optionally seeds one receipt through raw handles.
    fn create_v2_database(path: &Path, seed_receipt: Option<([u8; 32], Vec<u8>)>) {
        let mut names: Vec<&str> = V1_CFS.to_vec();
        names.push("receipts");
        create_layout(path, &names, Some(&2u32.to_be_bytes()));
        if let Some((task_id, bytes)) = seed_receipt {
            let db = raw(path);
            let cf = db.cf_handle("receipts").unwrap();
            db.put_cf(&cf, task_id, bytes).unwrap();
        }
    }

    type Snapshot = BTreeMap<String, Vec<(Vec<u8>, Vec<u8>)>>;

    /// Every column family of the database at `path`, with every key and
    /// value: what "logically unchanged" compares.
    fn snapshot(path: &Path) -> Snapshot {
        let names = DB::list_cf(&Options::default(), path).unwrap();
        let db = DB::open_cf_for_read_only(&Options::default(), path, &names, false).unwrap();
        names
            .iter()
            .map(|name| {
                let iter = if name == "default" {
                    db.iterator(::rocksdb::IteratorMode::Start)
                } else {
                    let cf = db.cf_handle(name).unwrap();
                    db.iterator_cf(&cf, ::rocksdb::IteratorMode::Start)
                };
                let entries = iter.map(|r| {
                    let (k, v) = r.unwrap();
                    (k.to_vec(), v.to_vec())
                });
                (name.clone(), entries.collect())
            })
            .collect()
    }

    /// Every file of `dir`, with its bytes, except RocksDB's info `LOG`
    /// files, which a read-only open may write.
    fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().unwrap().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with("LOG"))
            .map(|name| {
                let bytes = std::fs::read(dir.join(&name)).unwrap();
                (name, bytes)
            })
            .collect()
    }

    /// `open` and `open_chain` both refuse the directory, with a message
    /// containing `needle`, and leave it logically unchanged, and every file
    /// but RocksDB's info log byte for byte.
    fn assert_refused(path: &Path, needle: &str) {
        let (keys, bytes) = (snapshot(path), files(path));
        for result in [
            RocksDbStorage::open(path).map(|_| ()),
            RocksDbStorage::open_chain(path, &identity(1)).map(|_| ()),
        ] {
            match result {
                Err(StorageError::Schema(msg)) => {
                    assert!(msg.contains(needle), "expected {needle:?} in: {msg}");
                }
                Err(other) => panic!("expected a Schema refusal ({needle}), got {other:?}"),
                Ok(()) => panic!("expected a refusal ({needle}), the directory was opened"),
            }
        }
        assert_eq!(snapshot(path), keys, "{needle}: the database changed");
        assert_eq!(files(path), bytes, "{needle}: a file changed");
    }

    #[test]
    fn rocksdb_fresh_database_is_schema_4() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        assert_eq!(SCHEMA_VERSION_CURRENT, 4);
        assert_eq!(store.schema_version().unwrap(), Some(4));
        assert_eq!(store.chain_identity().unwrap(), None);
        assert!(!store.has_receipt(&[0u8; 32]).unwrap());
        assert!(!store.has_task(&[0u8; 32]).unwrap());
        drop(store);
        // The layout is exactly the eight column families and the default.
        let mut names = DB::list_cf(&Options::default(), dir.path()).unwrap();
        names.sort();
        let mut expected: Vec<String> = SCHEMA_4_CFS.iter().map(ToString::to_string).collect();
        expected.push("default".to_string());
        expected.sort();
        assert_eq!(names, expected);
        // Reopening a store that holds only its stamp writes nothing.
        let before = snapshot(dir.path());
        drop(RocksDbStorage::open(dir.path()).unwrap());
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn rocksdb_chain_data_without_an_identity_is_refused() {
        type Write = Box<dyn Fn(&RocksDbStorage)>;
        let cases: Vec<(&str, Write)> = vec![
            (
                "account",
                Box::new(|s| {
                    let (addr, account) = sample_account();
                    s.put_account(&addr, &account).unwrap();
                }),
            ),
            (
                "block",
                Box::new(|s| {
                    let (hash, block) = sample_block();
                    s.put_block(&hash, &block).unwrap();
                }),
            ),
            (
                "transaction",
                Box::new(|s| {
                    let (hash, tx) = sample_transaction();
                    s.put_transaction(&hash, &tx).unwrap();
                }),
            ),
            (
                "height index and meta latest_height",
                Box::new(|s| s.put_block_height_index(0, Hash([1u8; 32])).unwrap()),
            ),
            (
                "tx seq index",
                Box::new(|s| s.put_tx_seq_index(0, &Hash([2u8; 32])).unwrap()),
            ),
            (
                "meta only",
                Box::new(|s| s.set_last_included_tx_seq(1).unwrap()),
            ),
            (
                "receipt",
                Box::new(|s| s.write_batch(vec![BatchOp::PutReceipt([3u8; 32], vec![1])]).unwrap()),
            ),
            (
                "task",
                Box::new(|s| s.write_batch(vec![BatchOp::PutTask([4u8; 32], vec![1])]).unwrap()),
            ),
        ];
        assert_eq!(cases.len(), 8);
        for (name, write) in &cases {
            let dir = tempfile::tempdir().unwrap();
            write(&RocksDbStorage::open(dir.path()).unwrap());
            assert_refused(dir.path(), "chain data but no chain identity");
            let _ = name;
        }
        // The default column family, which no API writes, counts too.
        let dir = tempfile::tempdir().unwrap();
        drop(RocksDbStorage::open(dir.path()).unwrap());
        raw(dir.path()).put(b"stray", b"x").unwrap();
        assert_refused(dir.path(), "chain data but no chain identity");
    }

    #[test]
    fn rocksdb_earlier_schemas_are_refused_unchanged() {
        type Make = Box<dyn Fn(&Path)>;
        let cases: Vec<(&str, Make, &str)> = vec![
            (
                "schema 1, empty",
                Box::new(|p| create_v1_database(p, false)),
                "schema 1",
            ),
            (
                "schema 1, with data",
                Box::new(|p| create_v1_database(p, true)),
                "schema 1",
            ),
            (
                "schema 2, empty",
                Box::new(|p| create_v2_database(p, None)),
                "schema 2",
            ),
            (
                "schema 2, with a receipt",
                Box::new(|p| create_v2_database(p, Some(([7u8; 32], vec![1])))),
                "schema 2",
            ),
            (
                "schema 3, stamp only",
                Box::new(|p| create_v3_database(p, false)),
                "schema 3",
            ),
            (
                "schema 3, with a block",
                Box::new(|p| create_v3_database(p, true)),
                "schema 3",
            ),
        ];
        assert_eq!(cases.len(), 6, "cases stated before they run");
        for (name, make, needle) in &cases {
            let dir = tempfile::tempdir().unwrap();
            make(dir.path());
            assert_refused(dir.path(), needle);
            // Neither stamped nor given a column family.
            let after = snapshot(dir.path());
            let stamp = after["meta"].iter().find(|(k, _)| k == b"schema_version");
            match *needle {
                "schema 1" => assert!(stamp.is_none(), "{name}"),
                _ => assert_ne!(stamp.unwrap().1, 4u32.to_be_bytes().to_vec(), "{name}"),
            }
            if *needle != "schema 3" {
                assert!(
                    !after.contains_key("tasks"),
                    "{name}: a column family was added"
                );
            }
            assert!(
                matches!(RocksDbStorage::open(dir.path()), Err(StorageError::Schema(m)) if m.contains("not migrated")),
                "{name}"
            );
        }
    }

    /// What a refused open leaves on disk. Every file but RocksDB's info logs
    /// is byte for byte the same (asserted in every refusal test); this test
    /// records, without asserting, what happens to the info logs, which
    /// RocksDB may write even on a read-only open.
    #[test]
    fn rocksdb_refusal_changes_at_most_the_info_logs() {
        let dir = tempfile::tempdir().unwrap();
        create_v3_database(dir.path(), true);
        let logs = |dir: &Path| -> BTreeMap<String, u64> {
            std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap())
                .filter(|e| e.file_name().to_string_lossy().starts_with("LOG"))
                .map(|e| {
                    (
                        e.file_name().to_string_lossy().into_owned(),
                        e.metadata().unwrap().len(),
                    )
                })
                .collect()
        };
        let (before, bytes) = (logs(dir.path()), files(dir.path()));
        assert!(RocksDbStorage::open(dir.path()).is_err());
        let after = logs(dir.path());
        assert_eq!(files(dir.path()), bytes);
        println!("REFUSAL-LOGS before {before:?} after {after:?}");
    }

    #[test]
    fn rocksdb_newer_or_malformed_stamps_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS, Some(&5u32.to_be_bytes()));
        assert_refused(dir.path(), "schema 5 is newer than supported version 4");

        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS, Some(&[0, 0, 4]));
        assert_refused(dir.path(), "malformed schema_version");
    }

    #[test]
    fn rocksdb_unknown_or_missing_column_families_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut names = SCHEMA_4_CFS.to_vec();
        names.push("future_cf");
        create_layout(dir.path(), &names, Some(&4u32.to_be_bytes()));
        assert_refused(dir.path(), "unknown column family 'future_cf'");

        // Stamp 4 without `tasks`.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS[..7], Some(&4u32.to_be_bytes()));
        assert_refused(dir.path(), "missing column family 'tasks'");

        // No meta at all.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &["accounts", "blocks"], None);
        assert_refused(dir.path(), "missing column family 'meta'");
    }

    #[test]
    fn rocksdb_files_that_are_not_a_database_are_refused_unchanged() {
        // Any other file.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"keep me").unwrap();
        let before = files(dir.path());
        for result in [
            RocksDbStorage::open(dir.path()).map(|_| ()),
            RocksDbStorage::open_chain(dir.path(), &identity(1)).map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(StorageError::Schema(m)) if m.contains("no database")),
                "a directory of other files must be refused"
            );
        }
        assert_eq!(files(dir.path()), before);

        // Crash point 1: interrupted before the database existed. RocksDB
        // writes CURRENT last; without it the files are no database yet.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS, None);
        std::fs::remove_file(dir.path().join("CURRENT")).unwrap();
        let before = files(dir.path());
        assert!(matches!(
            RocksDbStorage::open(dir.path()),
            Err(StorageError::Schema(m)) if m.contains("no database")
        ));
        assert_eq!(files(dir.path()), before, "nothing reinitialized");
    }

    #[test]
    fn rocksdb_identity_is_checked_on_reopen() {
        let dir = tempfile::tempdir().unwrap();
        {
            let (store, opened) = RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap();
            assert_eq!(opened, Opened::Created);
            write_genesis(&store, 1);
            assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
        }
        let (store, opened) = RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap();
        assert_eq!(opened, Opened::Reopened);
        assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
        drop(store);

        // Another chain id, and the same chain id with another genesis hash.
        let other_chain = identity(2);
        let other_genesis = ChainIdentity {
            genesis_hash: Hash([0xEE; 32]),
            ..identity(1)
        };
        for other in [other_chain, other_genesis] {
            let (keys, bytes) = (snapshot(dir.path()), files(dir.path()));
            match RocksDbStorage::open_chain(dir.path(), &other) {
                Err(StorageError::Schema(msg)) => {
                    assert!(msg.contains("belongs to chain"), "{msg}");
                }
                Err(e) => panic!("expected a refusal, got {e:?}"),
                Ok(_) => panic!("another chain's directory was opened"),
            }
            assert_eq!(snapshot(dir.path()), keys);
            assert_eq!(files(dir.path()), bytes);
        }
        // Plain `open` does not choose a chain: it opens it.
        assert_eq!(
            RocksDbStorage::open(dir.path()).unwrap().chain_identity().unwrap(),
            Some(identity(1))
        );
    }

    #[test]
    fn rocksdb_partial_malformed_or_inconsistent_identities_are_refused() {
        type Damage = Box<dyn Fn(&DB)>;
        let cases: Vec<(&str, Damage, &str)> = vec![
            (
                "chain_id only",
                Box::new(|db| {
                    db.delete_cf(&db.cf_handle("meta").unwrap(), b"genesis_hash").unwrap()
                }),
                "partial",
            ),
            (
                "genesis_hash only",
                Box::new(|db| db.delete_cf(&db.cf_handle("meta").unwrap(), b"chain_id").unwrap()),
                "partial",
            ),
            (
                "chain_id of 31 bytes",
                Box::new(|db| {
                    db.put_cf(&db.cf_handle("meta").unwrap(), b"chain_id", [1u8; 31]).unwrap()
                }),
                "not 32 bytes",
            ),
            (
                "genesis_hash of 33 bytes",
                Box::new(|db| {
                    db.put_cf(&db.cf_handle("meta").unwrap(), b"genesis_hash", [1u8; 33]).unwrap();
                }),
                "not 32 bytes",
            ),
            (
                "genesis_hash not the indexed block 0",
                Box::new(|db| {
                    db.put_cf(
                        &db.cf_handle("height_index").unwrap(),
                        0u64.to_be_bytes(),
                        [9u8; 32],
                    )
                    .unwrap();
                }),
                "not this database's block 0",
            ),
            (
                "block 0 missing",
                Box::new(|db| {
                    db.delete_cf(&db.cf_handle("blocks").unwrap(), identity(1).genesis_hash.0)
                        .unwrap();
                }),
                "not this database's block 0",
            ),
        ];
        assert_eq!(cases.len(), 6, "cases stated before they run");
        for (name, damage, needle) in &cases {
            let dir = tempfile::tempdir().unwrap();
            write_genesis(&RocksDbStorage::open(dir.path()).unwrap(), 1);
            damage(&raw(dir.path()));
            assert_refused(dir.path(), needle);
            let _ = name;
        }
    }

    #[test]
    fn rocksdb_identity_and_genesis_are_one_atomic_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        write_genesis(&store, 1);
        assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
        assert!(store.get_block(&identity(1).genesis_hash).unwrap().is_some());

        // A second genesis batch of another chain is refused whole: its
        // block, account and identity are all absent afterwards.
        let before = snapshot(dir.path());
        drop(store);
        let store = RocksDbStorage::open(dir.path()).unwrap();
        assert!(matches!(
            store.write_batch_with(genesis_ops(2), Durability::Synced),
            Err(StorageError::Schema(m)) if m.contains("refusing to record chain")
        ));
        assert!(store.get_block(&identity(2).genesis_hash).unwrap().is_none());
        assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
        drop(store);
        assert_eq!(snapshot(dir.path()), before);
        // Writing the same identity again is allowed and changes nothing.
        let store = RocksDbStorage::open(dir.path()).unwrap();
        store.write_batch(vec![BatchOp::PutChainIdentity(identity(1))]).unwrap();
        drop(store);
        assert_eq!(snapshot(dir.path()), before);
    }

    #[test]
    fn rocksdb_interrupted_creation_resumes_only_when_empty() {
        // Crash points 2 and 3: every column family, no stamp, nothing else.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS, None);
        let (store, opened) = RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap();
        assert_eq!(opened, Opened::Resumed);
        assert_eq!(store.schema_version().unwrap(), Some(4));
        write_genesis(&store, 1);
        drop(store);
        assert_eq!(
            RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap().1,
            Opened::Reopened
        );

        // Crash point 4: stamp 4, nothing else.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS, Some(&4u32.to_be_bytes()));
        let (_, opened) = RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap();
        assert_eq!(opened, Opened::Resumed);

        // During column-family creation: some column families, no stamp. It
        // cannot be told from an empty v0.2 database, so it is refused.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &V1_CFS, None);
        assert_refused(dir.path(), "schema 1");

        // No stamp with any data is not an interrupted creation.
        let dir = tempfile::tempdir().unwrap();
        create_layout(dir.path(), &SCHEMA_4_CFS, None);
        let db = raw(dir.path());
        db.put_cf(&db.cf_handle("accounts").unwrap(), b"a", b"b").unwrap();
        drop(db);
        assert_refused(dir.path(), "schema 1");
    }

    #[test]
    fn rocksdb_one_store_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let (first, _) = RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap();
        // While it is open, nothing else opens the directory, whatever chain.
        for chain in [1, 2] {
            assert!(RocksDbStorage::open_chain(dir.path(), &identity(chain)).is_err());
        }
        write_genesis(&first, 1);
        drop(first);
        // Two starts at once on a new directory, for two chains: at most one
        // opens it, and the directory ends up with exactly one identity.
        for _ in 0..10 {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().to_path_buf();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let starts: Vec<_> = [1u8, 2]
                .into_iter()
                .map(|n| {
                    let (path, barrier) = (path.clone(), barrier.clone());
                    std::thread::spawn(move || {
                        barrier.wait();
                        let (store, _) = RocksDbStorage::open_chain(&path, &identity(n)).ok()?;
                        store.write_batch_with(genesis_ops(n), Durability::Synced).ok()?;
                        Some(n)
                    })
                })
                .collect();
            let won: Vec<u8> = starts.into_iter().filter_map(|t| t.join().unwrap()).collect();
            assert!(
                won.len() <= 1,
                "two starts initialized one directory: {won:?}"
            );
            if let [n] = won[..] {
                let store = RocksDbStorage::open_chain(&path, &identity(n)).unwrap().0;
                assert_eq!(store.chain_identity().unwrap(), Some(identity(n)));
            }
        }
    }

    #[test]
    fn rocksdb_a_directory_changed_while_opening_is_refused() {
        // Between the read-only decision and the lock, another start records
        // a chain in a directory that held only the stamp. The decision under
        // the lock sees it and refuses, whichever chain is expected: without
        // that second decision, the open would report a resumed creation.
        for chain in [1, 2] {
            let dir = tempfile::tempdir().unwrap();
            drop(RocksDbStorage::open(dir.path()).unwrap());
            crate::rocksdb::BETWEEN_INSPECT_AND_OPEN.with(|h| {
                *h.borrow_mut() = Some(Box::new(|p: &Path| {
                    let (other, _) = RocksDbStorage::open_chain(p, &identity(1)).unwrap();
                    write_genesis(&other, 1);
                }));
            });
            let err = RocksDbStorage::open_chain(dir.path(), &identity(chain))
                .err()
                .expect("a directory that changed while opening was accepted")
                .to_string();
            assert!(err.contains("changed while it was being opened"), "{err}");
            assert!(
                crate::rocksdb::BETWEEN_INSPECT_AND_OPEN.with(|h| h.borrow().is_none()),
                "the change ran"
            );
            // Left as the other start wrote it.
            let store = RocksDbStorage::open_chain(dir.path(), &identity(1)).unwrap().0;
            assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
        }
    }

    #[test]
    fn rocksdb_synced_writes_sync_the_log_and_buffered_writes_do_not() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open_with_statistics(dir.path()).unwrap();
        let base = store.wal_syncs();
        for i in 0..5u8 {
            store.write_batch(vec![BatchOp::PutTask([i; 32], vec![i])]).unwrap();
        }
        assert_eq!(store.wal_syncs(), base, "a buffered write synced the log");
        for i in 0..5u8 {
            store
                .write_batch_with(vec![BatchOp::PutTask([i; 32], vec![i])], Durability::Synced)
                .unwrap();
        }
        assert_eq!(
            store.wal_syncs(),
            base + 5,
            "each synced write syncs the log once"
        );
        // The plain write_batch is the buffered one.
        store
            .write_batch_with(
                vec![BatchOp::PutTask([9; 32], vec![9])],
                Durability::Buffered,
            )
            .unwrap();
        assert_eq!(store.wal_syncs(), base + 5);
    }

    #[test]
    fn memory_and_rocksdb_agree_on_identity() {
        let dir = tempfile::tempdir().unwrap();
        let rocks = RocksDbStorage::open(dir.path()).unwrap();
        let memory = InMemoryStorage::new();
        for store in [&rocks as &dyn Storage, &memory] {
            assert_eq!(store.chain_identity().unwrap(), None);
            write_genesis(store, 1);
            assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
            assert!(matches!(
                store.write_batch(genesis_ops(2)),
                Err(StorageError::Schema(m)) if m.contains("refusing to record chain")
            ));
            assert!(store.get_block(&identity(2).genesis_hash).unwrap().is_none());
            store.write_batch(vec![BatchOp::PutChainIdentity(identity(1))]).unwrap();
            assert_eq!(store.chain_identity().unwrap(), Some(identity(1)));
        }
    }

    /// The cost of a synced write against a buffered one, on this machine.
    /// A measurement, not a check: it changes nothing the node does.
    ///
    /// ```bash
    /// cargo test -p mbongo-storage --release -- --ignored --nocapture fsync_cost
    /// ```
    #[test]
    #[ignore = "measurement; run by hand, preferably with --release"]
    fn fsync_cost() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open_with_statistics(dir.path()).unwrap();
        let (_, block) = sample_block();
        let mut report = Vec::new();
        for durability in [Durability::Buffered, Durability::Synced] {
            let mut micros: Vec<u128> = (0..200u64)
                .map(|i| {
                    let mut b = block.clone();
                    b.header.height = i;
                    let hash = Hash([u8::try_from(i % 251).unwrap(); 32]);
                    let start = std::time::Instant::now();
                    store
                        .write_batch_with(
                            vec![
                                BatchOp::PutBlock(hash, b),
                                BatchOp::PutBlockHeightIndex(i, hash),
                            ],
                            durability,
                        )
                        .unwrap();
                    start.elapsed().as_micros()
                })
                .collect();
            micros.sort_unstable();
            report.push(format!(
                "{durability:?}: p50 {} us, p95 {} us, p99 {} us",
                micros[100], micros[190], micros[198]
            ));
        }
        println!(
            "FSYNC-COST {} | wal syncs {}",
            report.join(" | "),
            store.wal_syncs()
        );
    }
}
