//! The `mbongo-infer:v1` `execution_spec` layout (MVP-A §5).
//!
//! `ComputeTask.execution_spec` is opaque, bounded at 1,024 bytes and
//! application-versioned by convention (RFC 0005 §2.10, §2.12). The
//! inference convention is a **fixed 47-byte** value:
//!
//! ```text
//! offset  len  content
//! 0       15   ASCII tag "mbongo-infer:v1"   (the version lives in the tag)
//! 15      32   job_spec_digest              (BLAKE3, see job_spec::digest)
//! ```
//!
//! No length prefix is needed because every field is fixed-size; no
//! separator is needed because the tag is a fixed prefix. Any other length,
//! any other prefix and any other version are rejected before anything else
//! happens (worker validation step 1). The chain enforces none of this and
//! never sees the job spec itself.

use super::ContractError;

/// The tag that selects the inference contract, version 1.
pub const INFER_TAG_V1: &[u8] = b"mbongo-infer:v1";
/// The family prefix shared by every version of the inference contract.
pub const INFER_PREFIX: &[u8] = b"mbongo-infer:";
/// Length of the job-spec digest.
pub const DIGEST_LEN: usize = 32;
/// Exact length of a version-1 inference `execution_spec`.
pub const EXECUTION_SPEC_LEN: usize = 15 + DIGEST_LEN;

/// A decoded version-1 inference `execution_spec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InferenceExecutionSpec {
    /// Commitment to the off-chain job spec (`job_spec::digest`).
    pub job_spec_digest: [u8; DIGEST_LEN],
}

impl InferenceExecutionSpec {
    /// The 47 bytes to place in `ComputeTask.execution_spec`.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(EXECUTION_SPEC_LEN);
        out.extend_from_slice(INFER_TAG_V1);
        out.extend_from_slice(&self.job_spec_digest);
        out
    }

    /// Parses `execution_spec` bytes. Order of checks: family prefix, then
    /// version, then exact length.
    pub fn decode(bytes: &[u8]) -> Result<Self, ContractError> {
        if !bytes.starts_with(INFER_PREFIX) {
            return Err(ContractError::MalformedExecutionSpec(
                "not an mbongo-infer execution_spec",
            ));
        }
        if !bytes.starts_with(INFER_TAG_V1) {
            return Err(ContractError::UnsupportedInferenceSpecVersion);
        }
        if bytes.len() != EXECUTION_SPEC_LEN {
            return Err(ContractError::MalformedExecutionSpec(
                "mbongo-infer:v1 execution_spec must be exactly 47 bytes",
            ));
        }
        let mut job_spec_digest = [0u8; DIGEST_LEN];
        job_spec_digest.copy_from_slice(&bytes[INFER_TAG_V1.len()..]);
        Ok(Self { job_spec_digest })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_47_bytes_tag_then_digest() {
        assert_eq!(INFER_TAG_V1.len(), 15);
        assert_eq!(EXECUTION_SPEC_LEN, 47);
        let spec = InferenceExecutionSpec {
            job_spec_digest: [0xAB; 32],
        };
        let bytes = spec.encode();
        assert_eq!(bytes.len(), 47);
        assert_eq!(&bytes[..15], b"mbongo-infer:v1");
        assert_eq!(&bytes[15..], &[0xAB; 32]);
        assert_eq!(InferenceExecutionSpec::decode(&bytes).unwrap(), spec);
    }
}
