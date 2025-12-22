//! Core blockchain primitives for Mbongo Chain.
//!
//! This crate provides the foundational types and utilities used throughout
//! the Mbongo Chain blockchain, including:
//! - Block and transaction primitives
//! - Cryptographic helpers (hashing)
//!
//! # Block Primitives
//!
//! The `Block` type models a blockchain block consisting of a header and body.
//! The header contains chain-linkage, commitment roots and metadata; the body
//! contains the ordered list of transactions.
//!
//! ```rust
//! use mbongo_core::{Block, BlockHeader, BlockBody, Hash, Transaction, Address, TransactionType};
//! 
//! // Build a simple block with two transactions (typed + signed)
//! let txs = vec![
//!     Transaction { tx_type: TransactionType::Transfer, sender: Address::zero(), receiver: Address::zero(), amount: 1, nonce: 0, signature: [0u8; 64] },
//!     Transaction { tx_type: TransactionType::Stake, sender: Address::zero(), receiver: Address::zero(), amount: 1000, nonce: 1, signature: [0u8; 64] },
//! ];
//!
//! // Build a simple block with two transactions (opaque bytes)
//! let txs = vec![Transaction(vec![1,2,3]), Transaction(vec![4,5])];
//! let header = BlockHeader {
//!     parent_hash: Hash::zero(),
//!     state_root: Hash::zero(),
//!     transactions_root: mbongo_core::compute_transactions_root(&txs),
//!     timestamp: 1_700_000_000,
//!     height: 1,
//! };
//! let body = BlockBody { transactions: txs };
//! let _block = Block { header, body };
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

mod primitives;
pub mod storage;

pub use primitives::{compute_transactions_root, Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionType};
pub use storage::trie::{MerklePatriciaTrie, ProofNode};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn hash_invalid_length() {
        let too_short = "0x1234";  // Not 64 hex characters
        assert!(too_short. parse::<Hash>().is_err());
        
        let too_long = "0x". to_string() + &"0".repeat(65);
        assert!(too_long.parse::<Hash>().is_err());
    }
    
    #[test]
    fn hash_missing_prefix() {
        let no_prefix = "0". repeat(64);  // Missing "0x"
        assert!(no_prefix.parse::<Hash>().is_err());
    }
    
    #[test]
    fn block_serde_roundtrip() {
        let txs = vec![
            Transaction {
                tx_type: TransactionType::Transfer,
                sender: Address::zero(),
                receiver: Address::zero(),
                amount: 10,
                nonce: 1,
                signature: [0u8; 64],
            },
            Transaction {
                tx_type: TransactionType::Stake,
                sender: Address::zero(),
                receiver: Address::zero(),
                amount: 1000,
                nonce: 2,
                signature: [0u8; 64],
            },
        ];
        let header = BlockHeader {
            parent_hash: Hash::zero(),
            state_root: Hash::zero(),
            transactions_root: compute_transactions_root(&txs),
            timestamp: 123,
            height: 7,
        };
        let block = Block { header, body: BlockBody { transactions: txs } };
        let s = json::to_string(&block).unwrap();
        let round: Block = json::from_str(&s).unwrap();
        // Verify all header fields are preserved
        assert_eq!(round.header.parent_hash, block.header.parent_hash);
        assert_eq!(round.header. state_root, block.header. state_root);
        assert_eq!(round.header.transactions_root, block.header.transactions_root);
        assert_eq!(round.header.timestamp, 123);
        assert_eq!(round.header.height, 7);
        
        // Verify transaction contents are preserved
        assert_eq!(round.body.transactions.len(), 2);
        assert_eq!(round.body.transactions[0].0, vec![1, 2, 3]);
    
    #[test]
    fn ed25519_signature_verification_transfer() {
        let sk_bytes = [1u8; 32];
        let sk = SigningKey::from_bytes(&sk_bytes);
        let vk: VerifyingKey = sk.verifying_key();
        let sender = Address(vk.to_bytes());
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender,
            receiver: Address::zero(),
            amount: 42,
            nonce: 7,
            signature: [0u8; 64],
        };
        let payload = tx.signing_payload();
        let sig = sk.sign(&payload);
        let mut tx_signed = tx;
        tx_signed.signature = sig.to_bytes();
        assert!(tx_signed.verify_signature());
    }

    #[test]
    fn scale_roundtrip_all_tx_types() {
        let sender = Address([3u8; 32]);
        let receiver = Address([4u8; 32]);
        for tt in [
            TransactionType::Transfer,
            TransactionType::ComputeTask,
            TransactionType::Stake,
        ] {
            let tx = Transaction {
                tx_type: tt,
                sender,
                receiver,
                amount: 1234,
                nonce: 9,
                signature: [5u8; 64],
            };
            let enc = tx.encode();
            let dec = Transaction::decode(&mut &enc[..]).unwrap();
            assert_eq!(dec.tx_type, tt);
            assert_eq!(dec.sender, sender);
            assert_eq!(dec.receiver, receiver);
            assert_eq!(dec.amount, 1234);
            assert_eq!(dec.nonce, 9);
            assert_eq!(dec.signature, [5u8; 64]);
        }
    }
        assert_eq!(round.body.transactions[1].0, vec![]);
    }

    #[test]
    fn transactions_root_changes_with_body() {
        let a = vec![Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address::zero(),
            receiver: Address::zero(),
            amount: 1,
            nonce: 0,
            signature: [0u8; 64],
        }];
        let b = vec![Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address::zero(),
            receiver: Address::zero(),
            amount: 2,
            nonce: 0,
            signature: [0u8; 64],
        }];
        let ra = compute_transactions_root(&a);
        let rb = compute_transactions_root(&b);
        assert_ne!(ra, rb);
    }
}
