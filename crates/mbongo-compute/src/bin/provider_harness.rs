//! End-to-end proof on a real node, with every role in its own process: a
//! producer node, a provider service, a standalone `job_client`, and a runner
//! written independently of the core. Synthetic fixtures only.
//!
//! Exit code 0 on PASS. On failure, prints what the processes wrote.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use mbongo_compute::chain::{ChainClient, RpcChainClient};
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::job::{job_spec_digest, JobSpec};
use mbongo_compute::mvp::client::find_anchored_receipt;
use mbongo_core::{Address, Transaction, TransactionPayload, TransactionType};
use serde_json::{json, Value};

const RPC_PORT: u16 = 43944;
const REST_PORT: u16 = 43080;
const P2P_PORT: u16 = 53333;
/// The devnet's funded account.
const CLIENT_SEED: [u8; 32] = [0xAA; 32];
const EXECUTOR_SEED: [u8; 32] = [0xE1; 32];

/// Kills a child process when dropped.
struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn sibling(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("own path");
    let dir = exe.parent().expect("parent directory");
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn on_path(program: &str) -> Result<PathBuf, String> {
    let name = format!("{program}{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|d| d.join(&name))
        .find(|p| p.is_file())
        .ok_or_else(|| format!("{program} not found on PATH"))
}

fn spawn_logged(mut command: Command, log: &Path) -> Result<Process, String> {
    let out = std::fs::File::create(log).map_err(|e| e.to_string())?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    command
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
        .map(Process)
        .map_err(|e| format!("spawn: {e}"))
}

fn tail(log: &Path) -> String {
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(20)..].join("\n    ")
}

async fn fund(
    chain: &RpcChainClient,
    from: &LocalKey,
    to: Address,
    amount: u128,
) -> Result<(), String> {
    let nonce = chain.account_nonce(&from.address()).await.map_err(|e| e.to_string())?;
    let mut tx = Transaction {
        tx_type: TransactionType::Transfer,
        sender: from.address(),
        receiver: to,
        amount,
        nonce,
        payload: TransactionPayload::None,
        signature: [0u8; 64],
    };
    tx.signature = from.sign(&tx.signing_payload());
    chain.submit_transaction(&tx).await.map_err(|e| e.to_string())?;
    for _ in 0..200 {
        if chain.account_nonce(&from.address()).await.ok() > Some(nonce) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err("funding transfer not included".to_owned())
}

struct Stack {
    base: PathBuf,
    provider: String,
    executor: String,
}

impl Stack {
    fn client(
        &self,
        name: &str,
        spec: &[u8],
        input: &[u8],
    ) -> Result<std::process::Output, String> {
        self.client_command(name, spec, input)?
            .output()
            .map_err(|e| format!("job_client: {e}"))
    }

    fn client_command(&self, name: &str, spec: &[u8], input: &[u8]) -> Result<Command, String> {
        let spec_file = self.base.join(format!("{name}.job.json"));
        let input_file = self.base.join(format!("{name}.input"));
        std::fs::write(&spec_file, spec).map_err(|e| e.to_string())?;
        std::fs::write(&input_file, input).map_err(|e| e.to_string())?;
        let mut command = Command::new(sibling("job_client"));
        command
            .args(["--provider", &self.provider, "--executor", &self.executor])
            .arg("--job-spec")
            .arg(&spec_file)
            .arg("--input")
            .arg(&input_file)
            .arg("--key-file")
            .arg(self.base.join("client.key"))
            .args(["--rpc-url", &format!("http://127.0.0.1:{RPC_PORT}/rpc")])
            .args(["--rest-url", &format!("http://127.0.0.1:{REST_PORT}")])
            .args(["--timeout-secs", "90"]);
        Ok(command)
    }
}

/// Every vector, through every process, up to a verified anchored receipt.
async fn prove_vectors(stack: &Stack, chain: &RpcChainClient) -> Result<(), String> {
    let path = repo_root().join("test-vectors/job/job-contract-v1.json");
    let v: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let cases = v["cases"].as_array().ok_or("no cases")?;
    if cases.len() != 6 {
        return Err(format!("expected 6 vector cases, found {}", cases.len()));
    }
    for (i, case) in cases.iter().enumerate() {
        let workload = case["workload"].as_str().ok_or("workload")?;
        let spec = v["specs"]
            .as_array()
            .and_then(|s| s.iter().find(|s| s["name"] == workload))
            .and_then(|s| s["canonical"].as_str())
            .ok_or("spec")?
            .as_bytes();
        JobSpec::from_canonical_bytes(spec).map_err(|e| e.to_string())?;
        let input =
            hex::decode(case["input"].as_str().ok_or("input")?).map_err(|e| e.to_string())?;
        let run = stack.client(&format!("case{i}"), spec, &input)?;
        if !run.status.success() {
            return Err(format!(
                "case {i}: {}",
                String::from_utf8_lossy(&run.stderr).trim()
            ));
        }
        let report: Value = serde_json::from_slice(&run.stdout).map_err(|e| e.to_string())?;
        for key in ["output", "output_commitment", "input_commitment"] {
            if report[key] != case[key] {
                return Err(format!("case {i}: {key} differs from the vector"));
            }
        }
        if report["technical_level"] != "T1" || report["business_correctness"] != "NOT_ASSESSED" {
            return Err(format!("case {i}: report claims {report}"));
        }
        let task_id = report["task_id"].as_str().ok_or("task_id")?;
        let id: [u8; 32] =
            hex::decode(task_id).ok().and_then(|b| b.try_into().ok()).ok_or("task id")?;
        let anchored = find_anchored_receipt(chain, id, 1).await.map_err(|e| e.to_string())?;
        if anchored.is_none() {
            return Err(format!("case {i}: no receipt found independently"));
        }
        println!(
            "  case {i} {workload}: receipt at height {}, verified at T1",
            report["receipt_height"]
        );
    }
    Ok(())
}

/// A failing runner: a named code, three attempts, no receipt.
fn prove_failure(stack: &Stack) -> Result<(), String> {
    let path = repo_root().join("test-vectors/job/job-contract-v1.json");
    let v: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut spec = v["specs"][1]["spec"].clone();
    spec["params"] = json!({"mode": "exit-1"});
    let text = serde_json::to_string(&spec).map_err(|e| e.to_string())?;
    let canonical = mbongo_compute::inference::canonical::canonicalize(&text, 1 << 16)
        .map_err(|e| e.to_string())?;
    println!(
        "  failing job spec {}",
        hex::encode(&job_spec_digest(&canonical)[..8])
    );
    let run = stack.client("failing", &canonical, b"[1]")?;
    let err = String::from_utf8_lossy(&run.stderr);
    if run.status.code() != Some(3)
        || !err.contains("JOB_RUNTIME_FAILED")
        || !err.contains("3 attempt")
    {
        return Err(format!("failing job: {} {}", run.status, err.trim()));
    }
    println!("  failing runner: {}", err.trim());
    Ok(())
}

/// A real crash: the provider process is killed while a runner works, then
/// restarted on the same state. The client keeps waiting and gets a verified
/// result. The runner ran twice: at-least-once, with one receipt.
async fn prove_crash_recovery(
    stack: &Stack,
    provider: &mut Process,
    log: &Path,
    config_file: &Path,
    mut config: Value,
) -> Result<(), String> {
    let vectors: Value = serde_json::from_slice(
        &std::fs::read(repo_root().join("test-vectors/job/job-contract-v1.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let marker = stack.base.join("runner-starts");
    let mut spec = vectors["specs"][1]["spec"].clone();
    spec["params"] = json!({
        "mode": "slow",
        "delay_ms": 3000,
        "marker": marker.display().to_string(),
    });
    spec["limits"]["max_wall_ms"] = json!(30_000);
    let canonical = mbongo_compute::inference::canonical::canonicalize(&spec.to_string(), 1 << 16)
        .map_err(|e| e.to_string())?;
    let mut client = stack.client_command("crash", &canonical, b"[4,5]")?;
    let client = client
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let started = Instant::now();
    while !marker.exists() {
        if started.elapsed() > Duration::from_secs(60) {
            return Err("the runner never started".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    provider.0.kill().map_err(|e| e.to_string())?;
    let _ = provider.0.wait();
    println!("  provider killed while the runner was working");

    config["bind"] = json!(stack.provider.trim_start_matches("http://"));
    std::fs::write(config_file, config.to_string()).map_err(|e| e.to_string())?;
    let mut command = Command::new(sibling("provider_service"));
    command.arg("--config").arg(config_file);
    *provider = spawn_logged(command, log)?;
    wait_for_bind(log, provider).await?;
    // The orphaned runner is still alive: if it had inherited the listening
    // socket, this wait would last as long as it does.
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let wait: u64 = text
        .lines()
        .find_map(|l| l.strip_prefix("BIND_WAIT_MS="))
        .and_then(|ms| ms.parse().ok())
        .ok_or("no BIND_WAIT_MS line")?;
    if wait > 1_500 {
        return Err(format!(
            "the port stayed held for {wait} ms: a runner kept it"
        ));
    }
    println!("  provider restarted on its durable state, port free after {wait} ms");

    let run = client.wait_with_output().map_err(|e| e.to_string())?;
    if !run.status.success() {
        return Err(format!(
            "after the crash: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ));
    }
    let report: Value = serde_json::from_slice(&run.stdout).map_err(|e| e.to_string())?;
    if report["technical_level"] != "T1"
        || report["output"] != hex::encode("{\"count\":2,\"sum\":9}")
    {
        return Err(format!("after the crash: {report}"));
    }
    let starts = std::fs::read_to_string(&marker).map_err(|e| e.to_string())?.lines().count();
    if starts != 2 {
        return Err(format!("expected 2 runner starts, found {starts}"));
    }
    println!(
        "  after the crash: receipt at height {}, verified at T1; runner started {starts} times",
        report["receipt_height"]
    );
    Ok(())
}

async fn wait_for_node(chain: &RpcChainClient) -> Result<(), String> {
    for _ in 0..100 {
        if chain.latest_height().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err("node RPC did not answer".to_owned())
}

async fn wait_for_bind(log: &Path, provider: &mut Process) -> Result<String, String> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(30) {
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if let Some(line) = text.lines().find(|l| l.starts_with("PROVIDER_BIND=")) {
            return Ok(format!("http://{}", &line["PROVIDER_BIND=".len()..]));
        }
        if let Ok(Some(status)) = provider.0.try_wait() {
            return Err(format!("provider exited: {status}"));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("provider did not start".to_owned())
}

async fn run(base: &Path) -> Result<(), String> {
    std::fs::create_dir_all(base.join("artifacts")).map_err(|e| e.to_string())?;
    let node_log = base.join("node.log");
    let provider_log = base.join("provider.log");
    let mut node = Command::new(sibling("mbongo-node"));
    node.args(["--producer", "--block-time", "1"])
        .args(["--rpc-port", &RPC_PORT.to_string()])
        .args(["--rest-port", &REST_PORT.to_string()])
        .args(["--p2p-port", &P2P_PORT.to_string()])
        .arg("--data-dir")
        .arg(base.join("chain"));
    let _node = spawn_logged(node, &node_log)?;
    let chain = RpcChainClient::new(
        format!("http://127.0.0.1:{RPC_PORT}/rpc"),
        format!("http://127.0.0.1:{REST_PORT}"),
    );
    let result = async {
        wait_for_node(&chain).await?;
        let client = LocalKey::from_seed(&CLIENT_SEED);
        let executor = LocalKey::from_seed(&EXECUTOR_SEED);
        fund(&chain, &client, executor.address(), 1_000).await?;
        println!("  node up, executor funded");

        std::fs::write(base.join("client.key"), hex::encode(CLIENT_SEED))
            .map_err(|e| e.to_string())?;
        std::fs::write(base.join("executor.key"), hex::encode(EXECUTOR_SEED))
            .map_err(|e| e.to_string())?;
        std::fs::write(base.join("state.key"), hex::encode([0x57u8; 32]))
            .map_err(|e| e.to_string())?;
        let vectors: Value = serde_json::from_slice(
            &std::fs::read(repo_root().join("test-vectors/job/job-contract-v1.json"))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let table =
            hex::decode(vectors["table"].as_str().ok_or("table")?).map_err(|e| e.to_string())?;
        let sha = vectors["specs"][0]["spec"]["artifacts"][0]["sha256"].as_str().ok_or("sha256")?;
        std::fs::write(base.join("artifacts").join(sha), table).map_err(|e| e.to_string())?;

        let fixture = repo_root().join("scripts/fixtures/mbongo-runner-fixture.mjs");
        let config = json!({
            "bind": "127.0.0.1:0",
            "rpc_url": format!("http://127.0.0.1:{RPC_PORT}/rpc"),
            "rest_url": format!("http://127.0.0.1:{REST_PORT}"),
            "executor_key_file": base.join("executor.key"),
            "state_dir": base.join("state"),
            "state_key_file": base.join("state.key"),
            "artifact_dir": base.join("artifacts"),
            "work_dir": base.join("work"),
            "runtimes": [{
                "family": "test-fixture",
                "revision": "1",
                "accelerator_class": "cpu",
                "binary_sha256": null,
                "command": [on_path("node")?, fixture],
                "pass_env": ["SystemRoot"],
            }],
        });
        let config_file = base.join("provider.json");
        std::fs::write(&config_file, config.to_string()).map_err(|e| e.to_string())?;
        let mut command = Command::new(sibling("provider_service"));
        command.arg("--config").arg(&config_file);
        let mut provider = spawn_logged(command, &provider_log)?;
        let url = wait_for_bind(&provider_log, &mut provider).await?;
        println!("  provider service up at {url}");
        let stack = Stack {
            base: base.to_path_buf(),
            provider: url,
            executor: hex::encode(executor.address().0),
        };
        prove_vectors(&stack, &chain).await?;
        prove_failure(&stack)?;
        prove_crash_recovery(&stack, &mut provider, &provider_log, &config_file, config).await
    }
    .await;
    result.map_err(|e| {
        format!(
            "{e}\n  provider:\n    {}\n  node:\n    {}",
            tail(&provider_log),
            tail(&node_log)
        )
    })
}

#[tokio::main]
async fn main() {
    let base = std::env::temp_dir().join("mbongo_provider_harness");
    let _ = std::fs::remove_dir_all(&base);
    println!("=== Provider Flow Harness ===\n");
    let result = run(&base).await;
    let _ = std::fs::remove_dir_all(&base);
    match result {
        Ok(()) => println!("\nPROVIDER FLOW: PASS"),
        Err(e) => {
            eprintln!("\nPROVIDER FLOW: FAIL\n  Error: {e}");
            std::process::exit(1);
        }
    }
}
