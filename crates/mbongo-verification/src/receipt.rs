//! Receipt validation per `docs/specs/RECEIPT_SPEC_v0.1.md`.
//!
//! The canonical [`Receipt`] data type — struct, SCALE encoding,
//! `signing_payload()`, `receipt_hash()` — lives in `mbongo-core`
//! (relocated under RFC 0002 §6.1–6.2) and is re-exported here for
//! compatibility. This module owns all validation judgment: version
//! rules, Ed25519 receipt-signature verification, and duplicate
//! detection through the read-only [`ReceiptIndex`] port.
//!
//! This module is a pure library: it performs no I/O and holds no chain
//! state. Persistent duplicate-index composition happens in the node
//! layer; validation never mutates an index.

#![allow(clippy::module_name_repetitions)]

pub use mbongo_core::Receipt;
use parity_scale_codec::Decode;

/// The only receipt version accepted by this spec revision.
pub const RECEIPT_VERSION: u8 = 1;

/// Verifies that `receipt.signature` is a valid Ed25519 signature over
/// the raw 32-byte receipt hash by `receipt.executor`.
///
/// Signature verification is validation, owned by this crate — it does
/// not travel with the data type (RFC 0002 §6.1).
#[must_use]
pub fn verify_receipt_signature(receipt: &Receipt) -> bool {
    use ed25519_dalek::{Signature, Verifier};
    let Ok(pk) = ed25519_dalek::VerifyingKey::from_bytes(&receipt.executor.0) else {
        return false;
    };
    let sig = Signature::from_bytes(&receipt.signature);
    pk.verify(&receipt.receipt_hash().0, &sig).is_ok()
}

/// Validates a receipt against the v1 minimal rules
/// (`RECEIPT_SPEC_v0.1` Section 5): version, signature, and duplicate
/// `task_id`. Read-only: the index is never mutated; anchoring
/// (insertion) belongs to the consensus/storage integration.
///
/// This checks a *standalone* receipt only. Transaction-level anchoring
/// rules (e.g. `sender == executor`) are orchestrated by the node layer
/// and are not part of receipt validity (RFC 0002 §2).
///
/// # Errors
///
/// - [`ReceiptError::UnsupportedVersion`] if `version != 1`.
/// - [`ReceiptError::InvalidSignature`] if the signature does not verify.
/// - [`ReceiptError::DuplicateTaskId`] if `task_id` is already anchored.
/// - [`ReceiptError::Index`] if the index lookup fails.
pub fn validate_receipt(receipt: &Receipt, index: &impl ReceiptIndex) -> Result<(), ReceiptError> {
    if receipt.version != RECEIPT_VERSION {
        return Err(ReceiptError::UnsupportedVersion(receipt.version));
    }
    if !verify_receipt_signature(receipt) {
        return Err(ReceiptError::InvalidSignature);
    }
    if index.contains_task_id(&receipt.task_id)? {
        return Err(ReceiptError::DuplicateTaskId);
    }
    Ok(())
}

/// Decodes a receipt from SCALE bytes, then validates it.
///
/// Decoding and semantic validation are separate boundaries: malformed
/// bytes fail with [`ReceiptError::Decode`] before any rule is checked.
///
/// # Errors
///
/// [`ReceiptError::Decode`] if the bytes are not a valid SCALE-encoded
/// receipt; otherwise any error from [`validate_receipt`].
pub fn decode_and_validate(
    bytes: &[u8],
    index: &impl ReceiptIndex,
) -> Result<Receipt, ReceiptError> {
    let receipt = Receipt::decode(&mut &bytes[..]).map_err(ReceiptError::Decode)?;
    validate_receipt(&receipt, index)?;
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
    use mbongo_core::Address;
    use parity_scale_codec::Encode;

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
        /// [`validate_receipt`].
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
    fn valid_signature_passes_validation() {
        let receipt = signed_receipt(&test_signing_key());
        let index = InMemoryReceiptIndex::new();
        assert!(validate_receipt(&receipt, &index).is_ok());
    }

    #[test]
    fn tampered_field_rejected() {
        let mut receipt = signed_receipt(&test_signing_key());
        receipt.output_commitment = [9u8; 32];
        let index = InMemoryReceiptIndex::new();
        assert!(matches!(
            validate_receipt(&receipt, &index),
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
            validate_receipt(&receipt, &index),
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
            validate_receipt(&receipt, &index),
            Err(ReceiptError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn duplicate_task_id_rejected() {
        let receipt = signed_receipt(&test_signing_key());
        let mut index = InMemoryReceiptIndex::new();
        assert!(validate_receipt(&receipt, &index).is_ok());
        index.insert_task_id(receipt.task_id);
        assert!(matches!(
            validate_receipt(&receipt, &index),
            Err(ReceiptError::DuplicateTaskId)
        ));
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
