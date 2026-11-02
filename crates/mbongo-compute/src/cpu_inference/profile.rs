//! The `mbongo-infer:v1` execution profile and the profile dispatcher
//! (MVP-B §4–§5, §13, §17–§36).

use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::execution::{ExecutionError, ExecutionProfile};
use crate::inference::execution_spec::{InferenceExecutionSpec, INFER_TAG_V1};
use crate::inference::job_spec::InferenceJobSpecV1;
use crate::inference::{preflight, ContractError, WorkerInferenceConfig};
use crate::profiles::{self, KaayuProfiles};

use super::backend::{ChatRequest, InferenceBackend};

/// The safe, operational record of one inference attempt (MVP-B §36–§38).
/// No input, prompt or output content; token counts are **untrusted
/// observational metering** and never a settlement basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceObservation {
    /// The task's public `input_commitment` (correlates with `task_id` on-chain).
    pub input_commitment: [u8; 32],
    /// `job_spec_digest` from the task.
    pub job_spec_digest: [u8; 32],
    /// `llama.cpp/llama-server/<revision>`.
    pub runtime_id: String,
    /// `<model name>@<sha256 prefix>`.
    pub model_id: String,
    /// Unix milliseconds.
    pub started_at_ms: u128,
    /// Unix milliseconds.
    pub completed_at_ms: u128,
    /// Inference wall time (request → canonical result or failure).
    pub duration_ms: u128,
    /// Reported prompt tokens.
    pub input_tokens: Option<u64>,
    /// Reported completion tokens.
    pub output_tokens: Option<u64>,
    /// Canonical result length, when successful.
    pub result_bytes: Option<usize>,
    /// The contract error code, when failed.
    pub failure_code: Option<&'static str>,
    /// `finish_reason` as the runtime reported it (`stop`, `length`, …).
    pub finish_reason: Option<String>,
}

/// One worker → one pinned model → one pinned profile (MVP-B §13).
pub struct InferenceProfile {
    backend: Arc<dyn InferenceBackend>,
    system: &'static str,
    verified: WorkerInferenceConfig,
    spec_bytes: Vec<u8>,
    schema: serde_json::Value,
    schema_text: &'static str,
    observations: Arc<Mutex<Vec<InferenceObservation>>>,
}

impl InferenceProfile {
    /// Binds the frozen profile to a backend whose proven identity matches
    /// the frozen job spec. Refuses a backend running anything else
    /// (`RUNTIME_REVISION_MISMATCH`, `MODEL_DIGEST_MISMATCH`): there is no
    /// fallback model and no fallback runtime.
    pub fn new(backend: Arc<dyn InferenceBackend>) -> Result<Self, ContractError> {
        // One worker, one pinned profile: the active MVP profile (CC-1).
        let spec = profiles::active_job_spec();
        let id = backend.identity();
        let verified = WorkerInferenceConfig {
            runtime_family: id.runtime_family,
            runtime_mode: id.runtime_mode,
            runtime_revision: id.runtime_revision,
            runtime_commit: id.runtime_commit,
            model_artifact_sha256: Some(id.model_artifact_sha256),
        };
        crate::inference::check_runtime(&spec, &verified)?;
        crate::inference::check_model(&spec, &verified)?;
        let (system, schema_text) = profiles::assets(&spec).ok_or_else(|| {
            ContractError::JobSpecInvalid("no repository assets for the active profile".into())
        })?;
        let schema = serde_json::from_str(schema_text)
            .map_err(|_| ContractError::JobSpecInvalid("output schema is not JSON".into()))?;
        Ok(Self {
            backend,
            system,
            verified,
            spec_bytes: InferenceExecutionSpec {
                job_spec_digest: spec.digest(),
            }
            .encode(),
            schema,
            schema_text,
            observations: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// The exact 47-byte `execution_spec` this worker serves.
    pub fn execution_spec(&self) -> &[u8] {
        &self.spec_bytes
    }

    /// A handle on the observations recorded so far.
    pub fn observations(&self) -> Arc<Mutex<Vec<InferenceObservation>>> {
        Arc::clone(&self.observations)
    }

    /// The full contract pipeline for one task: `preflight` (steps 1–8),
    /// the frozen prompt, one completion, `finish` (steps 9). Returns the
    /// canonical result bytes the worker persists and commits.
    pub fn run(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ContractError> {
        let started = Instant::now();
        let started_at_ms = unix_ms();
        let mut obs = InferenceObservation {
            input_commitment: *input_commitment,
            job_spec_digest: [0u8; 32],
            runtime_id: format!(
                "{}/{}/{}",
                self.verified.runtime_family,
                self.verified.runtime_mode,
                self.verified.runtime_revision
            ),
            model_id: String::new(),
            started_at_ms,
            completed_at_ms: 0,
            duration_ms: 0,
            input_tokens: None,
            output_tokens: None,
            result_bytes: None,
            failure_code: None,
            finish_reason: None,
        };
        let outcome = self.run_inner(execution_spec, input_commitment, input, &mut obs);
        obs.completed_at_ms = unix_ms();
        obs.duration_ms = started.elapsed().as_millis();
        match &outcome {
            Ok(bytes) => obs.result_bytes = Some(bytes.len()),
            Err(e) => obs.failure_code = Some(e.code()),
        }
        log::info!(
            "cpu-inference: input {} job_spec {} runtime {} model {} duration_ms {} input_tokens {:?} output_tokens {:?} outcome {}",
            hex::encode(&obs.input_commitment[..8]),
            hex::encode(&obs.job_spec_digest[..8]),
            obs.runtime_id,
            obs.model_id,
            obs.duration_ms,
            obs.input_tokens,
            obs.output_tokens,
            obs.failure_code.unwrap_or("OK")
        );
        self.observations.lock().unwrap().push(obs);
        outcome
    }

    fn run_inner(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
        obs: &mut InferenceObservation,
    ) -> Result<Vec<u8>, ContractError> {
        let pre = preflight(
            execution_spec,
            &KaayuProfiles,
            &self.verified,
            input,
            input_commitment,
        )?;
        obs.job_spec_digest = pre.execution_spec.job_spec_digest;
        obs.model_id = model_id(&pre.job_spec);
        let request = ChatRequest::new(
            &pre.job_spec,
            self.system,
            &pre.input.cv_text,
            self.schema.clone(),
        )
        .with_schema_text(self.schema_text);
        let completion = self.backend.complete(&request)?;
        obs.input_tokens = completion.prompt_tokens;
        obs.output_tokens = completion.completion_tokens;
        obs.finish_reason.clone_from(&completion.finish_reason);
        let result = profiles::finish(&pre.job_spec, &completion.content)?;
        Ok(result.bytes)
    }
}

fn model_id(spec: &InferenceJobSpecV1) -> String {
    format!("{}@{}", spec.model.name, &spec.model.artifact_sha256[..12])
}

fn unix_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis())
}

fn to_execution_error(e: &ContractError) -> ExecutionError {
    // The Display of every ContractError is its code plus a field-level
    // detail; none carries input, prompt or output content.
    ExecutionError::Failed(e.to_string())
}

impl ExecutionProfile for InferenceProfile {
    fn spec_tag(&self) -> &'static [u8] {
        INFER_TAG_V1
    }

    fn spec_tags(&self) -> Vec<Vec<u8>> {
        vec![self.spec_bytes.clone()]
    }

    fn accepts(&self, execution_spec: &[u8]) -> bool {
        execution_spec == self.spec_bytes.as_slice()
    }

    fn execute(&self, _input: &[u8]) -> Result<Vec<u8>, ExecutionError> {
        Err(ExecutionError::Failed(
            "mbongo-infer:v1 needs the task's execution_spec and input_commitment".into(),
        ))
    }

    fn execute_task(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ExecutionError> {
        self.run(execution_spec, input_commitment, input)
            .map_err(|e| to_execution_error(&e))
    }
}

/// The name a dispatcher answers to; never equal to any task's
/// `execution_spec`.
pub const DISPATCH_TAG: &[u8] = b"mbongo-worker:profile-dispatch:v1";

/// Deterministic dispatch over a fixed list of profiles (MVP-B §5): the
/// first profile that accepts the task's exact `execution_spec` runs it;
/// none → `UNSUPPORTED_EXECUTION_PROFILE`. No fallback from one profile to
/// another: a failing inference never becomes a reverse-bytes result.
pub struct ProfileDispatch {
    profiles: Vec<Box<dyn ExecutionProfile>>,
}

impl ProfileDispatch {
    /// Dispatch over `profiles`, in order.
    pub fn new(profiles: Vec<Box<dyn ExecutionProfile>>) -> Self {
        Self { profiles }
    }
}

impl ExecutionProfile for ProfileDispatch {
    fn spec_tag(&self) -> &'static [u8] {
        DISPATCH_TAG
    }

    fn spec_tags(&self) -> Vec<Vec<u8>> {
        self.profiles.iter().flat_map(|p| p.spec_tags()).collect()
    }

    fn accepts(&self, execution_spec: &[u8]) -> bool {
        self.profiles.iter().any(|p| p.accepts(execution_spec))
    }

    fn execute(&self, _input: &[u8]) -> Result<Vec<u8>, ExecutionError> {
        Err(ExecutionError::UnsupportedSpec)
    }

    fn execute_task(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ExecutionError> {
        self.profiles
            .iter()
            .find(|p| p.accepts(execution_spec))
            .ok_or(ExecutionError::UnsupportedSpec)?
            .execute_task(execution_spec, input_commitment, input)
    }
}
