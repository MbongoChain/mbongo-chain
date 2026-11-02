//! The reference execution profile, and the reference commitment convention.
//!
//! # REFERENCE / TEST EXECUTION PROFILE
//!
//! The only profile this crate runs reverses the input bytes. It exists to
//! prove the lifecycle — fetch, verify, execute, persist, commit, receipt —
//! with a transform whose output is deterministic and checkable by anyone
//! holding the input. It is not, and does not pretend to be, the production
//! compute model. Nothing about it is consensus: the chain treats
//! `execution_spec` as opaque bytes (RFC 0005 §2.12) and never learns what a
//! worker did with them.
//!
//! # Reference-worker implementation conventions (non-consensus)
//!
//! Two conventions let a client and this worker agree without any protocol
//! change:
//!
//! - **profile selection** — `execution_spec` is the ASCII tag
//!   [`REVERSE_BYTES_SPEC`]. The worker refuses any other spec. This is the
//!   application-versioned tagging RFC 0005 §2.12 recommends; the chain
//!   enforces none of it.
//! - **commitments** — the RFC 0005 §2.4 interoperability convention,
//!   `BLAKE3(DOMAIN_INPUT || bytes)` and `BLAKE3(DOMAIN_OUTPUT || bytes)`,
//!   over the raw input and output bytes. The chain compares commitments
//!   for equality and never learns the derivation; a client that blinds its
//!   commitment is equally valid on-chain and would agree a different
//!   convention with its worker (F §7–8). The reference profile uses the
//!   plain form because it is the simplest one to verify in a test.
//!
//! # Plaintext
//!
//! This is **ordinary execution**: the process running the profile holds
//! the input and output in clear. Provider confidentiality is not claimed.
//! The one point where a confidential profile would differ is
//! [`ConfidentialExtension`], which this crate names and does not
//! implement.

/// The `execution_spec` bytes that select the reference profile.
pub const REVERSE_BYTES_SPEC: &[u8] = b"mbongo-ref:reverse-bytes:v1";

/// RFC 0005 §2.4 input-commitment domain (interoperability convention).
pub const DOMAIN_INPUT: &[u8] = b"mbongo:compute-input:v1";
/// RFC 0005 §2.4 output-commitment domain (interoperability convention).
pub const DOMAIN_OUTPUT: &[u8] = b"mbongo:compute-output:v1";

/// A deterministic transform over private bytes.
pub trait ExecutionProfile: Send + Sync {
    /// The `execution_spec` tag this profile answers to.
    fn spec_tag(&self) -> &'static [u8];
    /// Runs the transform. Pure: same input, same output.
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>, ExecutionError>;

    /// Every `execution_spec` value this profile answers to, as the session
    /// advertises them to the control plane (E §6 row 7). Default: exactly
    /// [`Self::spec_tag`], which is what the reference profile has always
    /// advertised.
    fn spec_tags(&self) -> Vec<Vec<u8>> {
        vec![self.spec_tag().to_vec()]
    }

    /// Whether this profile runs a task whose `execution_spec` is
    /// `execution_spec`. Default: byte equality with [`Self::spec_tag`].
    fn accepts(&self, execution_spec: &[u8]) -> bool {
        execution_spec == self.spec_tag()
    }

    /// Runs the profile for one task. Profiles whose execution depends on
    /// the committed `execution_spec` (the MVP inference profile resolves a
    /// job spec from it) override this; the default ignores the task context
    /// and calls [`Self::execute`], so the reference profile is unchanged.
    /// The worker has already verified `input` against `input_commitment`
    /// (F §7) before calling this.
    fn execute_task(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ExecutionError> {
        let _ = (execution_spec, input_commitment);
        self.execute(input)
    }
}

/// The reference profile: output is the input reversed.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReverseBytesProfile;

impl ExecutionProfile for ReverseBytesProfile {
    fn spec_tag(&self) -> &'static [u8] {
        REVERSE_BYTES_SPEC
    }

    fn execute(&self, input: &[u8]) -> Result<Vec<u8>, ExecutionError> {
        let mut out = input.to_vec();
        out.reverse();
        Ok(out)
    }
}

/// Why execution failed. Never carries input or output bytes.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// The task's `execution_spec` selects no profile this worker runs.
    #[error("unsupported execution_spec")]
    UnsupportedSpec,
    /// The profile itself failed.
    #[error("execution failed: {0}")]
    Failed(String),
}

/// The reference input commitment: `BLAKE3(DOMAIN_INPUT || bytes)`.
pub fn reference_input_commitment(bytes: &[u8]) -> [u8; 32] {
    mbongo_core::crypto::blake3_hash_multi(&[DOMAIN_INPUT, bytes])
}

/// The reference output commitment: `BLAKE3(DOMAIN_OUTPUT || bytes)`.
pub fn reference_output_commitment(bytes: &[u8]) -> [u8; 32] {
    mbongo_core::crypto::blake3_hash_multi(&[DOMAIN_OUTPUT, bytes])
}

/// Private bytes that are overwritten when dropped.
///
/// Best effort only: this zeroes the buffer the worker owns. It cannot
/// reach copies the allocator, the OS or a crashed process may have left,
/// and no cryptographic erasure is claimed (F §14).
pub struct Plaintext(Vec<u8>);

impl Plaintext {
    /// Takes ownership of private bytes.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// The bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Drop for Plaintext {
    fn drop(&mut self) {
        for b in &mut self.0 {
            // Volatile write so the compiler cannot elide the overwrite.
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl std::fmt::Debug for Plaintext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Plaintext({} bytes, redacted)", self.0.len())
    }
}

/// **Not implemented.** The single point at which a confidential profile
/// would attach (privacy architecture §12, F §9, E §14).
///
/// A confidential profile changes exactly one step of the lifecycle: the
/// content key for the input is released only to an attested environment,
/// and the output is encrypted inside it. The task envelope, the receipt,
/// the capability model and the lease model are unchanged. This trait
/// records that boundary so that a future implementation has a named place
/// to attach without redesigning the worker; nothing in this crate calls it.
pub trait ConfidentialExtension {
    /// Evidence that the environment about to receive the content key is the
    /// one the client's policy accepts. The reference worker has none.
    fn attestation_evidence(&self) -> Option<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_profile_is_deterministic_and_reversible() {
        let p = ReverseBytesProfile;
        let out = p.execute(b"mbongo").unwrap();
        assert_eq!(out, b"ognobm");
        assert_eq!(p.execute(&out).unwrap(), b"mbongo");
        assert_eq!(p.execute(b"").unwrap(), b"");
    }

    #[test]
    fn commitments_follow_the_rfc_0005_convention() {
        let bytes = b"hello";
        let mut h = blake3::Hasher::new();
        h.update(b"mbongo:compute-input:v1");
        h.update(bytes);
        assert_eq!(reference_input_commitment(bytes), *h.finalize().as_bytes());
        assert_ne!(
            reference_input_commitment(bytes),
            reference_output_commitment(bytes)
        );
    }

    #[test]
    fn plaintext_is_zeroed_on_drop_and_redacted_in_debug() {
        let p = Plaintext::new(vec![1, 2, 3]);
        assert!(!format!("{p:?}").contains("1, 2, 3"));
        let ptr = p.0.as_ptr();
        let len = p.0.len();
        drop(p);
        // The allocation is freed; reading it is undefined, so we only
        // assert the Drop ran through the API contract above.
        assert!(!ptr.is_null() && len == 3);
    }
}
