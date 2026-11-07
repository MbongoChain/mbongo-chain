//! [`JobProfile`]: `mbongo-job:v1` as an [`ExecutionProfile`], beside the
//! existing profiles. It knows no workload: the job spec says what to run,
//! and a provider's [`RuntimeAdapter`] runs it.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::execution::{reference_input_commitment, ExecutionError, ExecutionProfile};

use super::error::JobError;
use super::output::{check_input, check_output};
use super::runtime::{JobSource, MeteringSink, ResolvedJob, RunOutput, RuntimeAdapter};
use super::spec::{
    encode_execution_spec, job_spec_digest, parse_execution_spec, JobSpec, RuntimeRequirement,
    JOB_TAG,
};

/// Runs `mbongo-job:v1` tasks on a provider's adapters.
pub struct JobProfile {
    source: Arc<dyn JobSource>,
    adapters: Vec<Arc<dyn RuntimeAdapter>>,
    metering: Arc<dyn MeteringSink>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

impl JobProfile {
    /// A profile over a provider's job source, runtimes and metering sink.
    pub fn new(
        source: Arc<dyn JobSource>,
        adapters: Vec<Arc<dyn RuntimeAdapter>>,
        metering: Arc<dyn MeteringSink>,
    ) -> Self {
        Self {
            source,
            adapters,
            metering,
        }
    }

    fn select(&self, need: &RuntimeRequirement) -> Result<Arc<dyn RuntimeAdapter>, JobError> {
        let mut family =
            self.adapters.iter().filter(|a| a.identity().family == need.family).peekable();
        if family.peek().is_none() {
            return Err(JobError::RuntimeUnknown);
        }
        family
            .find(|a| {
                let id = a.identity();
                id.revision == need.revision
                    && id.accelerator_class == need.accelerator_class
                    && need
                        .binary_sha256
                        .as_ref()
                        .is_none_or(|pinned| id.binary_sha256.as_ref() == Some(pinned))
            })
            .map(Arc::clone)
            .ok_or(JobError::RuntimeMismatch)
    }

    /// Fetches and verifies every artifact, on every task: a provider's
    /// store can change between two tasks.
    fn artifacts(&self, spec: &JobSpec) -> Result<BTreeMap<String, Vec<u8>>, JobError> {
        spec.artifacts
            .iter()
            .map(|a| {
                let bytes = self.source.artifact(&a.sha256).ok_or(JobError::ArtifactNotFound)?;
                if u64::try_from(bytes.len()).ok() != Some(a.size_bytes) {
                    return Err(JobError::ArtifactSizeMismatch);
                }
                if hex(&Sha256::digest(&bytes)) != a.sha256 {
                    return Err(JobError::ArtifactDigestMismatch);
                }
                Ok((a.role.clone(), bytes))
            })
            .collect()
    }

    /// Runs the adapter on its own thread, so that a panic or a passed
    /// deadline fails the job and not the worker. An adapter that ignores
    /// the deadline keeps its thread until it returns; a hard stop needs the
    /// process boundary of Slice 2.
    fn run_adapter(
        adapter: Arc<dyn RuntimeAdapter>,
        job: ResolvedJob,
        input: Vec<u8>,
    ) -> Result<RunOutput, JobError> {
        let deadline = Duration::from_millis(job.spec.max_wall_ms);
        let (done, result) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = done.send(adapter.run(&job, &input));
        });
        match result.recv_timeout(deadline) {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => Err(JobError::RuntimeFailed),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(JobError::LimitWallTime),
        }
    }

    /// The pipeline of the job contract. The first failure stops the task.
    pub fn run(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, JobError> {
        let digest = parse_execution_spec(execution_spec)?;
        let bytes = self.source.job_spec(&digest).ok_or(JobError::SpecNotFound)?;
        if job_spec_digest(&bytes) != digest {
            return Err(JobError::SpecDigestMismatch);
        }
        let spec = JobSpec::from_canonical_bytes(&bytes)?;
        let adapter = self.select(&spec.runtime)?;
        let artifacts = self.artifacts(&spec)?;
        if reference_input_commitment(input) != *input_commitment {
            return Err(JobError::InputCommitmentMismatch);
        }
        check_input(input, &spec.input)?;
        let output_contract = spec.output.clone();
        let job = ResolvedJob {
            digest,
            spec,
            artifacts,
        };
        let output = Self::run_adapter(adapter, job, input.to_vec())?;
        check_output(&output.raw, &output_contract)?;
        self.metering.record(&digest, &output.metering);
        Ok(output.raw)
    }
}

impl ExecutionProfile for JobProfile {
    fn spec_tag(&self) -> &'static [u8] {
        JOB_TAG
    }

    fn spec_tags(&self) -> Vec<Vec<u8>> {
        self.source.job_spec_digests().iter().map(encode_execution_spec).collect()
    }

    fn accepts(&self, execution_spec: &[u8]) -> bool {
        execution_spec.starts_with(JOB_TAG)
    }

    fn execute(&self, _input: &[u8]) -> Result<Vec<u8>, ExecutionError> {
        Err(ExecutionError::Failed(
            "mbongo-job:v1 needs the task's execution_spec and input_commitment".into(),
        ))
    }

    fn execute_task(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ExecutionError> {
        self.run(execution_spec, input_commitment, input)
            .map_err(|e| ExecutionError::Failed(e.to_string()))
    }
}
