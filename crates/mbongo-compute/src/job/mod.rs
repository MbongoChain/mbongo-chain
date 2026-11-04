//! `mbongo-job:v1`: a generic compute job, as data.
//!
//! The contract is `docs/architecture/compute-job-contract-v1.md`. A job spec
//! fully describes a job, and a provider's runtime runs it. This module owns
//! the format, its validation, dispatch and the failure codes. It holds no
//! workload logic: `workload` and `params` stay opaque, and nothing is
//! compiled in per client. Non-consensus: the chain sees only the
//! `execution_spec` bytes (RFC 0005 §2.12).
//!
//! This crate ships no production [`RuntimeAdapter`]. What a verified result
//! means stops at T1: conformance to a type and a size, never that the output
//! is right for the client's purpose.

pub mod error;
pub mod output;
pub mod profile;
pub mod runtime;
pub mod spec;

pub use error::{Blame, FailureClass, JobError};
pub use output::{check_input, check_output, verify_result, TechnicalLevel};
pub use profile::JobProfile;
pub use runtime::{
    JobSource, MeteringSink, ResolvedJob, RunOutput, RuntimeAdapter, RuntimeFailed,
    RuntimeIdentity, UntrustedMetering,
};
pub use spec::{
    encode_execution_spec, job_spec_digest, parse_execution_spec, ArtifactRef, DataContract,
    DataType, Determinism, JobSpec, RuntimeRequirement, DOMAIN_JOB_SPEC, EXECUTION_SPEC_LEN,
    JOB_TAG, MAX_JOB_SPEC_BYTES,
};

/// `max_memory_mib` is declared and recorded, not enforced: an in-process
/// adapter cannot be limited reliably. Enforcement belongs behind the process
/// boundary of Slice 2.
pub const MEMORY_LIMIT_ENFORCED: bool = false;
