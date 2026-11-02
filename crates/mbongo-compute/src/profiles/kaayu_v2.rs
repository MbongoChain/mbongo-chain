//! `kaayu-cv-extract:v2` — the corrected MVP profile (MVP-B.1, change
//! control CC-1 of `docs/product/MVP_v0.1.md`).
//!
//! **Why it exists.** `kaayu-cv-extract:v1` allowed an output (15 positions
//! with 300-character descriptions, 40 skills, a 600-character summary) that
//! cannot be written within its own 400-token budget; on the reference host
//! two of ten synthetic CVs stopped at the budget and were correctly refused.
//! v2 narrows the output to the recruiting information the MVP needs — the
//! most recent positions, concise descriptions, a short summary — and
//! raises the budget moderately to 600 tokens. v1 is unchanged and stays
//! reproducible; v2 is a new execution identity.
//!
//! **What does not change.** The generic contract `mbongo-infer:v1`
//! (`execution_spec` framing, `InferenceJobSpecV1`, canonical JSON, digest
//! domain), the input contract `kaayu-cv-extract-input:v1`, the runtime,
//! the model, the output field set and enumerations, the error taxonomy,
//! the commitments and the receipt.
//!
//! | Artifact | Where | Pinned by |
//! |---|---|---|
//! | prompt template (system message, verbatim) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v2/prompt-template.txt` | `profile.prompt_template_sha256` |
//! | output JSON Schema `kaayu-cv-profile:v2` | `crates/mbongo-compute/profiles/kaayu-cv-extract-v2/output-schema.json` | `profile.output_schema_sha256` |
//! | job spec (canonical bytes) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v2/job-spec.json` | the digest in `execution_spec` |

use sha2::{Digest, Sha256};

use crate::inference::canonical::{self, Value};
use crate::inference::job_spec::InferenceJobSpecV1;
use crate::inference::kaayu::{self, CandidateProfileV1};
use crate::inference::{CanonicalResult, ContractError};

/// The v2 prompt template: used verbatim as the chat **system** message.
pub const PROMPT_TEMPLATE: &str =
    include_str!("../../profiles/kaayu-cv-extract-v2/prompt-template.txt");
/// The v2 output JSON Schema (`kaayu-cv-profile:v2`).
pub const OUTPUT_SCHEMA: &str =
    include_str!("../../profiles/kaayu-cv-extract-v2/output-schema.json");

/// Profile id (unchanged from v1).
pub const PROFILE_ID: &str = kaayu::PROFILE_ID;
/// Profile version.
pub const PROFILE_VERSION: u64 = 2;
/// Input contract (unchanged from v1).
pub const INPUT_CONTRACT: &str = kaayu::INPUT_CONTRACT_V1;
/// Output contract.
pub const OUTPUT_CONTRACT_V2: &str = "kaayu-cv-profile:v2";
/// Generation budget (v1: 400). Hard ceiling of change control CC-1: 600.
pub const MAX_TOKENS: u64 = 600;

/// The normative v2 output bounds (character counts are Unicode scalar
/// values, as JSON Schema `maxLength`). Every value is mirrored in
/// `output-schema.json`; the test group asserts they agree.
pub mod bounds {
    /// `summary` maxLength (v1: 600).
    pub const SUMMARY: usize = 200;
    /// `location` maxLength (v1: 120).
    pub const LOCATION: usize = 40;
    /// `skills` maxItems (v1: 40).
    pub const SKILLS_ITEMS: usize = 10;
    /// `skills[]` maxLength (v1: 60).
    pub const SKILL: usize = 24;
    /// `experience` maxItems (v1: 15).
    pub const EXPERIENCE_ITEMS: usize = 5;
    /// `experience[].title` maxLength (v1: 120).
    pub const TITLE: usize = 40;
    /// `experience[].organization` maxLength (v1: 120).
    pub const ORGANIZATION: usize = 40;
    /// `experience[].start` / `end` maxLength (v1: 10) — `YYYY-MM`, `YYYY` or `present`.
    pub const DATE: usize = 7;
    /// `experience[].description` maxLength (v1: 300).
    pub const DESCRIPTION: usize = 160;
    /// `education` maxItems (v1: 10).
    pub const EDUCATION_ITEMS: usize = 2;
    /// `education[].degree` maxLength (v1: 120).
    pub const DEGREE: usize = 40;
    /// `education[].institution` maxLength (v1: 120).
    pub const INSTITUTION: usize = 40;
    /// `education[].year` maxLength (v1: 10) — `YYYY`.
    pub const YEAR: usize = 4;
    /// `languages` maxItems (v1: 10).
    pub const LANGUAGES_ITEMS: usize = 3;
    /// `languages[].language` maxLength (v1: 40).
    pub const LANGUAGE: usize = 16;
    /// `certifications` maxItems (v1: 15).
    pub const CERTIFICATIONS_ITEMS: usize = 3;
    /// `certifications[]` maxLength (v1: 100).
    pub const CERTIFICATION: usize = 40;
}

/// Lowercase hex SHA-256 of the v2 prompt template bytes.
pub fn prompt_template_sha256() -> String {
    hex::encode(Sha256::digest(PROMPT_TEMPLATE.as_bytes()))
}

/// Lowercase hex SHA-256 of the v2 output schema bytes.
pub fn output_schema_sha256() -> String {
    hex::encode(Sha256::digest(OUTPUT_SCHEMA.as_bytes()))
}

/// The v2 job spec: the v1 job spec with exactly the fields this change
/// controls — profile version, output contract, prompt and schema digests,
/// `generation.max_tokens`. Runtime, model, sampling, seed and limits are
/// those of v1.
pub fn job_spec() -> InferenceJobSpecV1 {
    let mut spec = kaayu::job_spec();
    spec.profile.version = PROFILE_VERSION;
    spec.profile.output_contract = OUTPUT_CONTRACT_V2.into();
    spec.profile.prompt_template_sha256 = prompt_template_sha256();
    spec.profile.output_schema_sha256 = output_schema_sha256();
    spec.generation.max_tokens = MAX_TOKENS;
    spec
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

fn too_long(what: &str, max: usize) -> ContractError {
    ContractError::OutputSchemaInvalid(format!(
        "{what} exceeds {max} characters (kaayu-cv-profile:v2)"
    ))
}

fn too_many(what: &str, max: usize) -> ContractError {
    ContractError::OutputSchemaInvalid(format!("{what} exceeds {max} items (kaayu-cv-profile:v2)"))
}

/// Checks the v2 bounds on a profile that already satisfies the
/// `kaayu-cv-profile` field set and enumerations (validated by the v1 typed
/// validator, whose bounds are a strict superset of v2's).
pub fn check_v2_bounds(p: &CandidateProfileV1) -> Result<(), ContractError> {
    use bounds as b;
    if chars(&p.summary) > b::SUMMARY {
        return Err(too_long("summary", b::SUMMARY));
    }
    if chars(&p.location) > b::LOCATION {
        return Err(too_long("location", b::LOCATION));
    }
    if p.skills.len() > b::SKILLS_ITEMS {
        return Err(too_many("skills", b::SKILLS_ITEMS));
    }
    if p.skills.iter().any(|s| chars(s) > b::SKILL) {
        return Err(too_long("skills[]", b::SKILL));
    }
    if p.experience.len() > b::EXPERIENCE_ITEMS {
        return Err(too_many("experience", b::EXPERIENCE_ITEMS));
    }
    for e in &p.experience {
        if chars(&e.title) > b::TITLE {
            return Err(too_long("experience[].title", b::TITLE));
        }
        if chars(&e.organization) > b::ORGANIZATION {
            return Err(too_long("experience[].organization", b::ORGANIZATION));
        }
        if chars(&e.start) > b::DATE || chars(&e.end) > b::DATE {
            return Err(too_long("experience[].start/end", b::DATE));
        }
        if chars(&e.description) > b::DESCRIPTION {
            return Err(too_long("experience[].description", b::DESCRIPTION));
        }
    }
    if p.education.len() > b::EDUCATION_ITEMS {
        return Err(too_many("education", b::EDUCATION_ITEMS));
    }
    for e in &p.education {
        if chars(&e.degree) > b::DEGREE {
            return Err(too_long("education[].degree", b::DEGREE));
        }
        if chars(&e.institution) > b::INSTITUTION {
            return Err(too_long("education[].institution", b::INSTITUTION));
        }
        if chars(&e.year) > b::YEAR {
            return Err(too_long("education[].year", b::YEAR));
        }
    }
    if p.languages.len() > b::LANGUAGES_ITEMS {
        return Err(too_many("languages", b::LANGUAGES_ITEMS));
    }
    if p.languages.iter().any(|l| chars(&l.language) > b::LANGUAGE) {
        return Err(too_long("languages[].language", b::LANGUAGE));
    }
    if p.certifications.len() > b::CERTIFICATIONS_ITEMS {
        return Err(too_many("certifications", b::CERTIFICATIONS_ITEMS));
    }
    if p.certifications.iter().any(|c| chars(c) > b::CERTIFICATION) {
        return Err(too_long("certifications[]", b::CERTIFICATION));
    }
    Ok(())
}

/// Parses raw model output and validates it against `kaayu-cv-profile:v2`.
pub fn parse_output(raw: &str) -> Result<CandidateProfileV1, ContractError> {
    let p = CandidateProfileV1::from_model_output(raw)?;
    check_v2_bounds(&p)?;
    Ok(p)
}

/// Validates an already-parsed value against `kaayu-cv-profile:v2`.
pub fn profile_from_value(v: &Value) -> Result<CandidateProfileV1, ContractError> {
    let p = CandidateProfileV1::from_value(v)?;
    check_v2_bounds(&p)?;
    Ok(p)
}

/// The v2 post-execution rule — the MVP-A rule (contract §15, §17 step 9)
/// with the v2 schema: size bound, strict parse, schema validation,
/// canonical bytes by the unchanged MVP-A serializer, and the unchanged
/// output commitment.
pub fn finish(
    job_spec: &InferenceJobSpecV1,
    raw_output: &str,
) -> Result<CanonicalResult, ContractError> {
    if job_spec.profile.output_contract != OUTPUT_CONTRACT_V2 {
        return Err(ContractError::OutputSchemaInvalid(
            "profile output contract not supported".into(),
        ));
    }
    if raw_output.len() as u64 > job_spec.limits.max_output_bytes {
        return Err(ContractError::OutputSchemaInvalid(
            "output exceeds max_output_bytes".into(),
        ));
    }
    let profile = parse_output(raw_output)?;
    let bytes = profile.canonical_bytes();
    let output_commitment = profile.output_commitment();
    debug_assert!(canonical::is_canonical(&bytes, kaayu::MAX_OUTPUT_BYTES));
    Ok(CanonicalResult {
        profile,
        bytes,
        output_commitment,
    })
}

/// A maximal legal v2 profile: every array at `maxItems`, every string at
/// exactly `maxLength`, filled with realistic recruiting prose. Its token
/// cost under the real model tokenizer is the theoretical worst case the
/// budget must cover (recorded in `test-vectors/inference/kaayu-cv-extract-v2.json`).
pub fn maximal_profile() -> CandidateProfileV1 {
    use crate::inference::kaayu::{Education, Experience, Language};
    use bounds as b;
    const PROSE: &str = "Led a cross-functional team delivering a cloud data platform, improved reliability and reduced operating cost while mentoring engineers and partnering with product and security stakeholders on delivery. ";
    const WORDS: [&str; 8] = [
        "Kubernetes",
        "PostgreSQL",
        "TypeScript",
        "distributed systems",
        "data modelling",
        "observability",
        "stakeholder management",
        "Terraform",
    ];
    let fill = |n: usize| -> String { PROSE.repeat(n / PROSE.len() + 1).chars().take(n).collect() };
    let word = |i: usize, n: usize| -> String {
        let mut s = WORDS[i % WORDS.len()].to_owned();
        while s.chars().count() < n {
            s.push(' ');
            s.push_str(WORDS[(i + s.len()) % WORDS.len()]);
        }
        s.chars().take(n).collect()
    };
    CandidateProfileV1 {
        summary: fill(b::SUMMARY),
        seniority: "executive".into(),
        location: fill(b::LOCATION),
        skills: (0..b::SKILLS_ITEMS).map(|i| word(i, b::SKILL)).collect(),
        experience: (0..b::EXPERIENCE_ITEMS)
            .map(|_| Experience {
                title: fill(b::TITLE),
                organization: fill(b::ORGANIZATION),
                start: "2021-03".into(),
                end: "present".into(),
                description: fill(b::DESCRIPTION),
            })
            .collect(),
        education: (0..b::EDUCATION_ITEMS)
            .map(|_| Education {
                degree: fill(b::DEGREE),
                institution: fill(b::INSTITUTION),
                year: "2019".into(),
            })
            .collect(),
        languages: (0..b::LANGUAGES_ITEMS)
            .map(|i| Language {
                language: word(i, b::LANGUAGE),
                level: "intermediate".into(),
            })
            .collect(),
        certifications: (0..b::CERTIFICATIONS_ITEMS)
            .map(|i| word(i + 3, b::CERTIFICATION))
            .collect(),
    }
}
