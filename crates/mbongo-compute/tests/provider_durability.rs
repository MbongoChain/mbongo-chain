//! P1 acceptance tests: durable provider state. A restart here drops every
//! in-memory object and reopens the journal. The real process kill is in the
//! `provider_harness` binary.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::clock::SystemClock;
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::execution::reference_input_commitment;
use mbongo_compute::inference::canonical;
use mbongo_compute::job::{
    MeteringSink, ResolvedJob, RunOutput, RuntimeAdapter, RuntimeFailed, RuntimeIdentity,
    UntrustedMetering,
};
use mbongo_compute::mvp::client::submit_task;
use mbongo_compute::provider::auth::{sign_request, SignedRequest};
use mbongo_compute::provider::client::{JobClientError, ProviderClient};
use mbongo_compute::provider::durable::{
    AcceptError, DurableStore, HexBytes, RetentionPolicy, StoreError, UploadEntry,
};
use mbongo_compute::provider::journal::{Journal, JournalError};
use mbongo_compute::provider::service::{
    MeteringLog, ProviderConfig, ProviderParts, ProviderService, ProviderStore, ATTEMPTS_EXHAUSTED,
    EXECUTION_INTERRUPTED,
};
use mbongo_core::{Address, ComputeTask, COMPUTE_TASK_VERSION};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

const STATE_KEY: [u8; 32] = [7u8; 32];
const JOURNAL: &str = "provider-state.journal";

fn scratch() -> PathBuf {
    // Unique per run, not only per process: state is durable here, and a
    // reused process id must not reopen an earlier run's journal.
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let run = hex::encode(mbongo_compute::mvp::client::random_bytes::<8>());
    let dir = std::env::temp_dir().join(format!("mbongo-p1-test-{run}-{n}"));
    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    dir
}

/// A generic synthetic job. `determinism` decides whether it may be re-run.
fn spec(determinism: &str) -> Vec<u8> {
    let v = json!({
        "workload": "fixture-echo",
        "runtime": {"family": "test-fixture", "revision": "1", "accelerator_class": "cpu"},
        "artifacts": [],
        "input": {"type": "bytes", "max_bytes": 64},
        "output": {"type": "bytes", "max_bytes": 64},
        "limits": {"max_wall_ms": 5000, "max_memory_mib": 16},
        "determinism": determinism,
        "confidentiality": ["PUBLIC"],
        "params": {},
    });
    canonical::canonicalize(&v.to_string(), 1 << 16).unwrap()
}

#[derive(Default)]
struct MockRuntime {
    calls: AtomicUsize,
    fail: Mutex<bool>,
    /// Each call answers differently, as a non-deterministic job does.
    varies: Mutex<bool>,
}

impl RuntimeAdapter for MockRuntime {
    fn identity(&self) -> RuntimeIdentity {
        RuntimeIdentity {
            family: "test-fixture".to_owned(),
            revision: "1".to_owned(),
            binary_sha256: None,
            accelerator_class: "cpu".to_owned(),
        }
    }
    fn run(&self, _: &ResolvedJob, input: &[u8]) -> Result<RunOutput, RuntimeFailed> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if *self.fail.lock().unwrap() {
            return Err(RuntimeFailed);
        }
        let mut raw = input.to_vec();
        if *self.varies.lock().unwrap() {
            raw.extend_from_slice(format!("#{call}").as_bytes());
        }
        Ok(RunOutput {
            raw,
            metering: UntrustedMetering::default(),
        })
    }
}

struct Stack {
    chain: FakeChain,
    service: ProviderService<FakeChain>,
    durable: Arc<DurableStore>,
    runtime: Arc<MockRuntime>,
    base: String,
    server: JoinHandle<()>,
}

fn store(dir: &Path) -> Result<DurableStore, JournalError> {
    DurableStore::open(&dir.join("state"), &STATE_KEY, RetentionPolicy::default())
}

/// Starts or restarts a provider on `dir`: nothing survives but the journal
/// and the chain.
async fn start(dir: &Path, chain: FakeChain, fail: bool) -> Stack {
    // A connection task of the stopped service can hold it a moment longer.
    let mut opened = store(dir);
    for _ in 0..200 {
        if !matches!(opened, Err(JournalError::Locked)) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        opened = store(dir);
    }
    let durable = Arc::new(opened.unwrap());
    let runtime = Arc::new(MockRuntime::default());
    *runtime.fail.lock().unwrap() = fail;
    let service = ProviderService::open(
        chain.clone(),
        Arc::new(SystemClock),
        mbongo_compute::mvp::client::random_bytes::<32>(),
        LocalKey::from_seed(&[0xE1; 32]),
        ProviderParts {
            store: Arc::new(ProviderStore::new(dir.join("artifacts"))),
            runtimes: vec![Arc::clone(&runtime) as Arc<dyn RuntimeAdapter>],
            metering: Arc::new(MeteringLog::default()) as Arc<dyn MeteringSink>,
        },
        Arc::clone(&durable),
        ProviderConfig::default(),
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = service.router();
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    Stack {
        chain,
        service,
        durable,
        runtime,
        base,
        server,
    }
}

fn client_key() -> LocalKey {
    LocalKey::from_seed(&[0xAA; 32])
}

impl Stack {
    fn client(&self) -> ProviderClient {
        ProviderClient::new(&self.base, client_key(), self.service.executor()).unwrap()
    }

    fn calls(&self) -> usize {
        self.runtime.calls.load(Ordering::SeqCst)
    }

    /// Takes a job as far as a registered task, without running it.
    async fn register(&self, job_spec: &[u8], input: &[u8], salt: u8) -> [u8; 32] {
        let (task_id, upload) = self.commit(job_spec, input, salt).await;
        self.client().register(task_id, &upload).await.unwrap();
        task_id
    }

    /// Everything before registration.
    async fn commit(&self, job_spec: &[u8], input: &[u8], salt: u8) -> ([u8; 32], String) {
        let c = self.client();
        let execution_spec = c.put_job_spec(job_spec).await.unwrap();
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client_key().address(),
            executor: self.service.executor(),
            salt: [salt; 32],
            input_commitment: reference_input_commitment(input),
            execution_spec: execution_spec.clone(),
        };
        let task_id = task.task_id();
        let upload = c.upload(task_id, input).await.unwrap();
        submit_task(&self.chain, &client_key(), &task).await.unwrap();
        self.chain.produce_block();
        self.chain.produce_block();
        (task_id, upload.object_ref)
    }

    /// Drives blocks and ticks until the task leaves the open states.
    async fn drive(&self, task_id: [u8; 32]) -> Value {
        for _ in 0..60 {
            self.chain.produce_block();
            self.service.tick().await.unwrap();
            let status = self.client().status(task_id).await.unwrap();
            if status.state == "completed" || status.parked {
                return serde_json::to_value(status).unwrap();
            }
        }
        panic!("task did not finish");
    }

    /// A crash: nothing is flushed or cleaned up beyond what was acknowledged.
    fn crash(self) -> FakeChain {
        self.server.abort();
        self.chain
    }
}

// ── the journal ──────────────────────────────────────────────────────────

fn journal_with(path: &Path, records: &[&[u8]]) {
    let (mut j, read) = Journal::open(path, &STATE_KEY).unwrap();
    assert!(read.is_empty());
    for r in records {
        j.append(r, [1u8; 12]).unwrap();
    }
}

#[test]
fn acknowledged_records_come_back_in_order() {
    let path = scratch().join("j");
    journal_with(&path, &[b"one", b"two", b"three"]);
    let (_, read) = Journal::open(&path, &STATE_KEY).unwrap();
    assert_eq!(
        read,
        vec![b"one".to_vec(), b"two".to_vec(), b"three".to_vec()]
    );
}

#[test]
fn a_torn_last_frame_is_dropped_and_the_journal_goes_on() {
    let path = scratch().join("j");
    journal_with(&path, &[b"one", b"two"]);
    let full = std::fs::read(&path).unwrap();
    // Every prefix of the last frame: a crash may stop an append anywhere.
    let (_, read) = Journal::open(&path, &STATE_KEY).unwrap();
    assert_eq!(read.len(), 2);
    let last_frame = 16 + 12 + 3 + 16;
    for cut in 1..last_frame {
        std::fs::write(&path, &full[..full.len() - cut]).unwrap();
        let (mut j, read) = Journal::open(&path, &STATE_KEY).unwrap();
        assert_eq!(read, vec![b"one".to_vec()], "cut {cut}");
        drop(j.append(b"after", [2u8; 12]));
        drop(j);
        let (_, read) = Journal::open(&path, &STATE_KEY).unwrap();
        assert_eq!(read, vec![b"one".to_vec(), b"after".to_vec()], "cut {cut}");
    }
}

#[test]
fn any_flipped_byte_is_refused_never_skipped() {
    let path = scratch().join("j");
    journal_with(&path, &[b"one", b"two"]);
    let full = std::fs::read(&path).unwrap();
    let mut refused = 0;
    for at in 0..full.len() {
        let mut damaged = full.clone();
        damaged[at] ^= 0x40;
        std::fs::write(&path, &damaged).unwrap();
        match Journal::open(&path, &STATE_KEY) {
            Err(JournalError::Corrupt(_)) => refused += 1,
            Err(e) => panic!("byte {at}: {e}"),
            Ok((_, read)) => panic!("byte {at} accepted: {} records", read.len()),
        }
    }
    assert_eq!(refused, full.len());
}

#[test]
fn another_key_is_refused() {
    let path = scratch().join("j");
    journal_with(&path, &[b"one"]);
    assert!(matches!(
        Journal::open(&path, &[8u8; 32]),
        Err(JournalError::Corrupt(_))
    ));
}

#[test]
fn records_are_not_readable_on_disk() {
    let path = scratch().join("j");
    journal_with(&path, &[b"PLAINTEXT-CANARY"]);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.windows(16).any(|w| w == b"PLAINTEXT-CANARY"));
}

// ── replay protection ────────────────────────────────────────────────────

async fn probe(base: &str, path: &str, headers: &[(&'static str, String)]) -> (u16, String) {
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut req = http.get(format!("{base}{path}"));
    for (name, value) in headers {
        req = req.header(*name, value);
    }
    let resp = req.send().await.unwrap();
    let status = resp.status().as_u16();
    let body: Value = resp.json().await.unwrap();
    (status, body["error"].as_str().unwrap_or("").to_owned())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_accepted_request_stays_refused_after_a_restart() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let recipient = s.service.executor();
    let path = format!("/v1/tasks/{}", "0".repeat(64));
    let t = now();
    let signed = |id: u8| {
        sign_request(
            &client_key(),
            &SignedRequest {
                recipient,
                method: "GET",
                path: &path,
                body: b"",
                issued_at: t,
                expires_at: t + 60,
                request_id: [id; 16],
            },
        )
    };
    let first = signed(1);
    let accepted = (404, "TASK_NOT_FOUND".to_owned());
    assert_eq!(probe(&s.base, &path, &first).await, accepted);

    let s = start(&dir, s.crash(), false).await;
    assert_eq!(
        probe(&s.base, &path, &first).await,
        (401, "AUTH_REPLAYED".to_owned())
    );
    assert_eq!(probe(&s.base, &path, &signed(2)).await, accepted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_request_that_cannot_be_recorded_is_refused() {
    let s = start(&scratch(), FakeChain::new(), false).await;
    let path = format!("/v1/tasks/{}", "0".repeat(64));
    let t = now();
    let headers = sign_request(
        &client_key(),
        &SignedRequest {
            recipient: s.service.executor(),
            method: "GET",
            path: &path,
            body: b"",
            issued_at: t,
            expires_at: t + 60,
            request_id: [3; 16],
        },
    );
    s.durable.poison();
    assert_eq!(
        probe(&s.base, &path, &headers).await,
        (503, "AUTH_REPLAY_STORE_UNAVAILABLE".to_owned())
    );
}

#[test]
fn damaged_state_stops_the_store_from_opening() {
    let dir = scratch();
    let d = store(&dir).unwrap();
    d.accept_request(Address::zero(), [1; 16], u64::MAX, 0).unwrap();
    drop(d);
    let path = dir.join("state").join(JOURNAL);
    let mut bytes = std::fs::read(&path).unwrap();
    let n = bytes.len();
    bytes[n / 2] ^= 0x01;
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(store(&dir), Err(JournalError::Corrupt(_))));
}

// ── recovery ─────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_completed_job_keeps_its_status_and_result() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"kept", 1).await;
    assert_eq!(s.drive(id).await["state"], "completed");
    let s = start(&dir, s.crash(), false).await;
    let status = s.client().status(id).await.unwrap();
    assert_eq!(status.state, "completed");
    assert!(status.receipt_height.is_some());
    assert_eq!(s.client().result(id).await.unwrap(), b"kept");
    assert_eq!(s.calls(), 0, "a completed job must not run again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_parked_job_stays_parked() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), true).await;
    let id = s.register(&spec("BIT_EXACT"), b"x", 1).await;
    let status = s.drive(id).await;
    assert_eq!(status["failure_code"], "JOB_RUNTIME_FAILED");
    let s = start(&dir, s.crash(), false).await;
    let status = s.drive(id).await;
    assert_eq!(status["state"], "failed");
    assert_eq!(status["attempts"], 3);
    assert_eq!(s.calls(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_accepted_upload_can_be_registered_after_a_restart() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let (id, object_ref) = s.commit(&spec("BIT_EXACT"), b"later", 1).await;
    let s = start(&dir, s.crash(), false).await;
    s.client().register(id, &object_ref).await.unwrap();
    assert_eq!(s.drive(id).await["state"], "completed");
    assert_eq!(s.client().result(id).await.unwrap(), b"later");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_registered_job_that_never_started_runs_once() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("NON_DETERMINISTIC"), b"fresh", 1).await;
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.drive(id).await["state"], "completed");
    assert_eq!(s.calls(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_interrupted_deterministic_job_runs_again() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"again", 1).await;
    s.durable.started(id).unwrap();
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.drive(id).await["state"], "completed");
    assert_eq!(s.calls(), 1);
    assert_eq!(s.client().result(id).await.unwrap(), b"again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_interrupted_job_that_is_not_deterministic_is_never_rerun() {
    for class in ["NON_DETERMINISTIC", "TOLERANCE"] {
        let dir = scratch();
        let s = start(&dir, FakeChain::new(), false).await;
        let id = s.register(&spec(class), b"once", 1).await;
        s.durable.started(id).unwrap();
        let s = start(&dir, s.crash(), false).await;
        let status = s.drive(id).await;
        assert_eq!(status["failure_code"], EXECUTION_INTERRUPTED, "{class}");
        assert_eq!(s.calls(), 0, "{class}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_job_that_keeps_crashing_the_provider_stops_being_retried() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"loop", 1).await;
    for _ in 0..3 {
        s.durable.started(id).unwrap();
    }
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.drive(id).await["failure_code"], EXECUTION_INTERRUPTED);
    assert_eq!(s.calls(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stored_output_is_reused_and_the_runner_is_not_called() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("NON_DETERMINISTIC"), b"in", 1).await;
    s.durable.started(id).unwrap();
    s.durable.output(id, b"produced before the crash").unwrap();
    // A failing runner: calling it would fail the job.
    let s = start(&dir, s.crash(), true).await;
    assert_eq!(s.drive(id).await["state"], "completed");
    assert_eq!(s.calls(), 0);
    assert_eq!(
        s.client().result(id).await.unwrap(),
        b"produced before the crash"
    );
}

/// Runs an attempt up to a submitted receipt, and stops before it is seen.
async fn until_submitted(s: &Stack, id: [u8; 32]) {
    use mbongo_compute::worker::AttemptOutcome;
    for _ in 0..20 {
        s.chain.produce_block();
        if matches!(
            s.service.tick().await.unwrap(),
            Some(AttemptOutcome::Submitted { task_id, .. }) if task_id == id
        ) {
            return;
        }
    }
    panic!("no receipt submitted");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_crash_after_anchoring_binds_the_stored_output() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"anchored", 1).await;
    until_submitted(&s, id).await;
    s.chain.produce_block();
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.client().status(id).await.unwrap().state, "completed");
    assert_eq!(s.client().result(id).await.unwrap(), b"anchored");
    assert_eq!(s.calls(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_output_that_does_not_match_the_receipt_is_never_served() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"true output", 1).await;
    until_submitted(&s, id).await;
    s.chain.produce_block();
    s.durable.output(id, b"another output").unwrap();
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.client().status(id).await.unwrap().state, "completed");
    match s.client().result(id).await {
        Err(JobClientError::Rejected { status, code }) => {
            assert_eq!((status, code.as_str()), (410, "RESULT_LOST"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(s.calls(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failure_outside_the_job_still_parks() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"x", 1).await;
    for _ in 0..30 {
        s.chain.reject_next("refused by this test");
        s.chain.produce_block();
        s.service.tick().await.unwrap();
        if s.client().status(id).await.unwrap().parked {
            break;
        }
    }
    let status = s.client().status(id).await.unwrap();
    assert_eq!(status.failure_code.as_deref(), Some(ATTEMPTS_EXHAUSTED));
    assert_eq!(status.attempts, 3);
    assert_eq!(
        s.calls(),
        1,
        "the output is reused across the receipt retries"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn if_the_start_cannot_be_recorded_the_runner_is_not_called() {
    let s = start(&scratch(), FakeChain::new(), false).await;
    let _ = s.register(&spec("BIT_EXACT"), b"x", 1).await;
    s.durable.poison();
    for _ in 0..3 {
        s.chain.produce_block();
        let _ = s.service.tick().await;
    }
    assert_eq!(s.calls(), 0);
}

// ── task identity ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identical_jobs_with_different_salts_each_run_and_keep_their_own_output() {
    let s = start(&scratch(), FakeChain::new(), false).await;
    *s.runtime.varies.lock().unwrap() = true;
    let job = spec("NON_DETERMINISTIC");
    let first = s.register(&job, b"same input", 1).await;
    let second = s.register(&job, b"same input", 2).await;
    assert_ne!(first, second);
    assert_eq!(s.drive(first).await["state"], "completed");
    assert_eq!(s.drive(second).await["state"], "completed");
    assert_eq!(s.calls(), 2, "each task runs: an output is never shared");
    let (a, b) = (
        s.client().result(first).await.unwrap(),
        s.client().result(second).await.unwrap(),
    );
    assert_ne!(a, b, "two runs of a non-deterministic job");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stored_output_serves_its_own_task_only() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let job = spec("NON_DETERMINISTIC");
    let crashed = s.register(&job, b"same input", 1).await;
    let other = s.register(&job, b"same input", 2).await;
    s.durable.started(crashed).unwrap();
    s.durable.output(crashed, b"output of the first task").unwrap();
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.drive(crashed).await["state"], "completed");
    assert_eq!(s.drive(other).await["state"], "completed");
    assert_eq!(s.calls(), 1, "only the other task runs");
    assert_eq!(
        s.client().result(crashed).await.unwrap(),
        b"output of the first task"
    );
    assert_eq!(s.client().result(other).await.unwrap(), b"same input");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_result_matches_the_receipt_of_its_own_task() {
    use mbongo_compute::chain::{find_receipt, ChainClient};
    use mbongo_compute::execution::reference_output_commitment;
    let s = start(&scratch(), FakeChain::new(), false).await;
    *s.runtime.varies.lock().unwrap() = true;
    let job = spec("NON_DETERMINISTIC");
    let ids = [
        s.register(&job, b"same input", 1).await,
        s.register(&job, b"same input", 2).await,
    ];
    for id in ids {
        s.drive(id).await;
    }
    let tip = s.chain.latest_height().await.unwrap();
    for id in ids {
        let (_, receipt) = find_receipt(&s.chain, id, 1, tip).await.unwrap().unwrap();
        let output = s.client().result(id).await.unwrap();
        assert_eq!(
            receipt.output_commitment,
            reference_output_commitment(&output)
        );
    }
}

// ── exclusive ownership ──────────────────────────────────────────────────

#[test]
fn a_second_owner_is_refused_and_changes_nothing() {
    let dir = scratch();
    let first = store(&dir).unwrap();
    first.accept_request(Address::zero(), [1; 16], u64::MAX, 0).unwrap();
    let path = dir.join("state").join(JOURNAL);
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(store(&dir), Err(JournalError::Locked)));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    first.accept_request(Address::zero(), [2; 16], u64::MAX, 0).unwrap();
}

#[test]
fn ownership_is_released_when_the_store_closes() {
    let dir = scratch();
    let first = store(&dir).unwrap();
    first.accept_request(Address::zero(), [1; 16], u64::MAX, 0).unwrap();
    drop(first);
    let second = store(&dir).unwrap();
    assert_eq!(
        second.accept_request(Address::zero(), [1; 16], u64::MAX, 0),
        Err(AcceptError::Replayed)
    );
}

// ── retention ────────────────────────────────────────────────────────────

fn upload(caller: Address, at: u64) -> UploadEntry {
    UploadEntry {
        caller,
        object_ref: [1; 32],
        input_commitment: [2; 32],
        input: HexBytes(b"input".to_vec()),
        uploaded_at: at,
    }
}

#[test]
fn state_stays_bounded_and_an_upload_is_refused_not_evicted() {
    let dir = scratch();
    let policy = RetentionPolicy {
        max_tasks: 2,
        ..RetentionPolicy::default()
    };
    let d = DurableStore::open(&dir, &STATE_KEY, policy).unwrap();
    d.add_upload([1; 32], upload(Address::zero(), 0)).unwrap();
    d.add_upload([2; 32], upload(Address::zero(), 0)).unwrap();
    assert_eq!(
        d.add_upload([3; 32], upload(Address::zero(), 0)),
        Err(StoreError::Full)
    );
    assert_eq!(d.read(|s| s.uploads.len()).unwrap(), 2);
}

#[test]
fn uploads_are_kept_per_uploader() {
    let d = DurableStore::open(&scratch(), &STATE_KEY, RetentionPolicy::default()).unwrap();
    let (a, b) = (Address([1; 32]), Address([2; 32]));
    d.add_upload([9; 32], upload(a, 0)).unwrap();
    d.add_upload([9; 32], upload(b, 0)).unwrap();
    assert_eq!(d.read(|s| s.uploads.len()).unwrap(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn results_and_uploads_expire_on_policy() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"old", 1).await;
    s.drive(id).await;
    let policy = RetentionPolicy::default();
    s.durable.add_upload([5; 32], upload(Address::zero(), now())).unwrap();
    s.durable.maintain(now() + policy.input_ttl_secs + 10).unwrap();
    assert_eq!(s.durable.read(|d| d.uploads.len()).unwrap(), 0);
    assert_eq!(
        s.durable.read(|d| d.tasks.len()).unwrap(),
        1,
        "kept for now"
    );
    s.durable.maintain(now() + policy.result_retention_secs + 10).unwrap();
    assert_eq!(s.durable.read(|d| d.tasks.len()).unwrap(), 0);
}

#[test]
fn compaction_keeps_the_live_state_and_drops_the_rest() {
    let dir = scratch();
    let d = store(&dir).unwrap();
    let signer = Address([1; 32]);
    let t = now();
    d.accept_request(signer, [1; 16], t + 60, t).unwrap();
    d.accept_request(signer, [2; 16], t - 10, t - 20).unwrap();
    let key = [4u8; 32];
    d.attempt_failed(key, "JOB_RUNTIME_FAILED").unwrap();
    d.started(key).unwrap();
    for i in 0..200u8 {
        d.add_spec(&[i; 512]).unwrap();
    }
    let path = dir.join("state").join(JOURNAL);
    d.maintain(t).unwrap();
    d.compact().unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    drop(d);

    let d = store(&dir).unwrap();
    assert_eq!(
        d.accept_request(signer, [1; 16], t + 60, t),
        Err(AcceptError::Replayed),
        "a live identifier must survive compaction"
    );
    assert_eq!(d.read(|s| s.specs.len()).unwrap(), 200);
    assert_eq!(
        d.read(|s| s.started.get(&key).copied()).unwrap(),
        Some(1),
        "a retry that crashed must stay interrupted across compaction"
    );
    assert_eq!(
        d.read(|s| s.last_failure.get(&key).cloned()).unwrap().as_deref(),
        Some("JOB_RUNTIME_FAILED")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_completed_job_survives_compaction_and_a_restart() {
    let dir = scratch();
    let s = start(&dir, FakeChain::new(), false).await;
    let id = s.register(&spec("BIT_EXACT"), b"compacted", 1).await;
    s.drive(id).await;
    s.durable.compact().unwrap();
    let s = start(&dir, s.crash(), false).await;
    assert_eq!(s.client().result(id).await.unwrap(), b"compacted");
    assert_eq!(s.calls(), 0);
}
