//! Receipt anchoring primitive per `docs/specs/RECEIPT_SPEC_v0.1.md`.
//!
//! Implements the canonical receipt structure, SCALE encoding, BLAKE3
//! receipt hash, Ed25519 signature verification, and the v1 minimal
//! validation rules (version, signature, duplicate `task_id`).
//!
//! This module is a pure library: it performs no I/O and holds no chain
//! state. Duplicate detection is abstracted behind the read-only
//! [`ReceiptIndex`] trait; persistent storage integration is a separate,
//! later step. Validation never mutates the index.

#![allow(clippy::module_name_repetitions)]

use mbongo_core::crypto::blake3_hash;
use mbongo_core::{Address, Hash};
use parity_scale_codec::{Decode, Encode};

/// The only receipt version accepted by this spec revision.
pub const RECEIPT_VERSION: u8 = 1;

/// Canonical receipt structure (`RECEIPT_SPEC_v0.1` Section 2).
///
/// Field order is fixed and matches the spec's canonical SCALE encoding
/// order: `version`, `task_id`, `input_commitment`, `output_commitment`,
/// `executor`, `metadata`, `signature`. Adding, removing, or reordering
/// fields is a breaking change.
#[derive(Clone, PartialEq, Eq, Debug, Encode, Decode)]
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

    /// Verifies that `signature` is a valid Ed25519 signature over the raw
    /// 32-byte receipt hash by `executor`.
    #[must_use]
    pub fn verify_signature(&self) -> bool {
        use ed25519_dalek::{Signature, Verifier};
        let Ok(pk) = ed25519_dalek::VerifyingKey::from_bytes(&self.executor.0) else {
            return false;
        };
        let sig = Signature::from_bytes(&self.signature);
        pk.verify(&self.receipt_hash().0, &sig).is_ok()
    }

    /// Validates the receipt against the v1 minimal rules
    /// (`RECEIPT_SPEC_v0.1` Section 5): version, signature, and duplicate
    /// `task_id`. Read-only: the index is never mutated; anchoring
    /// (insertion) belongs to a later storage integration step.
    ///
    /// # Errors
    ///
    /// - [`ReceiptError::UnsupportedVersion`] if `version != 1`.
    /// - [`ReceiptError::InvalidSignature`] if the signature does not verify.
    /// - [`ReceiptError::DuplicateTaskId`] if `task_id` is already anchored.
    /// - [`ReceiptError::Index`] if the index lookup fails.
    pub fn validate(&self, index: &impl ReceiptIndex) -> Result<(), ReceiptError> {
        if self.version != RECEIPT_VERSION {
            return Err(ReceiptError::UnsupportedVersion(self.version));
        }
        if !self.verify_signature() {
            return Err(ReceiptError::InvalidSignature);
        }
        if index.contains_task_id(&self.task_id)? {
            return Err(ReceiptError::DuplicateTaskId);
        }
        Ok(())
    }
}

/// Decodes a receipt from SCALE bytes, then validates it.
///
/// Decoding and semantic validation are separate boundaries: malformed
/// bytes fail with [`ReceiptError::Decode`] before any rule is checked.
///
/// # Errors
///
/// [`ReceiptError::Decode`] if the bytes are not a valid SCALE-encoded
/// receipt; otherwise any error from [`Receipt::validate`].
pub fn decode_and_validate(
    bytes: &[u8],
    index: &impl ReceiptIndex,
) -> Result<Receipt, ReceiptError> {
    let receipt = Receipt::decode(&mut &bytes[..]).map_err(ReceiptError::Decode)?;
    receipt.validate(index)?;
    Ok(receipt)
}

/// Errors produced by receipt validation and decoding.
#[derive(Debug, thiserror::Error)]
pub enum ReceiptError {
    /// `version` is not the supported receipt version.
    #[error("unsupported receipt version: {0}, expected {RECEIPT_VERSION}")]
    UnsupportedVersion(u8),
    /// Ed25519 signature verification over the receipt hash failed.
    #[error("invalid receipt signature")]
    InvalidSignature,
    /// A receipt with the same `task_id` has already been anchored.
    #[error("duplicate task_id")]
    DuplicateTaskId,
    /// The bytes do not decode as a SCALE-encoded receipt.
    #[error("receipt decode error: {0}")]
    Decode(parity_scale_codec::Error),
    /// The receipt index lookup failed.
    #[error("receipt index error: {0}")]
    Index(String),
}

/// Read-only lookup of anchored `task_id`s for duplicate detection
/// (`RECEIPT_SPEC_v0.1` Section 6).
pub trait ReceiptIndex {
    /// Returns whether a receipt with `task_id` has already been anchored.
    ///
    /// # Errors
    ///
    /// [`ReceiptError::Index`] if the underlying lookup fails.
    fn contains_task_id(&self, task_id: &[u8; 32]) -> Result<bool, ReceiptError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use mbongo_core::crypto::hash_to_hex;

    /// In-memory receipt index, test-only. Not a persistence layer;
    /// storage-backed indexing is a separate, later integration step.
    #[derive(Debug, Default)]
    struct InMemoryReceiptIndex {
        anchored: std::collections::HashSet<[u8; 32]>,
    }

    impl InMemoryReceiptIndex {
        fn new() -> Self {
            Self::default()
        }

        /// Records `task_id` as anchored. Setup helper; never called by
        /// [`Receipt::validate`].
        fn insert_task_id(&mut self, task_id: [u8; 32]) {
            self.anchored.insert(task_id);
        }
    }

    impl ReceiptIndex for InMemoryReceiptIndex {
        fn contains_task_id(&self, task_id: &[u8; 32]) -> Result<bool, ReceiptError> {
            Ok(self.anchored.contains(task_id))
        }
    }

    /// Deterministic executor key for tests.
    fn test_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[42u8; 32])
    }

    /// Builds an unsigned receipt with fixed field values.
    fn unsigned_receipt(executor: Address) -> Receipt {
        Receipt {
            version: RECEIPT_VERSION,
            task_id: [1u8; 32],
            input_commitment: [2u8; 32],
            output_commitment: [3u8; 32],
            executor,
            metadata: vec![0xAA, 0xBB, 0xCC],
            signature: [0u8; 64],
        }
    }

    /// Builds a receipt signed by the given key over the raw receipt hash.
    fn signed_receipt(sk: &SigningKey) -> Receipt {
        let mut receipt = unsigned_receipt(Address(sk.verifying_key().to_bytes()));
        receipt.signature = sk.sign(&receipt.receipt_hash().0).to_bytes();
        receipt
    }

    #[test]
    fn scale_roundtrip() {
        let receipt = signed_receipt(&test_signing_key());
        let encoded = receipt.encode();
        let decoded = Receipt::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded, receipt);
    }

    #[test]
    fn encoded_field_order_is_canonical() {
        let receipt = signed_receipt(&test_signing_key());
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
        let receipt = signed_receipt(&test_signing_key());
        assert_eq!(receipt.receipt_hash(), receipt.receipt_hash());
        assert_eq!(receipt.clone().receipt_hash(), receipt.receipt_hash());
    }

    #[test]
    fn receipt_hash_fixed_test_vector() {
        // Fixed vector: any change to encoding order or hash inputs breaks
        // this test. Fields are the unsigned_receipt constants with the
        // executor derived from seed [42u8; 32].
        let receipt = unsigned_receipt(Address(test_signing_key().verifying_key().to_bytes()));

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
    fn valid_signature_passes_validation() {
        let receipt = signed_receipt(&test_signing_key());
        let index = InMemoryReceiptIndex::new();
        assert!(receipt.validate(&index).is_ok());
    }

    #[test]
    fn tampered_field_rejected() {
        let mut receipt = signed_receipt(&test_signing_key());
        receipt.output_commitment = [9u8; 32];
        let index = InMemoryReceiptIndex::new();
        assert!(matches!(
            receipt.validate(&index),
            Err(ReceiptError::InvalidSignature)
        ));
    }

    #[test]
    fn wrong_executor_rejected() {
        let mut receipt = signed_receipt(&test_signing_key());
        // Signature is valid for the original executor; swap the executor.
        let other = SigningKey::from_bytes(&[7u8; 32]);
        receipt.executor = Address(other.verifying_key().to_bytes());
        let index = InMemoryReceiptIndex::new();
        assert!(matches!(
            receipt.validate(&index),
            Err(ReceiptError::InvalidSignature)
        ));
    }

    #[test]
    fn wrong_version_rejected() {
        let sk = test_signing_key();
        let mut receipt = unsigned_receipt(Address(sk.verifying_key().to_bytes()));
        receipt.version = 2;
        // Sign correctly over the version-2 hash so the version rule, not
        // the signature rule, is what rejects it.
        receipt.signature = sk.sign(&receipt.receipt_hash().0).to_bytes();
        let index = InMemoryReceiptIndex::new();
        assert!(matches!(
            receipt.validate(&index),
            Err(ReceiptError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn duplicate_task_id_rejected() {
        let receipt = signed_receipt(&test_signing_key());
        let mut index = InMemoryReceiptIndex::new();
        assert!(receipt.validate(&index).is_ok());
        index.insert_task_id(receipt.task_id);
        assert!(matches!(
            receipt.validate(&index),
            Err(ReceiptError::DuplicateTaskId)
        ));
    }

    #[test]
    fn signature_excluded_from_receipt_hash() {
        let receipt = signed_receipt(&test_signing_key());
        let mut resigned = receipt.clone();
        resigned.signature = [0xFFu8; 64];
        assert_eq!(receipt.receipt_hash(), resigned.receipt_hash());
    }

    #[test]
    fn malformed_scale_bytes_rejected_at_decode_boundary() {
        let index = InMemoryReceiptIndex::new();

        // Truncated encoding.
        let receipt = signed_receipt(&test_signing_key());
        let encoded = receipt.encode();
        let truncated = &encoded[..encoded.len() - 1];
        assert!(Receipt::decode(&mut &truncated[..]).is_err());
        assert!(matches!(
            decode_and_validate(truncated, &index),
            Err(ReceiptError::Decode(_))
        ));

        // Garbage bytes.
        assert!(matches!(
            decode_and_validate(&[0xFFu8; 4], &index),
            Err(ReceiptError::Decode(_))
        ));

        // Empty input.
        assert!(matches!(
            decode_and_validate(&[], &index),
            Err(ReceiptError::Decode(_))
        ));
    }
}
