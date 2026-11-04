//! The provider side of `mbongo-job:v1`.
//!
//! Temporary scaffolding: an in-process Rust trait. Independently operated
//! providers will sit behind a process or network boundary, designed and
//! reviewed at Slice 2. A provider must never need to recompile this crate
//! for a client's workload, so nothing here is workload-specific.

use std::collections::BTreeMap;

use super::spec::JobSpec;

/// What a runtime states about itself. Self-declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIdentity {
    /// Its family.
    pub family: String,
    /// Its exact revision.
    pub revision: String,
    /// The hash of its build, if it has one.
    pub binary_sha256: Option<String>,
    /// `cpu`, or a provider-defined accelerator class.
    pub accelerator_class: String,
}

/// A job ready to run: its spec and its verified artifacts, by role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedJob {
    /// The job-spec digest.
    pub digest: [u8; 32],
    /// The job spec.
    pub spec: JobSpec,
    /// Artifact bytes, already verified by the worker.
    pub artifacts: BTreeMap<String, Vec<u8>>,
}

/// Resource use as the runtime reports it. **Untrusted**, and private: it
/// goes to a [`MeteringSink`], never into a receipt or the chain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UntrustedMetering {
    /// Wall time.
    pub wall_ms: Option<u64>,
    /// CPU time.
    pub cpu_ms: Option<u64>,
    /// Peak memory.
    pub peak_memory_mib: Option<u64>,
}

/// What a runtime returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    /// The output bytes, not yet checked.
    pub raw: Vec<u8>,
    /// Its reported resource use.
    pub metering: UntrustedMetering,
}

/// A runtime could not produce an output. It carries nothing, so a provider
/// cannot put job content in an error, or shift the blame to the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeFailed;

/// Implemented by providers. This crate ships no production implementation.
pub trait RuntimeAdapter: Send + Sync {
    /// What this runtime states about itself.
    fn identity(&self) -> RuntimeIdentity;
    /// Runs one job on verified artifacts.
    fn run(&self, job: &ResolvedJob, input: &[u8]) -> Result<RunOutput, RuntimeFailed>;
}

/// Where a provider keeps job specs and artifacts.
pub trait JobSource: Send + Sync {
    /// The digests of the job specs this worker offers to run.
    fn job_spec_digests(&self) -> Vec<[u8; 32]>;
    /// The job spec's bytes, if held.
    fn job_spec(&self, digest: &[u8; 32]) -> Option<Vec<u8>>;
    /// An artifact's bytes, if held.
    fn artifact(&self, sha256: &str) -> Option<Vec<u8>>;
}

/// The private side that receives metering (D14).
pub trait MeteringSink: Send + Sync {
    /// Records one job's reported resource use.
    fn record(&self, job_digest: &[u8; 32], metering: &UntrustedMetering);
}
