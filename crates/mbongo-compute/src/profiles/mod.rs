//! Application profiles of the MVP inference contract, by version.
//!
//! `mbongo-infer:v1` (module [`crate::inference`]) is the generic contract;
//! a profile fills it in for one workload. MVP-A froze
//! `kaayu-cv-extract:v1` ([`crate::inference::kaayu`], unchanged and still
//! reproducible). MVP-B.1 added `kaayu-cv-extract:v2` ([`kaayu_v2`],
//! change control CC-1). MVP-E adds `kaayu-cv-extract:v3` ([`kaayu_v3`],
//! change control CC-2: prompt-injection hardening, prompt only), which is
//! the **active MVP profile** (`docs/product/MVP_v0.1.md`). The registry is
//! repository-local, as the contract requires (A27): a worker resolves a
//! committed digest to one of these three job specs or to nothing.

pub mod kaayu_v2;
pub mod kaayu_v3;

use crate::inference::job_spec::InferenceJobSpecV1;
use crate::inference::{
    finish as finish_v1, kaayu, CanonicalResult, ContractError, JobSpecRegistry,
};

/// The profile the MVP executes (CC-2).
pub const ACTIVE_MVP_PROFILE: &str = "kaayu-cv-extract:v3";

/// The active MVP job spec.
pub fn active_job_spec() -> InferenceJobSpecV1 {
    kaayu_v3::job_spec()
}

/// The repository-defined Kaayu profiles: v1 and v2 (superseded for the
/// MVP, still resolvable and reproducible) and v3 (active).
#[derive(Debug, Default, Clone, Copy)]
pub struct KaayuProfiles;

impl JobSpecRegistry for KaayuProfiles {
    fn resolve(&self, digest: &[u8; 32]) -> Option<InferenceJobSpecV1> {
        [
            kaayu_v3::job_spec(),
            kaayu_v2::job_spec(),
            kaayu::job_spec(),
        ]
        .into_iter()
        .find(|s| s.digest() == *digest)
    }
}

/// The post-execution rule for whichever Kaayu profile a job spec names,
/// selected by its output contract (v3 shares v2's). Unknown contracts are
/// refused.
pub fn finish(
    job_spec: &InferenceJobSpecV1,
    raw_output: &str,
) -> Result<CanonicalResult, ContractError> {
    match job_spec.profile.output_contract.as_str() {
        kaayu_v2::OUTPUT_CONTRACT_V2 => kaayu_v2::finish(job_spec, raw_output),
        kaayu::OUTPUT_CONTRACT_V1 => finish_v1(job_spec, raw_output),
        _ => Err(ContractError::OutputSchemaInvalid(
            "profile output contract not supported".into(),
        )),
    }
}

/// The prompt template and output schema a job spec's profile pins, if it
/// is a repository profile.
pub fn assets(job_spec: &InferenceJobSpecV1) -> Option<(&'static str, &'static str)> {
    match (job_spec.profile.id.as_str(), job_spec.profile.version) {
        (kaayu::PROFILE_ID, 1) => Some((kaayu::PROMPT_TEMPLATE, kaayu::OUTPUT_SCHEMA)),
        (kaayu::PROFILE_ID, 2) => Some((kaayu_v2::PROMPT_TEMPLATE, kaayu_v2::OUTPUT_SCHEMA)),
        (kaayu::PROFILE_ID, 3) => Some((kaayu_v3::PROMPT_TEMPLATE, kaayu_v3::OUTPUT_SCHEMA)),
        _ => None,
    }
}
