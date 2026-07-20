//! Canonical receipt data type per `docs/specs/RECEIPT_SPEC_v0.1.md`,
//! relocated from `mbongo-verification` under RFC 0002 §6.1–6.2.
//!
//! This module owns the receipt *data definition only*: the struct, its
//! canonical SCALE encoding, the signing payload, and the receipt hash.
//! All validation judgment — signature verification, version rules,
//! duplicate detection — lives in `mbongo-verification`. Core defines
//! what a receipt is, never whether one is acceptable.

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::crypto::blake3_hash;
use crate::primitives::{serde_arr64, Address, Hash};

/// Canonical receipt structure (`RECEIPT_SPEC_v0.1` Section 2).
///
/// Field order is fixed and matches the spec's canonical SCALE encoding
/// order: `version`, `task_id`, `input_commitment`, `output_commitment`,
/// `executor`, `metadata`, `signature`. Adding, removing, or reordering
/// fields is a breaking change.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct Receipt {
    /// Protocol version. Must be 1 for this spec.
    pub version: u8,
    /// Unique task identifier. Opaque to the chain.
    pub task_id: [u8; 32],
    /// Commitment to the input (e.g. BLAKE3 of input). Opaque to the chain.
    pub input_commitment: [u8; 32],
    /// Commitment to the output (e.g. BLAKE3 of output). Opaque to the chain.
    pub output_commitment: [u8; 32],
    /// Ed25519 public key of the executor.
    pub executor: Address,
    /// Opaque metadata. Never interpreted by the protocol.
    pub metadata: Vec<u8>,
    /// Ed25519 signature over the raw 32-byte receipt hash.
    #[serde(with = "serde_arr64")]
    pub signature: [u8; 64],
}

impl Receipt {
    /// Returns the SCALE-encoded signing payload: all fields except
    /// `signature`, in canonical order.
    #[must_use]
    pub fn signing_payload(&self) -> Vec<u8> {
        #[derive(Encode)]
        struct Payload<'a> {
            version: u8,
            task_id: &'a [u8; 32],
            input_commitment: &'a [u8; 32],
            output_commitment: &'a [u8; 32],
            executor: &'a Address,
            metadata: &'a Vec<u8>,
        }
        Payload {
            version: self.version,
            task_id: &self.task_id,
            input_commitment: &self.input_commitment,
            output_commitment: &self.output_commitment,
            executor: &self.executor,
            metadata: &self.metadata,
        }
        .encode()
    }

    /// Computes the receipt hash: `BLAKE3(SCALE_encode(signing payload))`
    /// (`RECEIPT_SPEC_v0.1` Section 4).
    ///
    /// The returned [`Hash`] displays as `0x` + 64 lowercase hex characters;
    /// signing and verification use its raw 32 bytes, never the hex string.
    #[must_use]
    pub fn receipt_hash(&self) -> Hash {
        Hash(blake3_hash(&self.signing_payload()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hash_to_hex;

    /// The fixed executor key bytes used by the canonical test vector:
    /// the Ed25519 public key derived from seed `[42u8; 32]`.
    fn vector_executor() -> Address {
        use ed25519_dalek::SigningKey;
        Address(SigningKey::from_bytes(&[42u8; 32]).verifying_key().to_bytes())
    }

    /// The receipt used by the canonical byte/hash vectors. Field values
    /// must never change: they pin the encoding across the RFC 0002 §6.2
    /// relocation from mbongo-verification.
    fn vector_receipt() -> Receipt {
        Receipt {
            version: 1,
            task_id: [1u8; 32],
            input_commitment: [2u8; 32],
            output_commitment: [3u8; 32],
            executor: vector_executor(),
            metadata: vec![0xAA, 0xBB, 0xCC],
            signature: [0u8; 64],
        }
    }

    #[test]
    fn scale_roundtrip() {
        let receipt = vector_receipt();
        let encoded = receipt.encode();
        let decoded = Receipt::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded, receipt);
    }

    #[test]
    fn encoded_field_order_is_canonical() {
        let receipt = vector_receipt();
        let encoded = receipt.encode();

        // Manually assemble the canonical encoding: version, task_id,
        // input_commitment, output_commitment, executor, metadata
        // (compact length prefix + bytes), signature.
        let mut expected = Vec::new();
        expected.push(receipt.version);
        expected.extend_from_slice(&receipt.task_id);
        expected.extend_from_slice(&receipt.input_commitment);
        expected.extend_from_slice(&receipt.output_commitment);
        expected.extend_from_slice(&receipt.executor.0);
        // SCALE compact encoding of len 3 is (3 << 2) = 0x0C.
        expected.push(0x0C);
        expected.extend_from_slice(&receipt.metadata);
        expected.extend_from_slice(&receipt.signature);

        assert_eq!(encoded, expected);
    }

    #[test]
    fn receipt_hash_is_deterministic() {
        let receipt = vector_receipt();
        assert_eq!(receipt.receipt_hash(), receipt.receipt_hash());
        assert_eq!(receipt.clone().receipt_hash(), receipt.receipt_hash());
    }

    #[test]
    fn receipt_hash_fixed_test_vector() {
        // Fixed vector carried over byte-for-byte from the original
        // mbongo-verification implementation (RFC 0002 §6.2 requirement 2):
        // any change to encoding order or hash inputs breaks this test.
        let receipt = vector_receipt();

        // Cross-check: the hash must equal BLAKE3 over the manually
        // assembled signing payload (all fields except signature).
        let mut payload = Vec::new();
        payload.push(receipt.version);
        payload.extend_from_slice(&receipt.task_id);
        payload.extend_from_slice(&receipt.input_commitment);
        payload.extend_from_slice(&receipt.output_commitment);
        payload.extend_from_slice(&receipt.executor.0);
        payload.push(0x0C); // SCALE compact length prefix for 3 bytes
        payload.extend_from_slice(&receipt.metadata);
        assert_eq!(receipt.receipt_hash().0, blake3_hash(&payload));

        assert_eq!(
            hash_to_hex(&receipt.receipt_hash().0),
            "56510bc65a92b2655cbeba66b4c219705862d431181a244b0ce37ca04322a0f1"
        );
    }

    #[test]
    fn signature_excluded_from_receipt_hash() {
        let receipt = vector_receipt();
        let mut resigned = receipt.clone();
        resigned.signature = [0xFFu8; 64];
        assert_eq!(receipt.receipt_hash(), resigned.receipt_hash());
    }
}
