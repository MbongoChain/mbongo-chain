//! Failure codes of `mbongo-job:v1`. Every value is content-free by
//! construction: a code and, at most, a static field name.

/// Who a failure points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blame {
    /// The job the client described or submitted.
    Client,
    /// The provider's runtime, artifacts or environment.
    Provider,
    /// Either: the failure alone does not say.
    ClientOrProvider,
}

/// The family of a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// The `execution_spec` or the job spec.
    Spec,
    /// Choosing a runtime for the job.
    RuntimeSelection,
    /// An artifact the job names.
    Artifact,
    /// The task's input.
    Input,
    /// A limit the job declares.
    Limit,
    /// The runtime failed while running.
    Execution,
    /// The runtime's output.
    Output,
}

/// Why a job did not produce a result. Display is the code, plus a static
/// field name for [`JobError::SpecInvalid`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum JobError {
    /// The `execution_spec` has the tag but not the exact layout.
    #[error("JOB_SPEC_MALFORMED_EXECUTION_SPEC")]
    MalformedExecutionSpec,
    /// This worker holds no job spec for the digest.
    #[error("JOB_SPEC_NOT_FOUND")]
    SpecNotFound,
    /// The job spec's bytes do not hash to the committed digest.
    #[error("JOB_SPEC_DIGEST_MISMATCH")]
    SpecDigestMismatch,
    /// The job spec breaks a rule of the contract.
    #[error("JOB_SPEC_INVALID: {0}")]
    SpecInvalid(&'static str),
    /// No adapter of this family on this worker.
    #[error("JOB_RUNTIME_UNKNOWN")]
    RuntimeUnknown,
    /// An adapter of this family, at another revision, build or accelerator.
    #[error("JOB_RUNTIME_MISMATCH")]
    RuntimeMismatch,
    /// The provider does not hold a named artifact.
    #[error("JOB_ARTIFACT_NOT_FOUND")]
    ArtifactNotFound,
    /// An artifact does not hash to its SHA-256.
    #[error("JOB_ARTIFACT_DIGEST_MISMATCH")]
    ArtifactDigestMismatch,
    /// An artifact is not its declared size.
    #[error("JOB_ARTIFACT_SIZE_MISMATCH")]
    ArtifactSizeMismatch,
    /// The input does not match the task's commitment.
    #[error("JOB_INPUT_COMMITMENT_MISMATCH")]
    InputCommitmentMismatch,
    /// The input is longer than the job allows.
    #[error("JOB_INPUT_TOO_LARGE")]
    InputTooLarge,
    /// The input is not of the job's declared type.
    #[error("JOB_INPUT_TYPE_VIOLATION")]
    InputTypeViolation,
    /// The runtime passed `max_wall_ms`.
    #[error("JOB_LIMIT_WALL_TIME")]
    LimitWallTime,
    /// The output is longer than the job allows.
    #[error("JOB_LIMIT_OUTPUT_SIZE")]
    LimitOutputSize,
    /// The runtime reported a failure, or panicked.
    #[error("JOB_RUNTIME_FAILED")]
    RuntimeFailed,
    /// `canonical-json` output that is valid but not canonical.
    #[error("JOB_OUTPUT_NOT_CANONICAL")]
    OutputNotCanonical,
    /// The output is not of the job's declared type.
    #[error("JOB_OUTPUT_TYPE_VIOLATION")]
    OutputTypeViolation,
    /// The output does not match the receipt's commitment.
    #[error("JOB_OUTPUT_COMMITMENT_MISMATCH")]
    OutputCommitmentMismatch,
}

impl JobError {
    /// The family of this failure.
    pub fn class(&self) -> FailureClass {
        match self {
            Self::MalformedExecutionSpec
            | Self::SpecNotFound
            | Self::SpecDigestMismatch
            | Self::SpecInvalid(_) => FailureClass::Spec,
            Self::RuntimeUnknown | Self::RuntimeMismatch => FailureClass::RuntimeSelection,
            Self::ArtifactNotFound | Self::ArtifactDigestMismatch | Self::ArtifactSizeMismatch => {
                FailureClass::Artifact
            }
            Self::InputCommitmentMismatch | Self::InputTooLarge | Self::InputTypeViolation => {
                FailureClass::Input
            }
            Self::LimitWallTime | Self::LimitOutputSize => FailureClass::Limit,
            Self::RuntimeFailed => FailureClass::Execution,
            Self::OutputNotCanonical
            | Self::OutputTypeViolation
            | Self::OutputCommitmentMismatch => FailureClass::Output,
        }
    }

    /// Whether another attempt can change the outcome.
    pub fn retryable(&self) -> bool {
        self.class() == FailureClass::Execution
    }

    /// Who this failure points at.
    pub fn blame(&self) -> Blame {
        match self.class() {
            FailureClass::Spec | FailureClass::Input => Blame::Client,
            FailureClass::Artifact | FailureClass::Execution | FailureClass::Output => {
                Blame::Provider
            }
            FailureClass::RuntimeSelection | FailureClass::Limit => Blame::ClientOrProvider,
        }
    }
}
