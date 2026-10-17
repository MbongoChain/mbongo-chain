//! Mbongo MVP Inference Contract: the named test group for
//! `docs/product/MVP_INFERENCE_CONTRACT_v1.md` (`mbongo-infer:v1`,
//! `kaayu-cv-extract:v1`). Pins the golden vector in
//! `test-vectors/inference/`, drives every validation step of the worker
//! gate, and proves that three deliberately blind pipelines accept what the
//! strict gate refuses. No model runs here.

use std::collections::BTreeMap;

use mbongo_compute::execution::{reference_input_commitment, REVERSE_BYTES_SPEC};
use mbongo_compute::inference::canonical::{self, Value};
use mbongo_compute::inference::execution_spec::{
    InferenceExecutionSpec, EXECUTION_SPEC_LEN, INFER_TAG_V1,
};
use mbongo_compute::inference::job_spec::{
    digest_of_canonical_bytes, InferenceJobSpecV1, DOMAIN_JOB_SPEC, MAX_JOB_SPEC_BYTES,
};
use mbongo_compute::inference::kaayu::{
    self, CandidateProfileV1, CvExtractionInputV1, MAX_INPUT_BYTES, MAX_OUTPUT_BYTES,
};
use mbongo_compute::inference::{
    check_input_commitment, check_model, check_runtime, decode_execution_spec, finish, preflight,
    resolve_job_spec, validate_input, ContractError, JobSpecRegistry, RepositoryProfiles,
    WorkerInferenceConfig,
};
use mbongo_core::{ComputeTask, Receipt, MAX_EXECUTION_SPEC_BYTES};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/inference");

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{FIXTURES}/{name}");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn hex_field(v: &serde_json::Value, key: &str) -> Vec<u8> {
    hex::decode(v[key].as_str().unwrap_or_else(|| panic!("{key} missing"))).expect("hex")
}

fn hex32(v: &serde_json::Value, key: &str) -> [u8; 32] {
    hex_field(v, key).try_into().expect("32 bytes")
}

fn worker_config() -> WorkerInferenceConfig {
    let spec = kaayu::job_spec();
    WorkerInferenceConfig {
        runtime_family: spec.runtime.family.clone(),
        runtime_mode: spec.runtime.mode.clone(),
        runtime_revision: spec.runtime.revision.clone(),
        runtime_commit: spec.runtime.commit.clone(),
        model_artifact_sha256: Some(spec.model.artifact_sha256.clone()),
    }
}

fn synthetic_input() -> CvExtractionInputV1 {
    let g = fixture("input-example-v1.json");
    CvExtractionInputV1::new(g["cv_text"].as_str().unwrap().to_owned()).unwrap()
}

fn execution_spec_bytes() -> Vec<u8> {
    InferenceExecutionSpec {
        job_spec_digest: kaayu::job_spec().digest(),
    }
    .encode()
}

fn code(r: &Result<impl std::fmt::Debug, ContractError>) -> &'static str {
    match r {
        Ok(_) => "OK",
        Err(e) => e.code(),
    }
}

// ── A01–A03: canonical form, digest, execution_spec ─────────────────────

#[test]
fn a01_valid_job_spec_canonicalizes() {
    let spec = kaayu::job_spec();
    let bytes = spec.canonical_bytes();
    assert!(canonical::is_canonical(&bytes, MAX_JOB_SPEC_BYTES));
    assert!(bytes.len() <= MAX_JOB_SPEC_BYTES);
    // Only ASCII without escapes: the spec-class string rule holds.
    assert!(bytes.iter().all(|b| (0x20..=0x7E).contains(b)));
    assert!(!bytes.windows(2).any(|w| w == b"\\\""));
    // Parsing the pretty-printed form yields the same bytes.
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_slice::<serde_json::Value>(&bytes).unwrap())
            .unwrap();
    assert_ne!(pretty.as_bytes(), bytes.as_slice());
    assert_eq!(InferenceJobSpecV1::from_json(&pretty).unwrap(), spec);
    assert_eq!(
        InferenceJobSpecV1::from_json(&pretty).unwrap().canonical_bytes(),
        bytes
    );
}

#[test]
fn a02_digest_is_reproducible_and_domain_separated() {
    let spec = kaayu::job_spec();
    let bytes = spec.canonical_bytes();
    let expected = mbongo_core::crypto::blake3_hash_multi(&[DOMAIN_JOB_SPEC, &bytes]);
    assert_eq!(spec.digest(), expected);
    assert_eq!(digest_of_canonical_bytes(&bytes), expected);
    assert_eq!(kaayu::job_spec().digest(), spec.digest());
    assert_eq!(DOMAIN_JOB_SPEC, b"mbongo:infer-job-spec:v1");
    // Not the task, input or output domain.
    assert_ne!(spec.digest(), mbongo_core::crypto::blake3_hash(&bytes));
    assert_ne!(spec.digest(), reference_input_commitment(&bytes));
}

#[test]
fn a03_execution_spec_round_trips_and_fits_the_bound() {
    let bytes = execution_spec_bytes();
    assert_eq!(bytes.len(), EXECUTION_SPEC_LEN);
    assert!(bytes.len() <= MAX_EXECUTION_SPEC_BYTES);
    assert!(bytes.starts_with(INFER_TAG_V1));
    let decoded = decode_execution_spec(&bytes).unwrap();
    assert_eq!(decoded.job_spec_digest, kaayu::job_spec().digest());
    assert_eq!(decoded.encode(), bytes);
    // A task carrying it is a valid RFC 0005 envelope (rule n).
    let task = ComputeTask {
        version: 1,
        submitter: mbongo_core::Address([1; 32]),
        executor: mbongo_core::Address([2; 32]),
        salt: [0; 32],
        input_commitment: synthetic_input().input_commitment(),
        execution_spec: bytes,
    };
    assert!(task.execution_spec.len() <= MAX_EXECUTION_SPEC_BYTES);
    let _ = task.task_id();
}

// ── A04–A06: refusals before the job spec is even read ──────────────────

#[test]
fn a04_unknown_version_rejected() {
    let neg = fixture("execution-spec-negative-v1.json");
    for case in neg["cases"].as_array().unwrap() {
        let bytes = hex_field(case, "execution_spec_hex");
        let expected = case["expected_error"].as_str().unwrap();
        let got = decode_execution_spec(&bytes);
        assert_eq!(code(&got), expected, "case {}", case["name"]);
    }
    let v2 = [b"mbongo-infer:v2".as_slice(), &[0u8; 32]].concat();
    assert_eq!(
        decode_execution_spec(&v2),
        Err(ContractError::UnsupportedInferenceSpecVersion)
    );
}

#[test]
fn a05_malformed_execution_spec_rejected() {
    let good = execution_spec_bytes();
    assert!(matches!(
        decode_execution_spec(&good[..46]),
        Err(ContractError::MalformedExecutionSpec(_))
    ));
    let mut long = good.clone();
    long.push(0);
    assert!(matches!(
        decode_execution_spec(&long),
        Err(ContractError::MalformedExecutionSpec(_))
    ));
    assert!(matches!(
        decode_execution_spec(REVERSE_BYTES_SPEC),
        Err(ContractError::MalformedExecutionSpec(_))
    ));
    assert!(matches!(
        decode_execution_spec(b""),
        Err(ContractError::MalformedExecutionSpec(_))
    ));
}

#[test]
fn a06_job_spec_digest_mismatch_rejected() {
    let mut spec = decode_execution_spec(&execution_spec_bytes()).unwrap();
    spec.job_spec_digest[0] ^= 1;
    assert_eq!(
        resolve_job_spec(&RepositoryProfiles, &spec),
        Err(ContractError::JobSpecNotFound)
    );
    // A registry that answers with the wrong spec is caught by the digest re-check.
    struct WrongAnswer;
    impl JobSpecRegistry for WrongAnswer {
        fn resolve(&self, _: &[u8; 32]) -> Option<InferenceJobSpecV1> {
            let mut s = kaayu::job_spec();
            s.generation.seed += 1;
            Some(s)
        }
    }
    let good = decode_execution_spec(&execution_spec_bytes()).unwrap();
    assert_eq!(
        resolve_job_spec(&WrongAnswer, &good),
        Err(ContractError::JobSpecDigestMismatch)
    );
}

// ── A07–A10: identity exact-match ───────────────────────────────────────

#[test]
fn a07_runtime_revision_exact_match() {
    let spec = kaayu::job_spec();
    let mut cfg = worker_config();
    assert_eq!(check_runtime(&spec, &cfg), Ok(()));
    cfg.runtime_revision = "v0.7.1".into();
    assert_eq!(
        check_runtime(&spec, &cfg),
        Err(ContractError::RuntimeRevisionMismatch)
    );
    let mut cfg = worker_config();
    cfg.runtime_commit = format!("{}0", &cfg.runtime_commit[..39]);
    assert_eq!(
        check_runtime(&spec, &cfg),
        Err(ContractError::RuntimeRevisionMismatch)
    );
    let mut cfg = worker_config();
    cfg.runtime_family = "vllm".into();
    assert_eq!(
        check_runtime(&spec, &cfg),
        Err(ContractError::UnsupportedRuntime)
    );
    // Floating names are not revisions at all.
    for floating in ["latest", "current", "stable", "main", ""] {
        let mut s = kaayu::job_spec();
        s.runtime.revision = floating.into();
        assert!(
            matches!(s.validate(), Err(ContractError::JobSpecInvalid(_))),
            "{floating:?}"
        );
    }
}

#[test]
fn a08_model_artifact_digest_exact_match() {
    let spec = kaayu::job_spec();
    let mut cfg = worker_config();
    assert_eq!(check_model(&spec, &cfg), Ok(()));
    cfg.model_artifact_sha256 = Some(format!("0{}", &spec.model.artifact_sha256[1..]));
    assert_eq!(
        check_model(&spec, &cfg),
        Err(ContractError::ModelDigestMismatch)
    );
    cfg.model_artifact_sha256 = None;
    assert_eq!(check_model(&spec, &cfg), Err(ContractError::ModelNotFound));
    // The same human name with a different artifact is a different model.
    let neg = fixture("job-spec-negative-v1.json");
    let case = &neg["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "model_digest_mismatch")
        .unwrap();
    let other = InferenceJobSpecV1::from_json(&case["job_spec"].to_string()).unwrap();
    assert_eq!(other.model.name, spec.model.name);
    assert_ne!(other.digest(), spec.digest());
    assert_eq!(
        check_model(&other, &worker_config()),
        Err(ContractError::ModelDigestMismatch)
    );
}

#[test]
fn a09_input_contract_version_exact_match() {
    let spec = kaayu::job_spec();
    let input = synthetic_input();
    assert!(validate_input(&spec, &input.canonical_bytes()).is_ok());
    let mut other = spec.clone();
    other.profile.input_contract = "kaayu-cv-extract-input:v2".into();
    assert_eq!(
        validate_input(&other, &input.canonical_bytes()),
        Err(ContractError::InvalidInputFormat(
            "profile input contract not supported"
        ))
    );
    let wrong_tag = br#"{"contract":"kaayu-cv-extract-input:v2","cv_text":"x"}"#;
    assert!(matches!(
        validate_input(&spec, wrong_tag),
        Err(ContractError::InvalidInputFormat(_))
    ));
}

#[test]
fn a10_output_schema_version_exact_match() {
    let spec = kaayu::job_spec();
    let g = fixture("output-canonicalization-v1.json");
    let raw = g["raw_model_output"].as_str().unwrap();
    assert!(finish(&spec, raw).is_ok());
    let mut other = spec.clone();
    other.profile.output_contract = "kaayu-cv-profile:v2".into();
    assert!(matches!(
        finish(&other, raw),
        Err(ContractError::OutputSchemaInvalid(_))
    ));
    // The pinned schema digest is the one in the job spec.
    assert_eq!(
        spec.profile.output_schema_sha256,
        kaayu::output_schema_sha256()
    );
    assert_eq!(
        spec.profile.prompt_template_sha256,
        kaayu::prompt_template_sha256()
    );
}

// ── A11–A12: bounds ─────────────────────────────────────────────────────

#[test]
fn a11_invalid_generation_params_rejected() {
    let base = kaayu::job_spec();
    type Mutation = Box<dyn Fn(&mut InferenceJobSpecV1)>;
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "temperature",
            Box::new(|s| s.generation.temperature_milli = 201),
        ),
        ("top_p low", Box::new(|s| s.generation.top_p_milli = 499)),
        ("top_p high", Box::new(|s| s.generation.top_p_milli = 1001)),
        (
            "seed",
            Box::new(|s| s.generation.seed = u64::from(u32::MAX) + 1),
        ),
        ("max_tokens zero", Box::new(|s| s.generation.max_tokens = 0)),
        (
            "max_tokens high",
            Box::new(|s| s.generation.max_tokens = 401),
        ),
    ];
    for (name, mutate) in cases {
        let mut s = base.clone();
        mutate(&mut s);
        assert!(
            matches!(
                s.validate(),
                Err(ContractError::InvalidGenerationParameters(_))
            ),
            "{name}"
        );
    }
    let neg = fixture("job-spec-negative-v1.json");
    for case in neg["cases"].as_array().unwrap() {
        let got = InferenceJobSpecV1::from_json(&case["job_spec"].to_string());
        let expected = case["expected"].as_str().unwrap();
        if expected == "OK" {
            assert!(got.is_ok(), "case {}: {got:?}", case["name"]);
        } else {
            assert_eq!(code(&got), expected, "case {}", case["name"]);
        }
    }
}

#[test]
fn a12_input_too_large_rejected_by_contract_validator() {
    let big = "x".repeat(MAX_INPUT_BYTES);
    assert_eq!(
        CvExtractionInputV1::new(big.clone()),
        Err(ContractError::InputTooLarge)
    );
    let spec = kaayu::job_spec();
    let bytes = format!(r#"{{"contract":"kaayu-cv-extract-input:v1","cv_text":"{big}"}}"#);
    assert_eq!(
        validate_input(&spec, bytes.as_bytes()),
        Err(ContractError::InputTooLarge)
    );
    // A spec with a tighter limit rejects an input the profile would accept.
    let input = synthetic_input();
    let mut tight = spec.clone();
    tight.limits.max_input_bytes = 10;
    assert_eq!(
        validate_input(&tight, &input.canonical_bytes()),
        Err(ContractError::InputTooLarge)
    );
    // Empty and control characters.
    assert!(matches!(
        CvExtractionInputV1::new("  \n".into()),
        Err(ContractError::InvalidInputFormat(_))
    ));
    assert!(matches!(
        CvExtractionInputV1::new("a\u{0}b".into()),
        Err(ContractError::InvalidInputFormat(_))
    ));
}

// ── A13: output canonicalization ────────────────────────────────────────

#[test]
fn a13_canonical_output_is_stable() {
    let spec = kaayu::job_spec();
    let g = fixture("output-canonicalization-v1.json");
    let raw = g["raw_model_output"].as_str().unwrap();
    let r = finish(&spec, raw).unwrap();
    assert_eq!(r.bytes, hex_field(&g, "canonical_bytes_hex"));
    assert_eq!(r.output_commitment, hex32(&g, "output_commitment"));
    assert!(canonical::is_canonical(&r.bytes, MAX_OUTPUT_BYTES));
    // Whitespace and key order do not change the commitment; content does.
    let reordered = g["raw_model_output_reordered"].as_str().unwrap();
    assert_ne!(reordered, raw);
    assert_eq!(
        finish(&spec, reordered).unwrap().output_commitment,
        r.output_commitment
    );
    let mut changed = r.profile.clone();
    changed.seniority = "junior".into();
    assert_ne!(changed.output_commitment(), r.output_commitment);
    // Idempotent.
    let again = finish(&spec, std::str::from_utf8(&r.bytes).unwrap()).unwrap();
    assert_eq!(again.bytes, r.bytes);
    // Invalid outputs.
    for case in g["invalid"].as_array().unwrap() {
        let raw = case["raw_model_output"].as_str().unwrap();
        assert_eq!(
            code(&finish(&spec, raw)),
            case["expected_error"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

// ── A14–A17: what moves the digest, and what never enters execution_spec ─

#[test]
fn a14_model_or_runtime_change_changes_the_digest() {
    let base = kaayu::job_spec();
    let d0 = base.digest();
    let mut s = base.clone();
    s.model.artifact_sha256 = format!("0{}", &s.model.artifact_sha256[1..]);
    assert_ne!(s.digest(), d0);
    let mut s = base.clone();
    s.runtime.revision = "v0.8.0".into();
    assert_ne!(s.digest(), d0);
    let mut s = base.clone();
    s.profile.prompt_template_sha256 = format!("0{}", &s.profile.prompt_template_sha256[1..]);
    assert_ne!(s.digest(), d0);
    let mut s = base.clone();
    s.generation.seed = 43;
    assert_ne!(s.digest(), d0);
    let mut s = base.clone();
    s.profile.version = 2;
    assert_ne!(s.digest(), d0);
    // And a changed digest is a different execution_spec.
    assert_ne!(
        InferenceExecutionSpec {
            job_spec_digest: s.digest()
        }
        .encode(),
        execution_spec_bytes()
    );
}

#[test]
fn a15_input_payload_does_not_affect_the_job_spec_digest() {
    let spec = kaayu::job_spec();
    let a = CvExtractionInputV1::new("First synthetic CV. Skills: Rust.".into()).unwrap();
    let b = CvExtractionInputV1::new("Second synthetic CV. Skills: Go.".into()).unwrap();
    assert_ne!(a.input_commitment(), b.input_commitment());
    // The execution_spec is a function of the job spec only.
    let es = InferenceExecutionSpec {
        job_spec_digest: spec.digest(),
    }
    .encode();
    assert_eq!(es, execution_spec_bytes());
    // input_commitment and job_spec_digest are different authorities.
    assert_ne!(a.input_commitment(), spec.digest());
}

#[test]
fn a16_private_input_absent_from_execution_spec() {
    let input = synthetic_input();
    let es = execution_spec_bytes();
    let hay = String::from_utf8_lossy(&es);
    for word in input.cv_text.split_whitespace().filter(|w| w.len() >= 4) {
        assert!(!hay.contains(word), "input word {word:?} in execution_spec");
    }
    assert!(!es.windows(8).any(|w| input.canonical_bytes().windows(8).any(|iw| iw == w)));
    assert_eq!(es.len(), 47);
}

#[test]
fn a17_private_result_absent_from_execution_spec_and_job_spec() {
    let g = fixture("output-canonicalization-v1.json");
    let result = hex_field(&g, "canonical_bytes_hex");
    let es = execution_spec_bytes();
    let js = kaayu::job_spec().canonical_bytes();
    let text = String::from_utf8(result).unwrap();
    let profile = CandidateProfileV1::from_model_output(&text).unwrap();
    for s in [&profile.summary, &profile.location].into_iter().chain(profile.skills.iter()) {
        if s.len() >= 6 {
            assert!(
                !String::from_utf8_lossy(&js).contains(s.as_str()),
                "{s:?} in job spec"
            );
            assert!(!String::from_utf8_lossy(&es).contains(s.as_str()));
        }
    }
}

// ── A18–A19: wire shapes untouched ──────────────────────────────────────

#[test]
fn a18_receipt_wire_unchanged() {
    let v = serde_json::to_value(Receipt {
        version: 1,
        task_id: [0; 32],
        input_commitment: [0; 32],
        output_commitment: [0; 32],
        executor: mbongo_core::Address([0; 32]),
        metadata: vec![],
        signature: [0; 64],
    })
    .unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "executor",
            "input_commitment",
            "metadata",
            "output_commitment",
            "signature",
            "task_id",
            "version"
        ]
    );
    for forbidden in ["model", "runtime", "job_spec", "model_id", "runtime_id"] {
        assert!(!v.as_object().unwrap().contains_key(forbidden));
    }
}

#[test]
fn a19_compute_task_wire_unchanged() {
    let task = ComputeTask {
        version: 1,
        submitter: mbongo_core::Address([1; 32]),
        executor: mbongo_core::Address([2; 32]),
        salt: [0; 32],
        input_commitment: [3; 32],
        execution_spec: execution_spec_bytes(),
    };
    let v = serde_json::to_value(&task).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "execution_spec",
            "executor",
            "input_commitment",
            "salt",
            "submitter",
            "version"
        ]
    );
    assert_eq!(MAX_EXECUTION_SPEC_BYTES, 1024);
    // The inference spec is just bytes to the envelope: SCALE length prefix + 47 bytes
    // after the 22-byte task domain.
    let preimage = task.task_id_preimage();
    assert_eq!(preimage.len(), 22 + 1 + 32 + 32 + 32 + 32 + 1 + 47);
}

// ── A20: the golden vector ──────────────────────────────────────────────

#[test]
fn a20_golden_vector_exact() {
    let g = fixture("kaayu-cv-extract-v1.json");
    let spec = kaayu::job_spec();
    assert_eq!(g["contract"], "mbongo-infer:v1");
    assert_eq!(g["profile"], "kaayu-cv-extract:v1");
    assert_eq!(
        spec.canonical_bytes(),
        hex_field(&g, "job_spec_canonical_hex")
    );
    assert_eq!(
        String::from_utf8(spec.canonical_bytes()).unwrap(),
        g["job_spec_canonical"].as_str().unwrap()
    );
    assert_eq!(spec.digest(), hex32(&g, "job_spec_digest"));
    assert_eq!(execution_spec_bytes(), hex_field(&g, "execution_spec_hex"));
    assert_eq!(
        g["job_spec_domain"],
        std::str::from_utf8(DOMAIN_JOB_SPEC).unwrap()
    );
    assert_eq!(g["prompt_template_sha256"], kaayu::prompt_template_sha256());
    assert_eq!(g["output_schema_sha256"], kaayu::output_schema_sha256());
    // The committed job-spec document in docs/ is byte-identical to the canonical bytes.
    let committed = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/product/inference/kaayu-cv-extract-v1/job-spec.json"
    ))
    .unwrap();
    assert_eq!(committed, spec.canonical_bytes());
    // The job spec object in the fixture parses to the same spec.
    assert_eq!(
        InferenceJobSpecV1::from_json(&g["job_spec"].to_string()).unwrap(),
        spec
    );
}

// ── A21–A24: the full gate, input vector, prompt, schema agreement ───────

#[test]
fn a21_preflight_order_and_full_pass() {
    let input = synthetic_input();
    let bytes = input.canonical_bytes();
    let commitment = input.input_commitment();
    let cfg = worker_config();
    let p = preflight(
        &execution_spec_bytes(),
        &RepositoryProfiles,
        &cfg,
        &bytes,
        &commitment,
    )
    .unwrap();
    assert_eq!(p.job_spec, kaayu::job_spec());
    assert_eq!(p.input, input);
    // Each step fails closed, in order: the first failing step wins.
    let bad_spec = [b"mbongo-infer:v9".as_slice(), &[0; 32]].concat();
    let mut bad_cfg = cfg.clone();
    bad_cfg.model_artifact_sha256 = None;
    assert_eq!(
        preflight(
            &bad_spec,
            &RepositoryProfiles,
            &bad_cfg,
            &bytes,
            &commitment
        ),
        Err(ContractError::UnsupportedInferenceSpecVersion)
    );
    assert_eq!(
        preflight(
            &execution_spec_bytes(),
            &RepositoryProfiles,
            &bad_cfg,
            b"garbage",
            &commitment
        ),
        Err(ContractError::ModelNotFound)
    );
    assert_eq!(
        preflight(
            &execution_spec_bytes(),
            &RepositoryProfiles,
            &cfg,
            b"garbage",
            &commitment
        ),
        Err(ContractError::InputCommitmentMismatch)
    );
    let garbage_commitment = reference_input_commitment(b"garbage");
    assert!(matches!(
        preflight(
            &execution_spec_bytes(),
            &RepositoryProfiles,
            &cfg,
            b"garbage",
            &garbage_commitment
        ),
        Err(ContractError::InvalidInputFormat(_))
    ));
    assert_eq!(check_input_commitment(&bytes, &commitment), Ok(()));
}

#[test]
fn a22_input_vector_exact() {
    let g = fixture("input-example-v1.json");
    let input = synthetic_input();
    assert_eq!(
        input.canonical_bytes(),
        hex_field(&g, "canonical_bytes_hex")
    );
    assert_eq!(input.input_commitment(), hex32(&g, "input_commitment"));
    assert_eq!(
        CvExtractionInputV1::from_bytes(&input.canonical_bytes()).unwrap(),
        input
    );
    // Synthetic: no e-mail, no phone-looking run of digits.
    assert!(!input.cv_text.contains('@'));
    assert!(!input.cv_text.chars().filter(char::is_ascii_digit).count().gt(&40));
}

#[test]
fn a23_prompt_is_template_plus_input_only() {
    let input = synthetic_input();
    let p = kaayu::build_prompt(&input);
    assert_eq!(p.system, kaayu::PROMPT_TEMPLATE);
    assert_eq!(p.user, input.cv_text);
    assert!(p.system.contains("kaayu-cv-profile:v1"));
    assert!(!p.system.contains(&input.cv_text));
}

#[test]
fn a24_typed_validator_agrees_with_the_schema_file() {
    let schema: serde_json::Value = serde_json::from_str(kaayu::OUTPUT_SCHEMA).unwrap();
    let props = schema["properties"].as_object().unwrap();
    let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "certifications",
            "education",
            "experience",
            "languages",
            "location",
            "seniority",
            "skills",
            "summary"
        ]
    );
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let mut required_sorted = required.clone();
    required_sorted.sort_unstable();
    assert_eq!(required_sorted, keys);
    assert_eq!(schema["additionalProperties"], false);
    let sen: Vec<&str> = props["seniority"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(sen, kaayu::SENIORITY);
    let lvl: Vec<&str> = props["languages"]["items"]["properties"]["level"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(lvl, kaayu::LANGUAGE_LEVEL);
    assert_eq!(props["summary"]["maxLength"], 600);
    assert_eq!(props["skills"]["maxItems"], 40);
    assert_eq!(props["experience"]["maxItems"], 15);
    // The validator enforces the same bound.
    let mut v = BTreeMap::new();
    for k in [
        "certifications",
        "education",
        "experience",
        "languages",
        "skills",
    ] {
        v.insert(k.to_owned(), Value::Array(vec![]));
    }
    v.insert("location".into(), Value::String(String::new()));
    v.insert("seniority".into(), Value::String("unknown".into()));
    v.insert("summary".into(), Value::String("x".repeat(601)));
    assert!(matches!(
        CandidateProfileV1::from_value(&Value::Object(v)),
        Err(ContractError::OutputSchemaInvalid(_))
    ));
}

// ── negative controls: blind pipelines accept what the strict gate refuses ─

#[test]
fn negative_controls_are_caught() {
    let input = synthetic_input();
    let bytes = input.canonical_bytes();
    let commitment = input.input_commitment();
    let cfg = worker_config();

    // Digest-blind: resolves whatever the registry answers without re-hashing.
    struct WrongAnswer;
    impl JobSpecRegistry for WrongAnswer {
        fn resolve(&self, _: &[u8; 32]) -> Option<InferenceJobSpecV1> {
            let mut s = kaayu::job_spec();
            s.generation.temperature_milli = 200;
            Some(s)
        }
    }
    let spec = decode_execution_spec(&execution_spec_bytes()).unwrap();
    let digest_blind = WrongAnswer.resolve(&spec.job_spec_digest).map(|s| s.validate().map(|()| s));
    assert!(
        matches!(digest_blind, Some(Ok(_))),
        "digest-blind pipeline accepted the substituted spec"
    );
    assert_eq!(
        resolve_job_spec(&WrongAnswer, &spec),
        Err(ContractError::JobSpecDigestMismatch)
    );

    // Version-blind: reads a digest from any mbongo-infer:* spec.
    let v2 = [b"mbongo-infer:v2".as_slice(), &spec.job_spec_digest].concat();
    let version_blind = InferenceExecutionSpec {
        job_spec_digest: v2[15..].try_into().unwrap(),
    };
    assert!(
        resolve_job_spec(&RepositoryProfiles, &version_blind).is_ok(),
        "version-blind accepted v2"
    );
    assert_eq!(
        decode_execution_spec(&v2),
        Err(ContractError::UnsupportedInferenceSpecVersion)
    );

    // Model-digest-blind: trusts the model name, skips the artifact digest.
    let mut other_artifact = cfg.clone();
    other_artifact.model_artifact_sha256 = Some("ab".repeat(32));
    let job_spec = kaayu::job_spec();
    let model_blind = check_runtime(&job_spec, &other_artifact)
        .and_then(|()| check_input_commitment(&bytes, &commitment))
        .and_then(|()| validate_input(&job_spec, &bytes));
    assert!(
        model_blind.is_ok(),
        "model-digest-blind pipeline accepted another artifact"
    );
    assert_eq!(
        preflight(
            &execution_spec_bytes(),
            &RepositoryProfiles,
            &other_artifact,
            &bytes,
            &commitment
        ),
        Err(ContractError::ModelDigestMismatch)
    );
}
