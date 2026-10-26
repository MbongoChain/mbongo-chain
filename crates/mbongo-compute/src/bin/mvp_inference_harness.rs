//! MVP-B real-model harness: the pinned llama.cpp runtime and the pinned
//! Qwen3-4B-Instruct-2507 GGUF on CPU, executing the frozen
//! `mbongo-infer:v1` contract with the active MVP profile
//! `kaayu-cv-extract:v2` (change control CC-1).
//!
//! **Not run in CI** (it needs the 2.5 GB model). Prerequisite:
//! `scripts/setup-mvp-inference-runtime.sh` has populated the cache.
//!
//! ```text
//! mvp_inference_harness corpus [--scan-out <file>]   # REAL_LLAMA_CPP_TEST, REAL_MODEL_TEST
//! mvp_inference_harness e2e    [--scan-out <file>]   # REAL_WORKER_E2E on a live local node
//! ```
//!
//! Prints only safe facts: digests, timings, token counts, byte counts,
//! structural counts and error codes. It never prints a CV, a prompt or a
//! result. `--scan-out` writes the synthetic private strings to a file
//! **outside the repository** so a separate step can prove they appear in no
//! log.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mbongo_compute::chain::{find_receipt, scan_tasks, ChainClient, RpcChainClient};
use mbongo_compute::clock::{Clock, SystemClock};
use mbongo_compute::control_plane::{ControlPlane, ControlPlaneConfig, TaskState};
use mbongo_compute::cpu_inference::config::RuntimeConfig;
use mbongo_compute::cpu_inference::{
    InferenceBackend, InferenceProfile, LlamaServer, ProfileDispatch,
};
use mbongo_compute::data_plane::{
    CapabilityRequest, InMemoryDataPlane, LocalKey, Operation, Presentation,
};
use mbongo_compute::execution::{reference_output_commitment, ReverseBytesProfile};
use mbongo_compute::identity::{ExecutorKey, IdSource};
use mbongo_compute::inference::canonical;
use mbongo_compute::inference::kaayu::{CvExtractionInputV1, MAX_OUTPUT_BYTES};
use mbongo_compute::profiles::{self, kaayu_v2};
use mbongo_compute::worker::{AttemptOutcome, Worker};
use mbongo_core::{
    Address, ComputeTask, Transaction, TransactionPayload, TransactionType, COMPUTE_TASK_VERSION,
};
use mbongo_verification::verify_receipt_signature;
use tokio::process::{Child, Command};
use tokio::time::sleep;

const CORPUS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-vectors/inference/synthetic-cv-corpus-v1.json"
);
const RPC_PORT: u16 = 42944;
const REST_PORT: u16 = 42080;
const P2P_PORT: u16 = 52333;

struct Case {
    id: String,
    name: String,
    cv_text: String,
}

fn corpus() -> (Vec<Case>, Vec<(String, String, String)>) {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(CORPUS).expect("corpus"))
            .expect("corpus JSON");
    let cases = v["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| Case {
            id: c["id"].as_str().unwrap().into(),
            name: c["name"].as_str().unwrap().into(),
            cv_text: c["cv_text"].as_str().unwrap().into(),
        })
        .collect();
    let negatives = v["negatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let text = n.get("cv_text").and_then(serde_json::Value::as_str).map_or_else(
                || {
                    let r = &n["cv_text_repeat"];
                    r["unit"]
                        .as_str()
                        .unwrap()
                        .repeat(usize::try_from(r["count"].as_u64().unwrap()).unwrap())
                },
                str::to_owned,
            );
            (
                n["id"].as_str().unwrap().to_owned(),
                text,
                n["expected_error"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    (cases, negatives)
}

fn start_backend() -> Result<(Arc<LlamaServer>, InferenceProfile), String> {
    let spec = profiles::active_job_spec();
    let cfg = RuntimeConfig::from_env(&spec.model.artifact_name)?;
    println!(
        "runtime_path_file={}",
        cfg.runtime_path.file_name().unwrap().to_string_lossy()
    );
    println!(
        "model_file={}",
        cfg.model_path.file_name().unwrap().to_string_lossy()
    );
    println!(
        "threads={} ctx_size={} port={} gpu_layers=0 inference_timeout_s={}",
        cfg.threads,
        cfg.ctx_size,
        cfg.port,
        cfg.inference_timeout.as_secs()
    );
    let (server, report) =
        LlamaServer::start(cfg, &spec).map_err(|e| format!("start-up refused: {e}"))?;
    println!(
        "REAL_RUNTIME_VERSION={} build={} commit={}",
        report.runtime_version, report.runtime_build, report.runtime_commit_prefix
    );
    println!("REAL_RUNTIME_SHA256={}", report.runtime_sha256);
    println!("REAL_MODEL_SHA256={}", report.model_sha256);
    println!("MODEL_VERIFY_MS={}", report.model_verify_ms);
    println!("COLD_START_MS={}", report.cold_start_ms);
    let server = Arc::new(server);
    println!("SERVER_PID={}", server.pid().unwrap_or(0));
    let profile = InferenceProfile::new(Arc::clone(&server) as Arc<dyn InferenceBackend>)
        .map_err(|e| format!("profile refused: {e}"))?;
    Ok((server, profile))
}

fn percentile(sorted: &[u128], p: f64) -> u128 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let rank = ((p / 100.0) * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

fn structural(bytes: &[u8]) -> Result<String, String> {
    if !canonical::is_canonical(bytes, MAX_OUTPUT_BYTES) {
        return Err("result is not canonical".into());
    }
    let p = kaayu_v2::parse_output(std::str::from_utf8(bytes).map_err(|_| "result not UTF-8")?)
        .map_err(|e| e.code().to_string())?;
    if p.canonical_bytes() != bytes {
        return Err("re-canonicalization changed the bytes".into());
    }
    Ok(format!(
        "skills={} experience={} education={} languages={} certifications={} summary_chars={}",
        p.skills.len(),
        p.experience.len(),
        p.education.len(),
        p.languages.len(),
        p.certifications.len(),
        p.summary.chars().count()
    ))
}

#[allow(clippy::too_many_lines)]
fn run_corpus(scan: &mut Vec<String>) -> Result<(), String> {
    let (server, profile) = start_backend()?;
    let spec_bytes = profile.execution_spec().to_vec();
    let (cases, negatives) = corpus();
    println!("CORPUS_CASES={}", cases.len());
    let mut durations = Vec::new();
    let mut schema_ok = 0usize;
    let mut first_result: Option<Vec<u8>> = None;
    for c in &cases {
        let input =
            CvExtractionInputV1::new(c.cv_text.clone()).map_err(|e| format!("{}: {e}", c.id))?;
        let bytes = input.canonical_bytes();
        let commitment = input.input_commitment();
        scan.push(c.cv_text.clone());
        let t = Instant::now();
        let outcome = profile.run(&spec_bytes, &commitment, &bytes);
        let ms = t.elapsed().as_millis();
        let obs = profile.observations().lock().unwrap().last().cloned().unwrap();
        match outcome {
            Ok(result) => {
                let s = structural(&result).map_err(|e| format!("{}: {e}", c.id))?;
                if reference_output_commitment(&result) == [0u8; 32] {
                    return Err("impossible commitment".into());
                }
                schema_ok += 1;
                durations.push(ms);
                scan.push(String::from_utf8(result.clone()).unwrap());
                if c.id == "R01" {
                    first_result = Some(result.clone());
                }
                if c.id == "R06" {
                    println!(
                        "  R06 repeat of R01: canonical bytes {} the R01 result",
                        if first_result.as_deref() == Some(result.as_slice()) {
                            "IDENTICAL to"
                        } else {
                            "DIFFER from"
                        }
                    );
                }
                if c.id == "R05" {
                    let text = String::from_utf8_lossy(&result);
                    println!(
                        "  R05 injection: contains '@'={} contains 'principal'={}",
                        text.contains('@'),
                        text.contains("principal")
                    );
                }
                println!(
                    "  {} {:<22} OK  duration_ms={ms} input_tokens={:?} output_tokens={:?} finish={:?} result_bytes={} {s}",
                    c.id, c.name, obs.input_tokens, obs.output_tokens, obs.finish_reason, result.len()
                );
            }
            Err(e) => {
                let obs = profile.observations().lock().unwrap().last().cloned().unwrap();
                println!(
                    "  {} {:<22} FAIL {} output_tokens={:?} finish={:?}",
                    c.id,
                    c.name,
                    e.code(),
                    obs.output_tokens,
                    obs.finish_reason
                );
            }
        }
    }
    println!("REAL_MODEL_SCHEMA_PASS={schema_ok}/{}", cases.len());
    let mut sorted = durations.clone();
    sorted.sort_unstable();
    if !sorted.is_empty() {
        println!("WARM_DURATIONS_MS={durations:?}");
        println!(
            "WARM_P50_MS={} WARM_P95_MS={} WARM_MAX_MS={}",
            percentile(&sorted, 50.0),
            percentile(&sorted, 95.0),
            sorted[sorted.len() - 1]
        );
    }
    // Negatives: refused before any inference.
    let calls_before = profile.observations().lock().unwrap().len();
    for (id, text, expected) in &negatives {
        let got = CvExtractionInputV1::new(text.clone());
        let code = match got {
            Ok(i) => profile
                .run(&spec_bytes, &i.input_commitment(), &i.canonical_bytes())
                .err()
                .map_or("OK", |e| e.code()),
            Err(e) => e.code(),
        };
        println!("  {id} expected {expected} got {code}");
        if code != expected {
            return Err(format!("{id}: expected {expected}, got {code}"));
        }
    }
    let inferred_after = profile
        .observations()
        .lock()
        .unwrap()
        .iter()
        .skip(calls_before)
        .filter(|o| o.input_tokens.is_some())
        .count();
    println!("NEGATIVES_REACHED_MODEL={inferred_after}");
    // In-flight crash drill: kill the server while a request is running.
    let victim = CvExtractionInputV1::new(cases[1].cv_text.clone()).unwrap();
    let killer = {
        let s = Arc::clone(&server);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(4));
            s.kill_server();
        })
    };
    let drill = profile.run(
        &spec_bytes,
        &victim.input_commitment(),
        &victim.canonical_bytes(),
    );
    killer.join().unwrap();
    println!(
        "DRILL_KILL_IN_FLIGHT={}",
        drill.as_ref().err().map_or("OK (no failure!)", |e| e.code())
    );
    let again = profile.run(
        &spec_bytes,
        &victim.input_commitment(),
        &victim.canonical_bytes(),
    );
    println!(
        "DRILL_AFTER_RESTART={} restarts={}",
        again.as_ref().err().map_or("OK", |e| e.code()),
        server.restarts()
    );
    if drill.is_ok() || again.is_err() || server.restarts() != 1 {
        return Err("in-flight crash drill did not fail closed and recover".into());
    }
    if schema_ok != cases.len() {
        return Err("not every corpus case produced schema-valid canonical JSON".into());
    }
    Ok(())
}

// ── live chain end to end ────────────────────────────────────────────

fn node_binary_path() -> PathBuf {
    let exe = std::env::current_exe().expect("own path");
    exe.parent().unwrap().join(if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    })
}

fn spawn_producer(data_dir: &std::path::Path) -> Result<Child, String> {
    let bin = node_binary_path();
    if !bin.exists() {
        return Err(format!(
            "node binary not found at {}; build mbongo-node in the same profile",
            bin.display()
        ));
    }
    Command::new(&bin)
        .args(["--producer", "--block-time", "1"])
        .args([
            "--rpc-port",
            &RPC_PORT.to_string(),
            "--rest-port",
            &REST_PORT.to_string(),
            "--p2p-port",
            &P2P_PORT.to_string(),
        ])
        .arg("--data-dir")
        .arg(data_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("spawn producer: {e}"))
}

fn signed(
    from_sign: impl Fn(&[u8]) -> [u8; 64],
    sender: Address,
    tx_type: TransactionType,
    receiver: Address,
    amount: u128,
    nonce: u64,
    payload: TransactionPayload,
) -> Transaction {
    let mut tx = Transaction {
        tx_type,
        sender,
        receiver,
        amount,
        nonce,
        payload,
        signature: [0u8; 64],
    };
    tx.signature = from_sign(&tx.signing_payload());
    tx
}

async fn wait<T, F, Fut>(what: &str, secs: u64, mut f: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Option<T>, String>>,
{
    let start = Instant::now();
    loop {
        if let Some(v) = f().await? {
            return Ok(v);
        }
        if start.elapsed() > Duration::from_secs(secs) {
            return Err(format!("timed out waiting for {what}"));
        }
        sleep(Duration::from_millis(300)).await;
    }
}

#[allow(clippy::too_many_lines)]
async fn run_e2e(chain: &RpcChainClient, scan: &mut Vec<String>) -> Result<(), String> {
    let (server, profile) = start_backend()?;
    let spec_bytes = profile.execution_spec().to_vec();
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&SystemClock.now().to_le_bytes());
    let mut ids = IdSource::new(seed);
    let client = LocalKey::from_seed(&[0xAAu8; 32]);
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);

    println!("Phase 1: fund the executor");
    chain
        .submit_transaction(&signed(
            |m| client.sign(m),
            client.address(),
            TransactionType::Transfer,
            executor.address(),
            1_000,
            0,
            TransactionPayload::None,
        ))
        .await
        .map_err(|e| e.to_string())?;
    wait("funding", 90, || async {
        Ok(
            (chain.account_nonce(&client.address()).await.map_err(|e| e.to_string())? >= 1)
                .then_some(()),
        )
    })
    .await?;

    println!(
        "Phase 2: client stores the private input off-chain and commits an mbongo-infer:v1 task"
    );
    let (cases, _) = corpus();
    let cv = cases[0].cv_text.clone();
    scan.push(cv.clone());
    let input = CvExtractionInputV1::new(cv).map_err(|e| e.to_string())?;
    let input_bytes = input.canonical_bytes();
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&ids.next("salt"));
    let task = ComputeTask {
        version: COMPUTE_TASK_VERSION,
        submitter: client.address(),
        executor: executor.address(),
        salt,
        input_commitment: input.input_commitment(),
        execution_spec: spec_bytes.clone(),
    };
    let task_id = task.task_id();
    let mut dp = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(ids.next("dp")));
    let object = dp.store_input(
        &client,
        task_id,
        task.input_commitment,
        input_bytes.clone(),
        3_600,
    );
    dp.register_task(&client, task_id, executor.address())
        .map_err(|e| e.to_string())?;
    dp.delegate_issuer(&client, task_id, issuer.address())
        .map_err(|e| e.to_string())?;
    chain
        .submit_transaction(&signed(
            |m| client.sign(m),
            client.address(),
            TransactionType::ComputeTask,
            Address::zero(),
            0,
            1,
            TransactionPayload::ComputeTask(Box::new(task.clone())),
        ))
        .await
        .map_err(|e| format!("ComputeTask refused: {e}"))?;
    let observed = wait("the task in a block", 90, || async {
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        let tasks = scan_tasks(chain, 1, latest).await.map_err(|e| e.to_string())?;
        Ok(tasks.into_iter().find(|t| t.task_id == task_id).map(|t| t.height))
    })
    .await?;
    println!(
        "  COMPUTE_TASK_ACCEPTED height={observed} task_id={} execution_spec_len={}",
        hex::encode(&task_id[..8]),
        spec_bytes.len()
    );

    // Lease longer than the inference timeout: a lease must outlive the work.
    let mut cp = ControlPlane::new(
        Arc::clone(&clock),
        IdSource::new(ids.next("cp")),
        issuer,
        ControlPlaneConfig {
            lease_secs: 600,
            session_secs: 1_800,
            confirmation_depth: 1,
            capability_secs: 600,
        },
    );
    let start = Instant::now();
    loop {
        cp.observe(chain).await.map_err(|e| e.to_string())?;
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        if cp.task(task_id).is_some_and(|t| t.state == TaskState::Discovered) && latest > observed {
            break;
        }
        if start.elapsed() > Duration::from_secs(90) {
            return Err("timed out waiting for the confirmed task".into());
        }
        sleep(Duration::from_millis(300)).await;
    }
    cp.register_input(task_id, object).map_err(|e| e.to_string())?;

    let dispatch = ProfileDispatch::new(vec![Box::new(ReverseBytesProfile), Box::new(profile)]);
    let mut worker = Worker::new(&mut ids, executor.clone(), Box::new(dispatch));

    println!("Phase 3: in-flight crash drill — llama-server killed during the first attempt");
    let killer = {
        let s = Arc::clone(&server);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(4));
            s.kill_server();
        })
    };
    let first = worker.run_once(&mut cp, &mut dp, chain).await.map_err(|e| e.to_string())?;
    killer.join().unwrap();
    println!("  FIRST_ATTEMPT={first:?}");
    if !matches!(first, AttemptOutcome::Failed { .. }) {
        return Err("a killed server did not fail the attempt".into());
    }
    if dp.result_ref(task_id, executor.address()).is_some() {
        return Err("a result was persisted by a failed inference".into());
    }

    println!("Phase 4: retry — same task, same job spec; server restarted");
    let t = Instant::now();
    let outcome = worker.run_once(&mut cp, &mut dp, chain).await.map_err(|e| e.to_string())?;
    println!(
        "  SECOND_ATTEMPT={outcome:?} attempt_ms={} restarts={}",
        t.elapsed().as_millis(),
        server.restarts()
    );
    let AttemptOutcome::Submitted { .. } = outcome else {
        return Err(format!("expected a submitted receipt, got {outcome:?}"));
    };
    let result_ref = dp.result_ref(task_id, executor.address()).ok_or("result not persisted")?;

    let (height, receipt) = wait("the receipt to be anchored", 120, || async {
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        find_receipt(chain, task_id, observed, latest).await.map_err(|e| e.to_string())
    })
    .await?;
    println!("Phase 5: REAL_RECEIPT_ANCHORED height={height}");
    let bound = receipt.task_id == task_id
        && receipt.input_commitment == task.input_commitment
        && receipt.executor == executor.address()
        && receipt.metadata.is_empty();
    let sig = verify_receipt_signature(&receipt);
    println!("  CLIENT_RECEIPT_BINDING={bound} CLIENT_RECEIPT_SIGNATURE={sig}");
    if !bound || !sig || receipt.output_commitment != result_ref.output_commitment {
        return Err("receipt verification failed".into());
    }
    cp.observe(chain).await.map_err(|e| e.to_string())?;
    println!(
        "  CONTROL_PLANE_STATE={:?}",
        cp.task(task_id).map(|t| t.state)
    );

    println!("Phase 6: client retrieves the private result and verifies it");
    let get = dp
        .issue_capability(
            &client,
            &CapabilityRequest {
                task_id,
                operation: Operation::GetResult,
                resource: Some(result_ref.object),
                ttl_secs: 60,
                max_uses: 1,
            },
        )
        .map_err(|e| e.to_string())?;
    let ch = dp.issue_challenge(client.address());
    let result = dp
        .get_result(&Presentation::sign(get, ch, &client))
        .map_err(|e| e.to_string())?
        .as_bytes()
        .to_vec();
    let matches = reference_output_commitment(&result) == receipt.output_commitment;
    println!(
        "  REAL_RESULT_RETRIEVED=YES REAL_OUTPUT_COMMITMENT_MATCH={matches} {}",
        structural(&result)?
    );
    if !matches {
        return Err("retrieved result does not match the anchored output_commitment".into());
    }
    let result_text = String::from_utf8(result.clone()).map_err(|_| "result not UTF-8")?;
    scan.push(result_text.clone());

    println!("Phase 7: chain privacy scan");
    let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
    let mut needles: Vec<String> =
        input.cv_text.lines().filter(|l| l.len() >= 12).map(str::to_owned).collect();
    needles.push(hex::encode(&input_bytes));
    needles.push(hex::encode(&result));
    needles.push(result_text.clone());
    let profile_value = kaayu_v2::parse_output(&result_text).map_err(|e| e.to_string())?;
    needles.push(profile_value.summary.clone());
    let mut blocks = 0;
    for h in 0..=latest {
        if let Some(block) = chain.block_by_height(h).await.map_err(|e| e.to_string())? {
            blocks += 1;
            let json = serde_json::to_string(&block).map_err(|e| e.to_string())?;
            let as_array = |s: &str| {
                let j = serde_json::to_string(&s.as_bytes()).unwrap();
                j[1..j.len() - 1].to_owned()
            };
            for n in &needles {
                if n.len() >= 12 && (json.contains(n.as_str()) || json.contains(&as_array(n))) {
                    return Err(format!("private bytes found in block {h}"));
                }
            }
        }
    }
    println!(
        "  PRIVATE_INPUT_ONCHAIN=NO PRIVATE_RESULT_ONCHAIN=NO blocks_scanned={blocks} needles={}",
        needles.len()
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map_or("corpus", String::as_str).to_owned();
    let scan_out = args
        .iter()
        .position(|a| a == "--scan-out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let mut scan = Vec::new();
    println!("=== MVP-B real-model harness ({mode}) ===");
    let result = match mode.as_str() {
        "corpus" => run_corpus(&mut scan),
        "e2e" => {
            let base = std::env::temp_dir().join("mbongo_mvp_inference_e2e");
            let _ = std::fs::remove_dir_all(&base);
            let chain = RpcChainClient::new(
                format!("http://127.0.0.1:{RPC_PORT}/rpc"),
                format!("http://127.0.0.1:{REST_PORT}"),
            );
            match spawn_producer(&base.join("producer")) {
                Err(e) => Err(e),
                Ok(mut node) => {
                    sleep(Duration::from_secs(2)).await;
                    let mut ok = false;
                    for _ in 0..50 {
                        if chain.latest_height().await.is_ok() {
                            ok = true;
                            break;
                        }
                        sleep(Duration::from_millis(200)).await;
                    }
                    let r = if ok {
                        run_e2e(&chain, &mut scan).await
                    } else {
                        Err("node RPC not reachable".into())
                    };
                    let _ = node.kill().await;
                    r
                }
            }
        }
        other => Err(format!("unknown mode {other}; use corpus or e2e")),
    };
    if let Some(path) = scan_out {
        let _ = std::fs::write(&path, serde_json::to_string(&scan).unwrap());
        println!("scan targets written ({} strings)", scan.len());
    }
    match result {
        Ok(()) => println!("\nMVP INFERENCE {}: PASS", mode.to_uppercase()),
        Err(e) => {
            println!("\nMVP INFERENCE {}: FAIL — {e}", mode.to_uppercase());
            std::process::exit(1);
        }
    }
}
