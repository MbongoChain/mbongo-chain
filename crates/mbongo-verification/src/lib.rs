//! Compute verification primitives for Mbongo Chain.
//!
//! Currently implemented:
//! - Receipt anchoring ([`receipt`]) per `docs/specs/RECEIPT_SPEC_v0.1.md`:
//!   canonical receipt structure, SCALE encoding, BLAKE3 receipt hash,
//!   Ed25519 signature verification, and v1 minimal validation rules.
//!
//! Future phases (not implemented):
//! - Redundant execution, TEE attestation, ZK-ML proofs, fraud proofs.

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod receipt;

pub use receipt::{decode_and_validate, Receipt, ReceiptError, ReceiptIndex, RECEIPT_VERSION};
