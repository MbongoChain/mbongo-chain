//! Canonical compute task envelope per `docs/rfcs/0005-compute-task-commitment-v1.md`
//! (RFC 0005, Accepted).
//!
//! This module owns the task *data definition only*: the struct, its
//! canonical SCALE encoding, the identity preimage, and `task_id`
//! derivation. Every validation judgment — rules (k)–(p) of RFC 0005 §3 —
//! lives in the node's consensus layer. Core defines what a task is, never
//! whether one is acceptable, exactly as it does for [`crate::Receipt`].

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::crypto::blake3_hash;
use crate::primitives::Address;

/// The only envelope version defined by RFC 0005 (§2.1). Rule (m).
pub const COMPUTE_TASK_VERSION: u8 = 1;

/// Domain separator prepended raw to the canonical task bytes before
/// hashing (RFC 0005 §2.2): the literal ASCII bytes
/// `mbongo:compute-task:v1`, 22 bytes, **no NUL terminator and no SCALE
/// length prefix**. Concatenated as bytes, never encoded as a `Vec<u8>`.
pub const DOMAIN_TASK: &[u8; 22] = b"mbongo:compute-task:v1";

/// Maximum `execution_spec` length in bytes (RFC 0005 §2.10). Rule (n).
///
/// `execution_spec` is the only variable-length field, so this bounds the
/// whole envelope. It is a protocol safety bound, not an optimum; raising
/// it is a protocol version bump.
pub const MAX_EXECUTION_SPEC_BYTES: usize = 1024;

/// Length of the canonical SCALE encoding of a task whose `execution_spec`
/// is exactly [`MAX_EXECUTION_SPEC_BYTES`] long (RFC 0005 §2.10):
/// `1 + 32 + 32 + 32 + 32 + 2 (compact prefix of 1024) + 1024`.
pub const MAX_COMPUTE_TASK_BYTES: usize = 1155;

/// Length of the longest `task_id` preimage a rule-(n)-conforming task can
/// produce (RFC 0005 §2.10): [`DOMAIN_TASK`] plus [`MAX_COMPUTE_TASK_BYTES`].
pub const MAX_TASK_ID_PREIMAGE_BYTES: usize = 1177;

/// Canonical compute task envelope (RFC 0005 §2.1).
///
/// Six fields in this fixed order. `task_id` is **not** a field — it is
/// derived by [`ComputeTask::task_id`] and therefore never appears in its
/// own preimage. Adding, removing or reordering fields is a protocol
/// change.
///
/// The envelope carries no signature of its own: it is carried by an
/// ordinary signed [`crate::Transaction`], whose signature authenticates
/// the submission (RFC 0005 §2.5). Consensus requires `submitter` to equal
/// the carrying transaction's sender (rule o), so the field carries no
/// independent authority; it is kept so the stored task is self-describing
/// and task identity is per-client (§2.6, §2.11).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct ComputeTask {
    /// Protocol version of this envelope. Must be [`COMPUTE_TASK_VERSION`].
    pub version: u8,
    /// The account committing the task. Must equal the carrying
    /// transaction's sender (rule o).
    pub submitter: Address,
    /// The one executor authorised to answer this task. Named by the
    /// client before commitment; a different executor is a different task
    /// (§2.6). Nothing in the protocol reassigns it.
    pub executor: Address,
    /// Client-chosen opaque uniqueness value. Need not be random; a zero
    /// salt is legal (§2.6). Deliberately not the transaction nonce.
    pub salt: [u8; 32],
    /// Commitment to the input data. The data itself is off-chain, and
    /// consensus never learns how the commitment was derived (§2.4).
    pub input_commitment: [u8; 32],
    /// Opaque, bounded description of what was requested. The chain
    /// commits to it, stores it and hashes it into `task_id`; it never
    /// interprets it (§2.12).
    pub execution_spec: Vec<u8>,
}

impl ComputeTask {
    /// Returns the `task_id` preimage: [`DOMAIN_TASK`] followed by the
    /// canonical SCALE encoding of the six fields (RFC 0005 §2.2).
    ///
    /// The tag is prepended as raw bytes. Its length is bounded by
    /// [`MAX_TASK_ID_PREIMAGE_BYTES`] whenever rule (n) holds; callers that
    /// enforce consensus should check the bound before hashing.
    #[must_use]
    pub fn task_id_preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::with_capacity(DOMAIN_TASK.len() + self.size_hint());
        preimage.extend_from_slice(DOMAIN_TASK);
        self.encode_to(&mut preimage);
        preimage
    }

    /// Derives the task identity:
    /// `BLAKE3(DOMAIN_TASK || SCALE(ComputeTask))` (RFC 0005 §2.2).
    ///
    /// Content-derived over all six fields: changing any of them — the
    /// executor included — yields a different task. The carrying
    /// transaction's nonce is not part of the envelope, so a resubmission
    /// after a nonce race keeps its identity (§2.6). The hash is over raw
    /// bytes, never over a hexadecimal rendering.
    #[must_use]
    pub fn task_id(&self) -> [u8; 32] {
        blake3_hash(&self.task_id_preimage())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json as json;

    fn sample() -> ComputeTask {
        ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: Address([0x11u8; 32]),
            executor: Address([0x22u8; 32]),
            salt: [0x33u8; 32],
            input_commitment: [0x44u8; 32],
            execution_spec: vec![0xAA, 0xBB, 0xCC],
        }
    }

    /// Hand-assembled canonical bytes: the six fields concatenated in
    /// declaration order, `execution_spec` behind a SCALE compact prefix.
    fn manual_encoding(task: &ComputeTask, compact_prefix: &[u8]) -> Vec<u8> {
        let mut out = vec![task.version];
        out.extend_from_slice(&task.submitter.0);
        out.extend_from_slice(&task.executor.0);
        out.extend_from_slice(&task.salt);
        out.extend_from_slice(&task.input_commitment);
        out.extend_from_slice(compact_prefix);
        out.extend_from_slice(&task.execution_spec);
        out
    }

    #[test]
    fn domain_tag_is_the_literal_22_ascii_bytes() {
        assert_eq!(DOMAIN_TASK.len(), 22);
        assert_eq!(
            hex::encode(DOMAIN_TASK),
            "6d626f6e676f3a636f6d707574652d7461736b3a7631"
        );
        assert!(
            !DOMAIN_TASK.contains(&0),
            "the tag carries no NUL terminator"
        );
    }

    #[test]
    fn encoded_field_order_is_canonical() {
        let task = sample();
        // compact(3) = 3 << 2 = 0x0C.
        assert_eq!(task.encode(), manual_encoding(&task, &[0x0C]));
    }

    #[test]
    fn empty_execution_spec_has_one_byte_prefix() {
        let task = ComputeTask {
            execution_spec: Vec::new(),
            ..sample()
        };
        assert_eq!(task.encode(), manual_encoding(&task, &[0x00]));
        assert_eq!(task.encode().len(), 1 + 32 * 4 + 1);
    }

    #[test]
    fn compact_prefix_widens_at_64_bytes() {
        let task = ComputeTask {
            execution_spec: vec![0x5A; 63],
            ..sample()
        };
        assert_eq!(task.encode(), manual_encoding(&task, &[0xFC]));

        let task = ComputeTask {
            execution_spec: vec![0x5A; 64],
            ..sample()
        };
        // compact(64) = (64 << 2) | 0b01 = 0x0101, little-endian.
        assert_eq!(task.encode(), manual_encoding(&task, &[0x01, 0x01]));
    }

    #[test]
    fn maximal_task_matches_rfc_sizes() {
        let task = ComputeTask {
            execution_spec: vec![0x5A; MAX_EXECUTION_SPEC_BYTES],
            ..sample()
        };
        // compact(1024) = (1024 << 2) | 0b01 = 0x1001, little-endian.
        let encoded = task.encode();
        assert_eq!(encoded, manual_encoding(&task, &[0x01, 0x10]));
        assert_eq!(encoded.len(), MAX_COMPUTE_TASK_BYTES);
        assert_eq!(task.task_id_preimage().len(), MAX_TASK_ID_PREIMAGE_BYTES);
        assert_eq!(
            MAX_TASK_ID_PREIMAGE_BYTES,
            DOMAIN_TASK.len() + MAX_COMPUTE_TASK_BYTES
        );
    }

    #[test]
    fn scale_roundtrip() {
        for spec_len in [0usize, 1, 63, 64, MAX_EXECUTION_SPEC_BYTES] {
            let task = ComputeTask {
                execution_spec: vec![0x5A; spec_len],
                ..sample()
            };
            let encoded = task.encode();
            let decoded = ComputeTask::decode(&mut &encoded[..]).unwrap();
            assert_eq!(decoded, task);
        }
    }

    #[test]
    fn truncated_encodings_fail_to_decode() {
        let encoded = sample().encode();
        for cut in [0, 1, 32, 129, encoded.len() - 1] {
            assert!(
                ComputeTask::decode(&mut &encoded[..cut]).is_err(),
                "a {cut}-byte prefix must not decode"
            );
        }
    }

    #[test]
    fn declared_spec_length_beyond_input_fails_to_decode() {
        // Fixed fields, then a compact prefix claiming 1024 bytes with
        // only three present.
        let mut bytes = manual_encoding(&sample(), &[0x01, 0x10]);
        bytes.truncate(1 + 32 * 4 + 2 + 3);
        assert!(ComputeTask::decode(&mut &bytes[..]).is_err());
    }

    #[test]
    fn task_id_preimage_starts_with_raw_domain_tag() {
        let task = sample();
        let preimage = task.task_id_preimage();
        assert_eq!(&preimage[..22], DOMAIN_TASK);
        assert_eq!(&preimage[22..], task.encode().as_slice());
        // The tag is raw: no compact length byte (0x58 for 22) and no NUL
        // sits between it and the version byte.
        assert_eq!(preimage[22], COMPUTE_TASK_VERSION);
    }

    #[test]
    fn task_id_is_blake3_of_the_preimage_and_deterministic() {
        let task = sample();
        assert_eq!(task.task_id(), blake3_hash(&task.task_id_preimage()));
        assert_eq!(task.task_id(), task.clone().task_id());
        assert_eq!(
            task.task_id(),
            *blake3::hash(&[DOMAIN_TASK.as_slice(), &task.encode()].concat()).as_bytes()
        );
    }

    #[test]
    fn task_id_commits_to_every_field() {
        let base = sample();
        let variants = [
            ComputeTask {
                version: 2,
                ..base.clone()
            },
            ComputeTask {
                submitter: Address([0x12u8; 32]),
                ..base.clone()
            },
            ComputeTask {
                executor: Address([0x23u8; 32]),
                ..base.clone()
            },
            ComputeTask {
                salt: [0x34u8; 32],
                ..base.clone()
            },
            ComputeTask {
                input_commitment: [0x45u8; 32],
                ..base.clone()
            },
            ComputeTask {
                execution_spec: vec![0xAA, 0xBB, 0xCD],
                ..base.clone()
            },
        ];
        let ids: Vec<[u8; 32]> = variants.iter().map(ComputeTask::task_id).collect();
        for (i, id) in ids.iter().enumerate() {
            assert_ne!(*id, base.task_id(), "variant {i} must change task_id");
            for (j, other) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(id, other, "variants {i} and {j} must not collide");
                }
            }
        }
    }

    #[test]
    fn identical_envelopes_share_a_task_id() {
        let a = sample();
        let b = ComputeTask {
            execution_spec: a.execution_spec.clone(),
            ..a.clone()
        };
        assert_eq!(a.task_id(), b.task_id());
    }

    #[test]
    fn a_wrongly_tagged_preimage_produces_a_different_id() {
        // Three mistakes the fixture exists to catch: SCALE-encoding the
        // tag (compact length prefix 0x58), appending a NUL, omitting it.
        let task = sample();
        let body = task.encode();
        let scale_tagged = [&[0x58u8][..], DOMAIN_TASK, &body].concat();
        let nul_tagged = [DOMAIN_TASK, &[0u8][..], &body].concat();
        for wrong in [scale_tagged, nul_tagged, body] {
            assert_ne!(task.task_id(), blake3_hash(&wrong));
        }
    }

    #[test]
    fn serde_shape_matches_receipt_conventions() {
        // Address fields are 0x hex strings (custom serializer); plain
        // byte arrays fall through to serde's sequence handling, as the
        // receipt's do (test-vectors/transaction/README.md).
        let v = json::to_value(sample()).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(
            v["submitter"],
            json::json!(format!("0x{}", "11".repeat(32)))
        );
        assert_eq!(v["executor"], json::json!(format!("0x{}", "22".repeat(32))));
        assert_eq!(v["salt"], json::json!(vec![0x33u8; 32]));
        assert_eq!(v["input_commitment"], json::json!(vec![0x44u8; 32]));
        assert_eq!(v["execution_spec"], json::json!([0xAA, 0xBB, 0xCC]));
        let back: ComputeTask = json::from_value(v).unwrap();
        assert_eq!(back, sample());
    }
}
