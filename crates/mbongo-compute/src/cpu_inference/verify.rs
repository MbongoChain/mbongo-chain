//! Start-up verification (MVP-B §7, §9, §35, §93): the frozen profile
//! assets, the runtime executable and its self-reported revision, and the
//! model file — each against an expected value fixed before the worker ran.
//! A worker that fails any of these never becomes ready.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::inference::job_spec::InferenceJobSpecV1;
use crate::inference::{kaayu, ContractError};

use super::config::LLAMA_CPP_BUILD_NUMBER;

/// The frozen MVP-A job-spec digest (`test-vectors/inference/kaayu-cv-extract-v1.json`).
/// A rebuilt worker whose profile files changed computes another digest and
/// refuses to start.
pub const FROZEN_JOB_SPEC_DIGEST: &str =
    "be9ae960334c3e917ad600e2b5220440c27dd79c5e586e998e60cbc148affe0e";
/// The frozen prompt-template SHA-256.
pub const FROZEN_PROMPT_TEMPLATE_SHA256: &str =
    "6c950b4863bc339af94d904dbb609b3dd9098259740f01a6539406e641276d63";
/// The frozen output-schema SHA-256.
pub const FROZEN_OUTPUT_SCHEMA_SHA256: &str =
    "81521447e7b7b45c670ff1717d608fc2304765ab530587aba2370ba0a9191af5";

/// Streaming SHA-256 of a file, lowercase hex.
pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

/// Step 0: the profile the worker was built with is the frozen one.
pub fn verify_profile_assets(spec: &InferenceJobSpecV1) -> Result<(), ContractError> {
    if kaayu::prompt_template_sha256() != FROZEN_PROMPT_TEMPLATE_SHA256
        || spec.profile.prompt_template_sha256 != FROZEN_PROMPT_TEMPLATE_SHA256
    {
        return Err(ContractError::JobSpecInvalid(
            "prompt template differs from the frozen profile".into(),
        ));
    }
    if kaayu::output_schema_sha256() != FROZEN_OUTPUT_SCHEMA_SHA256
        || spec.profile.output_schema_sha256 != FROZEN_OUTPUT_SCHEMA_SHA256
    {
        return Err(ContractError::JobSpecInvalid(
            "output schema differs from the frozen profile".into(),
        ));
    }
    if hex::encode(spec.digest()) != FROZEN_JOB_SPEC_DIGEST {
        return Err(ContractError::JobSpecDigestMismatch);
    }
    spec.validate()
}

/// What `llama-server --version` says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportedVersion {
    /// e.g. `0.7.0-dev`.
    pub version: String,
    /// e.g. `11669`.
    pub build: u64,
    /// Abbreviated commit, e.g. `620d2225c`.
    pub commit_prefix: String,
}

/// Parses `version: 0.7.0-dev (build 11669, commit 620d2225c)`.
pub fn parse_version(text: &str) -> Option<ReportedVersion> {
    let line = text.lines().find(|l| l.trim_start().starts_with("version:"))?;
    let rest = line.trim_start().strip_prefix("version:")?.trim();
    let (version, tail) = rest.split_once(' ')?;
    let inner = tail.trim().strip_prefix('(')?.strip_suffix(')')?;
    let mut build = None;
    let mut commit = None;
    for part in inner.split(',') {
        let part = part.trim();
        if let Some(b) = part.strip_prefix("build ") {
            build = b.trim().parse().ok();
        } else if let Some(c) = part.strip_prefix("commit ") {
            commit = Some(c.trim().to_owned());
        }
    }
    Some(ReportedVersion {
        version: version.to_owned(),
        build: build?,
        commit_prefix: commit?,
    })
}

/// Parses `/props` `build_info`, `b11669-620d2225c`.
pub fn parse_build_info(text: &str) -> Option<(u64, String)> {
    let (b, c) = text.strip_prefix('b')?.split_once('-')?;
    Some((b.parse().ok()?, c.to_owned()))
}

/// A reported commit prefix names the job spec's commit: at least 7 lowercase
/// hex characters, and a prefix of the 40-character commit.
pub fn commit_matches(prefix: &str, full: &str) -> bool {
    prefix.len() >= 7
        && prefix.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        && full.starts_with(prefix)
}

/// The reported version is the job spec's runtime: same release core
/// (`v0.7.0` ↔ `0.7.0`, a `-dev` build stamp tolerated because upstream
/// stamps every CI build so), the pinned build number, and the job spec's
/// commit. The commit is authoritative.
pub fn check_reported_version(
    v: &ReportedVersion,
    spec: &InferenceJobSpecV1,
) -> Result<(), ContractError> {
    let core = v.version.split('-').next().unwrap_or_default();
    let wanted = spec.runtime.revision.trim_start_matches('v');
    if core != wanted
        || v.build != LLAMA_CPP_BUILD_NUMBER
        || !commit_matches(&v.commit_prefix, &spec.runtime.commit)
    {
        return Err(ContractError::RuntimeRevisionMismatch);
    }
    Ok(())
}

/// Verifies the runtime executable: present, the pinned SHA-256, and a
/// `--version` that names the job spec's revision and commit. The process is
/// started with structured arguments only.
pub fn verify_runtime_binary(
    runtime_path: &Path,
    expected_sha256: &str,
    spec: &InferenceJobSpecV1,
) -> Result<ReportedVersion, ContractError> {
    if !runtime_path.is_file() {
        return Err(ContractError::UnsupportedRuntime);
    }
    let actual = sha256_file(runtime_path).map_err(|_| ContractError::UnsupportedRuntime)?;
    if actual != expected_sha256 {
        return Err(ContractError::RuntimeRevisionMismatch);
    }
    let out = std::process::Command::new(runtime_path)
        .arg("--version")
        .output()
        .map_err(|_| ContractError::UnsupportedRuntime)?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let v = parse_version(&text).ok_or(ContractError::RuntimeRevisionMismatch)?;
    check_reported_version(&v, spec)?;
    Ok(v)
}

/// Verifies the model file: present, the job spec's size, and the job
/// spec's SHA-256 over every byte. Returns the digest.
pub fn verify_model(model_path: &Path, spec: &InferenceJobSpecV1) -> Result<String, ContractError> {
    let meta = std::fs::metadata(model_path).map_err(|_| ContractError::ModelNotFound)?;
    if !meta.is_file() {
        return Err(ContractError::ModelNotFound);
    }
    if meta.len() != spec.model.artifact_size_bytes {
        return Err(ContractError::ModelDigestMismatch);
    }
    let actual = sha256_file(model_path).map_err(|_| ContractError::ModelNotFound)?;
    if actual != spec.model.artifact_sha256 {
        return Err(ContractError::ModelDigestMismatch);
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_pinned_version_line() {
        let v = parse_version("version: 0.7.0-dev (build 11669, commit 620d2225c)\nbuilt with Clang 20.1.8 for Windows x86_64\n").unwrap();
        assert_eq!(v.version, "0.7.0-dev");
        assert_eq!(v.build, 11_669);
        assert_eq!(v.commit_prefix, "620d2225c");
        assert_eq!(
            parse_build_info("b11669-620d2225c"),
            Some((11_669, "620d2225c".into()))
        );
        assert!(parse_version("nothing here").is_none());
        assert!(commit_matches(
            "620d2225c",
            "620d2225cc436ab0b001e3200e590e3b286ef9ab"
        ));
        assert!(!commit_matches(
            "620d22",
            "620d2225cc436ab0b001e3200e590e3b286ef9ab"
        ));
        assert!(!commit_matches(
            "620d2225d",
            "620d2225cc436ab0b001e3200e590e3b286ef9ab"
        ));
    }
}
