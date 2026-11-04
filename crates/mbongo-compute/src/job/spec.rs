//! The `mbongo-job:v1` job spec: the contract as data.
//!
//! A job spec fully describes a job, in the canonical JSON of
//! [`crate::inference::canonical`]. `workload` and `params` stay opaque:
//! nothing here interprets a client's workload.

use std::collections::{BTreeMap, BTreeSet};

use crate::inference::canonical::{self, Value};

use super::error::JobError;

/// The tag that opens a `mbongo-job:v1` `execution_spec`.
pub const JOB_TAG: &[u8] = b"mbongo-job:v1";
/// The domain of the job-spec digest.
pub const DOMAIN_JOB_SPEC: &[u8] = b"mbongo:job-spec:v1";
/// Exact length of an `execution_spec`: the tag and a 32-byte digest.
pub const EXECUTION_SPEC_LEN: usize = JOB_TAG.len() + 32;
/// Upper bound on a job spec's canonical bytes.
pub const MAX_JOB_SPEC_BYTES: usize = 65_536;
/// Upper bound on a short string of the job spec.
const MAX_LABEL_BYTES: usize = 128;
/// The classes of `provider-policy-v1`.
const CONFIDENTIALITY_CLASSES: &[&str] = &["PUBLIC", "VERIFIED", "CONFIDENTIAL", "SOVEREIGN"];

/// `BLAKE3(DOMAIN_JOB_SPEC || canonical_bytes)`.
pub fn job_spec_digest(canonical_bytes: &[u8]) -> [u8; 32] {
    mbongo_core::crypto::blake3_hash_multi(&[DOMAIN_JOB_SPEC, canonical_bytes])
}

/// The `execution_spec` that commits to the job spec with this digest.
pub fn encode_execution_spec(digest: &[u8; 32]) -> Vec<u8> {
    [JOB_TAG, digest.as_slice()].concat()
}

/// The digest an `execution_spec` commits to.
pub fn parse_execution_spec(execution_spec: &[u8]) -> Result<[u8; 32], JobError> {
    execution_spec
        .strip_prefix(JOB_TAG)
        .and_then(|digest| <[u8; 32]>::try_from(digest).ok())
        .ok_or(JobError::MalformedExecutionSpec)
}

/// How reproducible a job's output is. Decides which verification is possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Determinism {
    /// The same output on any conforming worker.
    BitExact,
    /// Bit-exact only within one host class.
    BitExactPinned,
    /// Equal within a declared tolerance.
    Tolerance,
    /// No re-execution equality.
    NonDeterministic,
}

/// The type of a job's input or output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// Raw bytes.
    Bytes,
    /// A document in the crate's canonical JSON.
    CanonicalJson,
}

/// A bounded, typed input or output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataContract {
    /// Its type.
    pub data_type: DataType,
    /// Its largest allowed size.
    pub max_bytes: u64,
}

/// The runtime a job needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRequirement {
    /// Selects the adapter.
    pub family: String,
    /// The exact revision.
    pub revision: String,
    /// The exact build, if the job pins one.
    pub binary_sha256: Option<String>,
    /// `cpu`, or a provider-defined accelerator class.
    pub accelerator_class: String,
}

/// A program, weights or table the job needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// What the runtime calls it.
    pub role: String,
    /// Lowercase hex SHA-256.
    pub sha256: String,
    /// Exact size.
    pub size_bytes: u64,
}

/// A parsed, validated job spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSpec {
    /// The client's label. Opaque.
    pub workload: String,
    /// The runtime it needs.
    pub runtime: RuntimeRequirement,
    /// What it needs provisioned.
    pub artifacts: Vec<ArtifactRef>,
    /// Its input.
    pub input: DataContract,
    /// Its output.
    pub output: DataContract,
    /// Wall-clock limit, enforced by the worker.
    pub max_wall_ms: u64,
    /// Memory limit: declared, not enforced ([`super::MEMORY_LIMIT_ENFORCED`]).
    pub max_memory_mib: u64,
    /// How reproducible its output is.
    pub determinism: Determinism,
    /// The classes it may run under.
    pub confidentiality: Vec<String>,
    /// Passed to the adapter. Opaque.
    pub params: Value,
}

type Members = BTreeMap<String, Value>;

/// The members of `v`, none outside `allowed`.
fn object<'a>(
    v: &'a Value,
    allowed: &[&str],
    field: &'static str,
) -> Result<&'a Members, JobError> {
    let Value::Object(members) = v else {
        return Err(JobError::SpecInvalid(field));
    };
    if members.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(JobError::SpecInvalid(field));
    }
    Ok(members)
}

fn member<'a>(m: &'a Members, key: &'static str) -> Result<&'a Value, JobError> {
    m.get(key).ok_or(JobError::SpecInvalid(key))
}

fn label(m: &Members, key: &'static str) -> Result<String, JobError> {
    match member(m, key)? {
        Value::String(s) if !s.is_empty() && s.len() <= MAX_LABEL_BYTES => Ok(s.clone()),
        _ => Err(JobError::SpecInvalid(key)),
    }
}

fn positive(m: &Members, key: &'static str) -> Result<u64, JobError> {
    match member(m, key)? {
        Value::Uint(n) if *n > 0 => Ok(*n),
        _ => Err(JobError::SpecInvalid(key)),
    }
}

fn sha256_hex(s: &str, field: &'static str) -> Result<String, JobError> {
    let valid =
        s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    valid.then(|| s.to_owned()).ok_or(JobError::SpecInvalid(field))
}

fn data_contract(v: &Value, field: &'static str) -> Result<DataContract, JobError> {
    let m = object(v, &["type", "max_bytes"], field)?;
    let data_type = match label(m, "type")?.as_str() {
        "bytes" => DataType::Bytes,
        "canonical-json" => DataType::CanonicalJson,
        _ => return Err(JobError::SpecInvalid("type")),
    };
    Ok(DataContract {
        data_type,
        max_bytes: positive(m, "max_bytes")?,
    })
}

fn runtime(v: &Value) -> Result<RuntimeRequirement, JobError> {
    let keys = ["family", "revision", "binary_sha256", "accelerator_class"];
    let m = object(v, &keys, "runtime")?;
    let binary_sha256 = m
        .contains_key("binary_sha256")
        .then(|| sha256_hex(&label(m, "binary_sha256")?, "binary_sha256"))
        .transpose()?;
    Ok(RuntimeRequirement {
        family: label(m, "family")?,
        revision: label(m, "revision")?,
        binary_sha256,
        accelerator_class: label(m, "accelerator_class")?,
    })
}

fn artifacts(v: &Value) -> Result<Vec<ArtifactRef>, JobError> {
    let Value::Array(items) = v else {
        return Err(JobError::SpecInvalid("artifacts"));
    };
    let mut roles = BTreeSet::new();
    items
        .iter()
        .map(|item| {
            let m = object(item, &["role", "sha256", "size_bytes"], "artifacts")?;
            let role = label(m, "role")?;
            if !roles.insert(role.clone()) {
                return Err(JobError::SpecInvalid("role"));
            }
            Ok(ArtifactRef {
                role,
                sha256: sha256_hex(&label(m, "sha256")?, "sha256")?,
                size_bytes: positive(m, "size_bytes")?,
            })
        })
        .collect()
}

fn confidentiality(v: &Value) -> Result<Vec<String>, JobError> {
    const FIELD: &str = "confidentiality";
    let Value::Array(items) = v else {
        return Err(JobError::SpecInvalid(FIELD));
    };
    if items.is_empty() {
        return Err(JobError::SpecInvalid(FIELD));
    }
    items
        .iter()
        .map(|item| match item {
            Value::String(s) if CONFIDENTIALITY_CLASSES.contains(&s.as_str()) => Ok(s.clone()),
            _ => Err(JobError::SpecInvalid(FIELD)),
        })
        .collect()
}

impl JobSpec {
    /// Parses a job spec. Bytes that are not already canonical are refused,
    /// never normalised: the digest commits to exact bytes.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, JobError> {
        const KEYS: &[&str] = &[
            "workload",
            "runtime",
            "artifacts",
            "input",
            "output",
            "limits",
            "determinism",
            "confidentiality",
            "params",
        ];
        let text = std::str::from_utf8(bytes).map_err(|_| JobError::SpecInvalid("encoding"))?;
        let root = canonical::parse(text, MAX_JOB_SPEC_BYTES)
            .map_err(|_| JobError::SpecInvalid("json"))?;
        if canonical::serialize(&root) != bytes {
            return Err(JobError::SpecInvalid("not canonical"));
        }
        let m = object(&root, KEYS, "job spec")?;
        let limits = object(
            member(m, "limits")?,
            &["max_wall_ms", "max_memory_mib"],
            "limits",
        )?;
        let determinism = match label(m, "determinism")?.as_str() {
            "BIT_EXACT" => Determinism::BitExact,
            "BIT_EXACT_PINNED" => Determinism::BitExactPinned,
            "TOLERANCE" => Determinism::Tolerance,
            "NON_DETERMINISTIC" => Determinism::NonDeterministic,
            _ => return Err(JobError::SpecInvalid("determinism")),
        };
        let params = member(m, "params")?;
        if !matches!(params, Value::Object(_)) {
            return Err(JobError::SpecInvalid("params"));
        }
        Ok(Self {
            workload: label(m, "workload")?,
            runtime: runtime(member(m, "runtime")?)?,
            artifacts: artifacts(member(m, "artifacts")?)?,
            input: data_contract(member(m, "input")?, "input")?,
            output: data_contract(member(m, "output")?, "output")?,
            max_wall_ms: positive(limits, "max_wall_ms")?,
            max_memory_mib: positive(limits, "max_memory_mib")?,
            determinism,
            confidentiality: confidentiality(member(m, "confidentiality")?)?,
            params: params.clone(),
        })
    }
}
