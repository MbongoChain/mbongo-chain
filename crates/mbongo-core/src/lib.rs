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
//! use mbongo_core::{Block, BlockHeader, BlockBody, Hash, Transaction, TransactionPayload, Address, TransactionType, compute_transactions_root};
//!
//! // Build a simple block with two typed transactions (unsigned)
//! let txs = vec![
//!     Transaction { tx_type: TransactionType::Transfer, sender: Address::zero(), receiver: Address::zero(), amount: 1, nonce: 0, payload: TransactionPayload::None, signature: [0u8; 64] },
//!     Transaction { tx_type: TransactionType::Stake, sender: Address::zero(), receiver: Address::zero(), amount: 1000, nonce: 1, payload: TransactionPayload::None, signature: [0u8; 64] },
//! ];
//! let header = BlockHeader {
//!     parent_hash: Hash::zero(),
//!     state_root: Hash::zero(),
//!     transactions_root: compute_transactions_root(&txs),
//!     timestamp: 1_700_000_000,
//!     height: 1,
//! };
//! let body = BlockBody { transactions: txs };
//! let _block = Block { header, body };
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod account;
pub mod crypto;
mod primitives;
pub mod receipt;

pub use account::{Account, AccountError};
pub use primitives::{
    compute_transactions_root, Address, Block, BlockBody, BlockHeader, Hash, Transaction,
    TransactionPayload, TransactionType,
};
pub use receipt::Receipt;

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
    use parity_scale_codec::{Decode, Encode};
    use serde_json as json;

    #[test]
    fn hash_invalid_length() {
        let too_short = "0x1234"; // Not 64 hex characters
        assert!(too_short.parse::<Hash>().is_err());

        let too_long = "0x".to_string() + &"0".repeat(65);
        assert!(too_long.parse::<Hash>().is_err());
    }

    #[test]
    fn hash_missing_prefix() {
        let no_prefix = "0".repeat(64); // Missing "0x"
        assert!(no_prefix.parse::<Hash>().is_ok());
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
                payload: TransactionPayload::None,
                signature: [0u8; 64],
            },
            Transaction {
                tx_type: TransactionType::Stake,
                sender: Address::zero(),
                receiver: Address::zero(),
                amount: 1000,
                nonce: 2,
                payload: TransactionPayload::None,
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
        let block = Block {
            header,
            body: BlockBody {
                transactions: txs.clone(),
            },
        };
        let s = json::to_string(&block).unwrap();
        let round: Block = json::from_str(&s).unwrap();
        // Verify all header fields are preserved
        assert_eq!(round.header.parent_hash, block.header.parent_hash);
        assert_eq!(round.header.state_root, block.header.state_root);
        assert_eq!(
            round.header.transactions_root,
            block.header.transactions_root
        );
        assert_eq!(round.header.timestamp, 123);
        assert_eq!(round.header.height, 7);

        // Verify transaction contents are preserved
        assert_eq!(round.body.transactions.len(), 2);
        assert_eq!(round.body.transactions[0].tx_type, txs[0].tx_type);
        assert_eq!(round.body.transactions[1].tx_type, txs[1].tx_type);
    }

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
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        let payload = tx.signing_payload();
        let sig = sk.sign(&payload);
        let mut tx_signed = tx;
        tx_signed.signature = sig.to_bytes();
        assert!(tx_signed.verify_signature());
    }

    #[test]
    fn ed25519_signature_invalid_fails() {
        let sk_bytes = [7u8; 32];
        let sk = SigningKey::from_bytes(&sk_bytes);
        let vk: VerifyingKey = sk.verifying_key();
        let sender = Address(vk.to_bytes());
        let tx = Transaction {
            tx_type: TransactionType::Transfer,
            sender,
            receiver: Address::zero(),
            amount: 10,
            nonce: 1,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        };
        let sig = sk.sign(&tx.signing_payload());
        let mut tampered = tx.clone();
        tampered.amount = 11; // change payload after signing
        tampered.signature = sig.to_bytes();
        assert!(!tampered.verify_signature());
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
                payload: TransactionPayload::None,
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

    /// Sample receipt for payload tests (any well-formed receipt works;
    /// signature validity is irrelevant to encoding tests).
    fn sample_receipt() -> Receipt {
        Receipt {
            version: 1,
            task_id: [0x11u8; 32],
            input_commitment: [0x22u8; 32],
            output_commitment: [0x33u8; 32],
            executor: Address([0x44u8; 32]),
            metadata: vec![0xDE, 0xAD],
            signature: [0x55u8; 64],
        }
    }

    fn sample_transfer() -> Transaction {
        Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address([3u8; 32]),
            receiver: Address([4u8; 32]),
            amount: 1234,
            nonce: 9,
            payload: TransactionPayload::None,
            signature: [5u8; 64],
        }
    }

    #[test]
    fn transaction_payload_codec_indexes_pinned() {
        // TransactionPayload::None encodes as exactly one 0x00 byte.
        assert_eq!(TransactionPayload::None.encode(), vec![0x00]);

        // TransactionPayload::AnchorReceipt encodes as 0x01 followed by
        // the canonical receipt bytes.
        let receipt = sample_receipt();
        let mut expected = vec![0x01];
        expected.extend_from_slice(&receipt.encode());
        assert_eq!(
            TransactionPayload::AnchorReceipt(Box::new(receipt)).encode(),
            expected
        );

        // TransactionType indexes are pinned: 0..=2 match v0.2 implicit
        // order byte-for-byte, 3 is the v0.3 addition.
        assert_eq!(TransactionType::Transfer.encode(), vec![0x00]);
        assert_eq!(TransactionType::ComputeTask.encode(), vec![0x01]);
        assert_eq!(TransactionType::Stake.encode(), vec![0x02]);
        assert_eq!(TransactionType::AnchorReceipt.encode(), vec![0x03]);
    }

    #[test]
    fn transaction_payload_serde_shape_has_no_box_wrapper() {
        // Box<Receipt> must be invisible in the serde representation: the
        // JSON is {"AnchorReceipt": {<receipt fields>}} with the fields
        // directly inside, and None is the plain string "None".
        let v = json::to_value(TransactionPayload::AnchorReceipt(
            Box::new(sample_receipt()),
        ))
        .unwrap();
        assert_eq!(v["AnchorReceipt"]["version"], 1);
        assert_eq!(v["AnchorReceipt"]["metadata"], json::json!([0xDE, 0xAD]));

        let none = json::to_value(TransactionPayload::None).unwrap();
        assert_eq!(none, json::json!("None"));
    }

    #[test]
    fn transaction_payload_roundtrips() {
        for payload in [
            TransactionPayload::None,
            TransactionPayload::AnchorReceipt(Box::new(sample_receipt())),
        ] {
            let enc = payload.encode();
            let dec = TransactionPayload::decode(&mut &enc[..]).unwrap();
            assert_eq!(dec, payload);
        }
    }

    #[test]
    fn transaction_scale_roundtrip_with_anchor_receipt_payload() {
        let tx = Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender: Address([3u8; 32]),
            receiver: Address::zero(),
            amount: 0,
            nonce: 9,
            payload: TransactionPayload::AnchorReceipt(Box::new(sample_receipt())),
            signature: [5u8; 64],
        };
        let enc = tx.encode();
        let dec = Transaction::decode(&mut &enc[..]).unwrap();
        assert_eq!(dec, tx);
    }

    #[test]
    fn payload_changes_signing_payload_and_hash() {
        let tx_none = sample_transfer();
        let mut tx_anchor = sample_transfer();
        tx_anchor.payload = TransactionPayload::AnchorReceipt(Box::new(sample_receipt()));

        // The payload is covered by the signing payload...
        assert_ne!(tx_none.signing_payload(), tx_anchor.signing_payload());
        // ...and by the transaction hash (BLAKE3 over the full encoding).
        assert_ne!(
            crypto::blake3_hash(&tx_none.encode()),
            crypto::blake3_hash(&tx_anchor.encode())
        );
    }

    #[test]
    fn payload_tampering_invalidates_signature() {
        let sk = SigningKey::from_bytes(&[9u8; 32]);
        let sender = Address(sk.verifying_key().to_bytes());
        let mut tx = sample_transfer();
        tx.sender = sender;
        tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
        assert!(tx.verify_signature());

        // Swapping the payload after signing must invalidate the signature.
        tx.payload = TransactionPayload::AnchorReceipt(Box::new(sample_receipt()));
        assert!(!tx.verify_signature());
    }

    #[test]
    fn fixed_v0_3_transaction_encoding_vector() {
        // Canonical v0.3 field order: tx_type, sender, receiver, amount,
        // nonce, payload, signature. A transfer with None payload encodes
        // to exactly 154 bytes (v0.2 was 153: the payload adds one byte).
        let tx = sample_transfer();
        let mut expected = Vec::new();
        expected.push(0x00); // tx_type: Transfer (index 0)
        expected.extend_from_slice(&[3u8; 32]); // sender
        expected.extend_from_slice(&[4u8; 32]); // receiver
        expected.extend_from_slice(&1234u128.to_le_bytes()); // amount
        expected.extend_from_slice(&9u64.to_le_bytes()); // nonce
        expected.push(0x00); // payload: None (index 0)
        expected.extend_from_slice(&[5u8; 64]); // signature

        let encoded = tx.encode();
        assert_eq!(encoded.len(), 154);
        assert_eq!(encoded, expected);
    }

    #[test]
    fn v0_2_transaction_bytes_do_not_decode() {
        // A v0.2 transaction encoded without the payload field: tx_type,
        // sender, receiver, amount, nonce, signature = 153 bytes. Under
        // v0.3 decoding, signature[0] is consumed as the payload
        // discriminant and the remaining bytes cannot complete the
        // transaction — v0.2 bytes are structurally incompatible.
        let mut v0_2_bytes = Vec::new();
        v0_2_bytes.push(0x00); // tx_type: Transfer
        v0_2_bytes.extend_from_slice(&[3u8; 32]); // sender
        v0_2_bytes.extend_from_slice(&[4u8; 32]); // receiver
        v0_2_bytes.extend_from_slice(&1234u128.to_le_bytes()); // amount
        v0_2_bytes.extend_from_slice(&9u64.to_le_bytes()); // nonce
        v0_2_bytes.extend_from_slice(&[0u8; 64]); // signature (no payload)
        assert_eq!(v0_2_bytes.len(), 153);

        assert!(Transaction::decode(&mut &v0_2_bytes[..]).is_err());
    }

    #[test]
    fn block_roundtrip_with_payload_variants() {
        // A block carrying both a None-payload transfer and an
        // AnchorReceipt-payload transaction round-trips through SCALE and
        // serde. Pure data: no consensus rule is applied here.
        let txs = vec![
            sample_transfer(),
            Transaction {
                tx_type: TransactionType::AnchorReceipt,
                sender: Address([0x44u8; 32]),
                receiver: Address::zero(),
                amount: 0,
                nonce: 0,
                payload: TransactionPayload::AnchorReceipt(Box::new(sample_receipt())),
                signature: [0u8; 64],
            },
        ];
        let block = Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: compute_transactions_root(&txs),
                timestamp: 42,
                height: 1,
            },
            body: BlockBody { transactions: txs },
        };

        let enc = block.encode();
        let dec = Block::decode(&mut &enc[..]).unwrap();
        assert_eq!(dec, block);

        let s = json::to_string(&block).unwrap();
        let round: Block = json::from_str(&s).unwrap();
        assert_eq!(round, block);
    }

    #[test]
    fn transactions_root_changes_with_body() {
        let a = vec![Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address::zero(),
            receiver: Address::zero(),
            amount: 1,
            nonce: 0,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        }];
        let b = vec![Transaction {
            tx_type: TransactionType::Transfer,
            sender: Address::zero(),
            receiver: Address::zero(),
            amount: 2,
            nonce: 0,
            payload: TransactionPayload::None,
            signature: [0u8; 64],
        }];
        let ra = compute_transactions_root(&a);
        let rb = compute_transactions_root(&b);
        assert_ne!(ra, rb);
    }
}
