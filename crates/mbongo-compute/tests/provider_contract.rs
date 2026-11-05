//! G2 Slice 2 acceptance tests (`G2_MBONGO_JOB_V1_PLAN.md` §9.10): the
//! recipient-bound signature, `mbongo-runner:v1`, and the provider service.
//!
//! The runner is `scripts/fixtures/mbongo-runner-fixture.mjs`, written
//! independently of this crate. The end-to-end proof on a real node, with
//! separate processes, is the `provider_harness` binary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mbongo_compute::chain::testing::FakeChain;
use mbongo_compute::chain::ChainClient;
use mbongo_compute::clock::SystemClock;
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::execution::reference_input_commitment;
use mbongo_compute::inference::canonical;
use mbongo_compute::job::{
    encode_execution_spec, job_spec_digest, JobError, JobProfile, JobSource, MeteringSink,
    RuntimeAdapter, RuntimeIdentity, UntrustedMetering,
};
use mbongo_compute::provider::auth::{sign_request, SignedRequest, HEADER_RECIPIENT};
use mbongo_compute::provider::client::{
    run_job, JobClientError, JobReport, ProviderClient, WaitPolicy,
};
use mbongo_compute::provider::containment::{Containment, ContainmentConfig};
use mbongo_compute::provider::durable::{DurableStore, RetentionPolicy};
use mbongo_compute::provider::runner::{RunnerConfig, SubprocessRuntime};
use mbongo_compute::provider::service::{
    MeteringLog, ProviderConfig, ProviderParts, ProviderService, ProviderStore,
};
use mbongo_core::Address;
use parity_scale_codec::Encode;
use serde_json::{json, Value};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn vectors() -> Value {
    let text = std::fs::read_to_string(format!("{ROOT}/test-vectors/job/job-contract-v1.json"));
    serde_json::from_str(&text.unwrap()).unwrap()
}

fn spec_json(name: &str) -> Value {
    let v = vectors();
    let specs = v["specs"].as_array().unwrap();
    specs.iter().find(|s| s["name"] == name).unwrap()["spec"].clone()
}

fn with_mode(name: &str, mode: &str) -> Value {
    let mut spec = spec_json(name);
    spec["params"] = json!({ "mode": mode });
    spec
}

fn canon(v: &Value) -> Vec<u8> {
    canonical::canonicalize(&serde_json::to_string(v).unwrap(), 1 << 20).unwrap()
}

fn node() -> String {
    let name = format!("node{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
        .expect("node on PATH")
        .display()
        .to_string()
}

fn scratch() -> PathBuf {
    // Unique per run, not only per process: state is durable here, and a
    // reused process id must not reopen an earlier run's journal.
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let n = NEXT.fetch_add(1, Ordering::SeqCst);
    let run = hex::encode(mbongo_compute::mvp::client::random_bytes::<8>());
    let dir = std::env::temp_dir().join(format!("mbongo-provider-test-{run}-{n}"));
    std::fs::create_dir_all(dir.join("artifacts")).unwrap();
    std::fs::create_dir_all(dir.join("work")).unwrap();
    dir
}

fn identity() -> RuntimeIdentity {
    RuntimeIdentity {
        family: "test-fixture".to_owned(),
        revision: "1".to_owned(),
        binary_sha256: None,
        accelerator_class: "cpu".to_owned(),
    }
}

fn runner_config(dir: &Path) -> RunnerConfig {
    RunnerConfig {
        identity: identity(),
        command: vec![
            node(),
            format!("{ROOT}/scripts/fixtures/mbongo-runner-fixture.mjs"),
        ],
        pass_env: vec!["SystemRoot".to_owned()],
        work_dir: dir.join("work"),
    }
}

// ── the runner, through the real JobProfile ──────────────────────────────

#[derive(Default)]
struct Source {
    specs: Mutex<BTreeMap<[u8; 32], Vec<u8>>>,
    artifacts: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl JobSource for Source {
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

struct NoMetering;

impl MeteringSink for NoMetering {
    fn record(&self, _: &[u8; 32], _: &UntrustedMetering) {}
}

struct Runner {
    source: Arc<Source>,
    profile: JobProfile,
    dir: PathBuf,
}

impl Runner {
    fn new() -> Self {
        let dir = scratch();
        let source = Arc::new(Source::default());
        let table = hex::decode(vectors()["table"].as_str().unwrap()).unwrap();
        let sha = spec_json("fixture-byte-table")["artifacts"][0]["sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        source.artifacts.lock().unwrap().insert(sha, table);
        let runtime = SubprocessRuntime::new(
            runner_config(&dir),
            Arc::clone(&source) as Arc<dyn JobSource>,
        );
        let profile = JobProfile::new(
            Arc::clone(&source) as Arc<dyn JobSource>,
            vec![Arc::new(runtime) as Arc<dyn RuntimeAdapter>],
            Arc::new(NoMetering),
        );
        Self {
            source,
            profile,
            dir,
        }
    }

    fn run(&self, spec: &Value, input: &[u8]) -> Result<Vec<u8>, JobError> {
        let bytes = canon(spec);
        let digest = job_spec_digest(&bytes);
        self.source.specs.lock().unwrap().insert(digest, bytes);
        self.profile.run(
            &encode_execution_spec(&digest),
            &reference_input_commitment(input),
            input,
        )
    }
}

#[test]
fn s2_1_an_independent_runner_reproduces_the_vectors() {
    let r = Runner::new();
    let v = vectors();
    let cases = v["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for c in cases {
        let input = hex::decode(c["input"].as_str().unwrap()).unwrap();
        let out = r.run(&spec_json(c["workload"].as_str().unwrap()), &input).unwrap();
        assert_eq!(hex::encode(out), c["output"].as_str().unwrap());
    }
}

#[test]
fn s2_3_each_contract_breach_has_its_named_failure() {
    let r = Runner::new();
    let cases = [
        ("exit-1", JobError::RuntimeFailed),
        ("no-frame", JobError::RuntimeFailed),
        ("wrong-version", JobError::RuntimeFailed),
        ("short", JobError::RuntimeFailed),
        ("extra", JobError::RuntimeFailed),
        ("not-canonical", JobError::OutputNotCanonical),
    ];
    for (mode, expected) in cases {
        assert_eq!(
            r.run(&with_mode("fixture-sum", mode), b"[1]"),
            Err(expected),
            "{mode}"
        );
    }
}

#[test]
fn s2_4_output_over_the_limit_is_cut_and_killed() {
    let r = Runner::new();
    for mode in ["declare-huge", "flood"] {
        let started = Instant::now();
        assert_eq!(
            r.run(&with_mode("fixture-sum", mode), b"[1]"),
            Err(JobError::LimitOutputSize),
            "{mode}"
        );
        assert!(started.elapsed() < Duration::from_secs(4), "{mode}");
    }
}

#[test]
fn s2_4_a_runner_past_its_deadline_is_killed() {
    let r = Runner::new();
    let heartbeat = r.dir.join("heartbeat");
    let mut spec = with_mode("fixture-sum", "sleep");
    spec["params"]["heartbeat"] = json!(heartbeat.display().to_string());
    spec["limits"]["max_wall_ms"] = json!(1500);
    assert_eq!(r.run(&spec, b"[1]"), Err(JobError::LimitWallTime));

    std::thread::sleep(Duration::from_millis(300));
    let size = || std::fs::metadata(&heartbeat).map_or(0, |m| m.len());
    let after_kill = size();
    assert!(
        after_kill > 0,
        "the runner never started: the test proves nothing"
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(size(), after_kill, "the runner is still running");
}

#[test]
fn s2_5_the_runner_gets_no_credential_and_no_input_on_disk() {
    let r = Runner::new();
    std::env::set_var("MBONGO_MVP_EXECUTOR_KEY_FILE", "/secret/path");
    let mut spec = with_mode("fixture-byte-table", "report");
    spec["output"]["max_bytes"] = json!(4096);
    let report: Value = serde_json::from_slice(&r.run(&spec, b"private input").unwrap()).unwrap();

    let env: Vec<&str> =
        report["env"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(env.contains(&"MBONGO_JOB_DIR"));
    let allowed = ["MBONGO_JOB_DIR", "SystemRoot", "SYSTEMROOT"];
    assert!(env.iter().all(|k| allowed.contains(k)), "{env:?}");
    assert_eq!(report["files"], json!(["artifacts", "job.json"]));
    assert_eq!(report["cwd_is_job_dir"], true);
    let left = std::fs::read_dir(r.dir.join("work")).unwrap().count();
    assert_eq!(left, 0, "the job directory was not removed");
    // This shows nothing was handed over. It does not show key isolation.
}

#[test]
fn a_pinned_runner_binary_is_verified() {
    let dir = scratch();
    let mut config = runner_config(&dir);
    config.identity.binary_sha256 = Some("0".repeat(64));
    let runtime = SubprocessRuntime::new(config, Arc::new(Source::default()));
    assert!(runtime.verify_binary().is_err());
}

// ── the service, over HTTP ───────────────────────────────────────────────

struct Stack {
    chain: FakeChain,
    service: ProviderService<FakeChain>,
    metering: Arc<MeteringLog>,
    base: String,
}

async fn start(seed: u8, chain: FakeChain) -> Stack {
    let dir = scratch();
    let v = vectors();
    let sha = spec_json("fixture-byte-table")["artifacts"][0]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let table = hex::decode(v["table"].as_str().unwrap()).unwrap();
    std::fs::write(dir.join("artifacts").join(sha), table).unwrap();
    let store = Arc::new(ProviderStore::new(dir.join("artifacts")));
    let runtime = SubprocessRuntime::new(
        runner_config(&dir),
        Arc::clone(&store) as Arc<dyn JobSource>,
    );
    let metering = Arc::new(MeteringLog::default());
    let durable =
        DurableStore::open(&dir.join("state"), &[7u8; 32], RetentionPolicy::default()).unwrap();
    let service = ProviderService::open(
        chain.clone(),
        Arc::new(SystemClock),
        [seed; 32],
        LocalKey::from_seed(&[seed; 32]),
        ProviderParts {
            store,
            runtimes: vec![Arc::new(runtime) as Arc<dyn RuntimeAdapter>],
            metering: Arc::clone(&metering) as Arc<dyn MeteringSink>,
        },
        Arc::new(durable),
        ProviderConfig::default(),
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = service.router();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (c, s) = (chain.clone(), service.clone());
    tokio::spawn(async move {
        loop {
            c.produce_block();
            let _ = s.tick().await;
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    });
    Stack {
        chain,
        service,
        metering,
        base,
    }
}

const WAIT: WaitPolicy = WaitPolicy {
    timeout: Duration::from_secs(40),
    poll: Duration::from_millis(20),
};

impl Stack {
    fn client(&self, key: &LocalKey) -> ProviderClient {
        ProviderClient::new(&self.base, key.clone(), self.service.executor()).unwrap()
    }

    async fn job(&self, spec: &Value, input: &[u8]) -> Result<JobReport, JobClientError> {
        let key = LocalKey::from_seed(&[0xAA; 32]);
        run_job(
            &self.client(&key),
            &self.chain,
            &key,
            &canon(spec),
            input,
            WAIT,
        )
        .await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_2_a_job_reaches_a_receipt_verified_at_t1() {
    let s = start(0xB1, FakeChain::new()).await;
    let report = s.job(&spec_json("fixture-sum"), b"[2,3]").await.unwrap();
    assert_eq!(report.output, hex::encode(b"{\"count\":2,\"sum\":5}"));
    assert_eq!(report.technical_level, "T1");
    assert_eq!(report.business_correctness, "NOT_ASSESSED");
    assert_eq!(s.metering.len(), 1, "metering stays on the private side");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_3_retryable_failures_park_after_three_attempts_others_at_once() {
    let s = start(0xB2, FakeChain::new()).await;
    assert_eq!(
        s.job(&with_mode("fixture-sum", "exit-1"), b"[1]").await,
        Err(JobClientError::JobFailed {
            attempts: 3,
            code: "JOB_RUNTIME_FAILED".to_owned()
        })
    );
    assert_eq!(
        s.job(&with_mode("fixture-sum", "not-canonical"), b"[1]").await,
        Err(JobClientError::JobFailed {
            attempts: 1,
            code: "JOB_OUTPUT_NOT_CANONICAL".to_owned()
        })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_6_a_job_this_provider_cannot_run_never_reaches_the_chain() {
    let s = start(0xB3, FakeChain::new()).await;
    let mut no_runtime = spec_json("fixture-sum");
    no_runtime["runtime"]["family"] = json!("nope");
    let mut no_artifact = spec_json("fixture-byte-table");
    no_artifact["artifacts"][0]["sha256"] = json!("a".repeat(64));
    let mut too_long = spec_json("fixture-sum");
    too_long["limits"]["max_wall_ms"] = json!(900_000_000);
    for (spec, code) in [
        (no_runtime, "JOB_RUNTIME_UNKNOWN"),
        (no_artifact, "JOB_ARTIFACT_NOT_FOUND"),
        (too_long, "PROVIDER_LIMIT_EXCEEDED"),
    ] {
        match s.job(&spec, b"[1]").await {
            Err(JobClientError::Rejected { code: c, .. }) => assert_eq!(c, code),
            other => panic!("{code}: {other:?}"),
        }
    }
    let client = LocalKey::from_seed(&[0xAA; 32]).address();
    assert_eq!(s.chain.account_nonce(&client).await.unwrap(), 0);
}

/// Sends `GET /v1/tasks/{id}` with these headers; an accepted request ends in
/// 404, so anything else is the authentication's answer.
async fn probe(base: &str, path: &str, headers: &[(&'static str, String)]) -> (u16, String) {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut req = client.get(format!("{base}{path}"));
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_7_requests_bind_to_one_provider_and_are_used_once() {
    let chain = FakeChain::new();
    let a = start(0xB4, chain.clone()).await;
    let b = start(0xB5, chain).await;
    let key = LocalKey::from_seed(&[0xAA; 32]);
    let path = format!("/v1/tasks/{}", "0".repeat(64));
    let t = now();
    let request = |recipient: Address, issued_at: u64, expires_at: u64, id: u8| {
        sign_request(
            &key,
            &SignedRequest {
                recipient,
                method: "GET",
                path: &path,
                body: b"",
                issued_at,
                expires_at,
                request_id: [id; 16],
            },
        )
    };
    let (pa, pb) = (a.service.executor(), b.service.executor());
    let refused = |code: &str| (401, code.to_owned());
    let accepted = (404, "TASK_NOT_FOUND".to_owned());

    // Accepted once, by its recipient only.
    let for_a = request(pa, t, t + 60, 1);
    assert_eq!(probe(&a.base, &path, &for_a).await, accepted);
    assert_eq!(
        probe(&a.base, &path, &for_a).await,
        refused("AUTH_REPLAYED")
    );

    // A provider replays a client's request to another provider.
    let captured = request(pa, t, t + 60, 2);
    assert_eq!(
        probe(&b.base, &path, &captured).await,
        refused("AUTH_WRONG_RECIPIENT")
    );
    // Rewriting the recipient breaks the signature.
    let mut rewritten = captured.clone();
    for h in &mut rewritten {
        if h.0 == HEADER_RECIPIENT {
            h.1 = hex::encode(pb.0);
        }
    }
    assert_eq!(
        probe(&b.base, &path, &rewritten).await,
        refused("AUTH_BAD_SIGNATURE")
    );

    // Tampering: the same headers on another path.
    let other = format!("/v1/tasks/{}", "1".repeat(64));
    let tampered = request(pa, t, t + 60, 3);
    assert_eq!(
        probe(&a.base, &other, &tampered).await,
        refused("AUTH_BAD_SIGNATURE")
    );

    // The window.
    assert_eq!(
        probe(&a.base, &path, &request(pa, t - 200, t - 100, 4)).await,
        refused("AUTH_EXPIRED")
    );
    assert_eq!(
        probe(&a.base, &path, &request(pa, t + 90, t + 100, 5)).await,
        refused("AUTH_NOT_YET_VALID")
    );
    assert_eq!(
        probe(&a.base, &path, &request(pa, t, t + 3_600, 6)).await,
        refused("AUTH_BAD_WINDOW")
    );

    // Nothing signed at all.
    assert_eq!(probe(&a.base, &path, &[]).await, refused("AUTH_MISSING"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_7_a_body_change_and_another_submitter_are_refused() {
    let s = start(0xB6, FakeChain::new()).await;
    let report = s.job(&spec_json("fixture-sum"), b"[1]").await.unwrap();
    let task_id: [u8; 32] = hex::decode(&report.task_id).unwrap().try_into().unwrap();

    let stranger = LocalKey::from_seed(&[0xCC; 32]);
    match s.client(&stranger).result(task_id).await {
        Err(JobClientError::Rejected { code, .. }) => assert_eq!(code, "NOT_TASK_SUBMITTER"),
        other => panic!("{other:?}"),
    }

    let key = LocalKey::from_seed(&[0xAA; 32]);
    let body = canon(&spec_json("fixture-sum"));
    let t = now();
    let headers = sign_request(
        &key,
        &SignedRequest {
            recipient: s.service.executor(),
            method: "PUT",
            path: "/v1/job-specs",
            body: &body,
            issued_at: t,
            expires_at: t + 60,
            request_id: [9; 16],
        },
    );
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let mut req = http.put(format!("{}/v1/job-specs", s.base));
    for (name, value) in headers {
        req = req.header(name, value);
    }
    let changed = canon(&spec_json("fixture-byte-table"));
    let resp = req.body(changed).send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);
    let answer: Value = resp.json().await.unwrap();
    assert_eq!(answer["error"], "AUTH_BAD_SIGNATURE");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn s2_8_job_content_reaches_no_status_chain_or_metering() {
    const CANARY: &[u8] = b"CANARY-5e1d-private";
    let s = start(0xB7, FakeChain::new()).await;
    let mut spec = with_mode("fixture-byte-table", "stderr");
    spec["input"]["max_bytes"] = json!(64);
    let failure = s.job(&spec, CANARY).await.unwrap_err();
    let good = s.job(&spec_json("fixture-byte-table"), CANARY).await.unwrap();

    let text = String::from_utf8_lossy(CANARY).into_owned();
    assert!(!failure.to_string().contains(&text), "{failure}");
    assert!(!serde_json::to_string(&good).unwrap().contains(&text));
    let tip = s.chain.latest_height().await.unwrap();
    let mut blocks = 0;
    for height in 1..=tip {
        if let Some(block) = s.chain.block_by_height(height).await.unwrap() {
            blocks += 1;
            let encoded = block.encode();
            assert!(
                !encoded.windows(CANARY.len()).any(|w| w == CANARY),
                "the input reached block {height}"
            );
        }
    }
    assert!(blocks > 0, "no block was scanned: the check proves nothing");
}

#[test]
fn no_workload_is_compiled_into_the_provider_or_job_modules() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = 0;
    for dir in ["provider", "job"] {
        for entry in std::fs::read_dir(src.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap().to_lowercase();
            for forbidden in ["kaayu", "mbongopay", "cv_text"] {
                assert!(!text.contains(forbidden), "{}: {forbidden}", path.display());
            }
            files += 1;
        }
    }
    assert_eq!(files, 16);
}

/// `required` is proven or refused. It is never a warning, and the mode is
/// never assumed. The contained path itself runs under `provider_harness
/// --contained`, which needs root.
#[test]
fn isolation_is_stated_and_refused_when_it_cannot_be_proven() {
    let dir = scratch();
    let key = dir.join("key");
    std::fs::write(&key, hex::encode([0x57u8; 32])).unwrap();
    let mut config = json!({
        "bind": "127.0.0.1:0",
        "rpc_url": "http://127.0.0.1:1/rpc",
        "rest_url": "http://127.0.0.1:1",
        "executor_key_file": key,
        "state_dir": dir.join("state"),
        "state_key_file": key,
        "artifact_dir": dir.join("artifacts"),
        "work_dir": dir.join("work"),
        "runtimes": [],
    });
    let start = |config: &Value| {
        let file = dir.join("provider.json");
        std::fs::write(&file, config.to_string()).unwrap();
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_provider_service"))
            .arg("--config")
            .arg(&file)
            .output()
            .unwrap();
        assert!(!out.status.success());
        String::from_utf8_lossy(&out.stderr).into_owned()
    };

    let err = start(&config);
    assert!(err.contains("missing field `isolation`"), "{err}");

    config["isolation"] = json!({
        "mode": "required",
        "runner_uid": 54_321,
        "runner_gid": 54_321,
        "cgroup_root": dir.join("no-cgroup"),
        "max_memory_mib": 64,
        "max_pids": 16,
    });
    let err = start(&config);
    assert!(err.contains("missing field `network`"), "{err}");

    config["isolation"]["network"] = json!("denied");
    let err = start(&config);
    let expected = if cfg!(target_os = "linux") {
        [
            "ISOLATION_REFUSED=CGROUP_UNAVAILABLE",
            "ISOLATION_REFUSED=SERVICE_IDENTITY",
        ]
    } else {
        ["ISOLATION_REFUSED=UNSUPPORTED_PLATFORM"; 2]
    };
    assert!(expected.iter().any(|code| err.contains(code)), "{err}");
    assert!(!err.contains("KEY_ISOLATION_ENFORCED=YES"), "{err}");
}

#[test]
fn a_root_runner_is_refused_before_anything_else() {
    let dir = scratch();
    let refused = Containment::establish(ContainmentConfig {
        runner_uid: 0,
        runner_gid: 0,
        cgroup_root: dir.join("cgroup"),
        max_memory_mib: 64,
        max_pids: 16,
        secret_files: Vec::new(),
        private_dirs: Vec::new(),
        work_dir: dir.join("work"),
        denied_paths: Vec::new(),
        network_denied: true,
    })
    .unwrap_err();
    let expected: &[&str] = if cfg!(target_os = "linux") {
        &["RUNNER_IDENTITY", "SERVICE_IDENTITY"]
    } else {
        &["UNSUPPORTED_PLATFORM"]
    };
    assert!(expected.contains(&refused.code), "{refused}");
}
