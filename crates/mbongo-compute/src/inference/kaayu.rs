//! `kaayu-cv-extract:v1`: the frozen MVP profile (MVP-A §11–§17, §42–§44).
//!
//! The generic contract `mbongo-infer:v1` says how a job spec is encoded,
//! committed and verified. This profile fills it in for the one MVP
//! workload: **CV text → identity-free JSON candidate profile**. It is a
//! repository-defined constant, not a registry entry anywhere else.
//!
//! | Artifact | Where | Pinned by |
//! |---|---|---|
//! | prompt template (the system message, verbatim) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v1/prompt-template.txt` | `profile.prompt_template_sha256` |
//! | output JSON Schema (`kaayu-cv-profile:v1`) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v1/output-schema.json` | `profile.output_schema_sha256` |
//! | job spec (canonical bytes) | `crates/mbongo-compute/profiles/kaayu-cv-extract-v1/job-spec.json` | the digest in `execution_spec` |
//!
//! The typed output validator below is the executable form of the schema
//! file; a test asserts the two agree on their field set, bounds and
//! enumerations.

use sha2::{Digest, Sha256};

use super::canonical::{self, Value};
use super::job_spec::{
    GenerationParams, InferenceJobSpecV1, Limits, ModelIdentity, ProfileIdentity, RuntimeIdentity,
};
use super::ContractError;

/// The prompt template: used verbatim as the chat **system** message.
pub const PROMPT_TEMPLATE: &str =
    include_str!("../../profiles/kaayu-cv-extract-v1/prompt-template.txt");
/// The output JSON Schema (`kaayu-cv-profile:v1`), passed to llama-server
/// as `response_format` and mirrored by [`CandidateProfileV1::validate`].
pub const OUTPUT_SCHEMA: &str =
    include_str!("../../profiles/kaayu-cv-extract-v1/output-schema.json");

/// Profile id.
pub const PROFILE_ID: &str = "kaayu-cv-extract";
/// Profile version.
pub const PROFILE_VERSION: u64 = 1;
/// Input contract tag.
pub const INPUT_CONTRACT_V1: &str = "kaayu-cv-extract-input:v1";
/// Output contract tag.
pub const OUTPUT_CONTRACT_V1: &str = "kaayu-cv-profile:v1";
/// Canonical input-object bound, aligned with ≤ 2,500 input tokens at a
/// conservative four bytes per token. Token counts are never a contract
/// rule; bytes are.
pub const MAX_INPUT_BYTES: usize = 10_000;
/// Raw model-output bound before canonicalization.
pub const MAX_OUTPUT_BYTES: usize = 8_000;

/// Lowercase hex SHA-256 of the prompt template bytes.
pub fn prompt_template_sha256() -> String {
    hex::encode(Sha256::digest(PROMPT_TEMPLATE.as_bytes()))
}

/// Lowercase hex SHA-256 of the output schema bytes.
pub fn output_schema_sha256() -> String {
    hex::encode(Sha256::digest(OUTPUT_SCHEMA.as_bytes()))
}

/// The frozen MVP job spec. Every value here is a product decision of
/// `docs/product/MVP_v0.1.md` §5–§6 made exact; changing any of them
/// changes the digest and therefore the `execution_spec` of every task.
pub fn job_spec() -> InferenceJobSpecV1 {
    InferenceJobSpecV1 {
        runtime: RuntimeIdentity {
            family: "llama.cpp".into(),
            mode: "llama-server".into(),
            revision: "v0.7.0".into(),
            commit: "620d2225cc436ab0b001e3200e590e3b286ef9ab".into(),
            license: "MIT".into(),
        },
        model: ModelIdentity {
            family: "Qwen3".into(),
            name: "Qwen3-4B-Instruct-2507".into(),
            source: "https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF".into(),
            source_revision: "a06e946bb6b655725eafa393f4a9745d460374c9".into(),
            artifact_name: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf".into(),
            artifact_sha256: "3605803b982cb64aead44f6c1b2ae36e3acdb41d8e46c8a94c6533bc4c67e597"
                .into(),
            artifact_size_bytes: 2_497_281_120,
            format: "gguf".into(),
            quantization: "Q4_K_M".into(),
            license: "Apache-2.0".into(),
        },
        profile: ProfileIdentity {
            id: PROFILE_ID.into(),
            version: PROFILE_VERSION,
            input_contract: INPUT_CONTRACT_V1.into(),
            output_contract: OUTPUT_CONTRACT_V1.into(),
            prompt_template_sha256: prompt_template_sha256(),
            output_schema_sha256: output_schema_sha256(),
        },
        generation: GenerationParams {
            temperature_milli: 0,
            top_p_milli: 1000,
            seed: 42,
            max_tokens: 400,
        },
        limits: Limits {
            max_input_bytes: MAX_INPUT_BYTES as u64,
            max_output_bytes: MAX_OUTPUT_BYTES as u64,
        },
    }
}

// ── input ────────────────────────────────────────────────────────────────

/// The private input object (§12): `{"contract":"kaayu-cv-extract-input:v1","cv_text":"…"}`
/// in canonical form. Its canonical bytes are what `input_commitment`
/// commits to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CvExtractionInputV1 {
    /// The CV as UTF-8 text.
    pub cv_text: String,
}

impl CvExtractionInputV1 {
    /// Validates the text (non-empty; only `\n`, `\r`, `\t` among control
    /// characters) and the canonical size bound.
    pub fn new(cv_text: String) -> Result<Self, ContractError> {
        if cv_text.trim().is_empty() {
            return Err(ContractError::InvalidInputFormat("cv_text is empty"));
        }
        if cv_text.chars().any(|c| (c as u32) < 0x20 && !matches!(c, '\n' | '\r' | '\t')) {
            return Err(ContractError::InvalidInputFormat(
                "cv_text contains a control character other than newline, carriage return or tab",
            ));
        }
        let input = Self { cv_text };
        if input.canonical_bytes().len() > MAX_INPUT_BYTES {
            return Err(ContractError::InputTooLarge);
        }
        Ok(input)
    }

    /// The canonical input-object bytes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical::serialize(&canonical::build::obj(vec![
            ("contract", canonical::build::s(INPUT_CONTRACT_V1)),
            ("cv_text", canonical::build::s(&self.cv_text)),
        ]))
    }

    /// Parses the input object as fetched from the private data plane
    /// (canonical or not) and validates it.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ContractError> {
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(ContractError::InputTooLarge);
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| ContractError::InvalidInputFormat("input is not UTF-8"))?;
        let v = canonical::parse(text, MAX_INPUT_BYTES).map_err(|_| {
            ContractError::InvalidInputFormat("input is not a constrained JSON object")
        })?;
        let Value::Object(m) = &v else {
            return Err(ContractError::InvalidInputFormat(
                "input must be a JSON object",
            ));
        };
        let keys: Vec<&str> = m.keys().map(String::as_str).collect();
        if keys != ["contract", "cv_text"] {
            return Err(ContractError::InvalidInputFormat(
                "input must have exactly the fields contract and cv_text",
            ));
        }
        match m.get("contract") {
            Some(Value::String(c)) if c == INPUT_CONTRACT_V1 => {}
            _ => {
                return Err(ContractError::InvalidInputFormat(
                    "unsupported input contract",
                ))
            }
        }
        let Some(Value::String(text)) = m.get("cv_text") else {
            return Err(ContractError::InvalidInputFormat(
                "cv_text must be a string",
            ));
        };
        Self::new(text.clone())
    }

    /// RFC 0005 §2.4 convention: `BLAKE3("mbongo:compute-input:v1" || canonical bytes)`.
    pub fn input_commitment(&self) -> [u8; 32] {
        crate::execution::reference_input_commitment(&self.canonical_bytes())
    }
}

/// The chat prompt (§13): the template is the system message verbatim and
/// the CV text is the user message. No substitution, no other message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt<'a> {
    /// System message: [`PROMPT_TEMPLATE`].
    pub system: &'static str,
    /// User message: the CV text.
    pub user: &'a str,
}

/// Builds the prompt for an input.
pub fn build_prompt(input: &CvExtractionInputV1) -> Prompt<'_> {
    Prompt {
        system: PROMPT_TEMPLATE,
        user: &input.cv_text,
    }
}

// ── output ───────────────────────────────────────────────────────────────

/// Allowed `seniority` values.
pub const SENIORITY: &[&str] = &["unknown", "junior", "mid", "senior", "lead", "executive"];
/// Allowed language `level` values.
pub const LANGUAGE_LEVEL: &[&str] = &["unknown", "basic", "intermediate", "fluent", "native"];

/// A position (`experience[]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Experience {
    /// ≤ 120 characters.
    pub title: String,
    /// ≤ 120 characters.
    pub organization: String,
    /// ≤ 10 characters, as written.
    pub start: String,
    /// ≤ 10 characters, as written, or `"present"`.
    pub end: String,
    /// ≤ 300 characters.
    pub description: String,
}

/// A degree or programme (`education[]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Education {
    /// ≤ 120 characters.
    pub degree: String,
    /// ≤ 120 characters.
    pub institution: String,
    /// ≤ 10 characters.
    pub year: String,
}

/// A language (`languages[]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// ≤ 40 characters.
    pub language: String,
    /// One of [`LANGUAGE_LEVEL`].
    pub level: String,
}

/// The typed form of `kaayu-cv-profile:v1`. Field bounds are character
/// counts (Unicode scalar values), matching JSON Schema `maxLength`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateProfileV1 {
    /// ≤ 600 characters.
    pub summary: String,
    /// One of [`SENIORITY`].
    pub seniority: String,
    /// ≤ 120 characters; city, region or country only.
    pub location: String,
    /// ≤ 40 items of ≤ 60 characters.
    pub skills: Vec<String>,
    /// ≤ 15 items.
    pub experience: Vec<Experience>,
    /// ≤ 10 items.
    pub education: Vec<Education>,
    /// ≤ 10 items.
    pub languages: Vec<Language>,
    /// ≤ 15 items of ≤ 100 characters.
    pub certifications: Vec<String>,
}

impl CandidateProfileV1 {
    /// Parses raw model output (any JSON inside the constrained model) and
    /// validates it against the schema. Errors name the field, never the
    /// content.
    pub fn from_model_output(raw: &str) -> Result<Self, ContractError> {
        if raw.len() > MAX_OUTPUT_BYTES {
            return Err(ContractError::OutputSchemaInvalid(
                "output exceeds max_output_bytes".into(),
            ));
        }
        let v = canonical::parse(raw, MAX_OUTPUT_BYTES)
            .map_err(|e| ContractError::OutputNotJson(e.to_string()))?;
        Self::from_value(&v)
    }

    /// Validates a parsed value against the schema.
    pub fn from_value(v: &Value) -> Result<Self, ContractError> {
        let o = Obj::new(
            v,
            "profile",
            &[
                "certifications",
                "education",
                "experience",
                "languages",
                "location",
                "seniority",
                "skills",
                "summary",
            ],
        )?;
        let profile = Self {
            summary: o.str("summary", 600)?,
            seniority: o.enumerated("seniority", SENIORITY)?,
            location: o.str("location", 120)?,
            skills: o.str_array("skills", 40, 60)?,
            experience: o
                .array("experience", 15)?
                .iter()
                .map(|e| {
                    let e = Obj::new(
                        e,
                        "experience[]",
                        &["description", "end", "organization", "start", "title"],
                    )?;
                    Ok(Experience {
                        title: e.str("title", 120)?,
                        organization: e.str("organization", 120)?,
                        start: e.str("start", 10)?,
                        end: e.str("end", 10)?,
                        description: e.str("description", 300)?,
                    })
                })
                .collect::<Result<_, ContractError>>()?,
            education: o
                .array("education", 10)?
                .iter()
                .map(|e| {
                    let e = Obj::new(e, "education[]", &["degree", "institution", "year"])?;
                    Ok(Education {
                        degree: e.str("degree", 120)?,
                        institution: e.str("institution", 120)?,
                        year: e.str("year", 10)?,
                    })
                })
                .collect::<Result<_, ContractError>>()?,
            languages: o
                .array("languages", 10)?
                .iter()
                .map(|e| {
                    let e = Obj::new(e, "languages[]", &["language", "level"])?;
                    Ok(Language {
                        language: e.str("language", 40)?,
                        level: e.enumerated("level", LANGUAGE_LEVEL)?,
                    })
                })
                .collect::<Result<_, ContractError>>()?,
            certifications: o.str_array("certifications", 15, 100)?,
        };
        Ok(profile)
    }

    /// The constrained canonical model of this profile.
    pub fn to_value(&self) -> Value {
        use canonical::build::{obj, s};
        obj(vec![
            (
                "certifications",
                Value::Array(self.certifications.iter().map(|c| s(c)).collect()),
            ),
            (
                "education",
                Value::Array(
                    self.education
                        .iter()
                        .map(|e| {
                            obj(vec![
                                ("degree", s(&e.degree)),
                                ("institution", s(&e.institution)),
                                ("year", s(&e.year)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "experience",
                Value::Array(
                    self.experience
                        .iter()
                        .map(|e| {
                            obj(vec![
                                ("description", s(&e.description)),
                                ("end", s(&e.end)),
                                ("organization", s(&e.organization)),
                                ("start", s(&e.start)),
                                ("title", s(&e.title)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "languages",
                Value::Array(
                    self.languages
                        .iter()
                        .map(|l| obj(vec![("language", s(&l.language)), ("level", s(&l.level))]))
                        .collect(),
                ),
            ),
            ("location", s(&self.location)),
            ("seniority", s(&self.seniority)),
            (
                "skills",
                Value::Array(self.skills.iter().map(|c| s(c)).collect()),
            ),
            ("summary", s(&self.summary)),
        ])
    }

    /// The canonical result bytes: what the worker persists and what
    /// `output_commitment` commits to (§15, §17).
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical::serialize(&self.to_value())
    }

    /// RFC 0005 §2.4 convention: `BLAKE3("mbongo:compute-output:v1" || canonical bytes)`.
    pub fn output_commitment(&self) -> [u8; 32] {
        crate::execution::reference_output_commitment(&self.canonical_bytes())
    }
}

/// Strict object access for the output validator.
struct Obj<'a> {
    members: &'a std::collections::BTreeMap<String, Value>,
    what: &'static str,
}

impl<'a> Obj<'a> {
    fn new(v: &'a Value, what: &'static str, expected: &[&str]) -> Result<Self, ContractError> {
        let Value::Object(members) = v else {
            return Err(ContractError::OutputSchemaInvalid(format!(
                "{what} must be an object"
            )));
        };
        let keys: Vec<&str> = members.keys().map(String::as_str).collect();
        if keys != expected {
            return Err(ContractError::OutputSchemaInvalid(format!(
                "{what} must have exactly the fields {expected:?}"
            )));
        }
        Ok(Self { members, what })
    }

    fn get(&self, key: &str) -> Result<&'a Value, ContractError> {
        self.members.get(key).ok_or_else(|| {
            ContractError::OutputSchemaInvalid(format!("{}.{key} missing", self.what))
        })
    }

    fn str(&self, key: &str, max_chars: usize) -> Result<String, ContractError> {
        match self.get(key)? {
            Value::String(s) if s.chars().count() <= max_chars => Ok(s.clone()),
            Value::String(_) => Err(ContractError::OutputSchemaInvalid(format!(
                "{}.{key} exceeds {max_chars} characters",
                self.what
            ))),
            _ => Err(ContractError::OutputSchemaInvalid(format!(
                "{}.{key} must be a string",
                self.what
            ))),
        }
    }

    fn enumerated(&self, key: &str, allowed: &[&str]) -> Result<String, ContractError> {
        let s = self.str(key, 32)?;
        if allowed.contains(&s.as_str()) {
            Ok(s)
        } else {
            Err(ContractError::OutputSchemaInvalid(format!(
                "{}.{key} is not one of the allowed values",
                self.what
            )))
        }
    }

    fn array(&self, key: &str, max_items: usize) -> Result<&'a [Value], ContractError> {
        match self.get(key)? {
            Value::Array(items) if items.len() <= max_items => Ok(items),
            Value::Array(_) => Err(ContractError::OutputSchemaInvalid(format!(
                "{}.{key} exceeds {max_items} items",
                self.what
            ))),
            _ => Err(ContractError::OutputSchemaInvalid(format!(
                "{}.{key} must be an array",
                self.what
            ))),
        }
    }

    fn str_array(
        &self,
        key: &str,
        max_items: usize,
        max_chars: usize,
    ) -> Result<Vec<String>, ContractError> {
        self.array(key, max_items)?
            .iter()
            .map(|item| match item {
                Value::String(s) if s.chars().count() <= max_chars => Ok(s.clone()),
                Value::String(_) => Err(ContractError::OutputSchemaInvalid(format!(
                    "{}.{key}[] exceeds {max_chars} characters",
                    self.what
                ))),
                _ => Err(ContractError::OutputSchemaInvalid(format!(
                    "{}.{key}[] must be a string",
                    self.what
                ))),
            })
            .collect()
    }
}
