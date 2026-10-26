//! Wire types of the MVP HTTP front (`docs/product/MVP_HTTP_CLI.md` §4).
//!
//! No response carries CV text, a prompt, a capability, a key or result
//! JSON. The result route returns the persisted canonical bytes as its body
//! and nothing else; every other body is one of these types.

use serde::{Deserialize, Serialize};

/// `PUT /v1/inputs` body.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UploadRequest {
    /// Hex `task_id` of the `ComputeTask` the client is about to submit
    /// (the chain's identity, computed client-side; never a front-made id).
    pub task_id: String,
    /// The private input object, `kaayu-cv-extract-input:v1`.
    pub input: InputObject,
}

impl std::fmt::Debug for UploadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadRequest")
            .field("task_id", &self.task_id)
            .field("input", &self.input)
            .finish()
    }
}

/// The private input object, field for field the input contract
/// `kaayu-cv-extract-input:v1`.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputObject {
    /// Must be `kaayu-cv-extract-input:v1`.
    pub contract: String,
    /// The CV text.
    pub cv_text: String,
}

impl std::fmt::Debug for InputObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputObject")
            .field("contract", &self.contract)
            .field(
                "cv_text",
                &format_args!("<{} bytes redacted>", self.cv_text.len()),
            )
            .finish()
    }
}

/// `PUT /v1/inputs` response: public facts only.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UploadResponse {
    /// Echo of the task id.
    pub task_id: String,
    /// Hex id of the stored private object. A reference, not a capability:
    /// it grants nothing.
    pub object_ref: String,
    /// Hex `BLAKE3("mbongo:compute-input:v1" ‖ canonical input object)`,
    /// computed by the front with the worker's own function.
    pub input_commitment: String,
    /// Length of the canonical input object.
    pub input_bytes: usize,
}

/// `POST /v1/tasks/{task_id}/register` body (MVP v0.1 §9). Both fields are
/// checked against what already exists — the upload and the committed task
/// — and never select anything.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    /// The `object_ref` returned by the upload.
    pub object_ref: String,
    /// Hex executor address; must equal the committed task's `executor`.
    pub executor: String,
}

/// `GET /v1/tasks/{task_id}` (and the register response): the control
/// plane's lifecycle state plus safe operational facts. Timestamps are
/// observations of this front and worker, never trusted metering.
#[allow(clippy::struct_excessive_bools)] // a flat wire type: each flag is one fact
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct StatusView {
    /// Hex task id.
    pub task_id: String,
    /// `not_registered`, then the control-plane state: `queued`, `leased`,
    /// `input_authorized`, `input_consumed`, `executing`,
    /// `result_persisted`, `receipt_submitted`, `completed`, `failed`.
    pub state: String,
    /// Height of the block that carries the task.
    pub task_height: u64,
    /// Attempts issued so far.
    pub attempts: u32,
    /// The front stopped offering the task after its attempt limit.
    pub attempts_exhausted: bool,
    /// Last reported failure class (`input`, `execution`, `persistence`,
    /// `receipt`), if any.
    pub last_failure: Option<String>,
    /// Contract error code of the latest inference, if it failed.
    pub failure_code: Option<String>,
    /// Upload time, front clock, unix seconds.
    pub uploaded_at: Option<u64>,
    /// Registration time, front clock, unix seconds.
    pub registered_at: Option<u64>,
    /// Latest inference start, worker clock, unix milliseconds.
    pub started_at_ms: Option<u64>,
    /// Latest inference end, worker clock, unix milliseconds.
    pub completed_at_ms: Option<u64>,
    /// Latest inference wall time.
    pub inference_ms: Option<u64>,
    /// Reported prompt tokens (untrusted, observational).
    pub input_tokens: Option<u64>,
    /// Reported completion tokens (untrusted, observational).
    pub output_tokens: Option<u64>,
    /// `finish_reason` the runtime reported for the latest inference.
    pub finish_reason: Option<String>,
    /// A durable result exists in the private data plane.
    pub result_available: bool,
    /// The worker submitted an anchoring transaction, or one is observed.
    pub receipt_submitted: bool,
    /// Height of the anchored receipt, once the control plane observes it.
    /// Informational: the client resolves and verifies the receipt on-chain.
    pub receipt_height: Option<u64>,
    /// The worker was mid-attempt when this view was taken; `state` is the
    /// state before that attempt.
    pub worker_busy: bool,
    /// When this view was computed, front clock, unix seconds.
    pub as_of: u64,
}

/// Error body: a stable code and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ErrorBody {
    /// Stable error code.
    pub error: String,
}
