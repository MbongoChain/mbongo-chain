//! `kaayu-cv-extract:v3` — prompt-injection hardening of the MVP profile
//! (MVP-E, change control CC-2 of `docs/product/MVP_v0.1.md`).
//!
//! **Why it exists.** MVP-D found that a CV can carry instructions the
//! model obeys (D18: a `SYSTEM:` line forced `"summary":"HACKED"` and
//! `"seniority":"executive"` into a schema-valid result). The v2 system
//! prompt never told the model that the user message is untrusted data.
//! v3 changes **only the prompt**: a security section that defines the CV
//! as data, names the common attack forms and pins the attacked fields
//! (summary, seniority, skills, certifications) to the candidate's real
//! facts; a factual recipe for `summary`; and two closing rules. On the
//! MVP-E adversarial corpus the model's overrides fall from 5 of 10 attacks
//! (v2) to 1 of 10; the client-side injection screen
//! ([`crate::mvp::screen`]) flags the remaining one. This is tested
//! resistance, not immunity.
//!
//! **What does not change.** The output contract `kaayu-cv-profile:v2` and
//! its schema bytes (the v2 file, shared), the bounds, the generation
//! budget (600 tokens), sampling, seed, runtime, model, input contract,
//! the generic contract `mbongo-infer:v1`, the finish rule, commitments and
//! receipts. v2 is unchanged and stays resolvable and reproducible; v3 is a
//! new execution identity.
//!
//! | Artifact | Where | Pinned by |
//! |---|---|---|
//! | prompt template (system message, verbatim) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v3/prompt-template.txt` | `profile.prompt_template_sha256` |
//! | output JSON Schema `kaayu-cv-profile:v2` (unchanged bytes) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v2/output-schema.json` | `profile.output_schema_sha256` |
//! | job spec (canonical bytes) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v3/job-spec.json` | the digest in `execution_spec` |

use sha2::{Digest, Sha256};

use crate::inference::job_spec::InferenceJobSpecV1;

use super::kaayu_v2;

/// The v3 prompt template: used verbatim as the chat **system** message.
pub const PROMPT_TEMPLATE: &str =
    include_str!("../../profiles/kaayu-cv-extract-v3/prompt-template.txt");
/// The output JSON Schema: `kaayu-cv-profile:v2`, the v2 bytes.
pub const OUTPUT_SCHEMA: &str = kaayu_v2::OUTPUT_SCHEMA;
/// Profile id (unchanged).
pub const PROFILE_ID: &str = kaayu_v2::PROFILE_ID;
/// Profile version.
pub const PROFILE_VERSION: u64 = 3;
/// Output contract (unchanged from v2).
pub const OUTPUT_CONTRACT: &str = kaayu_v2::OUTPUT_CONTRACT_V2;

/// Lowercase hex SHA-256 of the v3 prompt template bytes.
pub fn prompt_template_sha256() -> String {
    hex::encode(Sha256::digest(PROMPT_TEMPLATE.as_bytes()))
}

/// The v3 job spec: the v2 job spec with exactly the fields this change
/// controls — profile version and prompt digest.
pub fn job_spec() -> InferenceJobSpecV1 {
    let mut spec = kaayu_v2::job_spec();
    spec.profile.version = PROFILE_VERSION;
    spec.profile.prompt_template_sha256 = prompt_template_sha256();
    spec
}
