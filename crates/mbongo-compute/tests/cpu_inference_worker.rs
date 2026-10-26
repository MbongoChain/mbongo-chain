//! Mbongo MVP CPU Inference Worker: the named CI test group for
//! `docs/product/MVP_CPU_INFERENCE_WORKER.md`.
//!
//! **MOCK_WORKER_TESTS.** Everything here runs without the 2.5 GB model:
//! the worker lifecycle is the real one (control plane, data plane,
//! in-memory chain double, receipts), the inference profile is the real one
//! (preflight, prompt, request, finish), and only the completion comes from
//! a scripted [`FakeBackend`] or a fake HTTP server. Real llama.cpp and
//! real-model runs are separate (`mvp_inference_harness`) and are never
//! counted here.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::chain::ChainClient;
use mbongo_compute::clock::{Clock, ManualClock};
use mbongo_compute::control_plane::{ControlPlane, ControlPlaneConfig, FailureClass};
use mbongo_compute::cpu_inference::backend::testing::FakeBackend;
use mbongo_compute::cpu_inference::config::{PINNED_LINUX_X64, PINNED_WINDOWS_X64};
use mbongo_compute::cpu_inference::http::{self, HttpError};
use mbongo_compute::cpu_inference::verify::{
    check_reported_version, parse_version, verify_model, verify_profile_assets,
    verify_runtime_binary, FROZEN_JOB_SPEC_DIGEST,
};
use mbongo_compute::cpu_inference::{
    BackendIdentity, ChatRequest, Completion, InferenceBackend, InferenceProfile, ProfileDispatch,
};
use mbongo_compute::data_plane::{
    CapabilityRequest, DataPlaneFault, InMemoryDataPlane, LocalKey, Operation, Presentation,
};
use mbongo_compute::execution::{
    reference_input_commitment, reference_output_commitment, ExecutionError, ExecutionProfile,
    ReverseBytesProfile, REVERSE_BYTES_SPEC,
};
use mbongo_compute::identity::{ExecutorKey, IdSource, ObjectId, SessionId};
use mbongo_compute::inference::execution_spec::InferenceExecutionSpec;
use mbongo_compute::inference::kaayu::{self, CvExtractionInputV1};
use mbongo_compute::inference::ContractError;
use mbongo_compute::policy::{
    compose, dimension as d, eligible, Advertisement, Claim, DerivedFacts, Input, PolicyFragment,
    Scope, SessionFacts, Source, TaskFacts, Value,
};
use mbongo_compute::profiles::{self, kaayu_v2};
use mbongo_compute::worker::{AttemptOutcome, Fault, Worker};
use mbongo_core::{
    Address, ComputeTask, Transaction, TransactionPayload, TransactionType, COMPUTE_TASK_VERSION,
};
use mbongo_verification::verify_receipt_signature;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/inference");
const LEASE_SECS: u64 = 60;

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{FIXTURES}/{name}");
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

// ── a capturing logger, installed once, shared by every test ────────────

static LOG: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

struct Capture;
impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, r: &log::Record<'_>) {
        LOG.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().push(format!(
            "{} {}",
            r.target(),
            r.args()
        ));
    }
    fn flush(&self) {}
}

fn install_logger() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = log::set_logger(&Capture);
        log::set_max_level(log::LevelFilter::Trace);
    });
}

fn captured() -> Vec<String> {
    LOG.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().clone()
}

// ── fixtures ────────────────────────────────────────────────────────────

fn cv_text() -> String {
    fixture("input-example-v1.json")["cv_text"].as_str().unwrap().to_owned()
}

fn raw_output() -> String {
    fixture("output-canonicalization-v1.json")["raw_model_output"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn canonical_output() -> Vec<u8> {
    hex::decode(
        fixture("output-canonicalization-v1.json")["canonical_bytes_hex"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

fn infer_spec() -> Vec<u8> {
    InferenceExecutionSpec {
        job_spec_digest: profiles::active_job_spec().digest(),
    }
    .encode()
}

// ── a world wired like the reference flow, with the inference profile ───

struct World {
    clock: ManualClock,
    ids: IdSource,
    chain: FakeChain,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    client: LocalKey,
    executor: ExecutorKey,
    backend: Arc<FakeBackend>,
    input: Vec<u8>,
    task: ComputeTask,
    task_id: [u8; 32],
    input_object: ObjectId,
}

impl World {
    fn new() -> Self {
        Self::with(
            infer_spec(),
            CvExtractionInputV1::new(cv_text()).unwrap().canonical_bytes(),
        )
    }

    fn with(execution_spec: Vec<u8>, input: Vec<u8>) -> Self {
        install_logger();
        let clock = ManualClock::starting_at(1_000);
        let clock_arc: Arc<dyn Clock> = Arc::new(clock.clone());
        let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
        let cp = ControlPlane::new(
            Arc::clone(&clock_arc),
            IdSource::new([0x22u8; 32]),
            issuer.clone(),
            ControlPlaneConfig {
                lease_secs: LEASE_SECS,
                session_secs: 600,
                confirmation_depth: 1,
                capability_secs: 120,
            },
        );
        let mut dp = InMemoryDataPlane::new(Arc::clone(&clock_arc), IdSource::new([0x33u8; 32]));
        let client = LocalKey::from_seed(&[0xAAu8; 32]);
        let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client.address(),
            executor: executor.address(),
            salt: [0x5Au8; 32],
            input_commitment: reference_input_commitment(&input),
            execution_spec,
        };
        let task_id = task.task_id();
        let input_object = dp.store_input(
            &client,
            task_id,
            task.input_commitment,
            input.clone(),
            3_600,
        );
        dp.register_task(&client, task_id, executor.address()).unwrap();
        dp.delegate_issuer(&client, task_id, issuer.address()).unwrap();
        let backend = Arc::new(FakeBackend::pinned());
        backend.push_content(&raw_output());
        let mut ids = IdSource::new([0x11u8; 32]);
        ids.next("warm");
        Self {
            clock,
            ids,
            chain: FakeChain::new(),
            cp,
            dp,
            client,
            executor,
            backend,
            input,
            task,
            task_id,
            input_object,
        }
    }

    fn task_tx(&self, nonce: u64) -> Transaction {
        let mut tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: self.client.address(),
            receiver: Address::zero(),
            amount: 0,
            nonce,
            payload: TransactionPayload::ComputeTask(Box::new(self.task.clone())),
            signature: [0u8; 64],
        };
        tx.signature = self.client.sign(&tx.signing_payload());
        tx
    }

    async fn commit_task(&mut self) {
        self.chain.submit_transaction(&self.task_tx(0)).await.unwrap();
        self.chain.produce_block();
        self.chain.produce_block();
        self.cp.observe(&self.chain).await.unwrap();
        self.cp.register_input(self.task_id, self.input_object).unwrap();
    }

    fn profile(&self) -> InferenceProfile {
        InferenceProfile::new(Arc::clone(&self.backend) as Arc<dyn InferenceBackend>).unwrap()
    }

    /// The MVP worker: dispatch over the reference profile and the inference profile.
    fn worker(&mut self) -> Worker {
        let dispatch = ProfileDispatch::new(vec![
            Box::new(ReverseBytesProfile),
            Box::new(self.profile()),
        ]);
        Worker::new(&mut self.ids, self.executor.clone(), Box::new(dispatch))
    }

    /// A worker that can only run the reference profile.
    fn reference_only_worker(&mut self) -> Worker {
        Worker::new(
            &mut self.ids,
            self.executor.clone(),
            Box::new(ReverseBytesProfile),
        )
    }

    async fn run(&mut self, w: &mut Worker) -> AttemptOutcome {
        w.run_once(&mut self.cp, &mut self.dp, &self.chain).await.unwrap()
    }

    fn retrieve(&mut self) -> Vec<u8> {
        let result = self
            .dp
            .result_ref(self.task_id, self.executor.address())
            .expect("result persisted");
        let get = self
            .dp
            .issue_capability(
                &self.client,
                &CapabilityRequest {
                    task_id: self.task_id,
                    operation: Operation::GetResult,
                    resource: Some(result.object),
                    ttl_secs: 60,
                    max_uses: 1,
                },
            )
            .unwrap();
        let ch = self.dp.issue_challenge(self.client.address());
        self.dp
            .get_result(&Presentation::sign(get, ch, &self.client))
            .unwrap()
            .as_bytes()
            .to_vec()
    }

    fn pending_receipt(&self) -> Option<mbongo_core::Receipt> {
        self.chain.pending().iter().find_map(|tx| match &tx.payload {
            TransactionPayload::AnchorReceipt(r) => Some(r.as_ref().clone()),
            _ => None,
        })
    }

    fn anchors(&self) -> usize {
        self.chain
            .pending()
            .iter()
            .filter(|tx| matches!(tx.payload, TransactionPayload::AnchorReceipt(_)))
            .count()
    }
}

fn failed_execution(w: &World) -> AttemptOutcome {
    AttemptOutcome::Failed {
        task_id: w.task_id,
        class: FailureClass::Execution,
    }
}

// ── B01–B07: dispatch and everything that must refuse before inference ──

#[tokio::test]
async fn b01_valid_inference_task_dispatches_to_the_inference_executor() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    let outcome = w.run(&mut worker).await;
    assert!(
        matches!(outcome, AttemptOutcome::Submitted { .. }),
        "{outcome:?}"
    );
    assert_eq!(w.backend.calls(), 1, "exactly one completion requested");
    assert_eq!(w.retrieve(), canonical_output());
}

#[tokio::test]
async fn b02_unknown_profile_is_refused_and_never_falls_back() {
    let dispatch = ProfileDispatch::new(vec![
        Box::new(ReverseBytesProfile),
        Box::new(InferenceProfile::new(Arc::new(FakeBackend::pinned())).unwrap()),
    ]);
    for spec in [
        b"mbongo-other:v1".to_vec(),
        [
            b"mbongo-infer:v2".as_slice(),
            &profiles::active_job_spec().digest(),
        ]
        .concat(),
        Vec::new(),
    ] {
        assert!(!dispatch.accepts(&spec));
        assert_eq!(
            dispatch.execute_task(&spec, &[0u8; 32], b"x"),
            Err(ExecutionError::UnsupportedSpec),
            "UNSUPPORTED_EXECUTION_PROFILE, no fallback to reverse-bytes"
        );
    }
    // The control plane never offers a task whose spec the session does not advertise.
    let mut w = World::with(b"mbongo-other:v1".to_vec(), b"input".to_vec());
    w.commit_task().await;
    let mut worker = w.worker();
    assert_eq!(w.run(&mut worker).await, AttemptOutcome::Idle);
    assert_eq!(w.backend.calls(), 0);
}

#[tokio::test]
async fn b03_job_spec_digest_mismatch_blocks_execution() {
    let mut wrong = profiles::active_job_spec().digest();
    wrong[0] ^= 1;
    let spec = InferenceExecutionSpec {
        job_spec_digest: wrong,
    }
    .encode();
    let mut w = World::with(
        spec.clone(),
        CvExtractionInputV1::new(cv_text()).unwrap().canonical_bytes(),
    );
    w.commit_task().await;
    let mut worker = w.worker();
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Idle,
        "never offered: not advertised"
    );
    let p = w.profile();
    assert!(!p.accepts(&spec));
    assert_eq!(
        p.run(&spec, &w.task.input_commitment, &w.input),
        Err(ContractError::JobSpecNotFound)
    );
    assert_eq!(w.backend.calls(), 0, "no inference");
}

#[test]
fn b04_runtime_mismatch_blocks_execution() {
    let pinned = FakeBackend::pinned().identity();
    for (revision, commit) in [
        ("v0.7.1", pinned.runtime_commit.clone()),
        (
            pinned.runtime_revision.as_str(),
            format!("{}0", &pinned.runtime_commit[..39]),
        ),
    ] {
        let mut id = pinned.clone();
        id.runtime_revision = revision.into();
        id.runtime_commit = commit;
        assert_eq!(
            InferenceProfile::new(Arc::new(FakeBackend::with_identity(id))).err(),
            Some(ContractError::RuntimeRevisionMismatch)
        );
    }
    let mut id = pinned;
    id.runtime_family = "vllm".into();
    assert_eq!(
        InferenceProfile::new(Arc::new(FakeBackend::with_identity(id))).err(),
        Some(ContractError::UnsupportedRuntime)
    );
}

#[test]
fn b05_model_digest_mismatch_blocks_execution() {
    let mut id = FakeBackend::pinned().identity();
    id.model_artifact_sha256 = "ab".repeat(32);
    assert_eq!(
        InferenceProfile::new(Arc::new(FakeBackend::with_identity(id))).err(),
        Some(ContractError::ModelDigestMismatch)
    );
    // The file check: wrong size, wrong bytes, missing file.
    let dir = std::env::temp_dir().join(format!("mbongo-b05-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    std::fs::write(&f, b"not the model").unwrap();
    let spec = kaayu::job_spec();
    assert_eq!(
        verify_model(&f, &spec),
        Err(ContractError::ModelDigestMismatch)
    );
    let mut tiny = spec.clone();
    tiny.model.artifact_size_bytes = 13;
    assert_eq!(
        verify_model(&f, &tiny),
        Err(ContractError::ModelDigestMismatch),
        "right size, wrong bytes"
    );
    assert_eq!(
        verify_model(&dir.join("absent.gguf"), &spec),
        Err(ContractError::ModelNotFound)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn b06_input_commitment_mismatch_blocks_execution() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    worker.inject(Fault::CorruptInput);
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Failed {
            task_id: w.task_id,
            class: FailureClass::Input
        }
    );
    assert_eq!(w.backend.calls(), 0, "no inference");
    assert!(
        w.dp.result_ref(w.task_id, w.executor.address()).is_none(),
        "no result"
    );
    assert_eq!(w.anchors(), 0, "no receipt");
    // The profile re-checks on its own.
    let p = w.profile();
    assert_eq!(
        p.run(&infer_spec(), &[7u8; 32], &w.input),
        Err(ContractError::InputCommitmentMismatch)
    );
    assert_eq!(w.backend.calls(), 0);
}

#[tokio::test]
async fn b07_invalid_input_is_blocked_before_inference() {
    for bad in [
        b"plain CV text, not an input object".to_vec(),
        br#"{"contract":"kaayu-cv-extract-input:v1","cv_text":"   "}"#.to_vec(),
        br#"{"contract":"kaayu-cv-extract-input:v2","cv_text":"x"}"#.to_vec(),
        format!(
            r#"{{"contract":"kaayu-cv-extract-input:v1","cv_text":"{}"}}"#,
            "x".repeat(10_000)
        )
        .into_bytes(),
    ] {
        let mut w = World::with(infer_spec(), bad);
        w.commit_task().await;
        let mut worker = w.worker();
        assert_eq!(w.run(&mut worker).await, failed_execution(&w));
        assert_eq!(w.backend.calls(), 0, "no inference");
        assert!(w.dp.result_ref(w.task_id, w.executor.address()).is_none());
        assert_eq!(w.anchors(), 0);
    }
}

// ── B08–B13: the request, the output rule, the persisted bytes ─────────

#[tokio::test]
async fn b08_valid_input_builds_the_exact_frozen_prompt() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let req = &w.backend.requests()[0];
    assert_eq!(
        req.system,
        kaayu_v2::PROMPT_TEMPLATE,
        "template verbatim, nothing prepended or appended"
    );
    assert_eq!(req.user, cv_text(), "the CV text verbatim");
    let body = req.body();
    let msgs = body["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 2, "two messages, no hidden third");
    assert_eq!(msgs[0]["role"], "system");
    assert_eq!(msgs[1]["role"], "user");
    // Debug never prints content.
    let dbg = format!("{req:?}");
    assert!(!dbg.contains("Example Logistics") && !dbg.contains("You extract"));
}

#[tokio::test]
async fn b09_exact_generation_params_are_sent() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let body = w.backend.requests()[0].body();
    let mut keys: Vec<&str> = body.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "cache_prompt",
            "max_tokens",
            "messages",
            "response_format",
            "seed",
            "stream",
            "temperature",
            "top_p"
        ],
        "no sampling knob beyond the job spec"
    );
    assert_eq!(body["temperature"], 0.0);
    assert_eq!(body["top_p"], 1.0);
    assert_eq!(body["seed"], 42);
    assert_eq!(body["max_tokens"], 600);
    assert_eq!(body["stream"], false);
    assert_eq!(body["cache_prompt"], false);
    assert_eq!(body["response_format"]["type"], "json_schema");
    let schema: serde_json::Value = serde_json::from_str(kaayu_v2::OUTPUT_SCHEMA).unwrap();
    assert_eq!(
        body["response_format"]["json_schema"]["schema"], schema,
        "the pinned schema, unmodified"
    );
    // The bytes sent carry the pinned schema verbatim, so the grammar keeps
    // the profile's property order (summary first), not a key-sorted order.
    let req = &w.backend.requests()[0];
    let sent = String::from_utf8(req.body_bytes()).unwrap();
    assert!(
        sent.contains(kaayu_v2::OUTPUT_SCHEMA.trim_end()),
        "schema bytes verbatim"
    );
    let parsed: serde_json::Value = serde_json::from_str(&sent).unwrap();
    assert_eq!(parsed["response_format"]["json_schema"]["schema"], schema);
    let schema_at = sent.find("\"$id\"").unwrap();
    assert!(
        sent[schema_at..].find("\"summary\"").unwrap()
            < sent[schema_at..].find("\"certifications\": {").unwrap()
    );
}

#[tokio::test]
async fn b10_output_must_be_json() {
    for raw in ["Here is the profile: {}", "", "{\"summary\": \"trunc"] {
        let mut w = World::new();
        w.backend.push_content(raw);
        w.backend.push_content(raw);
        w.commit_task().await;
        let mut worker = w.worker();
        // The first scripted answer (valid) was queued by World::new; consume it elsewhere.
        let _ = w.backend.complete(&w.backend_dummy_request());
        assert_eq!(w.run(&mut worker).await, failed_execution(&w));
        assert!(
            w.dp.result_ref(w.task_id, w.executor.address()).is_none(),
            "nothing persisted"
        );
        assert_eq!(w.anchors(), 0, "no success receipt");
    }
}

impl World {
    fn backend_dummy_request(&self) -> ChatRequest {
        ChatRequest::new(&kaayu::job_spec(), "s", "u", serde_json::json!({}))
    }
}

#[tokio::test]
async fn b11_schema_invalid_output_is_rejected() {
    let reordered = fixture("output-canonicalization-v1.json")["raw_model_output_reordered"]
        .as_str()
        .unwrap()
        .to_owned();
    for raw in [
        reordered.replace(
            "\"location\":\"\"",
            "\"location\":\"\",\"name\":\"Someone\"",
        ),
        reordered.replace("\"seniority\":\"senior\"", "\"seniority\":\"principal\""),
        reordered.replace("\"location\":\"\",", ""),
    ] {
        let mut w = World::new();
        let _ = w.backend.complete(&w.backend_dummy_request());
        w.backend.push_content(&raw);
        w.commit_task().await;
        let mut worker = w.worker();
        assert_eq!(w.run(&mut worker).await, failed_execution(&w));
        assert!(w.dp.result_ref(w.task_id, w.executor.address()).is_none());
        assert_eq!(w.anchors(), 0);
    }
}

#[tokio::test]
async fn b12_canonical_result_is_what_is_persisted() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let stored = w.retrieve();
    assert_ne!(stored, raw_output().into_bytes(), "not the raw model bytes");
    assert_eq!(stored, canonical_output(), "the canonical bytes of MVP-A");
}

#[tokio::test]
async fn b13_persisted_bytes_produce_the_anchored_output_commitment() {
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let receipt = w.pending_receipt().expect("receipt");
    let stored = w.retrieve();
    let expected: [u8; 32] = hex::decode(
        fixture("output-canonicalization-v1.json")["output_commitment"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
    .try_into()
    .unwrap();
    assert_eq!(
        reference_output_commitment(&stored),
        receipt.output_commitment
    );
    assert_eq!(receipt.output_commitment, expected);
    assert_eq!(receipt.task_id, w.task_id);
    assert_eq!(receipt.input_commitment, w.task.input_commitment);
    assert!(
        receipt.metadata.is_empty(),
        "no model/runtime metadata in the receipt"
    );
    assert!(verify_receipt_signature(&receipt));
}

#[tokio::test]
async fn b14_result_is_persisted_before_any_receipt() {
    let mut w = World::new();
    w.commit_task().await;
    w.dp.inject(DataPlaneFault::PutResultFails);
    let mut worker = w.worker();
    assert_eq!(
        w.run(&mut worker).await,
        AttemptOutcome::Failed {
            task_id: w.task_id,
            class: FailureClass::Persistence
        }
    );
    assert_eq!(w.anchors(), 0, "RESULT_PERSIST_FAILED: no receipt");
    let mut worker2 = w.worker();
    assert!(matches!(
        w.run(&mut worker2).await,
        AttemptOutcome::Submitted { .. }
    ));
    assert_eq!(w.anchors(), 1);
}

// ── B15–B18: server failures, retries, no fallback ─────────────────────

#[tokio::test]
async fn b15_inference_timeout_yields_no_receipt() {
    let mut w = World::new();
    let _ = w.backend.complete(&w.backend_dummy_request());
    w.backend.push(Err(ContractError::InferenceFailed(
        "inference timed out".into(),
    )));
    w.commit_task().await;
    let mut worker = w.worker();
    assert_eq!(w.run(&mut worker).await, failed_execution(&w));
    assert!(w.dp.result_ref(w.task_id, w.executor.address()).is_none());
    assert_eq!(w.anchors(), 0);
}

#[tokio::test]
async fn b16_server_crash_yields_no_success_receipt_and_a_retry_succeeds() {
    let mut w = World::new();
    let _ = w.backend.complete(&w.backend_dummy_request());
    w.backend.push(Err(ContractError::InferenceFailed(
        "server unavailable: i/o".into(),
    )));
    w.backend.push_content(&raw_output());
    w.commit_task().await;
    let mut worker = w.worker();
    assert_eq!(w.run(&mut worker).await, failed_execution(&w));
    assert_eq!(w.anchors(), 0);
    let mut worker2 = w.worker();
    assert!(matches!(
        w.run(&mut worker2).await,
        AttemptOutcome::Submitted { .. }
    ));
    assert_eq!(w.anchors(), 1, "exactly one receipt");
    assert_eq!(w.retrieve(), canonical_output());
}

#[tokio::test]
async fn b17_retry_preserves_the_job_spec_and_parameters() {
    let mut w = World::new();
    let _ = w.backend.complete(&w.backend_dummy_request());
    w.backend.push(Err(ContractError::InferenceFailed(
        "inference timed out".into(),
    )));
    w.backend.push_content(&raw_output());
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let mut worker2 = w.worker();
    w.run(&mut worker2).await;
    let reqs = w.backend.requests();
    let (first, second) = (&reqs[1], &reqs[2]);
    assert_eq!(
        first, second,
        "identical request on retry: prompt, parameters, schema"
    );
    assert_eq!(first.body(), second.body());
}

#[tokio::test]
async fn b18_a_different_model_is_never_used_as_fallback() {
    let mut w = World::new();
    let _ = w.backend.complete(&w.backend_dummy_request());
    w.backend.push(Err(ContractError::InferenceFailed(
        "server unavailable".into(),
    )));
    w.commit_task().await;
    // A worker whose only alternative is the reference profile does not
    // turn a failed inference into a reverse-bytes result.
    let mut worker = w.worker();
    assert_eq!(w.run(&mut worker).await, failed_execution(&w));
    assert!(w.dp.result_ref(w.task_id, w.executor.address()).is_none());
    // A backend with another model cannot even be bound.
    let mut other = FakeBackend::pinned().identity();
    other.model_artifact_sha256 =
        "90bc7227557ed8a417696b6ed8a94b2c351ad1d94de39bf2889ad8778366c68a".into(); // the Q4_K_S file
    assert_eq!(
        InferenceProfile::new(Arc::new(FakeBackend::with_identity(other))).err(),
        Some(ContractError::ModelDigestMismatch)
    );
}

// ── B19–B20: logs ───────────────────────────────────────────────────────

fn private_fragments() -> Vec<String> {
    let mut v: Vec<String> =
        cv_text().lines().filter(|l| l.len() >= 12).map(str::to_owned).collect();
    v.push(kaayu_v2::PROMPT_TEMPLATE.lines().next().unwrap().to_owned());
    v.push(String::from_utf8(canonical_output()).unwrap());
    for s in [
        "Designed the event-sourced order pipeline",
        "Fictional Institute of Technology",
        "Example Logistics",
        "Sample Analytics",
    ] {
        v.push(s.into());
    }
    v
}

#[tokio::test]
async fn b19_b20_private_input_and_output_are_absent_from_logs() {
    // Success, input failure, output failure, server failure: all logged.
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.worker();
    w.run(&mut worker).await;
    let mut w2 = World::new();
    let _ = w2.backend.complete(&w2.backend_dummy_request());
    w2.backend.push_content("Here is the profile of Example Logistics engineer");
    w2.commit_task().await;
    let mut worker2 = w2.worker();
    w2.run(&mut worker2).await;
    let lines = captured();
    assert!(
        lines.iter().any(|l| l.contains("cpu-inference: input")),
        "observations are logged"
    );
    for frag in private_fragments() {
        for l in &lines {
            assert!(
                !l.contains(frag.as_str()),
                "private fragment {frag:?} reached a log line"
            );
        }
    }
}

// ── B21–B22: reference path and K unchanged ─────────────────────────────

#[tokio::test]
async fn b21_public_reference_worker_is_unchanged() {
    let p = ReverseBytesProfile;
    assert_eq!(p.spec_tags(), vec![REVERSE_BYTES_SPEC.to_vec()]);
    assert!(p.accepts(REVERSE_BYTES_SPEC));
    assert!(!p.accepts(&infer_spec()));
    assert_eq!(
        p.execute_task(REVERSE_BYTES_SPEC, &[0; 32], b"abc").unwrap(),
        b"cba"
    );
    // The reference task through the MVP worker's dispatcher.
    let input = b"private input that never touches the chain".to_vec();
    let mut w = World::with(REVERSE_BYTES_SPEC.to_vec(), input.clone());
    w.commit_task().await;
    let mut worker = w.worker();
    assert!(matches!(
        w.run(&mut worker).await,
        AttemptOutcome::Submitted { .. }
    ));
    let mut expected = input;
    expected.reverse();
    assert_eq!(w.retrieve(), expected);
    assert_eq!(w.backend.calls(), 0, "no inference for a reference task");
    // And through the reference-only worker, exactly as before.
    let mut w = World::with(REVERSE_BYTES_SPEC.to_vec(), b"abc".to_vec());
    w.commit_task().await;
    let mut worker = w.reference_only_worker();
    assert!(matches!(
        w.run(&mut worker).await,
        AttemptOutcome::Submitted { .. }
    ));
}

#[test]
fn b22_confidential_path_is_unchanged() {
    // K20 installs its own process-wide logger to prove keys never reach a
    // log, and this binary already owns one (B19/B20). So the suite runs in
    // its own process: the unchanged `confidential_authorization` binary.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_confidential_authorization"))
        .output()
        .expect("run confidential_authorization");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "confidential-auth-v1 suite failed");
    assert!(text.contains("CONFIDENTIAL AUTHORIZATION: PASS"));
    assert!(text.contains("30/30"), "all 30 cases");
}

// ── B23–B24: advertisement and eligibility ─────────────────────────────

const INFER_TAG: &str = "mbongo-infer:v1";
const REF_TAG: &str = "mbongo-ref:reverse-bytes:v1";
const REPR: &str = "kaayu-cv-extract-input:v1";

fn session_with(executor: Address, tags: &[&str]) -> SessionFacts {
    let set = |xs: &[&str]| Value::Set(xs.iter().map(|s| (*s).to_owned()).collect());
    let mut claims = std::collections::BTreeMap::new();
    claims.insert(
        d::CONTRACT_VERSION.to_owned(),
        Claim::plain(Value::Token("provider-policy-v1".into()), Scope::Session),
    );
    claims.insert(
        d::REPRESENTATION_TAGS.to_owned(),
        Claim::plain(set(&[REPR]), Scope::Session),
    );
    claims.insert(
        d::PROFILE_TAGS.to_owned(),
        Claim::plain(set(tags), Scope::Session),
    );
    claims.insert(
        d::CONFIDENTIAL_EXECUTION.to_owned(),
        Claim::plain(Value::Bool(false), Scope::Session),
    );
    SessionFacts {
        session_id: SessionId([1; 32]),
        proved_executor: executor,
        worker_class: None,
        claims,
    }
}

fn decide_for(tags: &[&str]) -> bool {
    let a = Address([0xA1; 32]);
    let task = TaskFacts {
        executor: a,
        profile_tag: INFER_TAG.into(),
        representation_tag: REPR.into(),
    };
    let advert = Advertisement {
        provider_id: "mbongo".into(),
        executors: vec![a],
        issued_at: 900,
        expires_at: 1_100,
        revoked: false,
        claims: std::collections::BTreeMap::new(),
        worker_classes: std::collections::BTreeMap::new(),
    };
    let policy = compose(&[PolicyFragment {
        source: Source::DataOwner,
        requirements: vec![],
    }])
    .unwrap();
    let derived = DerivedFacts::default();
    eligible(
        &Input {
            task: &task,
            session: &session_with(a, tags),
            advertisement: &advert,
            derived: &derived,
            now: 1_000,
        },
        &policy,
    )
    .eligible
}

#[tokio::test]
async fn b23_worker_advertises_mbongo_infer_v1() {
    let p = InferenceProfile::new(Arc::new(FakeBackend::pinned())).unwrap();
    assert_eq!(p.spec_tag(), INFER_TAG.as_bytes(), "the J profile tag");
    assert_eq!(
        p.spec_tags(),
        vec![infer_spec()],
        "the exact execution_spec the control plane matches"
    );
    let dispatch = ProfileDispatch::new(vec![Box::new(ReverseBytesProfile), Box::new(p)]);
    assert_eq!(
        dispatch.spec_tags(),
        vec![REVERSE_BYTES_SPEC.to_vec(), infer_spec()]
    );
    assert!(
        decide_for(&[REF_TAG, INFER_TAG]),
        "J: eligible when mbongo-infer:v1 is advertised"
    );
}

#[tokio::test]
async fn b24_wrong_capability_worker_is_ineligible_and_gets_no_lease() {
    assert!(!decide_for(&[REF_TAG]), "J: profile tag not advertised");
    let mut w = World::new();
    w.commit_task().await;
    let mut worker = w.reference_only_worker();
    assert_eq!(w.run(&mut worker).await, AttemptOutcome::Idle, "no lease");
    assert_eq!(w.cp.task(w.task_id).unwrap().lease, None);
}

// ── B25: the golden vector ─────────────────────────────────────────────

#[test]
fn b25_golden_job_spec_vector_is_consumed_exactly() {
    let g = fixture("kaayu-cv-extract-v2.json");
    let p = InferenceProfile::new(Arc::new(FakeBackend::pinned())).unwrap();
    assert_eq!(
        hex::encode(p.execution_spec()),
        g["execution_spec_hex"].as_str().unwrap()
    );
    assert_eq!(
        FROZEN_JOB_SPEC_DIGEST,
        g["job_spec_digest"].as_str().unwrap()
    );
    assert_eq!(verify_profile_assets(&profiles::active_job_spec()), Ok(()));
    // The superseded v1 profile is not what this worker serves.
    assert!(verify_profile_assets(&kaayu::job_spec()).is_err());
    // A drifted profile is refused at start-up.
    let mut drifted = profiles::active_job_spec();
    drifted.generation.seed = 43;
    assert_eq!(
        verify_profile_assets(&drifted),
        Err(ContractError::JobSpecDigestMismatch)
    );
}

// ── crash-recovery matrix with the inference profile ───────────────────

async fn crash_then_recover(fault: Fault, phase: &str) -> (World, usize) {
    let mut w = World::new();
    w.commit_task().await;
    let mut w1 = w.worker();
    w1.inject(fault);
    assert_eq!(
        w.run(&mut w1).await,
        AttemptOutcome::Crashed {
            task_id: w.task_id,
            phase: match phase {
                "before-fetch" => "before-fetch",
                "after-fetch" => "after-fetch",
                "executing" => "executing",
                "after-persist" => "after-persist",
                _ => "after-submit",
            }
        }
    );
    let calls_after_crash = w.backend.calls();
    w.clock.advance(LEASE_SECS + 1);
    let mut w2 = w.worker();
    let outcome = w.run(&mut w2).await;
    assert!(
        matches!(
            outcome,
            AttemptOutcome::Submitted { .. } | AttemptOutcome::Completed { .. }
        ),
        "{phase}: {outcome:?}"
    );
    assert_eq!(w.anchors(), 1, "{phase}: exactly one anchored receipt");
    (w, calls_after_crash)
}

#[tokio::test]
async fn crash_before_inference_then_one_receipt() {
    let (w, calls) = crash_then_recover(Fault::CrashBeforeFetch, "before-fetch").await;
    assert_eq!(calls, 0);
    assert_eq!(w.backend.calls(), 1);
    let (w, calls) = crash_then_recover(Fault::CrashAfterFetch, "after-fetch").await;
    assert_eq!(calls, 0);
    assert_eq!(w.backend.calls(), 1);
}

#[tokio::test]
async fn crash_after_inference_before_persist_reinfers_once_then_one_receipt() {
    let (mut w, calls) = crash_then_recover(Fault::CrashDuringExecution, "executing").await;
    assert_eq!(calls, 1, "inference ran, nothing persisted");
    assert_eq!(
        w.backend.calls(),
        2,
        "at-least-once: inference repeated on retry"
    );
    assert_eq!(w.retrieve(), canonical_output());
}

#[tokio::test]
async fn crash_after_persist_reuses_the_result_without_reinference() {
    let (w, calls) = crash_then_recover(Fault::CrashAfterPersist, "after-persist").await;
    assert_eq!(calls, 1);
    assert_eq!(
        w.backend.calls(),
        1,
        "durable result reused; no second inference"
    );
}

#[tokio::test]
async fn crash_after_receipt_submit_never_signs_a_second_receipt() {
    let (w, _) = crash_then_recover(Fault::CrashAfterSubmit, "after-submit").await;
    assert_eq!(w.backend.calls(), 1);
    assert_eq!(w.anchors(), 1);
}

// ── runtime negatives: version, binary, HTTP (fake server) ─────────────

#[test]
fn wrong_binary_revision_is_refused() {
    let spec = kaayu::job_spec();
    let ok = parse_version("version: 0.7.0-dev (build 11669, commit 620d2225c)").unwrap();
    assert_eq!(check_reported_version(&ok, &spec), Ok(()));
    for bad in [
        "version: 0.7.1 (build 11669, commit 620d2225c)",
        "version: 0.7.0-dev (build 11670, commit 620d2225c)",
        "version: 0.7.0-dev (build 11669, commit 0123456ab)",
        "version: 0.7.0-dev (build 11669, commit 620d22)",
    ] {
        let v = parse_version(bad).unwrap();
        assert_eq!(
            check_reported_version(&v, &spec),
            Err(ContractError::RuntimeRevisionMismatch),
            "{bad}"
        );
    }
    // A file that is not the pinned executable is refused before it is run.
    let dir = std::env::temp_dir().join(format!("mbongo-rt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("llama-server");
    std::fs::write(
        &fake,
        b"#!/bin/sh\necho 'version: 0.7.0 (build 11669, commit 620d2225c)'\n",
    )
    .unwrap();
    assert_eq!(
        verify_runtime_binary(&fake, PINNED_WINDOWS_X64.server_sha256, &spec),
        Err(ContractError::RuntimeRevisionMismatch)
    );
    assert_eq!(
        verify_runtime_binary(&dir.join("absent"), PINNED_LINUX_X64.server_sha256, &spec),
        Err(ContractError::UnsupportedRuntime)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// One-shot fake HTTP server on a free loopback port.
fn serve_once(response: Vec<u8>, delay: Duration) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let mut buf = [0u8; 65536];
            let _ = s.read(&mut buf);
            std::thread::sleep(delay);
            let _ = s.write_all(&response);
        }
    });
    port
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn server_not_listening_malformed_oversized_slow_all_fail_closed() {
    let t = Duration::from_secs(5);
    assert!(matches!(
        http::request(free_port(), "GET", "/health", None, t, 1024),
        Err(HttpError::Connect(_))
    ));
    let p = serve_once(b"this is not http".to_vec(), Duration::ZERO);
    assert!(matches!(
        http::request(p, "GET", "/health", None, t, 1024),
        Err(HttpError::Malformed(_))
    ));
    let big = [
        b"HTTP/1.1 200 OK\r\nContent-Length: 5000\r\n\r\n".as_slice(),
        &vec![b'x'; 5000],
    ]
    .concat();
    let p = serve_once(big, Duration::ZERO);
    assert_eq!(
        http::request(p, "GET", "/x", None, t, 1024),
        Err(HttpError::TooLarge(1024))
    );
    let p = serve_once(
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec(),
        Duration::from_millis(1500),
    );
    assert_eq!(
        http::request(p, "GET", "/x", None, Duration::from_millis(400), 1024),
        Err(HttpError::Timeout)
    );
    let p = serve_once(
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n".to_vec(),
        Duration::ZERO,
    );
    assert_eq!(
        http::request(p, "GET", "/x", None, t, 1024).unwrap().status,
        503
    );
    // A 200 whose body is not a chat completion is a server malfunction.
    assert!(matches!(
        Completion::from_body(b"not json"),
        Err(ContractError::InferenceFailed(_))
    ));
    assert!(matches!(
        Completion::from_body(br#"{"choices":[]}"#),
        Err(ContractError::InferenceFailed(_))
    ));
    let c = Completion::from_body(br#"{"choices":[{"message":{"content":"{}"},"finish_reason":"stop"}],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#).unwrap();
    assert_eq!((c.prompt_tokens, c.completion_tokens), (Some(5), Some(2)));
    assert!(
        !format!("{c:?}").contains("{}\""),
        "Debug prints sizes, not content"
    );
}

#[test]
fn backend_identity_is_what_the_profile_checks() {
    let id: BackendIdentity = FakeBackend::pinned().identity();
    let spec = kaayu::job_spec();
    assert_eq!(id.runtime_commit, spec.runtime.commit);
    assert_eq!(id.model_artifact_sha256, spec.model.artifact_sha256);
    assert_eq!(
        PINNED_WINDOWS_X64.archive,
        "llama-b11669-bin-win-cpu-x64.zip"
    );
    assert_eq!(
        PINNED_LINUX_X64.archive,
        "llama-b11669-bin-ubuntu-x64.tar.gz"
    );
}
