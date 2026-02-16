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
pub use rocksdb::RocksDbStorage;
pub use storage::{BatchOp, Storage, StorageError};

#[cfg(test)]
mod tests {
    use super::*;
    use mbongo_core::{
        Account, Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionType,
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
            },
            body: BlockBody {
                transactions: vec![Transaction {
                    tx_type: TransactionType::Transfer,
                    sender: Address::zero(),
                    receiver: Address([6u8; 32]),
                    amount: 50,
                    nonce: 0,
                    signature: [0u8; 64],
                }],
            },
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
            },
            body: BlockBody {
                transactions: vec![],
            },
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
}
