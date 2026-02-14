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
pub use storage::{Storage, StorageError};

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
        let loaded = store
            .get_transaction(&hash)
            .unwrap()
            .expect("transaction missing");
        assert_eq!(loaded, tx);
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
        let loaded = store
            .get_transaction(&hash)
            .unwrap()
            .expect("transaction missing");
        assert_eq!(loaded, tx);
    }

    #[test]
    fn memory_full_roundtrip() {
        let store = InMemoryStorage::new();
        roundtrip_suite(&store);
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
        let loaded = store
            .get_transaction(&hash)
            .unwrap()
            .expect("transaction missing");
        assert_eq!(loaded, tx);
    }

    #[test]
    fn rocksdb_full_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = RocksDbStorage::open(dir.path()).unwrap();
        roundtrip_suite(&store);
    }
}
