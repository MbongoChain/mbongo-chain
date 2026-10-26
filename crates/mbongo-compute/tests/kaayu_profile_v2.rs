//! Mbongo Kaayu CV Profile v2: the named test group for
//! `docs/product/KAAYU_CV_EXTRACT_PROFILE_v2.md` (change control CC-1 of
//! `docs/product/MVP_v0.1.md`). Pins the v2 golden vector, the v2 bounds and
//! budget, the maximal-output analysis, and proves that v1 and the generic
//! contract `mbongo-infer:v1` still reproduce. No model runs here.

use mbongo_compute::execution::{reference_input_commitment, reference_output_commitment};
use mbongo_compute::inference::canonical;
use mbongo_compute::inference::execution_spec::{
    InferenceExecutionSpec, EXECUTION_SPEC_LEN, INFER_TAG_V1,
};
use mbongo_compute::inference::job_spec::{
    bounds as generic_bounds, InferenceJobSpecV1, DOMAIN_JOB_SPEC,
};
use mbongo_compute::inference::kaayu::{self, CandidateProfileV1, CvExtractionInputV1};
use mbongo_compute::inference::{
    preflight, validate_input, ContractError, JobSpecRegistry, WorkerInferenceConfig,
};
use mbongo_compute::profiles::{self, kaayu_v2, KaayuProfiles, ACTIVE_MVP_PROFILE};
use mbongo_core::{ComputeTask, Receipt, MAX_EXECUTION_SPEC_BYTES};

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/inference");
const PROFILES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/profiles");

fn vector(name: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{VECTORS}/{name}")).unwrap()).unwrap()
}

fn s<'a>(v: &'a serde_json::Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or_else(|| panic!("{k}"))
}

fn schema_v2() -> serde_json::Value {
    serde_json::from_str(kaayu_v2::OUTPUT_SCHEMA).unwrap()
}

fn worker_config() -> WorkerInferenceConfig {
    let spec = kaayu_v2::job_spec();
    WorkerInferenceConfig {
        runtime_family: spec.runtime.family.clone(),
        runtime_mode: spec.runtime.mode.clone(),
        runtime_revision: spec.runtime.revision.clone(),
        runtime_commit: spec.runtime.commit.clone(),
        model_artifact_sha256: Some(spec.model.artifact_sha256),
    }
}

fn v2_spec_bytes() -> Vec<u8> {
    InferenceExecutionSpec {
        job_spec_digest: kaayu_v2::job_spec().digest(),
    }
    .encode()
}

// ── V201: v1 untouched ──────────────────────────────────────────────────

#[test]
fn v201_v1_profile_is_unchanged_and_reproduces() {
    let g = vector("kaayu-cv-extract-v1.json");
    let v1 = kaayu::job_spec();
    assert_eq!(hex::encode(v1.digest()), s(&g, "job_spec_digest"));
    assert_eq!(
        hex::encode(v1.canonical_bytes()),
        s(&g, "job_spec_canonical_hex")
    );
    assert_eq!(
        kaayu::prompt_template_sha256(),
        s(&g, "prompt_template_sha256")
    );
    assert_eq!(kaayu::output_schema_sha256(), s(&g, "output_schema_sha256"));
    assert_eq!(v1.generation.max_tokens, 400);
    let committed = std::fs::read(format!("{PROFILES}/kaayu-cv-extract-v1/job-spec.json")).unwrap();
    assert_eq!(committed, v1.canonical_bytes());
    // v1 output parsing still works on the v1 fixture.
    let out = vector("output-canonicalization-v1.json");
    let r = mbongo_compute::inference::finish(&v1, s(&out, "raw_model_output")).unwrap();
    assert_eq!(hex::encode(r.bytes), s(&out, "canonical_bytes_hex"));
}

// ── V202–V206: the v2 vector ────────────────────────────────────────────

#[test]
fn v202_v2_schema_parses_and_names_the_v2_contract() {
    let schema = schema_v2();
    assert_eq!(schema["$id"], "kaayu-cv-profile:v2");
    assert_eq!(schema["additionalProperties"], false);
    let v1: serde_json::Value = serde_json::from_str(kaayu::OUTPUT_SCHEMA).unwrap();
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v["properties"].as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(keys(&schema), keys(&v1), "same field set as v1");
    assert_eq!(schema["required"], v1["required"]);
    assert_eq!(
        schema["properties"]["seniority"]["enum"],
        v1["properties"]["seniority"]["enum"]
    );
    assert_eq!(
        schema["properties"]["languages"]["items"]["properties"]["level"]["enum"],
        v1["properties"]["languages"]["items"]["properties"]["level"]["enum"]
    );
    assert!(!schema.to_string().contains("\"name\""), "identity-free");
}

#[test]
fn v203_v2_prompt_digest_is_exact() {
    let g = vector("kaayu-cv-extract-v2.json");
    assert_eq!(
        kaayu_v2::prompt_template_sha256(),
        s(&g, "prompt_template_sha256")
    );
    assert_eq!(
        kaayu_v2::prompt_template_sha256(),
        "93ec9176e8d267ec1753476cbdc26862f8672cff14c1c41e2104b084c39a1f87"
    );
}

#[test]
fn v204_v2_schema_digest_is_exact() {
    let g = vector("kaayu-cv-extract-v2.json");
    assert_eq!(
        kaayu_v2::output_schema_sha256(),
        s(&g, "output_schema_sha256")
    );
    assert_eq!(
        kaayu_v2::output_schema_sha256(),
        "0a81727a7cebdc73e97869f82211c07c7a67912e44b0ee24274003a67d9750ab"
    );
}

#[test]
fn v205_v2_job_spec_digest_is_exact() {
    let g = vector("kaayu-cv-extract-v2.json");
    let spec = kaayu_v2::job_spec();
    assert_eq!(
        hex::encode(spec.canonical_bytes()),
        s(&g, "job_spec_canonical_hex")
    );
    assert_eq!(hex::encode(spec.digest()), s(&g, "job_spec_digest"));
    assert_eq!(
        hex::encode(spec.digest()),
        "f38886b234dd411be1b90b25f94b21e4ec10afa5720317c3630dc0b9df4aac6f"
    );
    let committed = std::fs::read(format!("{PROFILES}/kaayu-cv-extract-v2/job-spec.json")).unwrap();
    assert_eq!(
        committed,
        spec.canonical_bytes(),
        "committed document is the canonical bytes"
    );
    assert_eq!(
        InferenceJobSpecV1::from_json(&g["job_spec"].to_string()).unwrap(),
        spec
    );
    // Only the controlled fields differ from v1.
    let mut back = spec.clone();
    let v1 = kaayu::job_spec();
    back.profile.version = 1;
    back.profile.output_contract = v1.profile.output_contract.clone();
    back.profile.prompt_template_sha256 = v1.profile.prompt_template_sha256.clone();
    back.profile.output_schema_sha256 = v1.profile.output_schema_sha256.clone();
    back.generation.max_tokens = 400;
    assert_eq!(
        back, v1,
        "v2 differs from v1 only in version, output contract, two digests and max_tokens"
    );
}

#[test]
fn v206_v2_execution_spec_vector_is_exact() {
    let g = vector("kaayu-cv-extract-v2.json");
    let es = v2_spec_bytes();
    assert_eq!(hex::encode(&es), s(&g, "execution_spec_hex"));
    assert_eq!(es.len(), EXECUTION_SPEC_LEN);
    assert!(es.starts_with(INFER_TAG_V1), "same framing, new digest");
    assert_ne!(
        es,
        InferenceExecutionSpec {
            job_spec_digest: kaayu::job_spec().digest()
        }
        .encode()
    );
}

// ── V207–V213: the bounds, schema file == code == chosen values ─────────

fn schema_num(path: &[&str]) -> u64 {
    let mut v = schema_v2();
    for p in path {
        v = v[*p].clone();
    }
    v.as_u64().unwrap_or_else(|| panic!("{path:?}"))
}

fn base_profile() -> CandidateProfileV1 {
    let out = vector("output-canonicalization-v1.json");
    kaayu_v2::parse_output(s(&out, "raw_model_output"))
        .expect("the v1 fixture output is also v2-valid")
}

fn rejects(p: &CandidateProfileV1) -> bool {
    matches!(
        kaayu_v2::check_v2_bounds(p),
        Err(ContractError::OutputSchemaInvalid(_))
    )
}

#[test]
fn v207_experience_max_items_is_5() {
    use kaayu_v2::bounds as b;
    assert_eq!(b::EXPERIENCE_ITEMS, 5);
    assert_eq!(schema_num(&["properties", "experience", "maxItems"]), 5);
    let mut p = base_profile();
    let e = p.experience[0].clone();
    p.experience = vec![e; 6];
    assert!(rejects(&p));
    p.experience.pop();
    assert!(!rejects(&p));
}

#[test]
fn v208_description_max_length_is_160() {
    use kaayu_v2::bounds as b;
    assert_eq!(b::DESCRIPTION, 160);
    assert_eq!(
        schema_num(&[
            "properties",
            "experience",
            "items",
            "properties",
            "description",
            "maxLength"
        ]),
        160
    );
    let mut p = base_profile();
    p.experience[0].description = "é".repeat(160);
    assert!(!rejects(&p), "160 characters, 320 bytes: characters count");
    p.experience[0].description.push('x');
    assert!(rejects(&p));
    // The other experience string bounds.
    assert_eq!(
        schema_num(&[
            "properties",
            "experience",
            "items",
            "properties",
            "title",
            "maxLength"
        ]) as usize,
        b::TITLE
    );
    assert_eq!(
        schema_num(&[
            "properties",
            "experience",
            "items",
            "properties",
            "organization",
            "maxLength"
        ]) as usize,
        b::ORGANIZATION
    );
    assert_eq!(
        schema_num(&[
            "properties",
            "experience",
            "items",
            "properties",
            "start",
            "maxLength"
        ]) as usize,
        b::DATE
    );
    assert_eq!(
        schema_num(&[
            "properties",
            "experience",
            "items",
            "properties",
            "end",
            "maxLength"
        ]) as usize,
        b::DATE
    );
}

#[test]
fn v209_summary_max_length_is_200() {
    assert_eq!(kaayu_v2::bounds::SUMMARY, 200);
    assert_eq!(schema_num(&["properties", "summary", "maxLength"]), 200);
    assert_eq!(
        schema_num(&["properties", "location", "maxLength"]) as usize,
        kaayu_v2::bounds::LOCATION
    );
    let mut p = base_profile();
    p.summary = "s".repeat(201);
    assert!(rejects(&p));
}

#[test]
fn v210_skills_max_items_is_10() {
    use kaayu_v2::bounds as b;
    assert_eq!(b::SKILLS_ITEMS, 10);
    assert_eq!(schema_num(&["properties", "skills", "maxItems"]), 10);
    assert_eq!(
        schema_num(&["properties", "skills", "items", "maxLength"]) as usize,
        b::SKILL
    );
    let mut p = base_profile();
    p.skills = vec!["Rust".into(); 11];
    assert!(rejects(&p));
}

#[test]
fn v211_education_max_items_is_2() {
    use kaayu_v2::bounds as b;
    assert_eq!(b::EDUCATION_ITEMS, 2);
    assert_eq!(schema_num(&["properties", "education", "maxItems"]), 2);
    assert_eq!(
        schema_num(&[
            "properties",
            "education",
            "items",
            "properties",
            "year",
            "maxLength"
        ]) as usize,
        b::YEAR
    );
    let mut p = base_profile();
    let e = p.education[0].clone();
    p.education = vec![e; 3];
    assert!(rejects(&p));
}

#[test]
fn v212_certifications_max_items_is_3() {
    assert_eq!(kaayu_v2::bounds::CERTIFICATIONS_ITEMS, 3);
    assert_eq!(schema_num(&["properties", "certifications", "maxItems"]), 3);
    let mut p = base_profile();
    p.certifications = vec!["CKA".into(); 4];
    assert!(rejects(&p));
}

#[test]
fn v213_languages_max_items_is_3() {
    assert_eq!(kaayu_v2::bounds::LANGUAGES_ITEMS, 3);
    assert_eq!(schema_num(&["properties", "languages", "maxItems"]), 3);
    let mut p = base_profile();
    let l = p.languages[0].clone();
    p.languages = vec![l; 4];
    assert!(rejects(&p));
}

// ── V214–V215: the budget and the maximal-output analysis ───────────────

#[test]
fn v214_max_tokens_is_600_within_the_cc1_ceiling() {
    let spec = kaayu_v2::job_spec();
    assert_eq!(spec.generation.max_tokens, 600);
    assert!(spec.generation.max_tokens <= generic_bounds::MAX_TOKENS_CEILING_V2);
    assert_eq!(spec.validate(), Ok(()));
    let mut over = spec.clone();
    over.generation.max_tokens = 601;
    assert!(matches!(
        over.validate(),
        Err(ContractError::InvalidGenerationParameters(_))
    ));
    // Version 1 keeps the MVP-A cap exactly.
    let mut v1 = kaayu::job_spec();
    v1.generation.max_tokens = 401;
    assert!(matches!(
        v1.validate(),
        Err(ContractError::InvalidGenerationParameters(_))
    ));
}

#[test]
fn v215_maximal_legal_output_fits_the_budget_with_headroom() {
    let g = vector("kaayu-cv-extract-v2.json");
    let m = &g["maximal_legal_output"];
    let maximal = kaayu_v2::maximal_profile();
    assert_eq!(
        kaayu_v2::check_v2_bounds(&maximal),
        Ok(()),
        "maximal profile is legal"
    );
    let bytes = maximal.canonical_bytes();
    assert_eq!(
        bytes.len() as u64,
        m["canonical_bytes"].as_u64().unwrap(),
        "same object as measured"
    );
    let digest = {
        use sha2::Digest as _;
        hex::encode(sha2::Sha256::digest(&bytes))
    };
    assert_eq!(digest, s(m, "canonical_hex_sha256"));
    let tokens = m["tokens_compact"].as_u64().unwrap();
    let budget = kaayu_v2::job_spec().generation.max_tokens;
    assert!(
        tokens < budget,
        "theoretical maximum {tokens} below the budget {budget}"
    );
    assert!(
        budget - tokens >= budget / 10,
        "at least 10% headroom on the theoretical maximum"
    );
    // v1's maximal output was far beyond its own budget: this is the defect CC-1 fixes.
    assert!(
        m["tokens_pretty_2space"].as_u64().unwrap() > tokens,
        "compact output is what the budget assumes"
    );
    // Every cardinality and length is at its bound; one more of anything is illegal.
    let mut more = maximal.clone();
    more.skills.push("x".into());
    assert!(rejects(&more));
}

// ── V216–V220: what CC-1 must not change ────────────────────────────────

#[test]
fn v216_canonicalization_is_unchanged() {
    let out = vector("output-canonicalization-v1.json");
    let spec = kaayu_v2::job_spec();
    let a = kaayu_v2::finish(&spec, s(&out, "raw_model_output")).unwrap();
    let b = kaayu_v2::finish(&spec, s(&out, "raw_model_output_reordered")).unwrap();
    assert_eq!(a.bytes, b.bytes, "whitespace and key order do not matter");
    assert_eq!(
        hex::encode(&a.bytes),
        s(&out, "canonical_bytes_hex"),
        "same serializer, same bytes as MVP-A"
    );
    assert_eq!(a.output_commitment, reference_output_commitment(&a.bytes));
    assert_eq!(
        hex::encode(a.output_commitment),
        s(&out, "output_commitment")
    );
    assert!(canonical::is_canonical(&a.bytes, kaayu::MAX_OUTPUT_BYTES));
}

#[test]
fn v217_input_contract_is_unchanged() {
    let spec = kaayu_v2::job_spec();
    assert_eq!(spec.profile.input_contract, kaayu::INPUT_CONTRACT_V1);
    assert_eq!(spec.limits, kaayu::job_spec().limits);
    let g = vector("input-example-v1.json");
    let input = CvExtractionInputV1::new(s(&g, "cv_text").to_owned()).unwrap();
    assert_eq!(
        hex::encode(input.input_commitment()),
        s(&g, "input_commitment")
    );
    assert_eq!(
        validate_input(&spec, &input.canonical_bytes()).unwrap(),
        input
    );
    assert_eq!(
        reference_input_commitment(&input.canonical_bytes()),
        input.input_commitment()
    );
}

#[test]
fn v218_receipt_semantics_are_unchanged() {
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
}

#[test]
fn v219_compute_task_wire_is_unchanged() {
    let task = ComputeTask {
        version: 1,
        submitter: mbongo_core::Address([1; 32]),
        executor: mbongo_core::Address([2; 32]),
        salt: [0; 32],
        input_commitment: [3; 32],
        execution_spec: v2_spec_bytes(),
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
    assert!(task.execution_spec.len() <= MAX_EXECUTION_SPEC_BYTES);
    assert_eq!(task.task_id_preimage().len(), 22 + 1 + 32 * 4 + 1 + 47);
}

#[test]
fn v220_generic_inference_contract_is_unchanged() {
    assert_eq!(INFER_TAG_V1, b"mbongo-infer:v1");
    assert_eq!(DOMAIN_JOB_SPEC, b"mbongo:infer-job-spec:v1");
    assert_eq!(kaayu_v2::job_spec().runtime, kaayu::job_spec().runtime);
    assert_eq!(kaayu_v2::job_spec().model, kaayu::job_spec().model);
    assert_eq!(kaayu_v2::job_spec().generation.temperature_milli, 0);
    assert_eq!(kaayu_v2::job_spec().generation.seed, 42);
    // The full generic gate accepts a v2 task through the repository registry.
    let input =
        CvExtractionInputV1::new("Warehouse operator. Forklift licence. Three years.".into())
            .unwrap();
    let p = preflight(
        &v2_spec_bytes(),
        &KaayuProfiles,
        &worker_config(),
        &input.canonical_bytes(),
        &input.input_commitment(),
    )
    .unwrap();
    assert_eq!(p.job_spec, kaayu_v2::job_spec());
}

// ── more ────────────────────────────────────────────────────────────────

#[test]
fn v221_registry_resolves_both_profiles_and_nothing_else() {
    assert_eq!(
        KaayuProfiles.resolve(&kaayu_v2::job_spec().digest()),
        Some(kaayu_v2::job_spec())
    );
    assert_eq!(
        KaayuProfiles.resolve(&kaayu::job_spec().digest()),
        Some(kaayu::job_spec())
    );
    assert_eq!(KaayuProfiles.resolve(&[0u8; 32]), None);
    assert_eq!(ACTIVE_MVP_PROFILE, "kaayu-cv-extract:v2");
    assert_eq!(profiles::active_job_spec(), kaayu_v2::job_spec());
}

#[test]
fn v222_v2_finish_refuses_v1_sized_and_malformed_output() {
    let spec = kaayu_v2::job_spec();
    let mut big = base_profile();
    let e = big.experience[0].clone();
    big.experience = vec![e; 12];
    let raw = String::from_utf8(big.canonical_bytes()).unwrap();
    assert!(
        mbongo_compute::inference::finish(&kaayu::job_spec(), &raw).is_ok(),
        "legal under v1"
    );
    assert!(
        matches!(
            kaayu_v2::finish(&spec, &raw),
            Err(ContractError::OutputSchemaInvalid(_))
        ),
        "illegal under v2"
    );
    assert!(matches!(
        kaayu_v2::finish(&spec, "Here is the profile: {}"),
        Err(ContractError::OutputNotJson(_))
    ));
    assert!(
        matches!(
            kaayu_v2::finish(&kaayu::job_spec(), &raw),
            Err(ContractError::OutputSchemaInvalid(_))
        ),
        "v2 finish refuses a v1 spec"
    );
}

#[test]
fn v223_finish_dispatches_by_output_contract() {
    let out = vector("output-canonicalization-v1.json");
    let raw = s(&out, "raw_model_output");
    assert_eq!(
        profiles::finish(&kaayu::job_spec(), raw).unwrap().bytes,
        profiles::finish(&kaayu_v2::job_spec(), raw).unwrap().bytes
    );
    let mut other = kaayu_v2::job_spec();
    other.profile.output_contract = "kaayu-cv-profile:v3".into();
    assert!(profiles::finish(&other, raw).is_err());
}

#[test]
fn v224_prompt_requires_concise_extraction_only() {
    let p = kaayu_v2::PROMPT_TEMPLATE;
    for must in [
        "kaayu-cv-profile:v2",
        "compact on a single line",
        "never transcribe the whole CV",
        "include exactly the 5 most recent",
        "employment positions only",
        "do not rank the candidate",
        "recommend hiring",
        "protected attributes",
        "Never invent facts",
        "Never include the candidate's name",
    ] {
        assert!(p.contains(must), "prompt must say: {must}");
    }
    assert!(
        p.len() < kaayu::PROMPT_TEMPLATE.len(),
        "v2 prompt is shorter than v1"
    );
}

#[test]
fn v225_profile_assets_are_pinned_per_version() {
    assert_eq!(
        profiles::assets(&kaayu_v2::job_spec()),
        Some((kaayu_v2::PROMPT_TEMPLATE, kaayu_v2::OUTPUT_SCHEMA))
    );
    assert_eq!(
        profiles::assets(&kaayu::job_spec()),
        Some((kaayu::PROMPT_TEMPLATE, kaayu::OUTPUT_SCHEMA))
    );
    let mut other = kaayu_v2::job_spec();
    other.profile.version = 3;
    assert_eq!(profiles::assets(&other), None);
}
