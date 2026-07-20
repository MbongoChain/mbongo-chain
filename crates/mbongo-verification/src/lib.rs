//! Compute verification primitives for Mbongo Chain.
//!
//! Currently implemented:
//! - Receipt validation ([`receipt`]) per `docs/specs/RECEIPT_SPEC_v0.1.md`:
//!   Ed25519 receipt-signature verification, version and duplicate rules,
//!   and the read-only [`ReceiptIndex`] port. The canonical `Receipt` data
//!   type lives in `mbongo-core` (RFC 0002 §6.1) and is re-exported here.
//!
//! Future phases (not implemented):
//! - Redundant execution, TEE attestation, ZK-ML proofs, fraud proofs.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod receipt;

pub use receipt::{
    decode_and_validate, validate_receipt, verify_receipt_signature, Receipt, ReceiptError,
    ReceiptIndex, RECEIPT_VERSION,
};
