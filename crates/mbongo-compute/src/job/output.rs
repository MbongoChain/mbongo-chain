//! Typed input and output checks, and technical verification up to T1.

use crate::execution::reference_output_commitment;
use crate::inference::canonical;

use super::error::JobError;
use super::spec::{DataContract, DataType, JobSpec};

fn too_large(bytes: &[u8], contract: &DataContract) -> bool {
    u64::try_from(bytes.len()).map_or(true, |n| n > contract.max_bytes)
}

/// Whether `bytes` are exactly canonical JSON, or else why not.
fn canonical_json(bytes: &[u8]) -> Result<bool, ()> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    let canonical = canonical::canonicalize(text, bytes.len()).map_err(|_| ())?;
    Ok(canonical == bytes)
}

/// Checks an input against its contract.
pub fn check_input(input: &[u8], contract: &DataContract) -> Result<(), JobError> {
    if too_large(input, contract) {
        return Err(JobError::InputTooLarge);
    }
    match contract.data_type {
        DataType::Bytes => Ok(()),
        DataType::CanonicalJson => match canonical_json(input) {
            Ok(true) => Ok(()),
            _ => Err(JobError::InputTypeViolation),
        },
    }
}

/// Checks an output against its contract (T1). Never normalises it: the
/// output commitment binds exact bytes.
pub fn check_output(output: &[u8], contract: &DataContract) -> Result<(), JobError> {
    if too_large(output, contract) {
        return Err(JobError::LimitOutputSize);
    }
    match contract.data_type {
        DataType::Bytes => Ok(()),
        DataType::CanonicalJson => match canonical_json(output) {
            Ok(true) => Ok(()),
            Ok(false) => Err(JobError::OutputNotCanonical),
            Err(()) => Err(JobError::OutputTypeViolation),
        },
    }
}

/// The technical verification level a job result reached. Levels above T1
/// are not available here; none of them says the output is right for the
/// client's purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TechnicalLevel {
    /// The bytes match the output commitment.
    T0,
    /// They also conform to the job's output contract.
    T1,
}

/// Verifies result bytes against a receipt's output commitment and the job's
/// output contract, and returns the highest level reached. The receipt's own
/// signature and anchoring are the existing client checks.
pub fn verify_result(
    spec: &JobSpec,
    output: &[u8],
    output_commitment: &[u8; 32],
) -> Result<TechnicalLevel, JobError> {
    if reference_output_commitment(output) != *output_commitment {
        return Err(JobError::OutputCommitmentMismatch);
    }
    check_output(output, &spec.output)?;
    Ok(TechnicalLevel::T1)
}
