//! `InferenceJobSpecV1`: the off-chain execution contract a task commits to
//! (MVP-A §6–§10, §14, §19–§22).
//!
//! The job spec says *how* to execute — runtime, model artifact, profile,
//! generation parameters, limits — and never *what* (the CV text is the
//! private input, committed separately by `input_commitment`). It is
//! serialized with the constrained canonical JSON of [`super::canonical`]
//! and identified by
//!
//! ```text
//! job_spec_digest = BLAKE3( "mbongo:infer-job-spec:v1" || canonical_job_spec_bytes )
//! ```
//!
//! an application-level commitment, not a consensus hash. Every string in a
//! job spec is restricted to printable ASCII without `"` or `\`, so the
//! canonical bytes contain no escapes at all.

use super::canonical::{self, build, Value};
use super::ContractError;

/// Domain prefix of the job-spec digest. Distinct from every RFC 0005 and
/// RFC 0002 domain.
pub const DOMAIN_JOB_SPEC: &[u8] = b"mbongo:infer-job-spec:v1";
/// The value of the `contract` field.
pub const CONTRACT_V1: &str = "mbongo-infer:v1";
/// Bound on the canonical job-spec bytes.
pub const MAX_JOB_SPEC_BYTES: usize = 4096;

/// Generation-parameter bounds frozen for the MVP (§20).
pub mod bounds {
    /// `temperature_milli` ∈ `[0, 200]` (0.0 to 0.2).
    pub const TEMPERATURE_MILLI_MAX: u64 = 200;
    /// `top_p_milli` ∈ `[500, 1000]`.
    pub const TOP_P_MILLI_MIN: u64 = 500;
    /// `top_p_milli` ∈ `[500, 1000]`.
    pub const TOP_P_MILLI_MAX: u64 = 1000;
    /// `seed` fits a 32-bit sampler seed.
    pub const SEED_MAX: u64 = u32::MAX as u64;
    /// `max_tokens` ∈ `[1, 400]` for profile version 1 (frozen by MVP-A).
    pub const MAX_TOKENS_MAX: u64 = 400;
    /// `max_tokens` ∈ `[1, 600]` for profile version 2 and later: the hard
    /// ceiling of change control CC-1 (`docs/product/MVP_v0.1.md` §20.1).
    pub const MAX_TOKENS_CEILING_V2: u64 = 600;
    /// `limits.max_input_bytes` ∈ `[1, 10000]`.
    pub const MAX_INPUT_BYTES_MAX: u64 = 10_000;
    /// `limits.max_output_bytes` ∈ `[1, 8000]`.
    pub const MAX_OUTPUT_BYTES_MAX: u64 = 8_000;
}

/// Runtime identity (§8). `revision` is an exact release tag and `commit`
/// the commit it resolves to; floating names are refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIdentity {
    /// `"llama.cpp"`.
    pub family: String,
    /// `"llama-server"`.
    pub mode: String,
    /// Exact release tag, e.g. `"v0.7.0"`.
    pub revision: String,
    /// The 40-hex commit the tag resolves to.
    pub commit: String,
    /// SPDX identifier of the runtime license.
    pub license: String,
}

/// Model identity (§9, §10). `artifact_sha256` is authoritative; the names
/// are for humans and logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentity {
    /// Model family, e.g. `"Qwen3"`.
    pub family: String,
    /// Human model name, e.g. `"Qwen3-4B-Instruct-2507"`.
    pub name: String,
    /// Where the artifact was obtained (URL of the repository).
    pub source: String,
    /// The repository revision the artifact was taken from.
    pub source_revision: String,
    /// Artifact file name.
    pub artifact_name: String,
    /// Lowercase hex SHA-256 of the exact artifact bytes.
    pub artifact_sha256: String,
    /// Artifact size in bytes.
    pub artifact_size_bytes: u64,
    /// `"gguf"`.
    pub format: String,
    /// `"Q4_K_M"`.
    pub quantization: String,
    /// SPDX identifier of the model license.
    pub license: String,
}

/// The application profile layered on the generic contract (§12–§15, §42).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileIdentity {
    /// Profile id, e.g. `"kaayu-cv-extract"`.
    pub id: String,
    /// Profile version; changes whenever any execution-identity field of the
    /// profile changes.
    pub version: u64,
    /// Input contract tag, e.g. `"kaayu-cv-extract-input:v1"`.
    pub input_contract: String,
    /// Output contract tag, e.g. `"kaayu-cv-profile:v1"`.
    pub output_contract: String,
    /// Lowercase hex SHA-256 of the prompt template bytes.
    pub prompt_template_sha256: String,
    /// Lowercase hex SHA-256 of the output JSON Schema bytes.
    pub output_schema_sha256: String,
}

/// Generation parameters (§19). Fractions are integer thousandths so the
/// job spec contains no floating-point number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationParams {
    /// Sampling temperature × 1000.
    pub temperature_milli: u64,
    /// Nucleus `top_p` × 1000.
    pub top_p_milli: u64,
    /// Sampler seed.
    pub seed: u64,
    /// Maximum generated tokens.
    pub max_tokens: u64,
}

/// Size limits (§20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum canonical input-object bytes.
    pub max_input_bytes: u64,
    /// Maximum raw model-output bytes before canonicalization.
    pub max_output_bytes: u64,
}

/// The job spec, version 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceJobSpecV1 {
    /// Runtime identity.
    pub runtime: RuntimeIdentity,
    /// Model identity.
    pub model: ModelIdentity,
    /// Application profile.
    pub profile: ProfileIdentity,
    /// Generation parameters.
    pub generation: GenerationParams,
    /// Limits.
    pub limits: Limits,
}

impl InferenceJobSpecV1 {
    /// The constrained canonical model of this spec.
    // The bindings mirror the job-spec sections; the names are the section names.
    #[allow(clippy::many_single_char_names)]
    pub fn to_value(&self) -> Value {
        let r = &self.runtime;
        let m = &self.model;
        let p = &self.profile;
        let g = self.generation;
        let l = self.limits;
        build::obj(vec![
            ("contract", build::s(CONTRACT_V1)),
            (
                "generation",
                build::obj(vec![
                    ("max_tokens", build::u(g.max_tokens)),
                    ("seed", build::u(g.seed)),
                    ("temperature_milli", build::u(g.temperature_milli)),
                    ("top_p_milli", build::u(g.top_p_milli)),
                ]),
            ),
            (
                "limits",
                build::obj(vec![
                    ("max_input_bytes", build::u(l.max_input_bytes)),
                    ("max_output_bytes", build::u(l.max_output_bytes)),
                ]),
            ),
            (
                "model",
                build::obj(vec![
                    ("artifact_name", build::s(&m.artifact_name)),
                    ("artifact_sha256", build::s(&m.artifact_sha256)),
                    ("artifact_size_bytes", build::u(m.artifact_size_bytes)),
                    ("family", build::s(&m.family)),
                    ("format", build::s(&m.format)),
                    ("license", build::s(&m.license)),
                    ("name", build::s(&m.name)),
                    ("quantization", build::s(&m.quantization)),
                    ("source", build::s(&m.source)),
                    ("source_revision", build::s(&m.source_revision)),
                ]),
            ),
            (
                "profile",
                build::obj(vec![
                    ("id", build::s(&p.id)),
                    ("input_contract", build::s(&p.input_contract)),
                    ("output_contract", build::s(&p.output_contract)),
                    ("output_schema_sha256", build::s(&p.output_schema_sha256)),
                    (
                        "prompt_template_sha256",
                        build::s(&p.prompt_template_sha256),
                    ),
                    ("version", build::u(p.version)),
                ]),
            ),
            (
                "runtime",
                build::obj(vec![
                    ("commit", build::s(&r.commit)),
                    ("family", build::s(&r.family)),
                    ("license", build::s(&r.license)),
                    ("mode", build::s(&r.mode)),
                    ("revision", build::s(&r.revision)),
                ]),
            ),
        ])
    }

    /// The canonical bytes (§7).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical::serialize(&self.to_value())
    }

    /// `BLAKE3(DOMAIN_JOB_SPEC || canonical_bytes)` (§8).
    pub fn digest(&self) -> [u8; 32] {
        digest_of_canonical_bytes(&self.canonical_bytes())
    }

    /// Parses canonical or non-canonical job-spec JSON text and validates
    /// it (§6 field set, §8–§10 identity rules, §20 bounds). Unknown and
    /// missing fields are errors.
    pub fn from_json(text: &str) -> Result<Self, ContractError> {
        let v = canonical::parse(text, MAX_JOB_SPEC_BYTES)
            .map_err(|e| ContractError::JobSpecInvalid(format!("not constrained JSON: {e}")))?;
        let spec = Self::from_value(&v)?;
        spec.validate()?;
        Ok(spec)
    }

    // The bindings mirror the job-spec sections; the names are the section names.
    #[allow(clippy::many_single_char_names)]
    fn from_value(v: &Value) -> Result<Self, ContractError> {
        let top = Fields::new(
            v,
            "job spec",
            &[
                "contract",
                "generation",
                "limits",
                "model",
                "profile",
                "runtime",
            ],
        )?;
        if top.str("contract")? != CONTRACT_V1 {
            return Err(ContractError::UnsupportedInferenceSpecVersion);
        }
        let g = Fields::new(
            top.get("generation")?,
            "generation",
            &["max_tokens", "seed", "temperature_milli", "top_p_milli"],
        )?;
        let l = Fields::new(
            top.get("limits")?,
            "limits",
            &["max_input_bytes", "max_output_bytes"],
        )?;
        let m = Fields::new(
            top.get("model")?,
            "model",
            &[
                "artifact_name",
                "artifact_sha256",
                "artifact_size_bytes",
                "family",
                "format",
                "license",
                "name",
                "quantization",
                "source",
                "source_revision",
            ],
        )?;
        let p = Fields::new(
            top.get("profile")?,
            "profile",
            &[
                "id",
                "input_contract",
                "output_contract",
                "output_schema_sha256",
                "prompt_template_sha256",
                "version",
            ],
        )?;
        let r = Fields::new(
            top.get("runtime")?,
            "runtime",
            &["commit", "family", "license", "mode", "revision"],
        )?;
        Ok(Self {
            runtime: RuntimeIdentity {
                family: r.str("family")?.to_owned(),
                mode: r.str("mode")?.to_owned(),
                revision: r.str("revision")?.to_owned(),
                commit: r.str("commit")?.to_owned(),
                license: r.str("license")?.to_owned(),
            },
            model: ModelIdentity {
                family: m.str("family")?.to_owned(),
                name: m.str("name")?.to_owned(),
                source: m.str("source")?.to_owned(),
                source_revision: m.str("source_revision")?.to_owned(),
                artifact_name: m.str("artifact_name")?.to_owned(),
                artifact_sha256: m.str("artifact_sha256")?.to_owned(),
                artifact_size_bytes: m.uint("artifact_size_bytes")?,
                format: m.str("format")?.to_owned(),
                quantization: m.str("quantization")?.to_owned(),
                license: m.str("license")?.to_owned(),
            },
            profile: ProfileIdentity {
                id: p.str("id")?.to_owned(),
                version: p.uint("version")?,
                input_contract: p.str("input_contract")?.to_owned(),
                output_contract: p.str("output_contract")?.to_owned(),
                prompt_template_sha256: p.str("prompt_template_sha256")?.to_owned(),
                output_schema_sha256: p.str("output_schema_sha256")?.to_owned(),
            },
            generation: GenerationParams {
                temperature_milli: g.uint("temperature_milli")?,
                top_p_milli: g.uint("top_p_milli")?,
                seed: g.uint("seed")?,
                max_tokens: g.uint("max_tokens")?,
            },
            limits: Limits {
                max_input_bytes: l.uint("max_input_bytes")?,
                max_output_bytes: l.uint("max_output_bytes")?,
            },
        })
    }

    /// Validates identity rules and bounds. Called by [`Self::from_json`];
    /// also applicable to a spec built in code.
    // The bindings mirror the job-spec sections; the names are the section names.
    #[allow(clippy::many_single_char_names)]
    pub fn validate(&self) -> Result<(), ContractError> {
        let r = &self.runtime;
        let m = &self.model;
        let p = &self.profile;
        for (name, s) in [
            ("runtime.family", &r.family),
            ("runtime.mode", &r.mode),
            ("runtime.revision", &r.revision),
            ("runtime.commit", &r.commit),
            ("runtime.license", &r.license),
            ("model.family", &m.family),
            ("model.name", &m.name),
            ("model.source", &m.source),
            ("model.source_revision", &m.source_revision),
            ("model.artifact_name", &m.artifact_name),
            ("model.artifact_sha256", &m.artifact_sha256),
            ("model.format", &m.format),
            ("model.quantization", &m.quantization),
            ("model.license", &m.license),
            ("profile.id", &p.id),
            ("profile.input_contract", &p.input_contract),
            ("profile.output_contract", &p.output_contract),
            ("profile.prompt_template_sha256", &p.prompt_template_sha256),
            ("profile.output_schema_sha256", &p.output_schema_sha256),
        ] {
            check_spec_string(name, s)?;
        }
        if r.family != "llama.cpp" || r.mode != "llama-server" {
            return Err(ContractError::UnsupportedRuntime);
        }
        if FLOATING_REVISIONS.contains(&r.revision.as_str()) || !r.revision.starts_with('v') {
            return Err(ContractError::JobSpecInvalid(
                "runtime.revision must be an exact release tag (vX.Y.Z), not a floating name"
                    .into(),
            ));
        }
        check_hex("runtime.commit", &r.commit, 40)?;
        check_hex("model.artifact_sha256", &m.artifact_sha256, 64)?;
        check_hex("model.source_revision", &m.source_revision, 40)?;
        check_hex(
            "profile.prompt_template_sha256",
            &p.prompt_template_sha256,
            64,
        )?;
        check_hex("profile.output_schema_sha256", &p.output_schema_sha256, 64)?;
        if m.format != "gguf" {
            return Err(ContractError::JobSpecInvalid(
                "model.format must be gguf".into(),
            ));
        }
        if m.artifact_size_bytes == 0 {
            return Err(ContractError::JobSpecInvalid(
                "model.artifact_size_bytes must be positive".into(),
            ));
        }
        if p.version == 0 {
            return Err(ContractError::JobSpecInvalid(
                "profile.version must be positive".into(),
            ));
        }
        let g = self.generation;
        if g.temperature_milli > bounds::TEMPERATURE_MILLI_MAX {
            return Err(ContractError::InvalidGenerationParameters(
                "temperature_milli above 200".into(),
            ));
        }
        if g.top_p_milli < bounds::TOP_P_MILLI_MIN || g.top_p_milli > bounds::TOP_P_MILLI_MAX {
            return Err(ContractError::InvalidGenerationParameters(
                "top_p_milli outside [500, 1000]".into(),
            ));
        }
        if g.seed > bounds::SEED_MAX {
            return Err(ContractError::InvalidGenerationParameters(
                "seed above u32::MAX".into(),
            ));
        }
        // CC-1: the cap is keyed by profile version, so every version-1 job
        // spec keeps exactly the MVP-A bound.
        let max_tokens_cap = if self.profile.version >= 2 {
            bounds::MAX_TOKENS_CEILING_V2
        } else {
            bounds::MAX_TOKENS_MAX
        };
        if g.max_tokens == 0 || g.max_tokens > max_tokens_cap {
            return Err(ContractError::InvalidGenerationParameters(format!(
                "max_tokens outside [1, {max_tokens_cap}]"
            )));
        }
        let l = self.limits;
        if l.max_input_bytes == 0 || l.max_input_bytes > bounds::MAX_INPUT_BYTES_MAX {
            return Err(ContractError::JobSpecInvalid(
                "limits.max_input_bytes outside [1, 10000]".into(),
            ));
        }
        if l.max_output_bytes == 0 || l.max_output_bytes > bounds::MAX_OUTPUT_BYTES_MAX {
            return Err(ContractError::JobSpecInvalid(
                "limits.max_output_bytes outside [1, 8000]".into(),
            ));
        }
        if self.canonical_bytes().len() > MAX_JOB_SPEC_BYTES {
            return Err(ContractError::JobSpecInvalid(
                "canonical job spec exceeds 4096 bytes".into(),
            ));
        }
        Ok(())
    }
}

/// Names that are never an exact runtime revision.
pub const FLOATING_REVISIONS: &[&str] =
    &["", "latest", "current", "stable", "main", "master", "HEAD"];

/// `BLAKE3(DOMAIN_JOB_SPEC || bytes)` over bytes already known to be
/// canonical.
pub fn digest_of_canonical_bytes(canonical: &[u8]) -> [u8; 32] {
    mbongo_core::crypto::blake3_hash_multi(&[DOMAIN_JOB_SPEC, canonical])
}

fn check_spec_string(name: &str, s: &str) -> Result<(), ContractError> {
    if s.is_empty() {
        return Err(ContractError::JobSpecInvalid(format!("{name} is empty")));
    }
    if s.len() > 256 {
        return Err(ContractError::JobSpecInvalid(format!(
            "{name} exceeds 256 bytes"
        )));
    }
    if !s.bytes().all(|b| (0x20..=0x7E).contains(&b) && b != b'"' && b != b'\\') {
        return Err(ContractError::JobSpecInvalid(format!(
            "{name} must be printable ASCII without quote or backslash"
        )));
    }
    Ok(())
}

fn check_hex(name: &str, s: &str, len: usize) -> Result<(), ContractError> {
    if s.len() != len || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(ContractError::JobSpecInvalid(format!(
            "{name} must be {len} lowercase hex characters"
        )));
    }
    Ok(())
}

/// Strict field access over an object: exact key set, typed getters.
struct Fields<'a> {
    members: &'a std::collections::BTreeMap<String, Value>,
    what: &'static str,
}

impl<'a> Fields<'a> {
    fn new(v: &'a Value, what: &'static str, expected: &[&str]) -> Result<Self, ContractError> {
        let Value::Object(members) = v else {
            return Err(ContractError::JobSpecInvalid(format!(
                "{what} must be an object"
            )));
        };
        let keys: Vec<&str> = members.keys().map(String::as_str).collect();
        if keys != expected {
            return Err(ContractError::JobSpecInvalid(format!(
                "{what} must have exactly the fields {expected:?}"
            )));
        }
        Ok(Self { members, what })
    }

    fn get(&self, key: &str) -> Result<&'a Value, ContractError> {
        self.members
            .get(key)
            .ok_or_else(|| ContractError::JobSpecInvalid(format!("{} is missing {key}", self.what)))
    }

    fn str(&self, key: &str) -> Result<&'a str, ContractError> {
        match self.get(key)? {
            Value::String(s) => Ok(s),
            _ => Err(ContractError::JobSpecInvalid(format!(
                "{}.{key} must be a string",
                self.what
            ))),
        }
    }

    fn uint(&self, key: &str) -> Result<u64, ContractError> {
        match self.get(key)? {
            Value::Uint(n) => Ok(*n),
            _ => Err(ContractError::JobSpecInvalid(format!(
                "{}.{key} must be an integer",
                self.what
            ))),
        }
    }
}
