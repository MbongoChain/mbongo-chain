//! The MVP HTTP front and CLI client (MVP-C).
//!
//! **Non-consensus, application-level.** Implements
//! `docs/product/MVP_HTTP_CLI.md`: a thin loopback HTTP adapter over the
//! existing control-plane (E) and private-data-plane (F) operations, and
//! the client orchestration and client-side verification the `mbongo-mvp`
//! CLI runs. It adds no authority:
//!
//! | Fact | Authority |
//! |---|---|
//! | `task_id`, submitter, executor, `execution_spec`, `input_commitment` | the chain (the committed `ComputeTask`) |
//! | leases, attempts, lifecycle state | the control plane |
//! | private input and result bytes, whether a result exists | the private data plane |
//! | the anchored receipt | the chain |
//!
//! | Module | Concern |
//! |---|---|
//! | [`auth`] | per-request signatures by the client's chain key |
//! | [`dto`] | request and response bodies |
//! | [`front`] | the routes, and the worker loop that drives the MVP-B worker |
//! | [`client`] | the HTTP client and the chain operations the CLI composes |
//! | [`cli`] | the `mbongo-mvp` commands, over any chain client |
//! | [`verify`] | the four independent verification results |
//! | [`screen`] | the injection screen: obvious instruction-following artifacts, reported apart |
//!
//! Ordinary execution, PUBLIC profile: the Mbongo-operated worker sees the
//! CV plaintext. Nothing here is confidential from the provider.

pub mod auth;
pub mod cli;
pub mod client;
pub mod dto;
pub mod front;
pub mod screen;
pub mod verify;

use crate::inference::execution_spec::InferenceExecutionSpec;
use crate::profiles;

/// The front's default bind address: loopback only.
pub const DEFAULT_BIND: &str = "127.0.0.1:8740";

/// Refuses any bind address that is not loopback. The MVP front has no TLS
/// and no network-level access control of its own; reaching it from
/// another host requires an external trusted boundary (VPN or a
/// TLS-terminating reverse proxy), never a wider bind
/// (`docs/product/MVP_LAUNCH_RUNBOOK.md`).
pub fn check_loopback_bind(bind: std::net::SocketAddr) -> Result<(), String> {
    if bind.ip().is_loopback() {
        Ok(())
    } else {
        Err(format!(
            "refusing to bind {bind}: the MVP front binds loopback only"
        ))
    }
}

/// Everything the MVP service holds only in process memory, and therefore
/// loses when it stops (`docs/product/MVP_LAUNCH_RUNBOOK.md` §7). Printed at
/// start-up so no operator has to infer it.
pub const IN_MEMORY_STATE: &[(&str, &str)] = &[
    (
        "uploaded private inputs",
        "LOST: the client must upload and submit a new task",
    ),
    (
        "persisted private results",
        "LOST: cannot be recovered; anchored receipts stay on-chain",
    ),
    (
        "task registrations and input references",
        "LOST: tasks registered before the restart are never executed",
    ),
    (
        "control-plane leases, attempts and parked tasks",
        "LOST: parking and attempt counts reset",
    ),
    (
        "status snapshots and inference timings",
        "LOST: status of earlier tasks answers TASK_NOT_FOUND",
    ),
    (
        "request replay history",
        "LOST: a captured request is replayable only within its 120 s timestamp window",
    ),
    (
        "the front's data-plane owner key and control-plane issuer key",
        "REGENERATED: fresh random keys each start",
    ),
];

/// Largest request body the front reads. A maximal legal input object is
/// 10,000 canonical bytes; JSON escaping can at most sextuple a byte
/// (`\u00XX`), so this bounds every legal upload and nothing much larger.
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Upper bound on a result response: the profile's `max_output_bytes`.
pub const MAX_RESULT_BYTES: usize = crate::inference::kaayu::MAX_OUTPUT_BYTES;

/// The only `execution_spec` the MVP constructs, registers or accepts:
/// `mbongo-infer:v1` over the active profile `kaayu-cv-extract:v3`.
pub fn frozen_execution_spec() -> Vec<u8> {
    InferenceExecutionSpec {
        job_spec_digest: profiles::active_job_spec().digest(),
    }
    .encode()
}

/// Human name of the frozen runtime profile.
pub const RUNTIME_PROFILE: &str = "mbongo-infer:v1 / kaayu-cv-extract:v3";

/// Deterministic CLI exit codes (`docs/product/MVP_HTTP_CLI.md` §14).
pub mod exit {
    /// The operation completed; for `verify`, receipt, result and schema
    /// all verified. Business accuracy is never assessed and never fails.
    pub const SUCCESS: i32 = 0;
    /// Command-line usage error (clap's own code).
    pub const USAGE: i32 = 2;
    /// Local configuration: key file, address, input file.
    pub const CONFIG: i32 = 3;
    /// The front or the chain could not be reached.
    pub const UNAVAILABLE: i32 = 4;
    /// The front or the chain refused the request.
    pub const REJECTED: i32 = 5;
    /// Not finished yet: no result, no anchored receipt, or `--wait` timed out.
    pub const NOT_READY: i32 = 6;
    /// The private result outlived its data-plane lifetime (`410`); the
    /// receipt may still verify.
    pub const RESULT_EXPIRED: i32 = 8;
    /// The task failed and the front stopped retrying it.
    pub const TASK_FAILED: i32 = 7;
    /// Receipt or result integrity failed: `VERIFICATION_FAILED`.
    pub const VERIFICATION_FAILED: i32 = 10;
    /// Integrity verified, schema conformity failed.
    pub const SCHEMA_FAILED: i32 = 11;
    /// Receipt, result and schema verified, but the injection screen
    /// flagged an instruction-following artifact: review before any use.
    pub const CONTENT_FLAGGED: i32 = 12;
}
