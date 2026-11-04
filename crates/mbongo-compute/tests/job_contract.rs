//! `mbongo-job:v1`: the G2 Slice 1 acceptance tests.
//!
//! Test ids are those of `docs/architecture/G2_MBONGO_JOB_V1_PLAN.md` §6. The
//! fixtures and the mock worker exist only here: they test the infrastructure
//! and are not workloads. Expected values come from
//! `test-vectors/job/job-contract-v1.json`, which an independent Node
//! implementation computes.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mbongo_compute::cpu_inference::backend::testing::FakeBackend;
use mbongo_compute::cpu_inference::{InferenceBackend, InferenceProfile, ProfileDispatch};
use mbongo_compute::execution::{
    reference_input_commitment, reference_output_commitment, ExecutionError, ExecutionProfile,
};
use mbongo_compute::inference::{canonical, kaayu};
use mbongo_compute::job::{
    check_output, encode_execution_spec, job_spec_digest, parse_execution_spec, verify_result,
    Blame, DataContract, DataType, Determinism, FailureClass, JobError, JobProfile, JobSource,
    JobSpec, MeteringSink, ResolvedJob, RunOutput, RuntimeAdapter, RuntimeFailed, RuntimeIdentity,
    TechnicalLevel, UntrustedMetering, DOMAIN_JOB_SPEC, EXECUTION_SPEC_LEN, JOB_TAG,
    MEMORY_LIMIT_ENFORCED,
};
use serde_json::{json, Value};

// ── vectors ───────────────────────────────────────────────────────────────

fn vectors() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/job/job-contract-v1.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn table() -> Vec<u8> {
    hex::decode(vectors()["table"].as_str().unwrap()).unwrap()
}

/// The vector spec of a fixture, as JSON to mutate.
fn spec_json(name: &str) -> Value {
    vectors()["specs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .unwrap()["spec"]
        .clone()
}

fn canon(v: &Value) -> Vec<u8> {
    canonical::canonicalize(&serde_json::to_string(v).unwrap(), 1 << 20).unwrap()
}

// ── the provider's side, in memory ───────────────────────────────────────

#[derive(Default)]
struct MemorySource {
    specs: Mutex<BTreeMap<[u8; 32], Vec<u8>>>,
    artifacts: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl MemorySource {
    /// Holds a job spec and returns its `execution_spec`.
    fn add_spec(&self, bytes: Vec<u8>) -> Vec<u8> {
        let digest = job_spec_digest(&bytes);
        self.specs.lock().unwrap().insert(digest, bytes);
        encode_execution_spec(&digest)
    }

    fn set_artifact(&self, sha256: &str, bytes: Vec<u8>) {
        self.artifacts.lock().unwrap().insert(sha256.to_owned(), bytes);
    }
}

impl JobSource for MemorySource {
    fn job_spec_digests(&self) -> Vec<[u8; 32]> {
        self.specs.lock().unwrap().keys().copied().collect()
    }
    fn job_spec(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        self.specs.lock().unwrap().get(digest).cloned()
    }
    fn artifact(&self, sha256: &str) -> Option<Vec<u8>> {
        self.artifacts.lock().unwrap().get(sha256).cloned()
    }
}

#[derive(Default)]
struct MemorySink(Mutex<Vec<UntrustedMetering>>);

impl MeteringSink for MemorySink {
    fn record(&self, _job_digest: &[u8; 32], metering: &UntrustedMetering) {
        self.0.lock().unwrap().push(*metering);
    }
}

/// What a mock runtime does, beyond the fixtures.
#[derive(Clone)]
enum Behaviour {
    Fixtures,
    Fail,
    Panic,
    Sleep(Duration),
    Return(Vec<u8>),
}

/// The mock runtime: the fixtures of the plan's §3.5, and misbehaviour.
struct MockRuntime {
    family: &'static str,
    accelerator: &'static str,
    behaviour: Mutex<Behaviour>,
    calls: AtomicUsize,
}

impl MockRuntime {
    fn new(family: &'static str, accelerator: &'static str) -> Arc<Self> {
        Arc::new(Self {
            family,
            accelerator,
            behaviour: Mutex::new(Behaviour::Fixtures),
            calls: AtomicUsize::new(0),
        })
    }
    fn behave(&self, b: Behaviour) {
        *self.behaviour.lock().unwrap() = b;
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

fn fixture(job: &ResolvedJob, input: &[u8]) -> Vec<u8> {
    match job.spec.workload.as_str() {
        "fixture-byte-table" => {
            let table = &job.artifacts["table"];
            input.iter().map(|b| table[usize::from(*b)]).collect()
        }
        "fixture-sum" => {
            let list: Vec<u64> = serde_json::from_slice(input).unwrap();
            let sum: u64 = list.iter().sum();
            format!("{{\"count\":{},\"sum\":{sum}}}", list.len()).into_bytes()
        }
        // Computes nothing real: only the shape of a different runtime.
        "fixture-future-runtime" => b"{\"value_e6\":1500000}".to_vec(),
        other => panic!("not a fixture: {other}"),
    }
}

impl RuntimeAdapter for MockRuntime {
    fn identity(&self) -> RuntimeIdentity {
        RuntimeIdentity {
            family: self.family.to_owned(),
            revision: "1".to_owned(),
            binary_sha256: None,
            accelerator_class: self.accelerator.to_owned(),
        }
    }

    fn run(&self, job: &ResolvedJob, input: &[u8]) -> Result<RunOutput, RuntimeFailed> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let behaviour = self.behaviour.lock().unwrap().clone();
        let raw = match behaviour {
            Behaviour::Fixtures => fixture(job, input),
            Behaviour::Fail => return Err(RuntimeFailed),
            Behaviour::Panic => panic!("mock runtime panic"),
            Behaviour::Sleep(d) => {
                std::thread::sleep(d);
                fixture(job, input)
            }
            Behaviour::Return(bytes) => bytes,
        };
        Ok(RunOutput {
            raw,
            metering: UntrustedMetering {
                wall_ms: Some(1),
                cpu_ms: None,
                peak_memory_mib: None,
            },
        })
    }
}

/// The mock worker: a provider's source, runtimes and metering, behind the
/// real [`JobProfile`].
struct MockWorker {
    source: Arc<MemorySource>,
    runtime: Arc<MockRuntime>,
    sink: Arc<MemorySink>,
    profile: JobProfile,
}

impl MockWorker {
    fn new() -> Self {
        let source = Arc::new(MemorySource::default());
        let runtime = MockRuntime::new("test-fixture", "cpu");
        let future = MockRuntime::new("test-future", "gpu:test");
        let sink = Arc::new(MemorySink::default());
        let profile = JobProfile::new(
            Arc::clone(&source) as Arc<dyn JobSource>,
            vec![
                Arc::clone(&runtime) as Arc<dyn RuntimeAdapter>,
                future as Arc<dyn RuntimeAdapter>,
            ],
            Arc::clone(&sink) as Arc<dyn MeteringSink>,
        );
        let t = table();
        let sha = spec_json("fixture-byte-table")["artifacts"][0]["sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        source.set_artifact(&sha, t);
        Self {
            source,
            runtime,
            sink,
            profile,
        }
    }

    fn add(&self, spec: &Value) -> Vec<u8> {
        self.source.add_spec(canon(spec))
    }

    fn run(&self, execution_spec: &[u8], input: &[u8]) -> Result<Vec<u8>, JobError> {
        self.profile.run(execution_spec, &reference_input_commitment(input), input)
    }
}

fn table_sha() -> String {
    spec_json("fixture-byte-table")["artifacts"][0]["sha256"]
        .as_str()
        .unwrap()
        .to_owned()
}

// ── JS: generic job specification ────────────────────────────────────────

#[test]
fn js1_spec_vectors_reproduce_the_independent_digests() {
    let v = vectors();
    let specs = v["specs"].as_array().unwrap();
    assert_eq!(specs.len(), 3);
    for s in specs {
        let bytes = s["canonical"].as_str().unwrap().as_bytes();
        assert_eq!(canon(&s["spec"]), bytes);
        let digest = job_spec_digest(bytes);
        assert_eq!(hex::encode(digest), s["digest"].as_str().unwrap());
        let encoded = encode_execution_spec(&digest);
        assert_eq!(hex::encode(&encoded), s["execution_spec"].as_str().unwrap());
        assert_eq!(encoded.len(), EXECUTION_SPEC_LEN);
        assert_eq!(parse_execution_spec(&encoded).unwrap(), digest);
        JobSpec::from_canonical_bytes(bytes).unwrap();
    }
}

#[test]
fn js2_a_bad_tag_or_length_is_malformed() {
    let good = encode_execution_spec(&[7u8; 32]);
    let mut wrong_tag = good.clone();
    wrong_tag[0] ^= 1;
    for bad in [
        &good[..44],
        &[good.as_slice(), &[0]].concat()[..],
        &wrong_tag[..],
        JOB_TAG,
        &[],
    ] {
        assert_eq!(
            parse_execution_spec(bad),
            Err(JobError::MalformedExecutionSpec)
        );
    }
}

#[test]
fn js3_non_canonical_incomplete_or_unknown_specs_are_refused() {
    let good = canon(&spec_json("fixture-sum"));
    let text = String::from_utf8(good.clone()).unwrap();
    let bad_documents = [
        format!(" {text}"),
        format!("{text}\n"),
        text.replace("\"workload\":", "\"workload\" :"),
        text.replacen("{", "{\"artifacts\":[],", 1),
        text.replace("5000", "5000.5"),
        text.replace("\"params\":{}", "\"params\":true"),
        text.replace("\"params\":{}", "\"params\":null"),
    ];
    assert_eq!(bad_documents.len(), 7);
    for doc in &bad_documents {
        assert!(
            matches!(
                JobSpec::from_canonical_bytes(doc.as_bytes()),
                Err(JobError::SpecInvalid(_))
            ),
            "accepted: {doc}"
        );
    }

    let spec = spec_json("fixture-byte-table");
    let required: Vec<String> = spec.as_object().unwrap().keys().cloned().collect();
    assert_eq!(required.len(), 9);
    for key in &required {
        let mut v = spec.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(
            matches!(
                JobSpec::from_canonical_bytes(&canon(&v)),
                Err(JobError::SpecInvalid(_))
            ),
            "missing {key} accepted"
        );
    }
    for (path, extra) in [
        ("", "extra"),
        ("/runtime", "extra"),
        ("/limits", "extra"),
        ("/input", "extra"),
        ("/artifacts/0", "extra"),
    ] {
        let mut v = spec.clone();
        v.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(extra.to_owned(), json!("x"));
        assert!(
            matches!(
                JobSpec::from_canonical_bytes(&canon(&v)),
                Err(JobError::SpecInvalid(_))
            ),
            "unknown field accepted at {path:?}"
        );
    }
    // Opaque to this contract: any object passes.
    let mut v = spec;
    v["params"] = json!({"anything": "the client wants"});
    JobSpec::from_canonical_bytes(&canon(&v)).unwrap();
}

#[test]
fn js3_field_rules_are_enforced() {
    let cases: [(&str, Value); 8] = [
        ("/workload", json!("")),
        ("/determinism", json!("EXACT-ISH")),
        ("/input/type", json!("xml")),
        ("/input/max_bytes", json!(0)),
        ("/limits/max_wall_ms", json!(0)),
        ("/confidentiality", json!([])),
        ("/confidentiality", json!(["SECRET"])),
        ("/artifacts/0/sha256", json!("ABC")),
    ];
    for (path, value) in cases {
        let mut v = spec_json("fixture-byte-table");
        *v.pointer_mut(path).unwrap() = value;
        assert!(
            matches!(
                JobSpec::from_canonical_bytes(&canon(&v)),
                Err(JobError::SpecInvalid(_))
            ),
            "{path} accepted"
        );
    }
    let mut v = spec_json("fixture-byte-table");
    let duplicate = v["artifacts"][0].clone();
    v["artifacts"].as_array_mut().unwrap().push(duplicate);
    assert_eq!(
        JobSpec::from_canonical_bytes(&canon(&v)),
        Err(JobError::SpecInvalid("role"))
    );
}

#[test]
fn js4_bytes_that_do_not_match_the_digest_are_refused() {
    let w = MockWorker::new();
    let digest = [9u8; 32];
    w.source.specs.lock().unwrap().insert(digest, canon(&spec_json("fixture-sum")));
    assert_eq!(
        w.run(&encode_execution_spec(&digest), b"[1]"),
        Err(JobError::SpecDigestMismatch)
    );
    assert_eq!(w.runtime.calls(), 0);
}

#[test]
fn js5_the_two_job_spec_domains_differ() {
    use mbongo_compute::inference::job_spec::DOMAIN_JOB_SPEC as INFER_DOMAIN;
    assert_ne!(DOMAIN_JOB_SPEC, INFER_DOMAIN);
    let bytes = b"{}";
    assert_ne!(
        job_spec_digest(bytes),
        mbongo_core::crypto::blake3_hash_multi(&[INFER_DOMAIN, bytes])
    );
}

// ── DS: runtime and contract dispatch ────────────────────────────────────

fn inference_profile(backend: &Arc<FakeBackend>) -> InferenceProfile {
    InferenceProfile::new(Arc::clone(backend) as Arc<dyn InferenceBackend>).unwrap()
}

#[test]
fn ds1_ds2_dispatch_reaches_each_profile_and_refuses_an_unknown_tag() {
    let w = MockWorker::new();
    let job_spec = w.add(&spec_json("fixture-sum"));
    let source = Arc::clone(&w.source) as Arc<dyn JobSource>;
    let runtime = Arc::clone(&w.runtime);
    let job = JobProfile::new(
        source,
        vec![Arc::clone(&runtime) as Arc<dyn RuntimeAdapter>],
        Arc::clone(&w.sink) as Arc<dyn MeteringSink>,
    );
    let backend = Arc::new(FakeBackend::pinned());
    let inference = inference_profile(&backend);
    let kaayu_spec = inference.execution_spec().to_vec();
    let dispatch = ProfileDispatch::new(vec![Box::new(inference), Box::new(job)]);

    let input = b"[1,2]";
    let commitment = reference_input_commitment(input);
    let out = dispatch.execute_task(&job_spec, &commitment, input).unwrap();
    assert_eq!(out, b"{\"count\":2,\"sum\":3}");
    assert_eq!((runtime.calls(), backend.calls()), (1, 0));

    backend.push_content("{}");
    let cv = canon(&json!({
        "contract": kaayu::INPUT_CONTRACT_V1,
        "cv_text": "Synthetic CV for a dispatch test. Clerk at Sample Ledger since 2020.",
    }));
    let _ = dispatch.execute_task(&kaayu_spec, &reference_input_commitment(&cv), &cv);
    assert_eq!(
        runtime.calls(),
        1,
        "the inference spec reached the job profile"
    );
    assert_eq!(
        backend.calls(),
        1,
        "the inference spec missed its own profile"
    );

    assert!(matches!(
        dispatch.execute_task(b"mbongo-unknown:v1", &commitment, input),
        Err(ExecutionError::UnsupportedSpec)
    ));
    assert!(dispatch.spec_tags().contains(&job_spec));
    assert!(dispatch.spec_tags().contains(&kaayu_spec));
}

#[test]
fn ds3_an_unknown_or_mismatched_runtime_never_reaches_an_adapter() {
    let w = MockWorker::new();
    let cases: [(&str, Value, JobError); 3] = [
        ("/runtime/revision", json!("2"), JobError::RuntimeMismatch),
        (
            "/runtime/accelerator_class",
            json!("gpu:other"),
            JobError::RuntimeMismatch,
        ),
        ("/runtime/family", json!("nope"), JobError::RuntimeUnknown),
    ];
    for (path, value, expected) in cases {
        let mut v = spec_json("fixture-sum");
        *v.pointer_mut(path).unwrap() = value;
        assert_eq!(w.run(&w.add(&v), b"[1]"), Err(expected), "{path}");
    }
    let mut pinned = spec_json("fixture-sum");
    pinned["runtime"]["binary_sha256"] = json!("a".repeat(64));
    assert_eq!(
        w.run(&w.add(&pinned), b"[1]"),
        Err(JobError::RuntimeMismatch)
    );
    assert_eq!(w.runtime.calls(), 0);
}

#[test]
fn ds4_a_differently_shaped_runtime_fits_with_no_change_to_the_job_module() {
    let w = MockWorker::new();
    let spec = spec_json("fixture-future-runtime");
    let parsed = JobSpec::from_canonical_bytes(&canon(&spec)).unwrap();
    assert_eq!(parsed.determinism, Determinism::Tolerance);
    assert_eq!(parsed.runtime.accelerator_class, "gpu:test");
    let out = w.run(&w.add(&spec), b"[]").unwrap();
    assert_eq!(out, b"{\"value_e6\":1500000}");
}

// ── AI: artifact integrity ───────────────────────────────────────────────

#[test]
fn ai1_ai2_a_changed_wrong_size_or_missing_artifact_stops_the_task() {
    let w = MockWorker::new();
    let spec = w.add(&spec_json("fixture-byte-table"));
    let mut changed = table();
    changed[0] ^= 1;
    w.source.set_artifact(&table_sha(), changed);
    assert_eq!(w.run(&spec, b"a"), Err(JobError::ArtifactDigestMismatch));

    w.source.set_artifact(&table_sha(), vec![0u8; 255]);
    assert_eq!(w.run(&spec, b"a"), Err(JobError::ArtifactSizeMismatch));

    w.source.artifacts.lock().unwrap().clear();
    assert_eq!(w.run(&spec, b"a"), Err(JobError::ArtifactNotFound));
    assert_eq!(w.runtime.calls(), 0, "the adapter ran on a bad artifact");
}

#[test]
fn ai3_an_artifact_replaced_between_tasks_is_caught() {
    let w = MockWorker::new();
    let spec = w.add(&spec_json("fixture-byte-table"));
    w.run(&spec, b"a").unwrap();
    let mut changed = table();
    changed[9] ^= 1;
    w.source.set_artifact(&table_sha(), changed);
    assert_eq!(w.run(&spec, b"a"), Err(JobError::ArtifactDigestMismatch));
    assert_eq!(w.runtime.calls(), 1);
}

// ── CO: canonical outputs · DR: deterministic replay · TV: verification ──

fn run_all_vectors(w: &MockWorker, reversed: bool) -> Vec<Vec<u8>> {
    let v = vectors();
    let mut cases = v["cases"].as_array().unwrap().clone();
    assert_eq!(cases.len(), 6);
    if reversed {
        cases.reverse();
    }
    let mut outputs: Vec<(String, Vec<u8>)> = cases
        .iter()
        .map(|c| {
            let spec = w.add(&spec_json(c["workload"].as_str().unwrap()));
            let input = hex::decode(c["input"].as_str().unwrap()).unwrap();
            assert_eq!(
                hex::encode(reference_input_commitment(&input)),
                c["input_commitment"].as_str().unwrap()
            );
            let out = w.run(&spec, &input).unwrap();
            assert_eq!(hex::encode(&out), c["output"].as_str().unwrap());
            assert_eq!(
                hex::encode(reference_output_commitment(&out)),
                c["output_commitment"].as_str().unwrap()
            );
            (c["input"].as_str().unwrap().to_owned(), out)
        })
        .collect();
    outputs.sort();
    outputs.into_iter().map(|(_, o)| o).collect()
}

#[test]
fn co1_dr1_dr2_vectors_reproduce_in_any_order() {
    let w = MockWorker::new();
    assert_eq!(run_all_vectors(&w, false), run_all_vectors(&w, true));
    assert_eq!(w.runtime.calls(), 12);
}

#[test]
fn co2_non_canonical_or_wrong_type_output_is_refused() {
    let contract = DataContract {
        data_type: DataType::CanonicalJson,
        max_bytes: 64,
    };
    assert_eq!(
        check_output(b"{\"b\":1,\"a\":2}", &contract),
        Err(JobError::OutputNotCanonical)
    );
    assert_eq!(
        check_output(b"not json", &contract),
        Err(JobError::OutputTypeViolation)
    );
    assert_eq!(
        check_output(&[0xFF, 0xFE], &contract),
        Err(JobError::OutputTypeViolation)
    );
    let w = MockWorker::new();
    let spec = w.add(&spec_json("fixture-sum"));
    w.runtime.behave(Behaviour::Return(b"{ }".to_vec()));
    assert_eq!(w.run(&spec, b"[1]"), Err(JobError::OutputNotCanonical));
}

#[test]
fn tv1_tv2_verification_stops_at_the_level_it_reached() {
    let spec = JobSpec::from_canonical_bytes(&canon(&spec_json("fixture-sum"))).unwrap();
    let good = b"{\"count\":1,\"sum\":1}";
    assert_eq!(
        verify_result(&spec, good, &reference_output_commitment(good)),
        Ok(TechnicalLevel::T1)
    );
    let tampered = b"{\"count\":1,\"sum\":2}";
    assert_eq!(
        verify_result(&spec, tampered, &reference_output_commitment(good)),
        Err(JobError::OutputCommitmentMismatch)
    );
    let wrong_type = b"not json";
    assert_eq!(
        verify_result(&spec, wrong_type, &reference_output_commitment(wrong_type)),
        Err(JobError::OutputTypeViolation)
    );
}

// ── RL: resource limits ──────────────────────────────────────────────────

#[test]
fn rl1_rl2_size_limits_are_exact() {
    let w = MockWorker::new();
    let mut v = spec_json("fixture-byte-table");
    v["input"]["max_bytes"] = json!(4);
    v["output"]["max_bytes"] = json!(3);
    let spec = w.add(&v);
    assert_eq!(w.run(&spec, b"abcde"), Err(JobError::InputTooLarge));
    assert_eq!(w.runtime.calls(), 0, "the adapter ran on an oversize input");
    assert_eq!(w.run(&spec, b"abcd"), Err(JobError::LimitOutputSize));
    assert_eq!(w.run(&spec, b"abc").unwrap().len(), 3);
}

#[test]
fn rl3_a_passed_deadline_fails_the_job_and_not_the_worker() {
    let w = MockWorker::new();
    let mut v = spec_json("fixture-sum");
    v["limits"]["max_wall_ms"] = json!(20);
    let spec = w.add(&v);
    w.runtime.behave(Behaviour::Sleep(Duration::from_millis(400)));
    assert_eq!(w.run(&spec, b"[1]"), Err(JobError::LimitWallTime));
    w.runtime.behave(Behaviour::Fixtures);
    assert_eq!(w.run(&spec, b"[1]").unwrap(), b"{\"count\":1,\"sum\":1}");
}

#[test]
fn rl4_the_memory_limit_is_declared_not_enforced() {
    const { assert!(!MEMORY_LIMIT_ENFORCED) };
    let spec = JobSpec::from_canonical_bytes(&canon(&spec_json("fixture-sum"))).unwrap();
    assert_eq!(spec.max_memory_mib, 16);
}

// ── FC: failure classification ───────────────────────────────────────────

/// Every code of the contract, with its class, retry rule and blame.
fn table_of_failures() -> Vec<(JobError, FailureClass, bool, Blame)> {
    use Blame::{Client, ClientOrProvider, Provider};
    use FailureClass as C;
    vec![
        (JobError::MalformedExecutionSpec, C::Spec, false, Client),
        (JobError::SpecNotFound, C::Spec, false, Client),
        (JobError::SpecDigestMismatch, C::Spec, false, Client),
        (JobError::SpecInvalid("field"), C::Spec, false, Client),
        (
            JobError::RuntimeUnknown,
            C::RuntimeSelection,
            false,
            ClientOrProvider,
        ),
        (
            JobError::RuntimeMismatch,
            C::RuntimeSelection,
            false,
            ClientOrProvider,
        ),
        (JobError::ArtifactNotFound, C::Artifact, false, Provider),
        (
            JobError::ArtifactDigestMismatch,
            C::Artifact,
            false,
            Provider,
        ),
        (JobError::ArtifactSizeMismatch, C::Artifact, false, Provider),
        (JobError::InputCommitmentMismatch, C::Input, false, Client),
        (JobError::InputTooLarge, C::Input, false, Client),
        (JobError::InputTypeViolation, C::Input, false, Client),
        (JobError::LimitWallTime, C::Limit, false, ClientOrProvider),
        (JobError::LimitOutputSize, C::Limit, false, ClientOrProvider),
        (JobError::RuntimeFailed, C::Execution, true, Provider),
        (JobError::OutputNotCanonical, C::Output, false, Provider),
        (JobError::OutputTypeViolation, C::Output, false, Provider),
        (
            JobError::OutputCommitmentMismatch,
            C::Output,
            false,
            Provider,
        ),
    ]
}

#[test]
fn fc1_every_code_is_reached_by_its_own_branch() {
    let mut reached: Vec<String> = Vec::new();
    let mut hit = |r: Result<Vec<u8>, JobError>, expected: JobError| {
        assert_eq!(r.as_ref().err(), Some(&expected));
        reached.push(expected.to_string());
    };
    let w = MockWorker::new();
    let sum = w.add(&spec_json("fixture-sum"));
    let table_job = w.add(&spec_json("fixture-byte-table"));

    hit(
        w.run(b"mbongo-job:v1", b"[]"),
        JobError::MalformedExecutionSpec,
    );
    hit(
        w.run(&encode_execution_spec(&[1u8; 32]), b"[]"),
        JobError::SpecNotFound,
    );
    let digest = [2u8; 32];
    w.source.specs.lock().unwrap().insert(digest, canon(&spec_json("fixture-sum")));
    hit(
        w.run(&encode_execution_spec(&digest), b"[]"),
        JobError::SpecDigestMismatch,
    );
    hit(
        w.run(&w.source.add_spec(b"{}".to_vec()), b"[]"),
        JobError::SpecInvalid("limits"),
    );
    let mut v = spec_json("fixture-sum");
    v["runtime"]["family"] = json!("nope");
    hit(w.run(&w.add(&v), b"[]"), JobError::RuntimeUnknown);
    let mut v = spec_json("fixture-sum");
    v["runtime"]["revision"] = json!("2");
    hit(w.run(&w.add(&v), b"[]"), JobError::RuntimeMismatch);

    let original = table();
    w.source.set_artifact(&table_sha(), vec![0u8; 3]);
    hit(w.run(&table_job, b"a"), JobError::ArtifactSizeMismatch);
    w.source.set_artifact(&table_sha(), vec![0u8; 256]);
    hit(w.run(&table_job, b"a"), JobError::ArtifactDigestMismatch);
    w.source.artifacts.lock().unwrap().clear();
    hit(w.run(&table_job, b"a"), JobError::ArtifactNotFound);
    w.source.set_artifact(&table_sha(), original);

    hit(
        w.profile.run(&sum, &[0u8; 32], b"[]"),
        JobError::InputCommitmentMismatch,
    );
    hit(w.run(&sum, &[b' '; 65]), JobError::InputTooLarge);
    hit(w.run(&sum, b"[ 1 ]"), JobError::InputTypeViolation);

    let mut v = spec_json("fixture-sum");
    v["limits"]["max_wall_ms"] = json!(10);
    let tight = w.add(&v);
    w.runtime.behave(Behaviour::Sleep(Duration::from_millis(300)));
    hit(w.run(&tight, b"[]"), JobError::LimitWallTime);

    w.runtime.behave(Behaviour::Return(vec![b'1'; 65]));
    hit(w.run(&sum, b"[]"), JobError::LimitOutputSize);
    w.runtime.behave(Behaviour::Fail);
    hit(w.run(&sum, b"[]"), JobError::RuntimeFailed);
    w.runtime.behave(Behaviour::Return(b"{ }".to_vec()));
    hit(w.run(&sum, b"[]"), JobError::OutputNotCanonical);
    w.runtime.behave(Behaviour::Return(b"nope".to_vec()));
    hit(w.run(&sum, b"[]"), JobError::OutputTypeViolation);

    let spec = JobSpec::from_canonical_bytes(&canon(&spec_json("fixture-sum"))).unwrap();
    hit(
        verify_result(&spec, b"[]", &[0u8; 32]).map(|_| Vec::new()),
        JobError::OutputCommitmentMismatch,
    );

    assert_eq!(reached.len(), table_of_failures().len());
}

#[test]
fn fc2_each_code_has_one_class_retry_rule_and_blame() {
    let table = table_of_failures();
    assert_eq!(table.len(), 18);
    for (error, class, retry, blame) in table {
        assert_eq!(error.class(), class, "{error}");
        assert_eq!(error.retryable(), retry, "{error}");
        assert_eq!(error.blame(), blame, "{error}");
    }
}

#[test]
fn fc3_an_adapter_panic_fails_the_job_and_not_the_worker() {
    let w = MockWorker::new();
    let spec = w.add(&spec_json("fixture-sum"));
    w.runtime.behave(Behaviour::Panic);
    assert_eq!(w.run(&spec, b"[1]"), Err(JobError::RuntimeFailed));
    w.runtime.behave(Behaviour::Fixtures);
    assert_eq!(w.run(&spec, b"[1]").unwrap(), b"{\"count\":1,\"sum\":1}");
}

// ── XI: cross-workload isolation ─────────────────────────────────────────

#[test]
fn xi1_each_profile_refuses_the_other_contract() {
    let w = MockWorker::new();
    let job_spec = w.add(&spec_json("fixture-sum"));
    let backend = Arc::new(FakeBackend::pinned());
    let inference = inference_profile(&backend);
    let kaayu_spec = inference.execution_spec().to_vec();

    assert!(!w.profile.accepts(&kaayu_spec));
    assert_eq!(
        w.run(&kaayu_spec, b"x"),
        Err(JobError::MalformedExecutionSpec)
    );
    assert!(!inference.accepts(&job_spec));
    assert!(inference
        .execute_task(&job_spec, &reference_input_commitment(b"x"), b"x")
        .is_err());
    assert_eq!((w.runtime.calls(), backend.calls()), (0, 0));
}

#[test]
fn xi2_jobs_share_no_state_and_a_failure_does_not_leak() {
    let w = MockWorker::new();
    let sum = w.add(&spec_json("fixture-sum"));
    let table_job = w.add(&spec_json("fixture-byte-table"));
    let alone_sum = w.run(&sum, b"[4,5]").unwrap();
    let alone_table = w.run(&table_job, b"ab").unwrap();
    for _ in 0..3 {
        assert_eq!(w.run(&table_job, b"ab").unwrap(), alone_table);
        assert_eq!(w.run(&sum, b"[ bad"), Err(JobError::InputTypeViolation));
        assert_eq!(w.run(&sum, b"[4,5]").unwrap(), alone_sum);
    }
}

fn job_sources() -> Vec<(String, String)> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/job");
    let sources: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (p.display().to_string(), std::fs::read_to_string(p).unwrap())
        })
        .collect();
    assert_eq!(sources.len(), 6);
    sources
}

#[test]
fn xi4_the_job_module_reaches_no_workload_code() {
    for (file, source) in job_sources() {
        let lower = source.to_lowercase();
        for forbidden in [
            "kaayu",
            "mbongopay",
            "crate::mvp",
            "crate::profiles",
            "cpu_inference",
        ] {
            assert!(!lower.contains(forbidden), "{file}: {forbidden}");
        }
    }
}

// ── PE: privacy-safe evidence ────────────────────────────────────────────

#[test]
fn pe1_no_failure_or_metering_carries_input_or_artifact_content() {
    const CANARY: &str = "CANARY-7f3a";
    let w = MockWorker::new();
    let spec = w.add(&spec_json("fixture-byte-table"));
    let sum = w.add(&spec_json("fixture-sum"));
    let input = CANARY.as_bytes();

    let mut rendered: Vec<String> = Vec::new();
    w.run(&spec, input).unwrap();
    for behaviour in [Behaviour::Fail, Behaviour::Return(input.to_vec())] {
        w.runtime.behave(behaviour);
        rendered.push(format!("{:?}", w.run(&sum, b"[1]")));
    }
    let commitment = reference_input_commitment(input);
    rendered.push(format!("{:?}", w.profile.run(&sum, &commitment, input)));
    rendered.push(format!("{:?}", w.sink.0.lock().unwrap()));
    assert_eq!(rendered.len(), 4);
    for r in &rendered {
        assert!(!r.contains(CANARY), "{r}");
    }
    // The job module writes nothing itself.
    for (file, source) in job_sources() {
        for forbidden in ["println!", "eprintln!", "log::", "tracing::"] {
            assert!(!source.contains(forbidden), "{file}: {forbidden}");
        }
    }
}

#[test]
fn pe2_every_code_renders_as_a_code_and_at_most_a_field_name() {
    for (error, ..) in table_of_failures() {
        let text = error.to_string();
        let (code, field) = text.split_once(": ").unwrap_or((&text, ""));
        assert!(code.starts_with("JOB_"), "{text}");
        assert!(
            code.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'),
            "{text}"
        );
        assert!(field.len() <= 16, "{text}");
    }
}

#[test]
fn metering_reaches_the_private_sink_on_success_only() {
    let w = MockWorker::new();
    let spec = w.add(&spec_json("fixture-sum"));
    w.run(&spec, b"[1]").unwrap();
    w.runtime.behave(Behaviour::Fail);
    assert_eq!(w.run(&spec, b"[1]"), Err(JobError::RuntimeFailed));
    assert_eq!(w.sink.0.lock().unwrap().len(), 1);
}
