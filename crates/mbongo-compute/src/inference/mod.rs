//! The MVP inference execution contract, `mbongo-infer:v1` (MVP-A).
//!
//! **Non-consensus.** This module is the executable form of
//! `docs/product/MVP_INFERENCE_CONTRACT_v1.md`: how a `ComputeTask` says
//! *which* inference it means — contract version, runtime, model artifact,
//! application profile, generation parameters — without any change to the
//! frozen v0.4 protocol. The chain sees 47 opaque bytes in `execution_spec`
//! (RFC 0005 §2.12); everything else lives off-chain.
//!
//! | Layer | Module | What it fixes |
//! |---|---|---|
//! | `execution_spec` layout | [`execution_spec`] | tag `mbongo-infer:v1` ‖ 32-byte job-spec digest |
//! | canonical bytes | [`canonical`] | the constrained canonical JSON both job specs and results use |
//! | job spec | [`job_spec`] | `InferenceJobSpecV1`, its canonical form, digest and validation |
//! | the MVP profile | [`kaayu`] | `kaayu-cv-extract:v1`: pinned runtime, model, prompt, schema, parameters; input and output types |
//! | worker gate | [`preflight`], [`finish`] | the exact pre-execution validation order and the post-execution result rule |
//!
//! Nothing here runs a model. MVP-B implements the execution profile on
//! top of [`preflight`] and [`finish`]; it must not reinterpret anything
//! this module decides.

pub mod canonical;
pub mod execution_spec;
pub mod job_spec;
pub mod kaayu;

use execution_spec::InferenceExecutionSpec;
use job_spec::InferenceJobSpecV1;
use kaayu::{CandidateProfileV1, CvExtractionInputV1};

/// Application-level failure classes (§16). None is a consensus error; the
/// worker maps every one of them to E's `FailureClass` as MVP-B decides.
/// Variants never carry input, prompt or output content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContractError {
    /// The `execution_spec` names an inference contract version this
    /// implementation does not support.
    #[error("UNSUPPORTED_INFERENCE_SPEC_VERSION")]
    UnsupportedInferenceSpecVersion,
    /// The `execution_spec` is not a well-formed inference spec.
    #[error("MALFORMED_EXECUTION_SPEC: {0}")]
    MalformedExecutionSpec(&'static str),
    /// No job spec with the committed digest is known to this worker.
    #[error("JOB_SPEC_NOT_FOUND")]
    JobSpecNotFound,
    /// The resolved job spec does not hash to the committed digest.
    #[error("JOB_SPEC_DIGEST_MISMATCH")]
    JobSpecDigestMismatch,
    /// The job spec violates the schema or an identity rule.
    #[error("JOB_SPEC_INVALID: {0}")]
    JobSpecInvalid(String),
    /// The job spec names a runtime family or mode this worker cannot run.
    #[error("UNSUPPORTED_RUNTIME")]
    UnsupportedRuntime,
    /// The worker's runtime revision or commit differs from the job spec.
    #[error("RUNTIME_REVISION_MISMATCH")]
    RuntimeRevisionMismatch,
    /// The worker has no model artifact configured.
    #[error("MODEL_NOT_FOUND")]
    ModelNotFound,
    /// The worker's model artifact digest differs from the job spec.
    #[error("MODEL_DIGEST_MISMATCH")]
    ModelDigestMismatch,
    /// The private input is not a valid input object for the profile.
    #[error("INVALID_INPUT_FORMAT: {0}")]
    InvalidInputFormat(&'static str),
    /// The private input exceeds the profile's byte bound.
    #[error("INPUT_TOO_LARGE")]
    InputTooLarge,
    /// The private input does not hash to `task.input_commitment`.
    #[error("INPUT_COMMITMENT_MISMATCH")]
    InputCommitmentMismatch,
    /// Generation parameters outside the frozen bounds.
    #[error("INVALID_GENERATION_PARAMETERS: {0}")]
    InvalidGenerationParameters(String),
    /// The runtime failed to produce an output.
    #[error("INFERENCE_FAILED: {0}")]
    InferenceFailed(String),
    /// The model output is not JSON inside the constrained model.
    #[error("OUTPUT_NOT_JSON: {0}")]
    OutputNotJson(String),
    /// The model output is JSON but violates the output schema.
    #[error("OUTPUT_SCHEMA_INVALID: {0}")]
    OutputSchemaInvalid(String),
    /// The canonical result could not be persisted.
    #[error("RESULT_PERSIST_FAILED: {0}")]
    ResultPersistFailed(String),
}

impl ContractError {
    /// The stable error code (the taxonomy name without detail).
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedInferenceSpecVersion => "UNSUPPORTED_INFERENCE_SPEC_VERSION",
            Self::MalformedExecutionSpec(_) => "MALFORMED_EXECUTION_SPEC",
            Self::JobSpecNotFound => "JOB_SPEC_NOT_FOUND",
            Self::JobSpecDigestMismatch => "JOB_SPEC_DIGEST_MISMATCH",
            Self::JobSpecInvalid(_) => "JOB_SPEC_INVALID",
            Self::UnsupportedRuntime => "UNSUPPORTED_RUNTIME",
            Self::RuntimeRevisionMismatch => "RUNTIME_REVISION_MISMATCH",
            Self::ModelNotFound => "MODEL_NOT_FOUND",
            Self::ModelDigestMismatch => "MODEL_DIGEST_MISMATCH",
            Self::InvalidInputFormat(_) => "INVALID_INPUT_FORMAT",
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::InputCommitmentMismatch => "INPUT_COMMITMENT_MISMATCH",
            Self::InvalidGenerationParameters(_) => "INVALID_GENERATION_PARAMETERS",
            Self::InferenceFailed(_) => "INFERENCE_FAILED",
            Self::OutputNotJson(_) => "OUTPUT_NOT_JSON",
            Self::OutputSchemaInvalid(_) => "OUTPUT_SCHEMA_INVALID",
            Self::ResultPersistFailed(_) => "RESULT_PERSIST_FAILED",
        }
    }
}

/// Where a worker finds the job spec a digest commits to. In the MVP the
/// only registry is the repository-defined profile set
/// ([`RepositoryProfiles`]); a network registry is out of scope.
pub trait JobSpecRegistry {
    /// The job spec committed by `digest`, if this worker knows it.
    fn resolve(&self, digest: &[u8; 32]) -> Option<InferenceJobSpecV1>;
}

/// The repository-defined profiles: exactly `kaayu-cv-extract:v1`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RepositoryProfiles;

impl JobSpecRegistry for RepositoryProfiles {
    fn resolve(&self, digest: &[u8; 32]) -> Option<InferenceJobSpecV1> {
        let spec = kaayu::job_spec();
        (spec.digest() == *digest).then_some(spec)
    }
}

/// What a worker knows about itself at start-up (§8–§10, §18): the runtime
/// it is running and the digest of the model file it loaded. Both are
/// compared to the job spec before any execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerInferenceConfig {
    /// Runtime family the worker runs.
    pub runtime_family: String,
    /// Runtime mode.
    pub runtime_mode: String,
    /// Exact runtime release tag.
    pub runtime_revision: String,
    /// Commit the runtime was built from.
    pub runtime_commit: String,
    /// Lowercase hex SHA-256 of the model file the worker loaded, if any.
    pub model_artifact_sha256: Option<String>,
}

/// Everything [`preflight`] established; MVP-B executes from this and from
/// nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preflight {
    /// The decoded `execution_spec`.
    pub execution_spec: InferenceExecutionSpec,
    /// The committed job spec, verified against the digest.
    pub job_spec: InferenceJobSpecV1,
    /// The validated private input.
    pub input: CvExtractionInputV1,
}

// ── the individual gates, in contract order ──────────────────────────────

/// Step 1: parse the `execution_spec`.
pub fn decode_execution_spec(
    execution_spec: &[u8],
) -> Result<InferenceExecutionSpec, ContractError> {
    InferenceExecutionSpec::decode(execution_spec)
}

/// Steps 2–4: resolve the job spec, verify its digest against the committed
/// one, validate it.
pub fn resolve_job_spec(
    registry: &dyn JobSpecRegistry,
    spec: &InferenceExecutionSpec,
) -> Result<InferenceJobSpecV1, ContractError> {
    let job_spec = registry.resolve(&spec.job_spec_digest).ok_or(ContractError::JobSpecNotFound)?;
    if job_spec.digest() != spec.job_spec_digest {
        return Err(ContractError::JobSpecDigestMismatch);
    }
    job_spec.validate()?;
    Ok(job_spec)
}

/// Step 5: the worker's runtime must be exactly the job spec's.
pub fn check_runtime(
    job_spec: &InferenceJobSpecV1,
    cfg: &WorkerInferenceConfig,
) -> Result<(), ContractError> {
    let r = &job_spec.runtime;
    if cfg.runtime_family != r.family || cfg.runtime_mode != r.mode {
        return Err(ContractError::UnsupportedRuntime);
    }
    if cfg.runtime_revision != r.revision || cfg.runtime_commit != r.commit {
        return Err(ContractError::RuntimeRevisionMismatch);
    }
    Ok(())
}

/// Step 6: the loaded model file must hash to the job spec's artifact
/// digest.
pub fn check_model(
    job_spec: &InferenceJobSpecV1,
    cfg: &WorkerInferenceConfig,
) -> Result<(), ContractError> {
    let loaded = cfg.model_artifact_sha256.as_deref().ok_or(ContractError::ModelNotFound)?;
    if loaded != job_spec.model.artifact_sha256 {
        return Err(ContractError::ModelDigestMismatch);
    }
    Ok(())
}

/// Step 7: the fetched input must hash to `task.input_commitment` under the
/// RFC 0005 §2.4 convention. The worker lifecycle already enforces this
/// before the profile runs (F §7); the contract re-checks it so that the
/// profile is safe to call on its own.
pub fn check_input_commitment(
    input: &[u8],
    input_commitment: &[u8; 32],
) -> Result<(), ContractError> {
    if crate::execution::reference_input_commitment(input) != *input_commitment {
        return Err(ContractError::InputCommitmentMismatch);
    }
    Ok(())
}

/// Step 8: the input must be a valid input object of the profile and within
/// the job spec's byte limit.
pub fn validate_input(
    job_spec: &InferenceJobSpecV1,
    input: &[u8],
) -> Result<CvExtractionInputV1, ContractError> {
    if job_spec.profile.input_contract != kaayu::INPUT_CONTRACT_V1 {
        return Err(ContractError::InvalidInputFormat(
            "profile input contract not supported",
        ));
    }
    if input.len() as u64 > job_spec.limits.max_input_bytes {
        return Err(ContractError::InputTooLarge);
    }
    CvExtractionInputV1::from_bytes(input)
}

/// The complete pre-execution gate in contract order (§17):
///
/// 1. parse `execution_spec`
/// 2. resolve the job spec by digest
/// 3. verify the resolved spec's digest equals the committed digest
/// 4. validate the job spec (schema, identity rules, generation bounds)
/// 5. runtime family, mode, revision and commit match the worker
/// 6. model artifact digest matches the loaded model
/// 7. input commitment matches the fetched bytes
/// 8. input object valid for the profile and within its limit
///
/// Any failure means: no execution, no result, no receipt.
pub fn preflight(
    execution_spec: &[u8],
    registry: &dyn JobSpecRegistry,
    cfg: &WorkerInferenceConfig,
    input: &[u8],
    input_commitment: &[u8; 32],
) -> Result<Preflight, ContractError> {
    let spec = decode_execution_spec(execution_spec)?;
    let job_spec = resolve_job_spec(registry, &spec)?;
    check_runtime(&job_spec, cfg)?;
    check_model(&job_spec, cfg)?;
    check_input_commitment(input, input_commitment)?;
    let input = validate_input(&job_spec, input)?;
    Ok(Preflight {
        execution_spec: spec,
        job_spec,
        input,
    })
}

/// The canonical result of a successful execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalResult {
    /// The validated profile.
    pub profile: CandidateProfileV1,
    /// The canonical bytes the worker persists.
    pub bytes: Vec<u8>,
    /// `BLAKE3("mbongo:compute-output:v1" || bytes)`.
    pub output_commitment: [u8; 32],
}

/// The post-execution rule (§15, §17 steps 9–10): the raw model output must
/// be JSON, must satisfy the output schema, and is then canonicalized. The
/// canonical bytes — not the raw output — are what the worker persists and
/// what the receipt's `output_commitment` commits to. Any failure means no
/// result and no receipt.
pub fn finish(
    job_spec: &InferenceJobSpecV1,
    raw_output: &str,
) -> Result<CanonicalResult, ContractError> {
    if job_spec.profile.output_contract != kaayu::OUTPUT_CONTRACT_V1 {
        return Err(ContractError::OutputSchemaInvalid(
            "profile output contract not supported".into(),
        ));
    }
    if raw_output.len() as u64 > job_spec.limits.max_output_bytes {
        return Err(ContractError::OutputSchemaInvalid(
            "output exceeds max_output_bytes".into(),
        ));
    }
    let profile = CandidateProfileV1::from_model_output(raw_output)?;
    let bytes = profile.canonical_bytes();
    let output_commitment = profile.output_commitment();
    Ok(CanonicalResult {
        profile,
        bytes,
        output_commitment,
    })
}
