//! Mbongo MVP Launch Operations: the named CI test group for the
//! operational limits of `docs/product/MVP_LAUNCH_RUNBOOK.md` (MVP-E) —
//! parking visibility, result-expiry UX, restart-state disclosure, safe
//! defaults, one job at a time, and the liveness probe the health check
//! uses. Real front on `127.0.0.1:0`, fake chain, scripted backend.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::clock::{Clock, ManualClock};
use mbongo_compute::cpu_inference::backend::testing::FakeBackend;
use mbongo_compute::cpu_inference::config::LOOPBACK;
use mbongo_compute::cpu_inference::{InferenceBackend, InferenceProfile};
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::execution::reference_output_commitment;
use mbongo_compute::identity::ExecutorKey;
use mbongo_compute::inference::kaayu::CvExtractionInputV1;
use mbongo_compute::inference::ContractError;
use mbongo_compute::mvp::cli::{self, Context, RESULT_EXPIRED_NOTE};
use mbongo_compute::mvp::client::{build_task, find_anchored_receipt, submit_task, FrontClient};
use mbongo_compute::mvp::front::{Front, FrontConfig};
use mbongo_compute::mvp::{
    check_loopback_bind, exit, DEFAULT_BIND, IN_MEMORY_STATE, MAX_BODY_BYTES,
};

const START: u64 = 1_790_000_000;
const CV: &str =
    "Synthetic ops-test CV.\n\nExperience\nShift Lead, Example Depot Ltd (2020-01 to present)\n";

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

fn raw_output() -> String {
    serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../test-vectors/inference/output-canonicalization-v1.json"
        ))
        .unwrap(),
    )
    .unwrap()["raw_model_output"]
        .as_str()
        .unwrap()
        .to_owned()
}

struct Rig {
    chain: FakeChain,
    front: Front<FakeChain>,
    clock: ManualClock,
    client: FrontClient,
    key: LocalKey,
    executor: ExecutorKey,
    backend: Arc<FakeBackend>,
}

async fn rig(backend: FakeBackend) -> Rig {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = log::set_logger(&Capture);
        log::set_max_level(log::LevelFilter::Trace);
    });
    let clock = ManualClock::starting_at(START);
    let chain = FakeChain::new();
    let backend = Arc::new(backend);
    let profile = InferenceProfile::new(Arc::clone(&backend) as Arc<dyn InferenceBackend>).unwrap();
    let observations = profile.observations();
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let front = Front::new(
        chain.clone(),
        Arc::new(clock.clone()) as Arc<dyn Clock>,
        [0x42u8; 32],
        executor.clone(),
        Arc::new(profile),
        Some(observations),
        FrontConfig::default(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = front.router();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let key = LocalKey::from_seed(&[0xAAu8; 32]);
    let client = FrontClient::new(&base, key.clone())
        .unwrap()
        .with_clock(Arc::new(clock.clone()));
    Rig {
        chain,
        front,
        clock,
        client,
        key,
        executor,
        backend,
    }
}

async fn ok_rig() -> Rig {
    let b = FakeBackend::pinned();
    b.push_content(&raw_output());
    rig(b).await
}

/// Uploads, commits and registers a task; returns its id.
async fn registered(r: &Rig, salt: u8) -> [u8; 32] {
    let input = CvExtractionInputV1::new(CV.to_owned()).unwrap();
    let task = build_task(
        r.key.address(),
        r.executor.address(),
        [salt; 32],
        input.input_commitment(),
    );
    let id = task.task_id();
    let up = r.client.upload(id, CV).await.unwrap();
    submit_task(&r.chain, &r.key, &task).await.unwrap();
    r.chain.produce_block();
    r.client.register(id, &up.object_ref, r.executor.address()).await.unwrap();
    id
}

async fn step(r: &Rig) {
    r.chain.produce_block();
    r.front.tick().await.unwrap();
}

fn ctx(r: &Rig) -> Context<'_, FakeChain> {
    Context {
        chain: &r.chain,
        front: &r.client,
        key: &r.key,
        executor: Some(r.executor.address()),
        reference_usd_per_hour: None,
        poll: Duration::from_millis(10),
    }
}

#[tokio::test]
async fn o01_parked_task_is_visible_without_private_data() {
    let b = FakeBackend::pinned();
    b.push(Err(ContractError::InferenceFailed("scripted".into())));
    let r = rig(b).await;
    let id = registered(&r, 0x01).await;
    for _ in 0..6 {
        step(&r).await;
    }
    let s = r.client.status(id).await.unwrap();
    assert_eq!(s.state, "failed");
    assert!(s.attempts_exhausted, "parked after three failed attempts");
    assert_eq!(s.attempts, 3);
    assert_eq!(s.failure_code.as_deref(), Some("INFERENCE_FAILED"));
    assert_eq!(r.backend.calls(), 3, "never offered again once parked");
    let logs = LOG.get().unwrap().lock().unwrap().join("\n");
    let line = format!(
        "task {} parked after 3 failed attempts",
        hex::encode(&id[..8])
    );
    assert!(
        logs.contains(&line),
        "the operator sees the parking in the service log"
    );
    // The CLI status command says so, with no content.
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::status(&ctx(&r), &hex::encode(id), false, &mut out, &mut err).await;
    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, exit::SUCCESS);
    assert!(out.contains("Status: failed") && out.contains("INFERENCE_FAILED"));
    assert!(!out.contains("Shift Lead"));
}

#[tokio::test]
async fn o02_result_expiry_is_explained_and_the_receipt_still_verifies() {
    let r = ok_rig().await;
    let id = registered(&r, 0x02).await;
    for _ in 0..3 {
        step(&r).await;
    }
    assert_eq!(r.client.status(id).await.unwrap().state, "completed");
    r.clock.advance(3_601);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = cli::verify(
        &ctx(&r),
        &hex::encode(id),
        1,
        None,
        false,
        false,
        &mut out,
        &mut err,
    )
    .await;
    let out = String::from_utf8(out).unwrap();
    assert_eq!(code, exit::RESULT_EXPIRED);
    assert!(out.contains("Receipt integrity: VERIFIED"), "{out}");
    assert!(out.contains("Result commitment: NOT FOUND (private result expired"));
    assert!(out.contains(RESULT_EXPIRED_NOTE));
    assert!(out.contains("OUTCOME=RESULT_EXPIRED"));
    assert!(
        !out.contains("VERIFICATION_FAILED"),
        "expiry is never reported as an integrity failure"
    );
    // JSON says the same, field by field.
    let mut out = Vec::new();
    let code = cli::verify(
        &ctx(&r),
        &hex::encode(id),
        1,
        None,
        true,
        false,
        &mut out,
        &mut Vec::new(),
    )
    .await;
    assert_eq!(code, exit::RESULT_EXPIRED);
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["receipt_integrity"]["status"], "VERIFIED");
    assert_eq!(v["result_integrity"]["status"], "NOT_FOUND");
    assert_eq!(v["result_expired"], true);
    assert_eq!(v["outcome"], "RESULT_EXPIRED");
    assert_eq!(v["business_accuracy"]["status"], "NOT_ASSESSED");
    // The result command explains it too.
    let mut err = Vec::new();
    let code = cli::result(
        &ctx(&r),
        &hex::encode(id),
        None,
        false,
        &mut Vec::new(),
        &mut err,
    )
    .await;
    assert_eq!(code, exit::RESULT_EXPIRED);
    assert!(String::from_utf8(err).unwrap().contains("cannot be recovered"));
    // The receipt is on-chain regardless.
    assert!(find_anchored_receipt(&r.chain, id, 1).await.unwrap().is_some());
}

#[test]
fn o03_restart_state_is_disclosed_item_by_item() {
    assert_eq!(IN_MEMORY_STATE.len(), 7);
    let items: Vec<&str> = IN_MEMORY_STATE.iter().map(|(i, _)| *i).collect();
    for needed in [
        "inputs",
        "results",
        "registrations",
        "parked",
        "replay",
        "status",
        "key",
    ] {
        assert!(
            items.iter().any(|i| i.contains(needed)),
            "{needed} not disclosed"
        );
    }
    for (item, consequence) in IN_MEMORY_STATE {
        assert!(
            consequence.starts_with("LOST") || consequence.starts_with("REGENERATED"),
            "{item}: {consequence}"
        );
    }
}

#[test]
fn o04_safe_defaults() {
    let bind: std::net::SocketAddr = DEFAULT_BIND.parse().unwrap();
    assert!(bind.ip().is_loopback());
    assert_eq!(LOOPBACK, "127.0.0.1", "llama-server binds loopback");
    for ok in ["127.0.0.1:8740", "[::1]:8740"] {
        assert!(check_loopback_bind(ok.parse().unwrap()).is_ok(), "{ok}");
    }
    for refused in ["0.0.0.0:8740", "192.168.1.10:8740", "[::]:8740"] {
        assert!(
            check_loopback_bind(refused.parse().unwrap()).is_err(),
            "{refused}"
        );
    }
    let c = FrontConfig::default();
    assert_eq!(c.max_attempts, 3);
    assert_eq!(c.input_ttl_secs, 3_600);
    assert_eq!(c.control_plane.capability_secs, 3_600, "result lifetime");
    assert_eq!(
        c.control_plane.lease_secs, 900,
        "a lease outlives the 180 s inference timeout"
    );
    assert_eq!(MAX_BODY_BYTES, 64 * 1024);
}

#[tokio::test]
async fn o05_one_job_at_a_time_and_no_cross_job_state() {
    let r = ok_rig().await;
    let first = registered(&r, 0x05).await;
    step(&r).await; // confirmation depth
    let calls = r.backend.calls();
    // A second job arrives while the first is queued.
    let second = registered(&r, 0x06).await;
    step(&r).await;
    assert_eq!(r.backend.calls(), calls + 1, "one inference per tick");
    for _ in 0..6 {
        step(&r).await;
    }
    for id in [first, second] {
        let s = r.client.status(id).await.unwrap();
        assert_eq!((s.state.as_str(), s.attempts), ("completed", 1));
        let (_, receipt) = find_anchored_receipt(&r.chain, id, 1).await.unwrap().unwrap();
        assert_eq!(receipt.task_id, id, "each receipt answers its own task");
        let bytes = r.client.result(id).await.unwrap();
        assert_eq!(
            reference_output_commitment(&bytes),
            receipt.output_commitment
        );
    }
    assert_eq!(r.backend.calls(), 2);
}

#[tokio::test]
async fn o06_liveness_probe_answers_without_credentials() {
    // The health check probes the front with an unsigned request: a fast,
    // content-free 401 proves the front is up without any key.
    let r = ok_rig().await;
    let base = r.client_base();
    let resp = reqwest::get(format!("{base}/v1/tasks/{}", "00".repeat(32))).await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);
    assert_eq!(resp.text().await.unwrap(), r#"{"error":"AUTH_MISSING"}"#);
}

impl Rig {
    fn client_base(&self) -> String {
        format!("{:?}", self.client)
            .split(", ")
            .next()
            .unwrap()
            .trim_start_matches("FrontClient(")
            .to_owned()
    }
}
