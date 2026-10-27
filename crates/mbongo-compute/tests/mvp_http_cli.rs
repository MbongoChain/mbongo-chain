//! Mbongo MVP HTTP + CLI: the named CI test group for
//! `docs/product/MVP_HTTP_CLI.md` (MVP-C).
//!
//! Everything runs without the model: the front is the real axum router on
//! `127.0.0.1:0`, the control plane, data plane, worker lifecycle and
//! inference profile are the real ones, the chain is the in-memory
//! [`FakeChain`] and the completion comes from a scripted [`FakeBackend`].
//! The CLI commands are the library functions the `mbongo-mvp` binary calls.
//!
//! C01–C24 are the cases MVP-C §47–§48 name; N01–N06 the negative
//! verification matrix (§42); I01–I02 the two independence tests (§44–§45).

use std::io::Write as _;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::clock::{Clock, ManualClock, SystemClock};
use mbongo_compute::cpu_inference::backend::testing::FakeBackend;
use mbongo_compute::cpu_inference::{InferenceBackend, InferenceProfile};
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::execution::{
    reference_input_commitment, reference_output_commitment, ReverseBytesProfile,
};
use mbongo_compute::identity::{ExecutorKey, IdSource};
use mbongo_compute::inference::kaayu::CvExtractionInputV1;
use mbongo_compute::inference::ContractError;
use mbongo_compute::mvp::auth::{sign_request, signing_message};
use mbongo_compute::mvp::cli::{self, Context, SubmitOptions};
use mbongo_compute::mvp::client::{
    build_task, find_anchored_receipt, find_task, submit_task, ClientError, FrontClient,
};
use mbongo_compute::mvp::dto::{StatusView, UploadResponse};
use mbongo_compute::mvp::front::{Front, FrontConfig};
use mbongo_compute::mvp::verify::{evaluate, Check, VerificationReport};
use mbongo_compute::mvp::{exit, frozen_execution_spec};
use mbongo_compute::profiles;
use mbongo_compute::worker::Worker;
use mbongo_core::{Address, ComputeTask, Receipt};

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors");
const FIXTURE_CV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/synthetic-cv.txt");

fn json_file(rel: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(format!("{VECTORS}/{rel}")).unwrap()).unwrap()
}

fn cv_text() -> String {
    std::fs::read_to_string(FIXTURE_CV).unwrap()
}

/// Distinctive fragments of the synthetic CV: none may appear in any log,
/// error, status body or CLI diagnostic.
const CV_FRAGMENTS: [&str; 6] = [
    "Zorvath",
    "Quillane",
    "Port Fictiva",
    "example.invalid",
    "Placeholder Logistics",
    "nightly batch time",
];

fn raw_output() -> String {
    json_file("inference/output-canonicalization-v1.json")["raw_model_output"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn canonical_output() -> Vec<u8> {
    hex::decode(
        json_file("inference/output-canonicalization-v1.json")["canonical_bytes_hex"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

/// A distinctive fragment of the (scripted) result: may appear only where
/// the user asked to see the result.
fn result_fragment() -> String {
    let p = profiles::kaayu_v2::parse_output(&raw_output()).unwrap();
    p.summary.chars().take(40).collect()
}

// ── a capturing logger, installed once, shared by every test ─────────────

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

// ── the rig: a real front on loopback over a fake chain and backend ──────

struct Rig {
    chain: FakeChain,
    front: Front<FakeChain>,
    base: String,
    client_key: LocalKey,
    client: FrontClient,
    executor: ExecutorKey,
    backend: Arc<FakeBackend>,
}

async fn rig_with(backend: FakeBackend) -> Rig {
    rig_clocked(backend, Arc::new(SystemClock), Duration::ZERO).await
}

/// A backend that blocks its thread for `delay` per completion, as the
/// real llama-server client does for the length of an inference.
struct Slow {
    inner: Arc<FakeBackend>,
    delay: Duration,
}

impl InferenceBackend for Slow {
    fn identity(&self) -> mbongo_compute::cpu_inference::BackendIdentity {
        self.inner.identity()
    }
    fn complete(
        &self,
        request: &mbongo_compute::cpu_inference::ChatRequest,
    ) -> Result<mbongo_compute::cpu_inference::Completion, ContractError> {
        std::thread::sleep(self.delay);
        self.inner.complete(request)
    }
}

/// A rig whose front and client share `clock`, with a backend that takes
/// `delay` per completion.
async fn rig_clocked(backend: FakeBackend, clock: Arc<dyn Clock>, delay: Duration) -> Rig {
    install_logger();
    let chain = FakeChain::new();
    let backend = Arc::new(backend);
    let slow: Arc<dyn InferenceBackend> = Arc::new(Slow {
        inner: Arc::clone(&backend),
        delay,
    });
    let profile = InferenceProfile::new(slow).unwrap();
    let observations = profile.observations();
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let front = Front::new(
        chain.clone(),
        Arc::clone(&clock),
        [0x42u8; 32],
        executor.clone(),
        Arc::new(profile),
        Some(observations),
        FrontConfig::default(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    assert!(addr.ip().is_loopback());
    let router = front.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let base = format!("http://{addr}");
    let client_key = LocalKey::from_seed(&[0xAAu8; 32]);
    let client = FrontClient::new(&base, client_key.clone()).unwrap().with_clock(clock);
    Rig {
        chain,
        front,
        base,
        client_key,
        client,
        executor,
        backend,
    }
}

async fn rig() -> Rig {
    let backend = FakeBackend::pinned();
    backend.push_content(&raw_output());
    rig_with(backend).await
}

/// A committed task: uploaded, on-chain, one block deep. Not registered.
struct Job {
    task: ComputeTask,
    task_id: [u8; 32],
    upload: UploadResponse,
}

async fn commit_job(r: &Rig, cv: &str, salt: u8) -> Job {
    let input = CvExtractionInputV1::new(cv.to_owned()).unwrap();
    let task = build_task(
        r.client_key.address(),
        r.executor.address(),
        [salt; 32],
        input.input_commitment(),
    );
    let task_id = task.task_id();
    let upload = r.client.upload(task_id, cv).await.unwrap();
    submit_task(&r.chain, &r.client_key, &task).await.unwrap();
    r.chain.produce_block();
    Job {
        task,
        task_id,
        upload,
    }
}

async fn register(r: &Rig, j: &Job) -> StatusView {
    r.client
        .register(j.task_id, &j.upload.object_ref, r.executor.address())
        .await
        .unwrap()
}

/// Drives blocks and worker ticks until the front reports `completed`.
async fn run_to_completion(r: &Rig, task_id: [u8; 32]) -> StatusView {
    for _ in 0..20 {
        r.chain.produce_block();
        r.front.tick().await.unwrap();
        let s = r.client.status(task_id).await.unwrap();
        if s.state == "completed" {
            return s;
        }
    }
    panic!("task did not complete");
}

async fn completed_job(r: &Rig) -> Job {
    let j = commit_job(r, &cv_text(), 0x5A).await;
    register(r, &j).await;
    run_to_completion(r, j.task_id).await;
    j
}

/// A request signed by `key`, sent without the client library.
async fn signed(
    base: &str,
    key: &LocalKey,
    method: &str,
    path: &str,
    body: &[u8],
) -> (u16, Vec<u8>, Option<String>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let nonce: [u8; 16] = rand_nonce();
    let mut req = reqwest::Client::new().request(
        reqwest::Method::from_bytes(method.as_bytes()).unwrap(),
        format!("{base}{path}"),
    );
    for (k, v) in sign_request(key, method, path, now, &nonce, body) {
        req = req.header(k, v);
    }
    let resp = req.body(body.to_vec()).send().await.unwrap();
    let status = resp.status().as_u16();
    let ct = resp.headers().get("content-type").map(|v| v.to_str().unwrap().to_owned());
    (status, resp.bytes().await.unwrap().to_vec(), ct)
}

fn rand_nonce() -> [u8; 16] {
    mbongo_compute::mvp::client::random_bytes::<16>()
}

fn error_code(body: &[u8]) -> String {
    let v: serde_json::Value = serde_json::from_slice(body).unwrap();
    assert_eq!(
        v.as_object().unwrap().len(),
        1,
        "an error body carries exactly one field"
    );
    v["error"].as_str().unwrap().to_owned()
}

fn assert_no_cv(what: &str, text: &str) {
    for f in CV_FRAGMENTS {
        assert!(!text.contains(f), "{what} contains CV fragment {f:?}");
    }
}

fn upload_body(task_id: [u8; 32], cv: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "task_id": hex::encode(task_id),
        "input": {"contract": "kaayu-cv-extract-input:v1", "cv_text": cv},
    }))
    .unwrap()
}

// ── HTTP integration: C01–C12 ────────────────────────────────────────────

#[tokio::test]
async fn c01_upload_valid_private_input() {
    let r = rig().await;
    let cv = cv_text();
    let input = CvExtractionInputV1::new(cv.clone()).unwrap();
    let up = r.client.upload([0x01u8; 32], &cv).await.unwrap();
    assert_eq!(up.task_id, hex::encode([0x01u8; 32]));
    assert_eq!(up.input_bytes, input.canonical_bytes().len());
    assert_eq!(up.object_ref.len(), 64);
    let (status, body, ct) = signed(
        &r.base,
        &r.client_key,
        "PUT",
        "/v1/inputs",
        &upload_body([0x02u8; 32], &cv),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(ct.as_deref(), Some("application/json"));
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["input_bytes", "input_commitment", "object_ref", "task_id"],
        "the upload returns public facts only"
    );
    assert_no_cv("upload response", &String::from_utf8_lossy(&body));
}

#[tokio::test]
async fn c02_reject_malformed_input() {
    let r = rig().await;
    let id = hex::encode([0x03u8; 32]);
    let cases: [(&str, Vec<u8>); 7] = [
        ("not JSON", b"Zorvath Quillane plain text".to_vec()),
        (
            "unknown field",
            format!(r#"{{"task_id":"{id}","input":{{"contract":"kaayu-cv-extract-input:v1","cv_text":"Zorvath"}},"extra":1}}"#).into_bytes(),
        ),
        (
            "wrong contract",
            format!(r#"{{"task_id":"{id}","input":{{"contract":"kaayu-cv-extract-input:v9","cv_text":"Zorvath"}}}}"#).into_bytes(),
        ),
        ("empty cv_text", upload_body([0x03u8; 32], "   ")),
        ("control character", upload_body([0x03u8; 32], "Zorvath\u{0007}Quillane")),
        (
            "missing input",
            format!(r#"{{"task_id":"{id}"}}"#).into_bytes(),
        ),
        (
            "cv_text not a string",
            format!(r#"{{"task_id":"{id}","input":{{"contract":"kaayu-cv-extract-input:v1","cv_text":7}}}}"#).into_bytes(),
        ),
    ];
    for (name, body) in cases {
        let (status, resp, _) = signed(&r.base, &r.client_key, "PUT", "/v1/inputs", &body).await;
        assert_eq!(status, 400, "{name}");
        assert_eq!(error_code(&resp), "INVALID_INPUT_FORMAT", "{name}");
    }
    let bad_id =
        br#"{"task_id":"zz","input":{"contract":"kaayu-cv-extract-input:v1","cv_text":"x"}}"#;
    let (status, resp, _) = signed(&r.base, &r.client_key, "PUT", "/v1/inputs", bad_id).await;
    assert_eq!(status, 400);
    assert_eq!(error_code(&resp), "INVALID_TASK_ID");
}

#[tokio::test]
async fn c03_reject_oversized_input_before_storing() {
    let r = rig().await;
    let task = build_task(
        r.client_key.address(),
        r.executor.address(),
        [0x04u8; 32],
        [0u8; 32],
    );
    let task_id = task.task_id();
    // Over the input contract's 10,000 canonical bytes.
    let (status, resp, _) = signed(
        &r.base,
        &r.client_key,
        "PUT",
        "/v1/inputs",
        &upload_body(task_id, &"Q".repeat(10_001)),
    )
    .await;
    assert_eq!(status, 413);
    assert_eq!(error_code(&resp), "INPUT_TOO_LARGE");
    // Over the front's body limit: refused before any handler reads it.
    let (status, _, _) = signed(
        &r.base,
        &r.client_key,
        "PUT",
        "/v1/inputs",
        &upload_body(task_id, &"Q".repeat(70 * 1024)),
    )
    .await;
    assert_eq!(status, 413);
    // Nothing was stored: the task cannot be registered.
    submit_task(&r.chain, &r.client_key, &task).await.unwrap();
    r.chain.produce_block();
    let e = r
        .client
        .register(task_id, &hex::encode([0u8; 32]), r.executor.address())
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("INPUT_NOT_UPLOADED"));
}

#[tokio::test]
async fn c04_uploaded_input_commitment_exact_and_worker_verified() {
    let r = rig().await;
    let cv = cv_text();
    let input = CvExtractionInputV1::new(cv.clone()).unwrap();
    let expected = reference_input_commitment(&input.canonical_bytes());
    let j = completed_job(&r).await;
    assert_eq!(j.upload.input_commitment, hex::encode(expected));
    assert_eq!(
        j.upload.input_commitment,
        hex::encode(input.input_commitment())
    );
    assert_eq!(j.task.input_commitment, expected);
    // Cross-boundary: the worker fetched the uploaded bytes, verified them
    // against the committed input_commitment, executed, and anchored a
    // receipt carrying that same commitment.
    let (_, receipt) = find_anchored_receipt(&r.chain, j.task_id, 1)
        .await
        .unwrap()
        .expect("anchored receipt");
    assert_eq!(receipt.input_commitment, expected);
    let s = r.client.status(j.task_id).await.unwrap();
    assert_eq!(s.last_failure, None, "the worker never refused the input");
    assert_eq!(s.attempts, 1);
}

#[tokio::test]
async fn c05_task_register_valid_idempotent_and_bound_to_the_chain() {
    let r = rig().await;
    let j = commit_job(&r, &cv_text(), 0x05).await;
    let s = register(&r, &j).await;
    assert_eq!(s.state, "queued");
    assert_eq!(s.task_id, hex::encode(j.task_id));
    assert!(s.registered_at.is_some() && s.uploaded_at.is_some());
    let again = register(&r, &j).await;
    assert_eq!(again.state, "queued", "registration is idempotent");

    // The committed task decides; the front only refuses.
    let other = LocalKey::from_seed(&[0xBBu8; 32]);
    let other_client = FrontClient::new(&r.base, other.clone()).unwrap();
    let e = other_client
        .register(j.task_id, &j.upload.object_ref, r.executor.address())
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("NOT_TASK_SUBMITTER"));

    let cv = cv_text();
    let cases: [(&str, ComputeTask, &str); 3] = [
        (
            "executor the front does not run",
            {
                let input = CvExtractionInputV1::new(cv.clone()).unwrap();
                build_task(
                    r.client_key.address(),
                    Address([0x77u8; 32]),
                    [0x51u8; 32],
                    input.input_commitment(),
                )
            },
            "EXECUTOR_NOT_SERVED",
        ),
        (
            "commitment differs from the upload",
            build_task(
                r.client_key.address(),
                r.executor.address(),
                [0x52u8; 32],
                [0x99u8; 32],
            ),
            "INPUT_COMMITMENT_MISMATCH",
        ),
        (
            "not the frozen execution_spec",
            {
                let input = CvExtractionInputV1::new(cv.clone()).unwrap();
                let mut t = build_task(
                    r.client_key.address(),
                    r.executor.address(),
                    [0x53u8; 32],
                    input.input_commitment(),
                );
                t.execution_spec = b"mbongo-ref:reverse-bytes:v1".to_vec();
                t
            },
            "UNSUPPORTED_EXECUTION_SPEC",
        ),
    ];
    for (name, task, code) in cases {
        let task_id = task.task_id();
        let up = r.client.upload(task_id, &cv).await.unwrap();
        submit_task(&r.chain, &r.client_key, &task).await.unwrap();
        r.chain.produce_block();
        let e = r.client.register(task_id, &up.object_ref, task.executor).await.unwrap_err();
        assert_eq!(e.code(), Some(code), "{name}");
    }
    // A registration naming another executor than the task, or another object.
    let j2 = commit_job(&r, &cv, 0x54).await;
    let e = r
        .client
        .register(j2.task_id, &j2.upload.object_ref, Address([0x77u8; 32]))
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("EXECUTOR_MISMATCH"));
    let e = r
        .client
        .register(j2.task_id, &hex::encode([0x66u8; 32]), r.executor.address())
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("OBJECT_REF_MISMATCH"));
}

#[tokio::test]
async fn c06_unknown_task_rejected() {
    let r = rig().await;
    let unknown = [0x06u8; 32];
    let e = r
        .client
        .register(unknown, &hex::encode([0u8; 32]), r.executor.address())
        .await
        .unwrap_err();
    assert!(matches!(e, ClientError::Rejected { status: 404, .. }));
    assert_eq!(e.code(), Some("TASK_NOT_ON_CHAIN"));
    let e = r.client.status(unknown).await.unwrap_err();
    assert_eq!(e.code(), Some("TASK_NOT_FOUND"));
    let e = r.client.result(unknown).await.unwrap_err();
    assert_eq!(e.code(), Some("TASK_NOT_FOUND"));
    let (status, body, _) = signed(&r.base, &r.client_key, "GET", "/v1/tasks/not-hex", b"").await;
    assert_eq!(status, 400);
    assert_eq!(error_code(&body), "INVALID_TASK_ID");
}

#[tokio::test]
async fn c07_status_hides_private_fields() {
    let r = rig().await;
    let j = completed_job(&r).await;
    let path = format!("/v1/tasks/{}", hex::encode(j.task_id));
    let (status, body, ct) = signed(&r.base, &r.client_key, "GET", &path, b"").await;
    assert_eq!(status, 200);
    assert_eq!(ct.as_deref(), Some("application/json"));
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    let expected = [
        "as_of",
        "attempts",
        "attempts_exhausted",
        "completed_at_ms",
        "failure_code",
        "finish_reason",
        "inference_ms",
        "input_tokens",
        "last_failure",
        "output_tokens",
        "receipt_height",
        "receipt_submitted",
        "registered_at",
        "result_available",
        "started_at_ms",
        "state",
        "task_height",
        "task_id",
        "uploaded_at",
        "worker_busy",
    ];
    assert_eq!(keys.len(), expected.len());
    assert_eq!(keys, expected);
    let text = String::from_utf8_lossy(&body);
    assert_no_cv("status body", &text);
    assert!(
        !text.contains(&result_fragment()),
        "status carries no result content"
    );
    assert_eq!(v["state"], "completed");
    assert_eq!(v["result_available"], true);
    assert!(v["receipt_height"].as_u64().is_some());
}

#[tokio::test]
async fn c08_result_unauthorized_rejected() {
    let r = rig().await;
    let j = completed_job(&r).await;
    let path = format!("/v1/tasks/{}/result", hex::encode(j.task_id));
    // Another account: authenticated, not the submitter.
    let other = LocalKey::from_seed(&[0xBBu8; 32]);
    let (status, body, _) = signed(&r.base, &other, "GET", &path, b"").await;
    assert_eq!(
        (status, error_code(&body).as_str()),
        (403, "NOT_TASK_SUBMITTER")
    );
    let other_status = signed(
        &r.base,
        &other,
        "GET",
        &format!("/v1/tasks/{}", hex::encode(j.task_id)),
        b"",
    )
    .await;
    assert_eq!(other_status.0, 403);
    // Unsigned.
    let resp = reqwest::get(format!("{}{path}", r.base)).await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);
    assert_eq!(error_code(&resp.bytes().await.unwrap()), "AUTH_MISSING");
    // Signed for another path, then replayed.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let nonce = rand_nonce();
    let h = sign_request(&r.client_key, "GET", "/v1/tasks/other", now, &nonce, b"");
    let mut req = reqwest::Client::new().get(format!("{}{path}", r.base));
    for (k, v) in &h {
        req = req.header(*k, v);
    }
    let resp = req.send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);
    assert_eq!(
        error_code(&resp.bytes().await.unwrap()),
        "AUTH_BAD_SIGNATURE"
    );
    let h = sign_request(&r.client_key, "GET", &path, now, &nonce, b"");
    for expected in [200, 401] {
        let mut req = reqwest::Client::new().get(format!("{}{path}", r.base));
        for (k, v) in &h {
            req = req.header(*k, v);
        }
        assert_eq!(req.send().await.unwrap().status().as_u16(), expected);
    }
    // Stale.
    let h = sign_request(&r.client_key, "GET", &path, now - 1_000, &rand_nonce(), b"");
    let mut req = reqwest::Client::new().get(format!("{}{path}", r.base));
    for (k, v) in &h {
        req = req.header(*k, v);
    }
    let resp = req.send().await.unwrap();
    assert_eq!(error_code(&resp.bytes().await.unwrap()), "AUTH_STALE");
}

#[tokio::test]
async fn c09_incomplete_task_result_rejected() {
    let r = rig().await;
    let j = commit_job(&r, &cv_text(), 0x09).await;
    // Not yet observed by the front's control plane.
    let e = r.client.result(j.task_id).await.unwrap_err();
    assert_eq!(e.code(), Some("TASK_NOT_FOUND"));
    r.front.tick().await.unwrap();
    let e = r.client.result(j.task_id).await.unwrap_err();
    assert_eq!(e.code(), Some("TASK_NOT_REGISTERED"));
    register(&r, &j).await;
    let e = r.client.result(j.task_id).await.unwrap_err();
    assert!(matches!(e, ClientError::Rejected { status: 409, .. }));
    assert_eq!(e.code(), Some("RESULT_NOT_READY"));
}

#[tokio::test]
async fn c10_completed_canonical_result_exact_bytes() {
    let r = rig().await;
    let j = completed_job(&r).await;
    let path = format!("/v1/tasks/{}/result", hex::encode(j.task_id));
    let (status, http_bytes, ct) = signed(&r.base, &r.client_key, "GET", &path, b"").await;
    assert_eq!(status, 200);
    assert_eq!(ct.as_deref(), Some("application/octet-stream"));
    let persisted = r.front.persisted_result(j.task_id).await.unwrap();
    // HTTP bytes == data-plane bytes == the frozen canonical bytes, before
    // any JSON parsing.
    assert_eq!(http_bytes, persisted);
    assert_eq!(http_bytes, canonical_output());
    let (_, receipt) = find_anchored_receipt(&r.chain, j.task_id, 1).await.unwrap().unwrap();
    assert_eq!(
        reference_output_commitment(&http_bytes),
        receipt.output_commitment
    );
    // The client library returns the same bytes, untouched.
    assert_eq!(r.client.result(j.task_id).await.unwrap(), persisted);
}

#[tokio::test]
async fn c10b_result_lifetime_is_the_data_planes() {
    // The data plane gives a result object the put grant's lifetime
    // (control-plane `capability_secs`, 3,600 s here); after it the front
    // answers 410 RESULT_EXPIRED rather than pretending to keep it.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let clock = ManualClock::starting_at(now);
    let backend = FakeBackend::pinned();
    backend.push_content(&raw_output());
    let r = rig_clocked(backend, Arc::new(clock.clone()), Duration::ZERO).await;
    let j = completed_job(&r).await;
    assert!(r.client.result(j.task_id).await.is_ok());
    clock.advance(3_590);
    assert!(
        r.client.result(j.task_id).await.is_ok(),
        "still within the lifetime"
    );
    clock.advance(20);
    let e = r.client.result(j.task_id).await.unwrap_err();
    assert!(matches!(e, ClientError::Rejected { status: 410, .. }));
    assert_eq!(e.code(), Some("RESULT_EXPIRED"));
}

#[tokio::test]
async fn c11_errors_redact_private_body() {
    let r = rig().await;
    let cv = cv_text();
    let task = build_task(
        r.client_key.address(),
        r.executor.address(),
        [0x11u8; 32],
        [0u8; 32],
    );
    // Refusals of bodies that carry CV text: each error body is one code.
    let bodies = [
        format!(
            "{{\"task_id\":\"{}\",\"input\":{{\"contract\":\"x\",\"cv_text\":{}}}}}",
            hex::encode(task.task_id()),
            serde_json::to_string(&cv).unwrap()
        )
        .into_bytes(),
        format!("not json at all {cv}").into_bytes(),
        upload_body(task.task_id(), &format!("{cv}\u{0001}")),
    ];
    for body in bodies {
        let (status, resp, _) = signed(&r.base, &r.client_key, "PUT", "/v1/inputs", &body).await;
        assert_eq!(status, 400);
        let text = String::from_utf8_lossy(&resp);
        assert_no_cv("error body", &text);
        error_code(&resp);
    }
    // The client library's errors carry the code only.
    let other = FrontClient::new(&r.base, LocalKey::from_seed(&[0xBBu8; 32])).unwrap();
    let e = other.upload([0x11u8; 32], &format!("{cv}\u{0002}")).await.unwrap_err();
    assert_no_cv("client error", &e.to_string());
    assert_no_cv("client error debug", &format!("{e:?}"));
    // Debug of the request types redacts the CV.
    let req = mbongo_compute::mvp::dto::UploadRequest {
        task_id: "00".into(),
        input: mbongo_compute::mvp::dto::InputObject {
            contract: "kaayu-cv-extract-input:v1".into(),
            cv_text: cv.clone(),
        },
    };
    assert_no_cv("UploadRequest debug", &format!("{req:?}"));
}

#[tokio::test]
async fn c12_http_logs_redact_private_content() {
    let r = rig().await;
    let j = completed_job(&r).await;
    let bytes = r.client.result(j.task_id).await.unwrap();
    let _ = signed(
        &r.base,
        &r.client_key,
        "PUT",
        "/v1/inputs",
        &upload_body([0x12u8; 32], &format!("{}\u{0003}", cv_text())),
    )
    .await;
    let logs = captured();
    let front_lines: Vec<&String> = logs.iter().filter(|l| l.contains("mvp-front:")).collect();
    assert!(
        front_lines.len() >= 6,
        "every request logs one line (got {})",
        front_lines.len()
    );
    let all = logs.join("\n");
    assert_no_cv("logs", &all);
    assert!(!all.contains(&result_fragment()), "result content in logs");
    assert!(!all.contains(&String::from_utf8_lossy(&bytes).to_string()));
    for needle in ["cv_text", "x-mbongo-signature", "x-mbongo-nonce", "proof"] {
        assert!(
            !front_lines.iter().any(|l| l.contains(needle)),
            "front log mentions {needle}"
        );
    }
    // The route lines carry method, route template, task prefix, status, code, duration.
    assert!(
        front_lines.iter().any(|l| l.contains("GET /v1/tasks/{task_id}/result")
            && l.contains(&hex::encode(&j.task_id[..8]))
            && l.contains("-> 200 OK")),
        "the result route is logged with method, route, task prefix, status"
    );
}

// ── CLI: C13–C24 ─────────────────────────────────────────────────────────

/// Produces blocks and ticks the worker in the background, as the
/// service's loop and a block producer would.
fn drive(r: &Rig) -> tokio::task::JoinHandle<()> {
    let chain = r.chain.clone();
    let front = r.front.clone();
    tokio::spawn(async move {
        loop {
            chain.produce_block();
            let _ = front.tick().await;
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    })
}

fn ctx(r: &Rig) -> Context<'_, FakeChain> {
    Context {
        chain: &r.chain,
        front: &r.client,
        key: &r.client_key,
        executor: Some(r.executor.address()),
        reference_usd_per_hour: Some(0.09),
        poll: Duration::from_millis(10),
    }
}

struct Run {
    code: i32,
    out: String,
    err: String,
}

async fn cli_submit(r: &Rig, opts: SubmitOptions) -> Run {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::submit(&ctx(r), cv_text(), opts, &mut out, &mut err).await;
    Run {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

const NO_WAIT: SubmitOptions = SubmitOptions {
    wait: false,
    timeout: Duration::from_secs(30),
    show_result: false,
    json: false,
};

fn task_id_from(out: &str) -> [u8; 32] {
    let line = out.lines().find(|l| l.starts_with("Task: ")).unwrap();
    mbongo_compute::mvp::client::parse_hex32(&line["Task: ".len()..]).unwrap()
}

async fn committed(r: &Rig, task_id: [u8; 32]) -> ComputeTask {
    find_task(&r.chain, task_id, 1).await.unwrap().unwrap().task
}

#[tokio::test]
async fn c13_c14_c15_submit_builds_the_frozen_task() {
    let r = rig().await;
    let d = drive(&r);
    let run = cli_submit(&r, NO_WAIT).await;
    d.abort();
    assert_eq!(run.code, exit::SUCCESS, "{}", run.err);
    let task_id = task_id_from(&run.out);
    let task = committed(&r, task_id).await;
    // C13: the frozen execution spec of the active profile (v3 since change
    // control CC-2), byte for byte its golden vector.
    let v3 = json_file("inference/kaayu-cv-extract-v3.json");
    assert_eq!(
        hex::encode(&task.execution_spec),
        v3["execution_spec_hex"].as_str().unwrap()
    );
    assert_eq!(task.execution_spec, frozen_execution_spec());
    assert_eq!(task.execution_spec.len(), 47);
    assert_eq!(task.executor, r.executor.address());
    assert_eq!(task.submitter, r.client_key.address());
    // C14: the exact input commitment.
    let input = CvExtractionInputV1::new(cv_text()).unwrap();
    assert_eq!(task.input_commitment, input.input_commitment());
    // C15: the printed task_id is the chain's own derivation.
    assert_eq!(task.task_id(), task_id);
    let s = r.client.status(task_id).await.unwrap();
    assert_ne!(s.state, "not_registered", "submit registered the task");
}

#[tokio::test]
async fn c16_status_maps_lifecycle() {
    let r = rig().await;
    let j = commit_job(&r, &cv_text(), 0x16).await;
    r.front.tick().await.unwrap();
    assert_eq!(
        r.client.status(j.task_id).await.unwrap().state,
        "not_registered"
    );
    assert_eq!(register(&r, &j).await.state, "queued");
    // Not yet confirmed (depth 1): nothing is offered.
    r.front.tick().await.unwrap();
    let s = r.client.status(j.task_id).await.unwrap();
    assert_eq!((s.state.as_str(), s.attempts), ("queued", 0));
    r.chain.produce_block();
    r.front.tick().await.unwrap();
    let s = r.client.status(j.task_id).await.unwrap();
    assert_eq!(s.state, "receipt_submitted");
    assert!(s.result_available && s.receipt_submitted && s.receipt_height.is_none());
    assert_eq!(s.attempts, 1);
    assert!(s.started_at_ms.is_some() && s.completed_at_ms.is_some() && s.inference_ms.is_some());
    assert_eq!(s.finish_reason.as_deref(), Some("stop"));
    // While the receipt waits for a block, ticks do not re-enter the attempt.
    let submissions = r.chain.submissions();
    r.front.tick().await.unwrap();
    assert_eq!(r.chain.submissions(), submissions);
    assert_eq!(r.backend.calls(), 1);
    assert_eq!(
        r.client.status(j.task_id).await.unwrap().state,
        "receipt_submitted"
    );
    r.chain.produce_block();
    r.front.tick().await.unwrap();
    let s = r.client.status(j.task_id).await.unwrap();
    assert_eq!(s.state, "completed");
    assert!(s.receipt_height.is_some());

    // The CLI status command reports the same, without content.
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::status(&ctx(&r), &hex::encode(j.task_id), false, &mut out, &mut err).await;
    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, exit::SUCCESS);
    assert!(out.contains("Status: completed"));
    assert!(out.contains("not verified here"));
    assert_no_cv("status output", &out);
}

/// The worker blocks a thread for the whole inference; the front must keep
/// answering status meanwhile (from its snapshot, `worker_busy`). The
/// server runs as `mvp_service` runs it — a multi-threaded runtime with a
/// timer-driven tick loop — and the client on its own runtime and thread,
/// as the CLI is a separate process.
#[test]
fn c16c_status_answers_during_inference() {
    let server = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let backend = FakeBackend::pinned();
    backend.push_content(&raw_output());
    let (r, j) = server.block_on(async {
        let r = rig_clocked(backend, Arc::new(SystemClock), Duration::from_millis(3_000)).await;
        let j = commit_job(&r, &cv_text(), 0x16).await;
        register(&r, &j).await;
        r.chain.produce_block();
        (r, j)
    });
    let front = r.front.clone();
    server.spawn(async move {
        loop {
            let _ = front.tick().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let base = r.base.clone();
    let key = r.client_key.clone();
    let task_id = j.task_id;
    let client = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let client = FrontClient::new(&base, key).unwrap();
            tokio::time::sleep(Duration::from_millis(600)).await;
            let mut answers = Vec::new();
            for _ in 0..8 {
                let t = std::time::Instant::now();
                let s = tokio::time::timeout(Duration::from_millis(1_000), client.status(task_id))
                    .await
                    .map(Result::unwrap);
                answers.push((s.map(|s| s.worker_busy), t.elapsed()));
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            answers
        })
    });
    let answers = client.join().unwrap();
    assert_eq!(answers.len(), 8);
    let busy = answers.iter().filter(|(a, _)| matches!(a, Ok(true))).count();
    assert!(
        answers.iter().all(|(a, _)| a.is_ok()),
        "every status request answered within 1 s while the worker ran: {answers:?}"
    );
    assert!(busy >= 5, "answers during inference: {answers:?}");
    server.block_on(async {
        for _ in 0..50 {
            if r.client.status(task_id).await.unwrap().state == "receipt_submitted" {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the attempt did not finish");
    });
}

#[tokio::test]
async fn c16b_failed_task_is_parked_and_does_not_starve_the_next() {
    let backend = FakeBackend::pinned();
    for _ in 0..3 {
        backend.push(Err(ContractError::InferenceFailed("scripted".into())));
    }
    backend.push_content(&raw_output());
    let r = rig_with(backend).await;
    let a = commit_job(&r, &cv_text(), 0xA1).await;
    let b = commit_job(&r, &cv_text(), 0xB2).await;
    register(&r, &a).await;
    register(&r, &b).await;
    let mut parked = None;
    for _ in 0..12 {
        r.chain.produce_block();
        r.front.tick().await.unwrap();
        let s = r.client.status(a.task_id).await.unwrap();
        if s.attempts_exhausted {
            parked = Some(s);
            break;
        }
    }
    let s = parked.expect("task A parked");
    assert_eq!(s.state, "failed");
    assert_eq!(s.attempts, 3);
    assert_eq!(s.last_failure.as_deref(), Some("execution"));
    assert_eq!(s.failure_code.as_deref(), Some("INFERENCE_FAILED"));
    let sb = run_to_completion(&r, b.task_id).await;
    assert_eq!(sb.attempts, 1);
    assert_eq!(r.backend.calls(), 4, "A three times, B once, A never again");
    for _ in 0..3 {
        r.chain.produce_block();
        r.front.tick().await.unwrap();
    }
    assert_eq!(r.backend.calls(), 4);
}

#[tokio::test]
async fn c17_result_retrieves_canonical_bytes() {
    let r = rig().await;
    let j = completed_job(&r).await;
    let dir = std::env::temp_dir().join(format!("mbongo-mvp-c17-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("result.bin");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::result(
        &ctx(&r),
        &hex::encode(j.task_id),
        Some(&file),
        false,
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, exit::SUCCESS);
    let written = std::fs::read(&file).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(written, r.front.persisted_result(j.task_id).await.unwrap());
    assert!(out.is_empty(), "nothing printed unless asked");
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("run verify"));
    assert!(!err.contains(&result_fragment()));
}

#[tokio::test]
async fn c18_c22_c23_submit_wait_verifies_and_keeps_layers_apart() {
    let r = rig().await;
    let d = drive(&r);
    let run = cli_submit(
        &r,
        SubmitOptions {
            wait: true,
            ..NO_WAIT
        },
    )
    .await;
    d.abort();
    assert_eq!(run.code, exit::SUCCESS, "{}", run.err);
    // C18: the four results, apart, in the frozen presentation.
    for line in [
        "Status: completed",
        "Receipt integrity: VERIFIED",
        "Result commitment: VERIFIED",
        "Schema conformity: VERIFIED",
        // C22
        "Business extraction accuracy: NOT ASSESSED",
        "Runtime profile: mbongo-infer:v1 / kaayu-cv-extract:v3",
        "Business accuracy has not been evaluated by Mbongo",
        "processes the CV in plaintext (PUBLIC profile)",
        "OUTCOME=VERIFIED",
        "Reference cost: US$",
    ] {
        assert!(run.out.contains(line), "missing {line:?} in:\n{}", run.out);
    }
    let task_id = task_id_from(&run.out);
    assert!(run.out.contains(&format!("Executor: {}", r.executor.address())));
    for banned in [
        "AI result verified",
        "verified inference",
        "confidential",
        "TEE",
        "encrypted",
    ] {
        assert!(
            !run.out.contains(banned) && !run.err.contains(banned),
            "{banned}"
        );
    }
    // C23: no CV text and no result content anywhere unless asked.
    assert_no_cv("stdout", &run.out);
    assert_no_cv("stderr", &run.err);
    assert!(!run.out.contains(&result_fragment()) && !run.err.contains(&result_fragment()));

    // verify on its own, as JSON: the same four fields, independent values.
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::verify(
        &ctx(&r),
        &hex::encode(task_id),
        1,
        None,
        true,
        false,
        &mut out,
        &mut err,
    )
    .await;
    assert_eq!(code, exit::SUCCESS);
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["receipt_integrity"]["status"], "VERIFIED");
    assert_eq!(v["result_integrity"]["status"], "VERIFIED");
    assert_eq!(v["schema_conformity"]["status"], "VERIFIED");
    assert_eq!(v["business_accuracy"]["status"], "NOT_ASSESSED");
    assert_eq!(v["execution_status"], "completed");
    assert_eq!(v["outcome"], "VERIFIED");
    assert!(v.get("verified").is_none(), "no collapsed boolean");
    assert!(v.get("result_canonical").is_none());

    // --show-result: the profile appears after the report, labelled, and
    // the CV still does not.
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::verify(
        &ctx(&r),
        &hex::encode(task_id),
        1,
        None,
        false,
        true,
        &mut out,
        &mut err,
    )
    .await;
    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, exit::SUCCESS);
    let report_end = out.find("OUTCOME=VERIFIED").unwrap();
    let shown = out.find(&result_fragment()).expect("result shown on request");
    assert!(
        shown > report_end,
        "the result is displayed after verification"
    );
    assert!(out.contains("display rendering"));
    assert_no_cv("show-result stdout", &out);
}

#[tokio::test]
async fn c18b_submit_wait_json_is_the_mvp_d_record() {
    let r = rig().await;
    let d = drive(&r);
    let run = cli_submit(
        &r,
        SubmitOptions {
            wait: true,
            json: true,
            ..NO_WAIT
        },
    )
    .await;
    d.abort();
    assert_eq!(run.code, exit::SUCCESS, "{}", run.err);
    let lines: Vec<&str> = run.out.lines().collect();
    assert_eq!(lines.len(), 1, "one JSON record per job");
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    for k in [
        "submitted_at_ms",
        "task_included_at_ms",
        "registered_at_ms",
        "inference_started_at_ms",
        "inference_completed_at_ms",
        "inference_ms",
        "completed_at_ms",
        "verified_at_ms",
        "total_ms",
    ] {
        assert!(v["timings"][k].as_u64().is_some(), "timings.{k}");
    }
    assert_eq!(v["outcome"], "VERIFIED");
    assert!(v["task_id"].as_str().unwrap().len() == 64);
    assert!(v["reference_cost_usd"].as_f64().is_some());
    assert_no_cv("json record", &run.out);
}

/// A completed job's real artefacts, for the library-level verification
/// matrix: the committed task, its anchored receipt, the result bytes.
async fn artefacts() -> (Rig, ComputeTask, Receipt, Vec<u8>) {
    let r = rig().await;
    let j = completed_job(&r).await;
    let (_, receipt) = find_anchored_receipt(&r.chain, j.task_id, 1).await.unwrap().unwrap();
    let bytes = r.client.result(j.task_id).await.unwrap();
    (r, j.task, receipt, bytes)
}

fn resign(mut receipt: Receipt, key: &ExecutorKey) -> Receipt {
    receipt.signature = key.sign(&receipt.receipt_hash().0);
    receipt
}

fn assert_failed(name: &str, report: &VerificationReport) {
    assert_eq!(
        report.outcome(),
        "VERIFICATION_FAILED",
        "{name}: {report:?}"
    );
    assert_eq!(report.exit_code(), exit::VERIFICATION_FAILED, "{name}");
    assert!(!report.integrity_verified(), "{name}");
    assert_eq!(report.business_accuracy, Check::NotAssessed, "{name}");
}

#[tokio::test]
async fn c19_n06_verify_rejects_modified_result() {
    let (r, task, receipt, bytes) = artefacts().await;
    let id = task.task_id();
    let ok = evaluate(
        id,
        &task,
        Some(&receipt),
        Some(&bytes[..]),
        Some(r.executor.address()),
    );
    assert_eq!(ok.outcome(), "VERIFIED");
    let mut modified = bytes.clone();
    let i = modified.iter().position(|b| b.is_ascii_lowercase()).unwrap();
    modified[i] = modified[i].to_ascii_uppercase();
    let report = evaluate(
        id,
        &task,
        Some(&receipt),
        Some(&modified[..]),
        Some(r.executor.address()),
    );
    assert_failed("modified result bytes", &report);
    assert!(report.receipt_integrity.is_verified());
    assert_eq!(
        report.result_integrity,
        Check::Failed("output commitment mismatch".into())
    );
    // A re-serialization is a modification too: pretty-printed JSON fails.
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_slice::<serde_json::Value>(&bytes).unwrap())
            .unwrap();
    let report = evaluate(id, &task, Some(&receipt), Some(pretty.as_bytes()), None);
    assert_failed("pretty-printed result", &report);
}

#[tokio::test]
async fn c20_n05_verify_rejects_invalid_receipt() {
    let (r, task, mut receipt, bytes) = artefacts().await;
    receipt.signature[0] ^= 0x01;
    let report = evaluate(
        task.task_id(),
        &task,
        Some(&receipt),
        Some(&bytes[..]),
        Some(r.executor.address()),
    );
    assert_failed("invalid signature", &report);
    assert_eq!(
        report.receipt_integrity,
        Check::Failed("receipt signature".into())
    );
    assert!(
        matches!(report.result_integrity, Check::Failed(_)),
        "a bad receipt's commitment is not trusted"
    );
}

#[tokio::test]
async fn n01_to_n04_negative_receipt_matrix() {
    let (r, task, receipt, bytes) = artefacts().await;
    let id = task.task_id();
    let exec = r.executor.address();
    let impostor = ExecutorKey::from_seed(&[0x13u8; 32]);
    let mut wrong_task = receipt.clone();
    wrong_task.task_id = [0x01u8; 32];
    let mut wrong_executor = receipt.clone();
    wrong_executor.executor = impostor.address();
    let mut wrong_input = receipt.clone();
    wrong_input.input_commitment = [0x02u8; 32];
    let mut wrong_output = receipt.clone();
    wrong_output.output_commitment = [0x03u8; 32];
    // Each altered receipt is validly signed by its own claimed executor:
    // only the binding can catch it.
    let cases: Vec<(&str, VerificationReport)> = vec![
        (
            "N01 wrong task_id (receipt)",
            evaluate(
                id,
                &task,
                Some(&resign(wrong_task, &r.executor)),
                Some(&bytes[..]),
                Some(exec),
            ),
        ),
        (
            "N01 wrong task_id (requested)",
            evaluate(
                [0x04u8; 32],
                &task,
                Some(&receipt),
                Some(&bytes[..]),
                Some(exec),
            ),
        ),
        (
            "N02 wrong executor (receipt)",
            evaluate(
                id,
                &task,
                Some(&resign(wrong_executor, &impostor)),
                Some(&bytes[..]),
                Some(exec),
            ),
        ),
        (
            "N02 wrong executor (configured)",
            evaluate(
                id,
                &task,
                Some(&receipt),
                Some(&bytes[..]),
                Some(impostor.address()),
            ),
        ),
        (
            "N03 wrong input commitment",
            evaluate(
                id,
                &task,
                Some(&resign(wrong_input, &r.executor)),
                Some(&bytes[..]),
                Some(exec),
            ),
        ),
        (
            "N04 wrong output commitment",
            evaluate(
                id,
                &task,
                Some(&resign(wrong_output, &r.executor)),
                Some(&bytes[..]),
                Some(exec),
            ),
        ),
    ];
    assert_eq!(cases.len(), 6);
    for (name, report) in &cases {
        assert_failed(name, report);
    }
    assert_eq!(
        cases[0].1.receipt_integrity,
        Check::Failed("receipt task binding".into())
    );
    assert!(matches!(cases[1].1.task_profile, Check::Failed(_)));
    assert_eq!(
        cases[2].1.receipt_integrity,
        Check::Failed("receipt executor binding".into())
    );
    assert_eq!(
        cases[4].1.receipt_integrity,
        Check::Failed("receipt input binding".into())
    );
    assert!(cases[5].1.receipt_integrity.is_verified());
    assert_eq!(
        cases[5].1.result_integrity,
        Check::Failed("output commitment mismatch".into())
    );
    // No receipt at all is NOT_READY, never VERIFIED.
    let none = evaluate(id, &task, None, Some(&bytes[..]), Some(exec));
    assert_eq!(none.outcome(), "NOT_READY");
    assert_eq!(none.exit_code(), exit::NOT_READY);
    assert!(matches!(none.receipt_integrity, Check::NotFound(_)));
    assert!(
        none.schema_conformity.is_verified(),
        "schema is checked regardless"
    );
}

#[tokio::test]
async fn c21_i01_schema_invalid_but_correctly_committed() {
    let (r, task, _, _) = artefacts().await;
    // The executor commits to bytes that are not a v2 profile: the receipt
    // and the commitment verify; the schema does not.
    let mut ids = IdSource::new([0x21u8; 32]);
    let signer = Worker::new(&mut ids, r.executor.clone(), Box::new(ReverseBytesProfile));
    for (name, bytes) in [
        ("malformed JSON", b"{\"summary\":".to_vec()),
        ("wrong profile shape", b"{\"name\":\"x\"}".to_vec()),
    ] {
        let receipt = signer.bound_receipt(&task, reference_output_commitment(&bytes));
        let report = evaluate(
            task.task_id(),
            &task,
            Some(&receipt),
            Some(&bytes[..]),
            Some(r.executor.address()),
        );
        assert_eq!(report.receipt_integrity, Check::Verified, "{name}");
        assert_eq!(report.result_integrity, Check::Verified, "{name}");
        assert!(
            matches!(report.schema_conformity, Check::Failed(_)),
            "{name}"
        );
        assert_eq!(report.business_accuracy, Check::NotAssessed, "{name}");
        assert_eq!(report.outcome(), "SCHEMA_FAILED", "{name}");
        assert_eq!(report.exit_code(), exit::SCHEMA_FAILED, "{name}");
    }
}

#[tokio::test]
async fn c21_i02_schema_valid_but_not_the_committed_bytes() {
    let (r, task, _, bytes) = artefacts().await;
    let mut ids = IdSource::new([0x22u8; 32]);
    let signer = Worker::new(&mut ids, r.executor.clone(), Box::new(ReverseBytesProfile));
    // The receipt commits to other bytes; the retrieved bytes are a valid
    // canonical v2 profile.
    let receipt = signer.bound_receipt(&task, reference_output_commitment(b"other bytes"));
    let report = evaluate(
        task.task_id(),
        &task,
        Some(&receipt),
        Some(&bytes[..]),
        Some(r.executor.address()),
    );
    assert_eq!(report.schema_conformity, Check::Verified);
    assert_eq!(report.receipt_integrity, Check::Verified);
    assert_eq!(
        report.result_integrity,
        Check::Failed("output commitment mismatch".into())
    );
    assert_eq!(report.business_accuracy, Check::NotAssessed);
    assert_eq!(
        report.outcome(),
        "VERIFICATION_FAILED",
        "schema validity proves no integrity"
    );
    assert_eq!(report.exit_code(), exit::VERIFICATION_FAILED);
}

#[tokio::test]
async fn c22_business_accuracy_is_never_assessed() {
    let (r, task, receipt, bytes) = artefacts().await;
    let id = task.task_id();
    let reports = [
        evaluate(
            id,
            &task,
            Some(&receipt),
            Some(&bytes[..]),
            Some(r.executor.address()),
        ),
        evaluate(id, &task, None, None, None),
        evaluate(id, &task, Some(&receipt), Some(&b"x"[..]), None),
    ];
    assert_eq!(reports.len(), 3);
    for rep in &reports {
        assert_eq!(rep.business_accuracy, Check::NotAssessed);
        assert_eq!(rep.business_accuracy.label(), "NOT ASSESSED");
    }
    assert_eq!(
        reports[0].exit_code(),
        exit::SUCCESS,
        "NOT_ASSESSED is not an error"
    );
    let json = serde_json::to_value(&reports[0]).unwrap();
    assert_eq!(json["business_accuracy"]["status"], "NOT_ASSESSED");
    assert_eq!(json.as_object().unwrap().len(), 6);
}

#[tokio::test]
async fn c23b_cli_errors_carry_no_cv_text() {
    let r = rig().await;
    // Input refused locally: the code, not the text.
    let mut out = Vec::new();
    let mut err = Vec::new();
    let bad = format!("{}\u{0007}", cv_text());
    let code = cli::submit(&ctx(&r), bad, NO_WAIT, &mut out, &mut err).await;
    assert_eq!(code, exit::CONFIG);
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("INVALID_INPUT_FORMAT"));
    assert_no_cv("cli error", &err);
    // Front unreachable mid-flow.
    let dead = FrontClient::new("http://127.0.0.1:9", r.client_key.clone()).unwrap();
    let c = Context {
        front: &dead,
        ..ctx(&r)
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::submit(&c, cv_text(), NO_WAIT, &mut out, &mut err).await;
    assert_eq!(code, exit::UNAVAILABLE);
    let err = String::from_utf8(err).unwrap();
    assert_no_cv("cli transport error", &err);
    assert!(out.is_empty());
    // No executor configured.
    let c = Context {
        executor: None,
        ..ctx(&r)
    };
    let mut err = Vec::new();
    let code = cli::submit(&c, cv_text(), NO_WAIT, &mut Vec::new(), &mut err).await;
    assert_eq!(code, exit::CONFIG);
}

#[test]
fn c24_golden_vectors_match_rust() {
    // The active profile (v3 since change control CC-2), reproduced; the
    // superseded v2 vector still reproduces from its own job spec.
    let v2 = json_file("inference/kaayu-cv-extract-v2.json");
    assert_eq!(
        hex::encode(profiles::kaayu_v2::job_spec().digest()),
        v2["job_spec_digest"].as_str().unwrap()
    );
    let v2 = json_file("inference/kaayu-cv-extract-v3.json");
    let spec = profiles::active_job_spec();
    assert_eq!(
        hex::encode(spec.digest()),
        v2["job_spec_digest"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(frozen_execution_spec()),
        v2["execution_spec_hex"].as_str().unwrap()
    );
    // The MVP-A input and output vectors, through the MVP-C code paths.
    let input = json_file("inference/input-example-v1.json");
    let i = CvExtractionInputV1::new(input["cv_text"].as_str().unwrap().to_owned()).unwrap();
    assert_eq!(
        hex::encode(i.canonical_bytes()),
        input["canonical_bytes_hex"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(i.input_commitment()),
        input["input_commitment"].as_str().unwrap()
    );
    let output = json_file("inference/output-canonicalization-v1.json");
    assert_eq!(
        hex::encode(reference_output_commitment(&canonical_output())),
        output["output_commitment"].as_str().unwrap()
    );
    // The MVP-C vector: the synthetic fixture, a fixed task, a signed request.
    let m = json_file("mvp/mvp-http-cli-v1.json");
    let cv = cv_text();
    assert_eq!(
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(cv.as_bytes())),
        m["fixture"]["sha256"].as_str().unwrap()
    );
    let fi = CvExtractionInputV1::new(cv).unwrap();
    assert_eq!(
        fi.canonical_bytes().len() as u64,
        m["fixture"]["input_canonical_bytes"].as_u64().unwrap()
    );
    assert_eq!(
        hex::encode(fi.input_commitment()),
        m["fixture"]["input_commitment"].as_str().unwrap()
    );
    let seed = |k: &str| -> [u8; 32] {
        hex::decode(m["task"][k].as_str().unwrap()).unwrap().try_into().unwrap()
    };
    let submitter = LocalKey::from_seed(&seed("submitter_seed"));
    let executor = ExecutorKey::from_seed(&seed("executor_seed"));
    assert_eq!(
        hex::encode(submitter.address().0),
        m["task"]["submitter"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(executor.address().0),
        m["task"]["executor"].as_str().unwrap()
    );
    let mut task = build_task(
        submitter.address(),
        executor.address(),
        seed("salt"),
        fi.input_commitment(),
    );
    // build_task constructs the active profile's spec (v3 since change
    // control CC-2); the MVP-C vector pinned the task with the spec active
    // then (v2), which it records — the derivation is the same.
    assert_eq!(task.execution_spec, frozen_execution_spec());
    task.execution_spec = hex::decode(m["task"]["execution_spec"].as_str().unwrap()).unwrap();
    assert_eq!(
        hex::encode(&task.execution_spec),
        json_file("inference/kaayu-cv-extract-v2.json")["execution_spec_hex"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        hex::encode(task.task_id()),
        m["task"]["task_id"].as_str().unwrap()
    );
    let a = &m["auth"];
    let key = LocalKey::from_seed(
        &hex::decode(a["key_seed"].as_str().unwrap()).unwrap().try_into().unwrap(),
    );
    let nonce: [u8; 16] = hex::decode(a["nonce"].as_str().unwrap()).unwrap().try_into().unwrap();
    let msg = signing_message(
        a["method"].as_str().unwrap(),
        a["path"].as_str().unwrap(),
        a["timestamp"].as_u64().unwrap(),
        &nonce,
        a["body"].as_str().unwrap().as_bytes(),
    );
    assert_eq!(
        String::from_utf8(msg.clone()).unwrap(),
        a["signing_message"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(key.sign(&msg)),
        a["signature"].as_str().unwrap()
    );
}

/// Prints the MVP-C vector for regeneration. Not a check.
#[test]
#[ignore = "generator: run with --ignored --nocapture to regenerate test-vectors/mvp/mvp-http-cli-v1.json"]
fn generate_mvp_vector() {
    let cv = cv_text();
    let fi = CvExtractionInputV1::new(cv.clone()).unwrap();
    let submitter = LocalKey::from_seed(&[0xAAu8; 32]);
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let task = build_task(
        submitter.address(),
        executor.address(),
        [0x5Au8; 32],
        fi.input_commitment(),
    );
    let key = LocalKey::from_seed(&[0x07u8; 32]);
    let nonce = [0x09u8; 16];
    let body = r#"{"object_ref":"00","executor":"00"}"#;
    let path = format!("/v1/tasks/{}/register", hex::encode(task.task_id()));
    let msg = signing_message("POST", &path, 1_790_000_000, &nonce, body.as_bytes());
    let v = serde_json::json!({
        "$comment": "Golden vector for the MVP HTTP front and CLI (docs/product/MVP_HTTP_CLI.md §15). Synthetic data only. Regenerate with: cargo test -p mbongo-compute --test mvp_http_cli generate_mvp_vector -- --ignored --nocapture",
        "fixture": {
            "path": "crates/mbongo-compute/fixtures/synthetic-cv.txt",
            "sha256": hex::encode(<sha2::Sha256 as sha2::Digest>::digest(cv.as_bytes())),
            "input_contract": "kaayu-cv-extract-input:v1",
            "input_canonical_bytes": fi.canonical_bytes().len(),
            "input_commitment": hex::encode(fi.input_commitment()),
            "input_commitment_rule": "BLAKE3(\"mbongo:compute-input:v1\" || canonical input object)",
        },
        "task": {
            "submitter_seed": hex::encode([0xAAu8; 32]),
            "executor_seed": hex::encode([0xE1u8; 32]),
            "salt": hex::encode([0x5Au8; 32]),
            "submitter": hex::encode(submitter.address().0),
            "executor": hex::encode(executor.address().0),
            "execution_spec": hex::encode(&task.execution_spec),
            "task_id": hex::encode(task.task_id()),
        },
        "auth": {
            "key_seed": hex::encode([0x07u8; 32]),
            "method": "POST",
            "path": path,
            "timestamp": 1_790_000_000u64,
            "nonce": hex::encode(nonce),
            "body": body,
            "signing_message": String::from_utf8(msg.clone()).unwrap(),
            "signature": hex::encode(key.sign(&msg)),
        },
    });
    let mut s = serde_json::to_string_pretty(&v).unwrap();
    s.push('\n');
    std::io::stdout().write_all(s.as_bytes()).unwrap();
}
