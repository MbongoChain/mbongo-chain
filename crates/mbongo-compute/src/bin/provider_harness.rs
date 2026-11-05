//! End-to-end proof on a real node, with every role in its own process: a
//! producer node, a provider service, a standalone `job_client`, and a runner
//! written independently of the core. Synthetic fixtures only.
//!
//! With `--contained` (Linux, root) the same flow runs under the reference
//! launcher, then attacks the boundary from inside a runner. Root is the
//! harness's, to set up and to observe: the service and the runner never have it.
//!
//! Exit code 0 on PASS. On failure, prints what the processes wrote.

use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mbongo_compute::chain::{ChainClient, RpcChainClient};
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::job::{job_spec_digest, JobSpec};
use mbongo_compute::mvp::client::find_anchored_receipt;
use mbongo_compute::provider::auth::{sign_request, SignedRequest};
use mbongo_core::{Address, Transaction, TransactionPayload, TransactionType};
use serde_json::{json, Value};

// Below the Linux ephemeral range (32768-60999). A recent outbound connection
// in TIME_WAIT on a port in that range blocks a listener from binding it.
const RPC_PORT: u16 = 23944;
const REST_PORT: u16 = 23080;
const P2P_PORT: u16 = 23333;
/// The devnet's funded account.
const CLIENT_SEED: [u8; 32] = [0xAA; 32];
const EXECUTOR_SEED: [u8; 32] = [0xE1; 32];
/// The provider's ceiling on a job's memory, in the contained run.
const MEMORY_CEILING_MIB: u64 = 512;
/// Processes and threads in one job, in the contained run.
const MAX_PIDS: u64 = 256;
/// Every way a contained start must be refused.
const REFUSALS: usize = 23;
/// Two key files, two directories, the journal, the lock, two `/proc` entries.
const PROBED_PATHS: u64 = 8;
const NO_CAPABILITIES: &str = "0000000000000000";
/// The launcher's table.
const NFT_TABLE: &str = "inet mbongo_runner";
/// A second network namespace stands in for a remote client. One host: this
/// proves traffic crosses an interface, and nothing about separate hosts.
const REMOTE_NS: &str = "mbongo-remote";
const VETH: &str = "mbv0";
const VETH_PEER: &str = "mbv1";
const HOST_IP: &str = "10.203.0.1";
const REMOTE_IP: &str = "10.203.0.2";
const SERVER_NAME: &str = "provider.test";
/// Below the ephemeral range, like the other harness ports.
const TLS_PORT: u16 = 24443;
const RUNNER_LISTEN_PORT: u16 = 24555;
const CLOSED_PORT: u16 = 24556;
/// An address outside the host, tried only if the harness itself can reach it.
const OUTSIDE: &str = "1.1.1.1:443";
/// `CAP_SETGID` and `CAP_SETUID`.
const SERVICE_CAPABILITIES: &str = "00000000000000c0";

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

/// A contained deployment: the reference launcher, and the accounts it made.
struct Contained {
    launcher: PathBuf,
    service: PathBuf,
    base: PathBuf,
    /// The runner's own directory: the only place it can write here.
    scratch: PathBuf,
    cgroup: PathBuf,
    runner_uid: u32,
    runner_gid: u32,
    provider_uid: u32,
}

fn shell(script: &str) -> Result<(), String> {
    let status = Command::new("sh").args(["-c", script]).status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{script}: {status}"))
    }
}

impl Contained {
    fn prepare(&self) -> Result<String, String> {
        let out = Command::new("bash")
            .arg(&self.launcher)
            .arg("prepare")
            .arg(&self.base)
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "launcher prepare: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Accounts, layout, and copies of what the accounts must be able to run:
    /// the build directory may be out of their reach.
    fn set_up(base: &Path) -> Result<Self, String> {
        let bin = base.join("bin");
        std::fs::create_dir_all(&bin).map_err(|e| e.to_string())?;
        let service = bin.join("provider_service");
        std::fs::copy(sibling("provider_service"), &service).map_err(|e| e.to_string())?;
        std::fs::copy(
            repo_root().join("scripts/fixtures/mbongo-runner-fixture.mjs"),
            bin.join("mbongo-runner-fixture.mjs"),
        )
        .map_err(|e| e.to_string())?;
        let mut contained = Self {
            launcher: repo_root().join("scripts/provider-containment-linux.sh"),
            service,
            base: base.to_path_buf(),
            scratch: base.join("scratch"),
            cgroup: PathBuf::new(),
            runner_uid: 0,
            runner_gid: 0,
            provider_uid: 0,
        };
        let printed = contained.prepare()?;
        let value = |name: &str| {
            printed
                .lines()
                .find_map(|l| l.strip_prefix(name)?.strip_prefix('='))
                .ok_or(format!("launcher printed no {name}"))
        };
        let id = |name: &str| value(name)?.parse::<u32>().map_err(|e| e.to_string());
        contained.runner_uid = id("RUNNER_UID")?;
        contained.runner_gid = id("RUNNER_GID")?;
        contained.provider_uid = id("PROVIDER_UID")?;
        contained.cgroup = PathBuf::from(value("CGROUP_ROOT")?);
        std::fs::create_dir_all(&contained.scratch).map_err(|e| e.to_string())?;
        shell(&format!(
            "chown {}:{} {dir} && chmod 0700 {dir} && chmod 0755 {base}",
            contained.runner_uid,
            contained.runner_gid,
            dir = contained.scratch.display(),
            base = base.display(),
        ))?;
        Ok(contained)
    }

    fn oom_kills(&self) -> Result<u64, String> {
        let events = std::fs::read_to_string(self.cgroup.join("memory.events"))
            .map_err(|e| e.to_string())?;
        events
            .lines()
            .find_map(|l| l.strip_prefix("oom_kill "))
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| "no oom_kill counter".to_owned())
    }

    /// A file of the one running job's cgroup.
    fn job_file(&self, name: &str) -> Result<String, String> {
        let mut jobs: Vec<PathBuf> = std::fs::read_dir(&self.cgroup)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("job-"))
            .map(|e| e.path())
            .collect();
        let job = jobs.pop().ok_or("no running job")?;
        if !jobs.is_empty() {
            return Err("more than one running job".to_owned());
        }
        std::fs::read_to_string(job.join(name)).map_err(|e| e.to_string())
    }

    /// Every process in the job cgroups, and how many cgroups there are.
    fn job_processes(&self) -> Result<(usize, Vec<String>), String> {
        let mut groups = 0;
        let mut pids = Vec::new();
        for entry in std::fs::read_dir(&self.cgroup).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name().to_string_lossy().starts_with("job-") {
                groups += 1;
                let procs = std::fs::read_to_string(entry.path().join("cgroup.procs"))
                    .map_err(|e| e.to_string())?;
                pids.extend(procs.lines().map(str::to_owned));
            }
        }
        Ok((groups, pids))
    }
}

fn service_command(contained: Option<&Contained>, config_file: &Path) -> Command {
    if let Some(c) = contained {
        let mut command = Command::new("bash");
        command
            .arg(&c.launcher)
            .arg("run")
            .arg(&c.base)
            .arg(&c.service)
            .arg(config_file);
        return command;
    }
    let mut command = Command::new(sibling("provider_service"));
    command.arg("--config").arg(config_file);
    command
}

async fn gone(pid: &str) -> Result<(), String> {
    let path = PathBuf::from(format!("/proc/{pid}"));
    let started = Instant::now();
    while path.exists() {
        if started.elapsed() > Duration::from_secs(5) {
            return Err(format!("process {pid} survived"));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(())
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

/// How many sockets sit in TIME_WAIT now (Linux only).
fn report_time_wait() {
    let Ok(table) = std::fs::read_to_string("/proc/net/tcp") else {
        return;
    };
    let count = table.lines().filter(|l| l.split_whitespace().nth(3) == Some("06")).count();
    println!("  sockets in TIME_WAIT at start: {count}");
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
    contained: Option<Contained>,
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

/// A second process on the same state directory must refuse to start, and
/// change nothing.
fn prove_exclusive_state(stack: &Stack, config_file: &Path) -> Result<(), String> {
    let journal = stack.base.join("state").join("provider-state.journal");
    let before = std::fs::read(&journal).map_err(|e| e.to_string())?;
    let second = service_command(stack.contained.as_ref(), config_file)
        .output()
        .map_err(|e| e.to_string())?;
    let err = String::from_utf8_lossy(&second.stderr);
    if second.status.success() || !err.contains("STATE_LOCKED") {
        return Err(format!("second provider: {} {}", second.status, err.trim()));
    }
    if std::fs::read(&journal).map_err(|e| e.to_string())? != before {
        return Err("a refused provider changed the journal".to_owned());
    }
    println!("  a second provider on the same state: refused, journal unchanged");
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
    let contained = stack.contained.as_ref();
    let marker = contained.map_or(stack.base.as_path(), |c| &c.scratch).join("runner-starts");
    let mut spec = vectors["specs"][1]["spec"].clone();
    spec["params"] = json!({
        "mode": "slow",
        // Contained, long enough that only the sweep can end the orphan.
        "delay_ms": if contained.is_some() { 8000 } else { 3000 },
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
    let orphans = match contained {
        Some(c) => {
            let (groups, pids) = c.job_processes()?;
            if groups != 1 || pids.is_empty() {
                return Err(format!("expected one live job, found {groups}: {pids:?}"));
            }
            println!(
                "  the contained runner outlived the provider: {} process(es)",
                pids.len()
            );
            pids
        }
        None => Vec::new(),
    };

    config["bind"] = json!(stack.provider.trim_start_matches("http://"));
    std::fs::write(config_file, config.to_string()).map_err(|e| e.to_string())?;
    *provider = spawn_logged(service_command(contained, config_file), log)?;
    wait_for_bind(log, provider).await?;
    if contained.is_some() {
        if !std::fs::read_to_string(log).unwrap_or_default().contains("CONTAINMENT_SWEPT=1") {
            return Err("the restart did not sweep the orphaned job".to_owned());
        }
        for pid in &orphans {
            gone(pid).await?;
        }
        println!("  the restart killed the orphan before running anything");
    }
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

/// A job spec for the fixture, with `params`, up to `max_bytes` of output.
fn fixture_spec(params: Value, max_bytes: u64, limits: Value) -> Result<Vec<u8>, String> {
    let vectors: Value = serde_json::from_slice(
        &std::fs::read(repo_root().join("test-vectors/job/job-contract-v1.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut spec = vectors["specs"][1]["spec"].clone();
    spec["params"] = params;
    spec["output"]["max_bytes"] = json!(max_bytes);
    spec["limits"] = limits;
    mbongo_compute::inference::canonical::canonicalize(&spec.to_string(), 1 << 16)
        .map_err(|e| e.to_string())
}

/// Runs a job that must succeed at T1, and returns its output.
fn verified_output(stack: &Stack, name: &str, spec: &[u8]) -> Result<Vec<u8>, String> {
    let run = stack.client(name, spec, b"[1]")?;
    if !run.status.success() {
        return Err(format!(
            "{name}: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ));
    }
    let report: Value = serde_json::from_slice(&run.stdout).map_err(|e| e.to_string())?;
    if report["technical_level"] != "T1" {
        return Err(format!("{name}: {report}"));
    }
    hex::decode(report["output"].as_str().ok_or("output")?).map_err(|e| e.to_string())
}

/// Runs a job that must fail with `code`.
fn refused_job(stack: &Stack, name: &str, spec: &[u8], code: &str) -> Result<(), String> {
    let run = stack.client(name, spec, b"[1]")?;
    let err = String::from_utf8_lossy(&run.stderr);
    if run.status.code() != Some(3) || !err.contains(code) {
        return Err(format!("{name}: {} {}", run.status, err.trim()));
    }
    Ok(())
}

/// Each way a contained start can be wrong, and the refusal it must reach.
fn prove_refusals(c: &Contained, config: &Value) -> Result<(), String> {
    enum Start {
        Launcher,
        NoCapabilities,
        AsRoot,
        /// Without the launcher, so that only the service stands in the way.
        Direct,
        /// With a probe's tool replaced, in a private mount namespace, by a
        /// script reporting what a broken deployment would report.
        BrokenProbe(&'static str, &'static str),
    }
    let base = c.base.display();
    let root = c.cgroup.display();
    let (provider, runner) = (c.provider_uid, c.runner_gid);
    let runner_uid = c.runner_uid;
    let repair = format!("bash {} prepare {base} > /dev/null", c.launcher.display());
    let cases: Vec<(&str, &str, String, String, Value, Start)> = vec![
        (
            "executor key world-readable",
            "PROTECTED_PATH",
            format!("chmod 0644 {base}/keys/executor.key"),
            format!("chmod 0600 {base}/keys/executor.key"),
            json!({}),
            Start::Launcher,
        ),
        (
            "state key group-readable",
            "PROTECTED_PATH",
            format!("chmod 0640 {base}/keys/state.key"),
            format!("chmod 0600 {base}/keys/state.key"),
            json!({}),
            Start::Launcher,
        ),
        (
            "key under a directory others can write",
            "PROTECTED_PATH",
            format!("chmod 0777 {base}/keys"),
            format!("chmod 0700 {base}/keys"),
            json!({}),
            Start::Launcher,
        ),
        (
            "state directory open",
            "PROTECTED_PATH",
            format!("chmod 0755 {base}/state"),
            format!("chmod 0700 {base}/state"),
            json!({}),
            Start::Launcher,
        ),
        (
            "artifact directory open",
            "PROTECTED_PATH",
            format!("chmod 0755 {base}/artifacts"),
            format!("chmod 0700 {base}/artifacts"),
            json!({}),
            Start::Launcher,
        ),
        (
            "work directory of the wrong group",
            "WORK_DIR",
            format!("chgrp {provider} {base}/work"),
            format!("chgrp {runner} {base}/work && chmod 2750 {base}/work"),
            json!({}),
            Start::Launcher,
        ),
        (
            "runner is root",
            "RUNNER_IDENTITY",
            "true".to_owned(),
            "true".to_owned(),
            json!({"runner_uid": 0}),
            Start::Launcher,
        ),
        (
            "runner is the service's account",
            "RUNNER_IDENTITY",
            "true".to_owned(),
            "true".to_owned(),
            json!({"runner_uid": provider}),
            Start::Launcher,
        ),
        (
            "cgroup missing",
            "CGROUP_UNAVAILABLE",
            "true".to_owned(),
            "true".to_owned(),
            json!({"cgroup_root": "/sys/fs/cgroup/mbongo-absent"}),
            Start::Launcher,
        ),
        (
            "memory controller not delegated",
            "CGROUP_UNAVAILABLE",
            format!("echo -memory > {root}/cgroup.subtree_control"),
            format!("echo +memory > {root}/cgroup.subtree_control"),
            json!({}),
            Start::Launcher,
        ),
        (
            "no capability to change identity",
            "SPAWN_DENIED",
            "true".to_owned(),
            "true".to_owned(),
            json!({}),
            Start::NoCapabilities,
        ),
        (
            "service running as root",
            "SERVICE_IDENTITY",
            "true".to_owned(),
            "true".to_owned(),
            json!({}),
            Start::AsRoot,
        ),
        (
            "runner account in use by another process",
            "RUNNER_UID_SHARED",
            format!(
                "setsid setpriv --reuid={runner_uid} --regid={runner} --clear-groups \
                 /bin/sleep 60 > /dev/null 2>&1 & sleep 0.3"
            ),
            format!("pkill -KILL -u {runner_uid}; sleep 0.3"),
            json!({}),
            Start::Launcher,
        ),
        (
            "network policy absent",
            "NETWORK_POLICY_INEFFECTIVE",
            format!("nft delete table {NFT_TABLE}"),
            repair.clone(),
            json!({}),
            Start::Direct,
        ),
        (
            "network policy naming another account",
            "NETWORK_POLICY_INEFFECTIVE",
            format!(
                "nft flush chain {NFT_TABLE} output && \
                 nft add rule {NFT_TABLE} output meta skuid 54321 counter reject"
            ),
            repair.clone(),
            json!({}),
            Start::Direct,
        ),
        (
            "network policy letting loopback through",
            "NETWORK_POLICY_INEFFECTIVE",
            format!("nft insert rule {NFT_TABLE} output oif lo accept"),
            repair.clone(),
            json!({}),
            Start::Launcher,
        ),
        (
            "network policy letting IPv6 through",
            "NETWORK_POLICY_INEFFECTIVE",
            format!("nft insert rule {NFT_TABLE} output meta nfproto ipv6 accept"),
            repair.clone(),
            json!({}),
            Start::Launcher,
        ),
        (
            "network probe reports success without connecting",
            "NETWORK_PROBE_INCONCLUSIVE",
            "true".to_owned(),
            "true".to_owned(),
            json!({}),
            Start::BrokenProbe("bash", "exit 0"),
        ),
        (
            "identity probe reports the wrong account",
            "PROBE_IDENTITY",
            "true".to_owned(),
            "true".to_owned(),
            json!({}),
            Start::BrokenProbe("cat", "echo 'Uid: 0 0 0 0'"),
        ),
        (
            "descriptor probe finds an inherited descriptor",
            "PROBE_DESCRIPTORS",
            "true".to_owned(),
            "true".to_owned(),
            json!({}),
            Start::BrokenProbe("ls", "echo 0 1 2 3 9"),
        ),
        (
            "kill probe sees a process end by itself",
            "PROBE_KILL",
            "true".to_owned(),
            "true".to_owned(),
            json!({}),
            Start::BrokenProbe("sleep", "exit 0"),
        ),
        (
            "a protected path the runner can read",
            "PROBE_READABLE",
            "true".to_owned(),
            "true".to_owned(),
            json!({"denied_paths": ["/etc/passwd"]}),
            Start::Launcher,
        ),
        (
            "a protected path that does not exist",
            "PROBE_INCONCLUSIVE",
            "true".to_owned(),
            "true".to_owned(),
            json!({"denied_paths": ["/nonexistent/mbongo"]}),
            Start::Launcher,
        ),
    ];
    if cases.len() != REFUSALS {
        return Err(format!(
            "expected {REFUSALS} refusal cases, found {}",
            cases.len()
        ));
    }
    let file = c.base.join("refused.json");
    for (name, code, breaks, restores, patch, start) in &cases {
        let mut broken = config.clone();
        for (key, value) in patch.as_object().ok_or("patch")? {
            broken["isolation"][key] = value.clone();
        }
        std::fs::write(&file, broken.to_string()).map_err(|e| e.to_string())?;
        shell(breaks)?;
        let enter = format!("echo $$ > {root}/service/cgroup.procs; umask 027");
        let (service, shown) = (c.service.display(), file.display());
        let direct = format!(
            "{enter}; exec setpriv --reuid={provider} --regid={provider} --clear-groups \
             --bounding-set=-all,+setuid,+setgid --inh-caps=-all,+setuid,+setgid \
             --ambient-caps=-all,+setuid,+setgid --no-new-privs {service} --config {shown}"
        );
        let mut command = match start {
            Start::Launcher => service_command(Some(c), &file),
            Start::NoCapabilities => {
                let mut command = Command::new("sh");
                command.arg("-c").arg(format!(
                    "{enter}; exec setpriv --reuid={provider} --regid={provider} --clear-groups \
                     {service} --config {shown}"
                ));
                command
            }
            Start::AsRoot => {
                let mut command = Command::new("sh");
                command.arg("-c").arg(format!("{enter}; exec {service} --config {shown}"));
                command
            }
            Start::Direct => {
                let mut command = Command::new("sh");
                command.arg("-c").arg(&direct);
                command
            }
            Start::BrokenProbe(tool, body) => {
                let fake = c.base.join("bin").join(format!("broken-{tool}"));
                std::fs::write(&fake, format!("#!/bin/sh\n{body}\n")).map_err(|e| e.to_string())?;
                shell(&format!("chmod 0755 {}", fake.display()))?;
                let mut command = Command::new("unshare");
                command.args(["-m", "sh", "-c"]).arg(format!(
                    "mount --bind {} /bin/{tool} && {direct}",
                    fake.display(),
                ));
                command
            }
        };
        let run = command.output().map_err(|e| e.to_string());
        shell(restores)?;
        let run = run?;
        let err = String::from_utf8_lossy(&run.stderr);
        if run.status.success() || !err.contains(&format!("ISOLATION_REFUSED={code}")) {
            return Err(format!("{name}: {} {}", run.status, err.trim()));
        }
        println!("  refused, {name}: {code}");
    }
    let (groups, pids) = c.job_processes()?;
    if groups != 0 || !pids.is_empty() {
        return Err(format!(
            "refused starts left {groups} job cgroups: {pids:?}"
        ));
    }
    // The launcher checks too, before it starts anything.
    shell(&format!("nft delete table {NFT_TABLE}"))?;
    let run = service_command(Some(c), &file).output().map_err(|e| e.to_string());
    c.prepare()?;
    let run = run?;
    let err = String::from_utf8_lossy(&run.stderr);
    if run.status.success() || !err.contains("network policy is not installed") {
        return Err(format!(
            "launcher without a policy: {} {}",
            run.status,
            err.trim()
        ));
    }
    println!("  refused, by the launcher, no network policy installed");
    prove_inbound_refusals(c, &file)
}

/// The service is what the launcher promised, and says so.
async fn prove_service_identity(
    c: &Contained,
    stack: &Stack,
    service_pid: u32,
    log: &Path,
) -> Result<(), String> {
    let status = std::fs::read_to_string(format!("/proc/{service_pid}/status"))
        .map_err(|e| e.to_string())?;
    let field = |name: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name)?.strip_prefix(':'))
            .map_or("", str::trim)
    };
    let uids: Vec<&str> = field("Uid").split_whitespace().collect();
    let expected = c.provider_uid.to_string();
    if uids.len() != 4 || uids.iter().any(|u| *u != expected) {
        return Err(format!("service uid: {uids:?}"));
    }
    for name in ["CapEff", "CapPrm", "CapAmb", "CapBnd"] {
        if field(name) != SERVICE_CAPABILITIES {
            return Err(format!("service {name}: {}", field(name)));
        }
    }
    if field("NoNewPrivs") != "1" {
        return Err("the service can gain privileges".to_owned());
    }
    let text = std::fs::read_to_string(log).unwrap_or_default();
    for line in [
        "RUNNER_NETWORK=DENIED".to_owned(),
        format!("ISOLATION_PROBE_PATHS={PROBED_PATHS}"),
        "KEY_ISOLATION_ENFORCED=YES".to_owned(),
        "MEMORY_LIMIT_ENFORCED=YES".to_owned(),
    ] {
        if !text.lines().any(|l| l == line) {
            return Err(format!("the provider did not print {line}"));
        }
    }
    let capabilities: Value = reqwest::get(format!("{}/v1/capabilities", stack.provider))
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if capabilities["key_isolation_enforced"] != true
        || capabilities["memory_limit_enforced"] != true
        || capabilities["runner_network_denied"] != true
    {
        return Err(format!("capabilities: {capabilities}"));
    }
    println!("  service: uid {expected}, setuid and setgid only, probed {PROBED_PATHS} paths");
    Ok(())
}

/// A compromised runner, through the real job path, tries what it must not.
fn prove_attack(c: &Contained, stack: &Stack, service_pid: u32) -> Result<(), String> {
    let base = &c.base;
    let targets = [
        base.join("keys/executor.key"),
        base.join("keys/state.key"),
        base.join("state/provider-state.journal"),
        base.join("state/provider-state.lock"),
    ];
    let spec = fixture_spec(
        json!({
            "mode": "attack",
            "targets": targets,
            "service_pid": service_pid,
            "cgroup_root": c.cgroup,
        }),
        65_536,
        json!({"max_wall_ms": 10_000, "max_memory_mib": 256}),
    )?;
    let output = verified_output(stack, "attack", &spec)?;
    let r: Value = serde_json::from_slice(&output).map_err(|e| e.to_string())?;
    for action in ["read", "write", "unlink"] {
        let found = r[action].as_array().ok_or(format!("no {action} results"))?;
        if found.len() != targets.len() || found.iter().any(|v| v != "EACCES") {
            return Err(format!("{action}: {found:?}"));
        }
    }
    let expected = [
        ("/service/environ", "EACCES"),
        ("/service/mem", "EACCES"),
        ("/service/signal", "EPERM"),
        ("/cgroup/into_parent", "EACCES"),
        ("/cgroup/into_service", "EACCES"),
        ("/cgroup/raise_memory", "EACCES"),
        ("/cgroup/new_child", "EACCES"),
        ("/setuid_root", "EPERM"),
        ("/job_dir_write", "EACCES"),
        // The control: the same process, the same call, on what it may read.
        ("/job_spec", "OK"),
    ];
    for (pointer, outcome) in expected {
        if r.pointer(pointer).and_then(Value::as_str) != Some(outcome) {
            return Err(format!("{pointer}: {:?}", r.pointer(pointer)));
        }
    }
    let ids = |id: u32| vec![id.to_string(); 4].join(" ");
    let identity = &r["identity"];
    if identity["uid"] != ids(c.runner_uid).as_str()
        || identity["gid"] != ids(c.runner_gid).as_str()
        || identity["groups"] != ""
        || identity["no_new_privs"] != "1"
        || identity["session_leader"] != "YES"
    {
        return Err(format!("runner identity: {identity}"));
    }
    for name in ["cap_eff", "cap_prm", "cap_inh", "cap_amb"] {
        if identity[name] != NO_CAPABILITIES {
            return Err(format!("runner {name}: {identity}"));
        }
    }
    if !identity["cgroup"].as_str().unwrap_or_default().contains("/job-") {
        return Err(format!("runner cgroup: {identity}"));
    }

    // What the service holds open, read from the outside, while it holds it.
    let held: Vec<String> = std::fs::read_dir(format!("/proc/{service_pid}/fd"))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|e| std::fs::read_link(e.path()).ok())
        .map(|p| p.display().to_string())
        .filter(|p| p.starts_with("socket:") || p.starts_with(&base.display().to_string()))
        .collect();
    let sockets = held.iter().filter(|p| p.starts_with("socket:")).count();
    if sockets == 0 || !held.iter().any(|p| p.ends_with("provider-state.lock")) {
        return Err(format!("the service holds no listener or lock: {held:?}"));
    }
    let inherited = r["descriptors"].as_array().ok_or("no descriptors")?;
    if inherited.is_empty() || inherited.iter().any(|d| held.iter().any(|h| d == h.as_str())) {
        return Err(format!("the runner inherited: {inherited:?} of {held:?}"));
    }
    println!(
        "  attack: {} paths x read, write, unlink denied; {} further attempts as expected",
        targets.len(),
        expected.len(),
    );
    println!(
        "  descriptors: none of the service's {} sockets and files reached the runner",
        held.len()
    );
    Ok(())
}

/// A descendant in its own session dies with its job, whether the parent
/// exits by itself or is killed at the deadline.
async fn prove_descendants(c: &Contained, stack: &Stack) -> Result<(), String> {
    for hang in [false, true] {
        let name = if hang {
            "killed-parent"
        } else {
            "exited-parent"
        };
        let heartbeat = c.scratch.join(format!("{name}.heartbeat"));
        let spec = fixture_spec(
            json!({"mode": "daemon", "heartbeat": heartbeat, "hang": u8::from(hang)}),
            1_024,
            json!({"max_wall_ms": if hang { 4_000 } else { 20_000 }, "max_memory_mib": 256}),
        )?;
        if hang {
            refused_job(stack, name, &spec, "JOB_LIMIT_WALL_TIME")?;
        } else {
            let output = verified_output(stack, name, &spec)?;
            let r: Value = serde_json::from_slice(&output).map_err(|e| e.to_string())?;
            if r["descendant_session"] == r["own_session"] {
                return Err(format!("the descendant did not detach: {r}"));
            }
        }
        let beats = std::fs::read_to_string(&heartbeat).map_err(|e| e.to_string())?;
        let count = beats.lines().count();
        let pid = beats.lines().next().ok_or("no heartbeat")?.to_owned();
        if count < 3 {
            return Err(format!("{name}: {count} heartbeats"));
        }
        // At the deadline the failure is reported up to `KILL_GRACE` before the
        // kill, so the count only settles once the process is gone.
        gone(&pid).await?;
        let settled = std::fs::read_to_string(&heartbeat).map_err(|e| e.to_string())?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        let after = std::fs::read_to_string(&heartbeat).map_err(|e| e.to_string())?;
        if after != settled {
            return Err(format!("{name}: the heartbeat continued"));
        }
        println!("  {name}: descendant alive for {count} heartbeats, then dead");
    }
    Ok(())
}

/// A real endpoint, counting what reaches it, so that a denial is judged at the
/// receiving end and not by what the runner reports.
struct Sink {
    name: &'static str,
    proto: &'static str,
    host: String,
    port: u16,
    hits: Arc<AtomicUsize>,
}

impl Sink {
    fn tcp(name: &'static str, bind: &str, host: &str) -> Result<Self, String> {
        let listener = TcpListener::bind(bind).map_err(|e| format!("{name}: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        std::thread::spawn(move || {
            for _ in listener.incoming() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        Ok(Self {
            name,
            proto: "tcp",
            host: host.to_owned(),
            port,
            hits,
        })
    }

    fn udp(name: &'static str, bind: &str, host: &str) -> Result<Self, String> {
        let socket = UdpSocket::bind(bind).map_err(|e| format!("{name}: {e}"))?;
        let port = socket.local_addr().map_err(|e| e.to_string())?.port();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        std::thread::spawn(move || {
            let mut buffer = [0; 16];
            while socket.recv_from(&mut buffer).is_ok() {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        Ok(Self {
            name,
            proto: "udp",
            host: host.to_owned(),
            port,
            hits,
        })
    }

    fn target(&self) -> Value {
        json!({"name": self.name, "proto": self.proto, "host": self.host, "port": self.port})
    }

    fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }
}

/// This host's address on its way out, if it has one. Sends nothing.
fn host_address() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(OUTSIDE).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback()).then(|| ip.to_string())
}

async fn stop_service(provider: &mut Process) -> Result<(), String> {
    shell(&format!("kill -TERM {}", provider.0.id()))?;
    let started = Instant::now();
    loop {
        if let Some(status) = provider.0.try_wait().map_err(|e| e.to_string())? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("the service stopped with {status}"))
            };
        }
        if started.elapsed() > Duration::from_secs(30) {
            return Err("the service did not stop".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn start_service(
    stack: &Stack,
    provider: &mut Process,
    log: &Path,
    config_file: &Path,
    config: &Value,
    network: &str,
) -> Result<(), String> {
    let mut config = config.clone();
    config["bind"] = json!(stack.provider.trim_start_matches("http://"));
    config["isolation"]["network"] = json!(network);
    std::fs::write(config_file, config.to_string()).map_err(|e| e.to_string())?;
    let mut command = service_command(stack.contained.as_ref(), config_file);
    command.env("MBONGO_RUNNER_NETWORK", network);
    *provider = spawn_logged(command, log)?;
    wait_for_bind(log, provider).await.map(|_| ())
}

/// From inside a job, and from a detached descendant: every address family,
/// the node, the service, and the IPv6 forms of an IPv4 address. Nothing may
/// arrive. Then the same job without the rule, where all of it must arrive.
async fn prove_network(
    stack: &Stack,
    provider: &mut Process,
    log: &Path,
    config_file: &Path,
    config: &Value,
) -> Result<(), String> {
    let c = stack.contained.as_ref().ok_or("not contained")?;
    let mut sinks = vec![
        Sink::tcp("tcp-loopback-v4", "127.0.0.1:0", "127.0.0.1")?,
        Sink::tcp("tcp-loopback-v6", "[::1]:0", "::1")?,
        Sink::tcp("tcp-v4-mapped-in-v6", "127.0.0.1:0", "::ffff:127.0.0.1")?,
        Sink::udp("udp-loopback-v4", "127.0.0.1:0", "127.0.0.1")?,
        Sink::udp("udp-loopback-v6", "[::1]:0", "::1")?,
    ];
    if let Some(host) = host_address() {
        sinks.push(Sink::tcp("tcp-host-address", &format!("{host}:0"), &host)?);
    } else {
        println!("  this host has no outward address: not tested");
    }
    let provider_port: u16 = stack
        .provider
        .rsplit(':')
        .next()
        .and_then(|p| p.parse().ok())
        .ok_or("provider port")?;
    let mut targets: Vec<Value> = sinks.iter().map(Sink::target).collect();
    targets
        .push(json!({"name": "node-rpc", "proto": "tcp", "host": "127.0.0.1", "port": RPC_PORT}));
    targets
        .push(json!({"name": "node-rest", "proto": "tcp", "host": "127.0.0.1", "port": REST_PORT}));
    targets.push(
        json!({"name": "provider", "proto": "tcp", "host": "127.0.0.1", "port": provider_port}),
    );
    let outside: SocketAddr = OUTSIDE.parse().map_err(|_| "outside address")?;
    let reachable = TcpStream::connect_timeout(&outside, Duration::from_secs(2)).is_ok();
    if reachable {
        let (host, port) = (outside.ip().to_string(), outside.port());
        targets.push(json!({"name": "outside", "proto": "tcp", "host": host, "port": port}));
    } else {
        println!("  no address outside the host is reachable from here: not tested");
    }
    let spec = fixture_spec(
        json!({"mode": "net", "targets": targets}),
        65_536,
        json!({"max_wall_ms": 60_000, "max_memory_mib": 256}),
    )?;
    let run = |name: &str| -> Result<Value, String> {
        let output = verified_output(stack, name, &spec)?;
        serde_json::from_slice(&output).map_err(|e| e.to_string())
    };

    let denied = run("network-denied")?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let attempts = denied.as_object().ok_or("results")?;
    if attempts.len() != targets.len() {
        return Err(format!(
            "{} results for {} targets",
            attempts.len(),
            targets.len()
        ));
    }
    for (name, outcome) in attempts {
        if outcome["main"] == "OK" || outcome["descendant"] != "DENIED" {
            return Err(format!("denied: {name} {outcome}"));
        }
    }
    for sink in &sinks {
        if sink.hits() != 0 {
            return Err(format!(
                "denied: {} reached {} times",
                sink.name,
                sink.hits()
            ));
        }
    }
    println!(
        "  {} targets, from the runner and a detached descendant: all refused, nothing arrived",
        targets.len()
    );

    // The control: same job, same endpoints, no rule.
    stop_service(provider).await?;
    shell(&format!("nft delete table {NFT_TABLE}"))?;
    start_service(stack, provider, log, config_file, config, "open").await?;
    let open = run("network-open")?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    for (name, outcome) in open.as_object().ok_or("results")? {
        if name != "outside" && (outcome["main"] != "OK" || outcome["descendant"] != "OK") {
            return Err(format!("control: {name} {outcome}"));
        }
    }
    for sink in &sinks {
        if sink.hits() == 0 {
            return Err(format!("control: nothing reached {}", sink.name));
        }
    }
    println!("  the same job without the rule: every endpoint reached");
    stop_service(provider).await?;
    c.prepare()?;
    start_service(stack, provider, log, config_file, config, "denied").await
}

/// A policy removed or changed while the service runs: the next job does not
/// start, nor does any after it, until a restart proves the policy again.
async fn prove_policy_loss(
    stack: &Stack,
    provider: &mut Process,
    log: &Path,
    config_file: &Path,
    config: &Value,
) -> Result<(), String> {
    let c = stack.contained.as_ref().ok_or("not contained")?;
    let changes = [
        ("removed", format!("nft delete table {NFT_TABLE}")),
        (
            "changed",
            format!("nft insert rule {NFT_TABLE} output oif lo accept"),
        ),
    ];
    for (name, change) in &changes {
        shell(change)?;
        let marker = c.scratch.join(format!("policy-{name}"));
        let spec = fixture_spec(
            json!({"mode": "slow", "delay_ms": 0, "marker": marker}),
            64,
            json!({"max_wall_ms": 5_000, "max_memory_mib": 256}),
        )?;
        refused_job(
            stack,
            &format!("policy-{name}"),
            &spec,
            "JOB_RUNTIME_FAILED",
        )?;
        let text = std::fs::read_to_string(log).unwrap_or_default();
        if !text.contains("NETWORK_POLICY_LOST") {
            return Err(format!("policy {name}: not recorded"));
        }
        c.prepare()?;
        refused_job(
            stack,
            &format!("policy-{name}-repaired"),
            &spec,
            "JOB_RUNTIME_FAILED",
        )?;
        if marker.exists() {
            return Err(format!("policy {name}: a runner started"));
        }
        println!("  policy {name} after startup: no runner started, even once repaired");
        stop_service(provider).await?;
        start_service(stack, provider, log, config_file, config, "denied").await?;
    }
    Ok(())
}

/// The launcher refuses a missing or malformed inbound list, and any installed
/// policy that is not the one asked for.
fn prove_inbound_refusals(c: &Contained, file: &Path) -> Result<(), String> {
    let prepare = |allow: Option<&str>| -> Result<(bool, String), String> {
        let mut command = Command::new("bash");
        command
            .arg(&c.launcher)
            .arg("prepare")
            .arg(&c.base)
            .env_remove("MBONGO_INBOUND_ALLOW");
        if let Some(allow) = allow {
            command.env("MBONGO_INBOUND_ALLOW", allow);
        }
        let run = command.output().map_err(|e| e.to_string())?;
        Ok((
            run.status.success(),
            String::from_utf8_lossy(&run.stderr).into_owned(),
        ))
    };
    let lists = [
        (None, "is required"),
        (Some(""), "is required"),
        (Some("22;reboot"), "malformed"),
        (Some("abc"), "malformed"),
        (Some("22,,80"), "malformed"),
        (Some(",22"), "malformed"),
        (Some("22,"), "malformed"),
        (Some("0"), "out of range"),
        (Some("70000"), "out of range"),
        (Some("000000080"), "out of range"),
    ];
    for (allow, reason) in lists {
        let (ok, err) = prepare(allow)?;
        if ok || !err.contains(reason) {
            return Err(format!("inbound list {allow:?}: {ok} {err}"));
        }
    }
    println!(
        "  refused, by the launcher: {} missing or malformed inbound lists",
        lists.len()
    );
    let changes = [
        (
            "no inbound chain",
            format!("nft delete chain {NFT_TABLE} input"),
            "inbound policy is not installed",
        ),
        (
            "inbound policy set to accept",
            format!("nft 'chain {NFT_TABLE} input {{ policy accept; }}'"),
            "inbound policy is not installed",
        ),
        (
            "a port nobody listed",
            format!("nft add rule {NFT_TABLE} input tcp dport 9999 ct state new accept"),
            "does not match",
        ),
    ];
    for (name, change, reason) in &changes {
        shell(change)?;
        let run = service_command(Some(c), file).output().map_err(|e| e.to_string());
        c.prepare()?;
        let run = run?;
        let err = String::from_utf8_lossy(&run.stderr);
        if run.status.success() || !err.contains(reason) {
            return Err(format!("{name}: {} {}", run.status, err.trim()));
        }
        println!("  refused, by the launcher: {name}");
    }
    Ok(())
}

fn remote(script: &str) -> Command {
    let mut command = Command::new("ip");
    command.args(["netns", "exec", REMOTE_NS, "sh", "-c", script]);
    command
}

/// Whether the remote namespace can open a connection to this host.
fn reachable(port: u16) -> Result<bool, String> {
    let script = format!("timeout 3 bash -c 'exec 3<>/dev/tcp/{HOST_IP}/{port}'");
    let status = remote(&script).status().map_err(|e| e.to_string())?;
    Ok(status.success())
}

/// One request through nginx from the remote namespace. Returns the status and
/// the body.
fn curl(
    tls: &Path,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: &Path,
) -> Result<(u16, String), String> {
    let mut script = format!(
        "curl -s -m 10 -o - -w '\\n%{{http_code}}' --cacert {}/ca.pem -X {method} --data-binary @{}",
        tls.display(),
        body.display()
    );
    for (name, value) in headers {
        script.push_str(&format!(" -H '{name}: {value}'"));
    }
    script.push_str(&format!(" 'https://{SERVER_NAME}:{TLS_PORT}{path}'"));
    let out = remote(&script).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').ok_or(format!("curl: {text}"))?;
    let code = code.trim().parse().map_err(|_| format!("curl: {text}"))?;
    Ok((code, body.to_owned()))
}

fn signed(method: &str, path: &str, body: &[u8]) -> Result<Vec<(&'static str, String)>, String> {
    let executor = LocalKey::from_seed(&EXECUTOR_SEED).address();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let headers = sign_request(
        &LocalKey::from_seed(&CLIENT_SEED),
        &SignedRequest {
            recipient: executor,
            method,
            path,
            body,
            issued_at: now,
            expires_at: now + 60,
            request_id: mbongo_compute::mvp::client::random_bytes::<16>(),
        },
    );
    Ok(headers.to_vec())
}

/// Removes the namespace and the interface when dropped.
struct RemoteNamespace;

impl Drop for RemoteNamespace {
    fn drop(&mut self) {
        let _ = shell(&format!(
            "ip netns del {REMOTE_NS} 2>/dev/null; ip link del {VETH} 2>/dev/null; \
             rm -rf /etc/netns/{REMOTE_NS}"
        ));
    }
}

fn set_up_remote() -> Result<RemoteNamespace, String> {
    drop(RemoteNamespace);
    shell(&format!(
        "ip netns add {REMOTE_NS} && \
         ip link add {VETH} type veth peer name {VETH_PEER} && \
         ip link set {VETH_PEER} netns {REMOTE_NS} && \
         ip addr add {HOST_IP}/30 dev {VETH} && ip link set {VETH} up && \
         ip netns exec {REMOTE_NS} ip addr add {REMOTE_IP}/30 dev {VETH_PEER} && \
         ip netns exec {REMOTE_NS} ip link set {VETH_PEER} up && \
         ip netns exec {REMOTE_NS} ip link set lo up && \
         mkdir -p /etc/netns/{REMOTE_NS} && \
         echo '{HOST_IP} {SERVER_NAME}' > /etc/netns/{REMOTE_NS}/hosts"
    ))?;
    Ok(RemoteNamespace)
}

/// A private authority and a certificate for the server name, made here.
fn make_certificates(tls: &Path) -> Result<(), String> {
    std::fs::create_dir_all(tls).map_err(|e| e.to_string())?;
    let dir = tls.display();
    shell(&format!(
        "cd {dir} && \
         openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 \
           -keyout ca.key -out ca.pem -subj /CN=mbongo-test-authority \
           -addext basicConstraints=critical,CA:TRUE 2> /dev/null && \
         openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes \
           -keyout leaf.key -out leaf.csr -subj /CN={SERVER_NAME} 2> /dev/null && \
         echo subjectAltName=DNS:{SERVER_NAME} > leaf.ext && \
         openssl x509 -req -in leaf.csr -CA ca.pem -CAkey ca.key -CAcreateserial -days 2 \
           -extfile leaf.ext -out leaf.pem 2> /dev/null"
    ))
}

/// The reference configuration, with its values filled in. Nothing else
/// changes: this is what is tested.
fn start_nginx(tls: &Path, backend: &str) -> Result<Process, String> {
    let template =
        std::fs::read_to_string(repo_root().join("scripts/provider-nginx.conf.template"))
            .map_err(|e| e.to_string())?;
    let dir = tls.display().to_string();
    let conf = template
        .replace("@PREFIX@", &dir)
        .replace("@LISTEN@", &format!("{HOST_IP}:{TLS_PORT}"))
        .replace("@SERVER_NAME@", SERVER_NAME)
        .replace("@CERTIFICATE@", &format!("{dir}/leaf.pem"))
        .replace("@CERTIFICATE_KEY@", &format!("{dir}/leaf.key"))
        .replace("@BACKEND@", backend);
    if conf.contains('@') {
        return Err("a value of the nginx template was left unfilled".to_owned());
    }
    let file = tls.join("nginx.conf");
    std::fs::write(&file, conf).map_err(|e| e.to_string())?;
    let mut command = Command::new("nginx");
    command
        .args(["-p", &dir, "-e", "stderr", "-c"])
        .arg(&file)
        .args(["-g", "daemon off;"]);
    let process = spawn_logged(command, &tls.join("nginx.log"))?;
    let started = Instant::now();
    while !reachable(TLS_PORT)? {
        if started.elapsed() > Duration::from_secs(10) {
            return Err(format!(
                "nginx did not start: {}",
                tail(&tls.join("nginx.log"))
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(process)
}

fn remote_job(
    stack: &Stack,
    tls: &Path,
    name: &str,
    spec: &[u8],
    input: &[u8],
) -> Result<Command, String> {
    let spec_file = stack.base.join(format!("{name}.job.json"));
    let input_file = stack.base.join(format!("{name}.input"));
    std::fs::write(&spec_file, spec).map_err(|e| e.to_string())?;
    std::fs::write(&input_file, input).map_err(|e| e.to_string())?;
    let mut command = Command::new("ip");
    command
        .args(["netns", "exec", REMOTE_NS])
        .arg(sibling("job_client"))
        .arg("--provider-endpoint")
        .arg(tls.join("endpoint.json"))
        .arg("--job-spec")
        .arg(&spec_file)
        .arg("--input")
        .arg(&input_file)
        .arg("--key-file")
        .arg(stack.base.join("client.key"))
        .args(["--rpc-url", &format!("http://{HOST_IP}:{RPC_PORT}/rpc")])
        .args(["--rest-url", &format!("http://{HOST_IP}:{REST_PORT}")])
        .args(["--timeout-secs", "90"]);
    Ok(command)
}

fn t1_report(run: &std::process::Output, name: &str) -> Result<Value, String> {
    if !run.status.success() {
        return Err(format!(
            "{name}: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        ));
    }
    let report: Value = serde_json::from_slice(&run.stdout).map_err(|e| e.to_string())?;
    if report["technical_level"] != "T1" || report["receipt_height"].as_u64().unwrap_or(0) == 0 {
        return Err(format!("{name}: {report}"));
    }
    Ok(report)
}

/// From the remote namespace: what reaches this host, a job through nginx, and
/// what nginx does to signed requests.
async fn prove_transport(
    stack: &Stack,
    provider: &mut Process,
    log: &Path,
    config_file: &Path,
    config: &Value,
) -> Result<(), String> {
    let c = stack.contained.as_ref().ok_or("not contained")?;
    let tls = stack.base.join("tls");
    make_certificates(&tls)?;
    let _namespace = set_up_remote()?;
    std::env::set_var(
        "MBONGO_INBOUND_ALLOW",
        format!("{TLS_PORT},{RPC_PORT},{REST_PORT}"),
    );
    c.prepare()?;
    let backend = stack.provider.trim_start_matches("http://").to_owned();
    let backend_port: u16 =
        backend.rsplit(':').next().and_then(|p| p.parse().ok()).ok_or("backend")?;
    let _nginx = start_nginx(&tls, &backend)?;
    let executor = hex::encode(LocalKey::from_seed(&EXECUTOR_SEED).address().0);
    std::fs::write(
        tls.join("endpoint.json"),
        json!({
            "schema": "provider-endpoint-v1",
            "executor": executor,
            "url": format!("https://{SERVER_NAME}:{TLS_PORT}"),
            "trust_anchors": "ca.pem",
        })
        .to_string(),
    )
    .map_err(|e| e.to_string())?;

    // What the firewall lets through, and what it does not.
    let closed = TcpListener::bind((HOST_IP, CLOSED_PORT)).map_err(|e| e.to_string())?;
    if !reachable(TLS_PORT)? {
        return Err("the remote namespace cannot reach nginx".to_owned());
    }
    if reachable(backend_port)? {
        return Err("the remote namespace reached the backend".to_owned());
    }
    if reachable(CLOSED_PORT)? {
        return Err("the remote namespace reached a port nobody listed".to_owned());
    }
    drop(closed);
    println!("  from the remote namespace: nginx reachable; backend and an unlisted port not");

    // A complete job.
    let spec = fixture_spec(
        json!({}),
        64,
        json!({"max_wall_ms": 5_000, "max_memory_mib": 256}),
    )?;
    let run = remote_job(stack, &tls, "remote", &spec, b"[2,3]")?
        .output()
        .map_err(|e| e.to_string())?;
    let report = t1_report(&run, "remote job")?;
    println!(
        "  a job from the remote namespace, through nginx: receipt at height {}, verified at T1",
        report["receipt_height"]
    );

    // TLS version, seen from a client that would accept less.
    let old = remote(&format!(
        "curl -s -m 5 --tlsv1.2 --tls-max 1.2 --cacert {}/ca.pem \
         https://{SERVER_NAME}:{TLS_PORT}/v1/capabilities",
        tls.display()
    ))
    .status()
    .map_err(|e| e.to_string())?;
    if old.success() {
        return Err("nginx accepted TLS 1.2".to_owned());
    }

    // Signed requests through the proxy.
    let empty = tls.join("empty");
    std::fs::write(&empty, b"").map_err(|e| e.to_string())?;
    let task = report["task_id"].as_str().ok_or("task id")?;
    let path = format!("/v1/tasks/{task}");
    let headers = signed("GET", &path, b"")?;
    let first = curl(&tls, "GET", &path, &headers, &empty)?;
    let second = curl(&tls, "GET", &path, &headers, &empty)?;
    if first.0 != 200 || second.0 != 401 || !second.1.contains("REPLAY") {
        return Err(format!("replay: {first:?} {second:?}"));
    }
    println!("  a signed request passes through nginx once; its replay is refused");
    let query = format!("{path}?injected=1");
    let headers = signed("GET", &path, b"")?;
    let added = curl(&tls, "GET", &query, &headers, &empty)?;
    if added.0 != 400 || !added.1.contains("AUTH_QUERY_NOT_ALLOWED") {
        return Err(format!("query: {added:?}"));
    }
    let body = tls.join("altered");
    std::fs::write(&body, b"{}").map_err(|e| e.to_string())?;
    let headers = signed("PUT", "/v1/job-specs", &spec)?;
    let altered = curl(&tls, "PUT", "/v1/job-specs", &headers, &body)?;
    if altered.0 != 401 || !altered.1.contains("AUTH_BAD_SIGNATURE") {
        return Err(format!("altered body: {altered:?}"));
    }
    let large = tls.join("large");
    std::fs::write(&large, vec![b'x'; 300 * 1024]).map_err(|e| e.to_string())?;
    let oversized = curl(&tls, "PUT", "/v1/job-specs", &[], &large)?;
    if oversized.0 != 413 {
        return Err(format!("size limit: {}", oversized.0));
    }
    println!(
        "  through nginx: added query, altered body and oversized body refused; TLS 1.2 refused"
    );

    // The backend goes away mid-job, and comes back.
    let marker = c.scratch.join("restart-starts");
    let slow = fixture_spec(
        json!({"mode": "slow", "delay_ms": 4_000, "marker": marker}),
        64,
        json!({"max_wall_ms": 20_000, "max_memory_mib": 256}),
    )?;
    let client = remote_job(stack, &tls, "restart", &slow, b"[4,5]")?
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    while !marker.exists() {
        if started.elapsed() > Duration::from_secs(60) {
            return Err("the job never started".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    stop_service(provider).await?;
    let down = curl(&tls, "GET", "/v1/capabilities", &[], &empty)?;
    if down.0 != 502 {
        return Err(format!("backend down: {}", down.0));
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    start_service(stack, provider, log, config_file, config, "denied").await?;
    let run = client.wait_with_output().map_err(|e| e.to_string())?;
    let report = t1_report(&run, "after the restart")?;
    let access = std::fs::read_to_string(tls.join("access.log")).map_err(|e| e.to_string())?;
    let failed = access
        .lines()
        .filter(|l| l.contains("/v1/tasks/") && l.contains("\" 502 "))
        .count();
    if failed == 0 {
        return Err("the client saw no gateway failure: nothing was tested".to_owned());
    }
    println!(
        "  backend stopped mid-job: {failed} tracking request(s) got 502, the client recovered, \
         receipt at height {}, verified at T1",
        report["receipt_height"]
    );

    // A port the runner listens on, under each policy.
    let listening = |name: &str| -> Result<(std::process::Child, PathBuf), String> {
        let marker = c.scratch.join(format!("{name}.listening"));
        let spec = fixture_spec(
            json!({"mode": "listen", "port": RUNNER_LISTEN_PORT, "marker": marker, "hold_ms": 5_000}),
            64,
            json!({"max_wall_ms": 20_000, "max_memory_mib": 256}),
        )?;
        let child = stack
            .client_command(name, &spec, b"[1]")?
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok((child, marker))
    };
    let try_it = |name: &'static str| async move {
        let (child, marker) = listening(name)?;
        let started = Instant::now();
        while !marker.exists() {
            if started.elapsed() > Duration::from_secs(60) {
                return Err::<(bool, u64), String>("the listener never started".to_owned());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let reached = reachable(RUNNER_LISTEN_PORT)?;
        let run = child.wait_with_output().map_err(|e| e.to_string())?;
        let report = t1_report(&run, name)?;
        let bytes =
            hex::decode(report["output"].as_str().ok_or("output")?).map_err(|e| e.to_string())?;
        let output: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok((reached, output["connections"].as_u64().unwrap_or(0)))
    };
    let (reached, connections) = try_it("listen-denied").await?;
    if reached || connections != 0 {
        return Err(format!("runner listener: {reached} {connections}"));
    }
    println!("  a port the runner listens on: not reachable from the remote namespace");
    shell(&format!("nft delete chain {NFT_TABLE} input"))?;
    let (reached, connections) = try_it("listen-output-only").await?;
    println!(
        "  observed, with only the output rule: reachable {reached}, {connections} connection(s)"
    );
    c.prepare()?;
    stop_service(provider).await?;
    shell(&format!("nft delete table {NFT_TABLE}"))?;
    start_service(stack, provider, log, config_file, config, "open").await?;
    let (reached, connections) = try_it("listen-open").await?;
    if !reached || connections == 0 {
        return Err(format!("control: {reached} {connections}"));
    }
    println!("  the control, with no policy: reachable, {connections} connection(s) accepted");
    stop_service(provider).await?;
    c.prepare()?;
    std::env::set_var("MBONGO_INBOUND_ALLOW", "none");
    c.prepare()?;
    start_service(stack, provider, log, config_file, config, "denied").await
}

/// A job that forks without end stops at the limit, and takes nothing with it.
/// A runner cannot read its own cgroup, so the counters are read from outside.
async fn prove_process_limit(c: &Contained, stack: &Stack) -> Result<(), String> {
    let attempts = MAX_PIDS * 3;
    let marker = c.scratch.join("forks.done");
    let spec = fixture_spec(
        json!({
            "mode": "fork",
            "attempts": attempts,
            "counter": c.scratch.join("forks"),
            "marker": marker,
            "hold_ms": 2_000,
        }),
        1_024,
        json!({"max_wall_ms": 20_000, "max_memory_mib": 256}),
    )?;
    let mut client = stack.client_command("fork", &spec, b"[1]")?;
    let client = client
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let waiting = Instant::now();
    while !marker.exists() {
        if waiting.elapsed() > Duration::from_secs(60) {
            return Err("the forking job never finished forking".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (limit, current, events) = (
        c.job_file("pids.max")?,
        c.job_file("pids.current")?,
        c.job_file("pids.events")?,
    );
    let limit: u64 = limit.trim().parse().map_err(|_| format!("pids.max: {limit}"))?;
    let current: u64 = current.trim().parse().map_err(|_| format!("pids.current: {current}"))?;
    let denied = events
        .lines()
        .find_map(|l| l.strip_prefix("max "))
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or(format!("pids.events: {events}"))?;
    if limit != MAX_PIDS || current > limit || current * 2 < limit || denied == 0 {
        return Err(format!("limit {limit}, current {current}, denied {denied}"));
    }
    let run = client.wait_with_output().map_err(|e| e.to_string())?;
    let report: Value = serde_json::from_slice(&run.stdout)
        .map_err(|_| format!("fork: {}", String::from_utf8_lossy(&run.stderr).trim()))?;
    let output =
        hex::decode(report["output"].as_str().ok_or("output")?).map_err(|e| e.to_string())?;
    let result: Value = serde_json::from_slice(&output).map_err(|e| e.to_string())?;
    let forked = result["started"].as_u64().ok_or(format!("fork: {result}"))?;
    if !run.status.success() || report["technical_level"] != "T1" || forked >= attempts {
        return Err(format!("fork: {report}"));
    }
    let started = Instant::now();
    loop {
        let (groups, pids) = c.job_processes()?;
        if groups == 0 && pids.is_empty() {
            break;
        }
        if started.elapsed() > Duration::from_secs(5) {
            return Err(format!("{} processes outlived the job", pids.len()));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    println!(
        "  {attempts} attempts under pids.max {limit}: {forked} forked, {current} running, \
         {denied} denied, none left"
    );
    Ok(())
}

/// `SIGTERM` with a job running: the service takes the job's whole tree down
/// and exits cleanly. The next start runs the task again, to a verified result.
async fn prove_sigterm(
    stack: &Stack,
    provider: &mut Process,
    log: &Path,
    config_file: &Path,
    config: &Value,
) -> Result<(), String> {
    let c = stack.contained.as_ref().ok_or("not contained")?;
    let heartbeat = c.scratch.join("sigterm.heartbeat");
    let spec = fixture_spec(
        json!({"mode": "daemon", "heartbeat": heartbeat, "hold_ms": 6_000}),
        1_024,
        json!({"max_wall_ms": 30_000, "max_memory_mib": 256}),
    )?;
    let mut client = stack.client_command("sigterm", &spec, b"[1]")?;
    let client = client
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let lines = || std::fs::read_to_string(&heartbeat).map_or(0, |b| b.lines().count());
    while lines() < 3 {
        if started.elapsed() > Duration::from_secs(60) {
            return Err("the job never started".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let (groups, pids) = c.job_processes()?;
    if groups != 1 || pids.len() < 2 {
        return Err(format!(
            "expected a runner and a descendant: {groups} {pids:?}"
        ));
    }
    shell(&format!("kill -TERM {}", provider.0.id()))?;
    let status = loop {
        if let Some(status) = provider.0.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(90) {
            return Err("the service did not stop".to_owned());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    if !status.success() {
        return Err(format!("the service stopped with {status}"));
    }
    let stopped = std::fs::read_to_string(log).unwrap_or_default();
    if !stopped.lines().any(|l| l == "SHUTDOWN_JOBS_KILLED=1") {
        return Err("the service did not report the job it killed".to_owned());
    }
    for pid in &pids {
        gone(pid).await?;
    }
    let (groups, left) = c.job_processes()?;
    if groups != 0 || !left.is_empty() {
        return Err(format!("left after SIGTERM: {groups} {left:?}"));
    }
    println!(
        "  SIGTERM with a job running: exit 0, {} processes dead, no cgroup left",
        pids.len()
    );

    let mut config = config.clone();
    config["bind"] = json!(stack.provider.trim_start_matches("http://"));
    std::fs::write(config_file, config.to_string()).map_err(|e| e.to_string())?;
    *provider = spawn_logged(service_command(Some(c), config_file), log)?;
    wait_for_bind(log, provider).await?;
    if !std::fs::read_to_string(log).unwrap_or_default().contains("CONTAINMENT_SWEPT=0") {
        return Err("SIGTERM left a job for the next start to sweep".to_owned());
    }
    let run = client.wait_with_output().map_err(|e| e.to_string())?;
    let report: Value = serde_json::from_slice(&run.stdout).map_err(|_| {
        format!(
            "after SIGTERM: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        )
    })?;
    if !run.status.success() || report["technical_level"] != "T1" {
        return Err(format!("after SIGTERM: {report}"));
    }
    println!("  restarted: nothing to sweep, the task completed at T1");
    Ok(())
}

/// Under the limit it runs. Over it, the kernel kills the job and says so.
/// A spec cannot lift the provider's ceiling.
fn prove_memory(c: &Contained, stack: &Stack, log: &Path) -> Result<(), String> {
    let cause_logged = || {
        std::fs::read_to_string(log)
            .unwrap_or_default()
            .contains("memory limit reached")
    };
    let job = |limit: u64, mib: u64| {
        fixture_spec(
            json!({"mode": "alloc", "mib": mib}),
            64,
            json!({"max_wall_ms": 20_000, "max_memory_mib": limit}),
        )
    };
    let before = c.oom_kills()?;
    let output = verified_output(stack, "under-limit", &job(256, 64)?)?;
    if output != b"{\"count\":1,\"sum\":1}" || c.oom_kills()? != before {
        return Err("a job under its limit was disturbed".to_owned());
    }
    if cause_logged() {
        return Err("an out-of-memory cause logged with none".to_owned());
    }
    println!("  64 MiB under a 256 MiB limit: verified at T1, no OOM kill");
    refused_job(stack, "over-limit", &job(128, 512)?, "JOB_RUNTIME_FAILED")?;
    let after = c.oom_kills()?;
    if after <= before {
        return Err("the job failed, but not by the memory limit".to_owned());
    }
    if !cause_logged() {
        return Err("the out-of-memory cause was not recorded".to_owned());
    }
    println!("  512 MiB under a 128 MiB limit: killed, oom_kill {before} -> {after}");
    let huge = 1 << 20;
    refused_job(
        stack,
        "over-ceiling",
        &job(huge, 1_024)?,
        "JOB_RUNTIME_FAILED",
    )?;
    let last = c.oom_kills()?;
    if last <= after {
        return Err("a spec lifted the provider's ceiling".to_owned());
    }
    println!(
        "  1 GiB under a declared 1 TiB, ceiling {MEMORY_CEILING_MIB} MiB: killed, \
         oom_kill {after} -> {last}"
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

async fn run(base: &Path, contain: bool) -> Result<(), String> {
    if contain {
        std::env::set_var("MBONGO_INBOUND_ALLOW", "none");
    }
    std::fs::create_dir_all(base.join("artifacts")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(base.join("keys")).map_err(|e| e.to_string())?;
    report_time_wait();
    let contained = if contain {
        Some(Contained::set_up(base)?)
    } else {
        None
    };
    let node_log = base.join("node.log");
    let provider_log = base.join("provider.log");
    let mut node = Command::new(sibling("mbongo-node"));
    node.args(["--producer", "--block-time", "1"])
        .args(["--rpc-port", &RPC_PORT.to_string()])
        .args(["--rest-port", &REST_PORT.to_string()])
        .args(["--p2p-port", &P2P_PORT.to_string()])
        .arg("--data-dir")
        .arg(base.join("chain"));
    if contain {
        // The remote client's node. Its transport is outside P3-C, and the
        // firewall decides who reaches it.
        node.args(["--rpc-host", "0.0.0.0", "--rest-host", "0.0.0.0"]);
    }
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
        std::fs::write(base.join("keys/executor.key"), hex::encode(EXECUTOR_SEED))
            .map_err(|e| e.to_string())?;
        std::fs::write(base.join("keys/state.key"), hex::encode([0x57u8; 32]))
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

        let fixture = match &contained {
            Some(c) => {
                c.prepare()?;
                base.join("bin/mbongo-runner-fixture.mjs")
            }
            None => repo_root().join("scripts/fixtures/mbongo-runner-fixture.mjs"),
        };
        let isolation = match &contained {
            Some(c) => json!({
                "mode": "required",
                "runner_uid": c.runner_uid,
                "runner_gid": c.runner_gid,
                "cgroup_root": c.cgroup,
                "max_memory_mib": MEMORY_CEILING_MIB,
                "max_pids": MAX_PIDS,
                "network": "denied",
            }),
            None => json!({"mode": "none"}),
        };
        let config = json!({
            "bind": "127.0.0.1:0",
            "rpc_url": format!("http://127.0.0.1:{RPC_PORT}/rpc"),
            "rest_url": format!("http://127.0.0.1:{REST_PORT}"),
            "executor_key_file": base.join("keys/executor.key"),
            "state_dir": base.join("state"),
            "state_key_file": base.join("keys/state.key"),
            "artifact_dir": base.join("artifacts"),
            "work_dir": base.join("work"),
            "isolation": isolation,
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
        if let Some(c) = &contained {
            prove_refusals(c, &config)?;
        }
        let command = service_command(contained.as_ref(), &config_file);
        let mut provider = spawn_logged(command, &provider_log)?;
        let url = wait_for_bind(&provider_log, &mut provider).await?;
        println!("  provider service up at {url}");
        let stack = Stack {
            base: base.to_path_buf(),
            provider: url,
            executor: hex::encode(executor.address().0),
            contained,
        };
        if let Some(c) = &stack.contained {
            let pid = provider.0.id();
            prove_service_identity(c, &stack, pid, &provider_log).await?;
            prove_attack(c, &stack, pid)?;
            prove_descendants(c, &stack).await?;
            prove_memory(c, &stack, &provider_log)?;
            prove_process_limit(c, &stack).await?;
            prove_network(&stack, &mut provider, &provider_log, &config_file, &config).await?;
            prove_policy_loss(&stack, &mut provider, &provider_log, &config_file, &config).await?;
            prove_transport(&stack, &mut provider, &provider_log, &config_file, &config).await?;
        }
        prove_vectors(&stack, &chain).await?;
        prove_failure(&stack)?;
        prove_exclusive_state(&stack, &config_file)?;
        if stack.contained.is_some() {
            prove_sigterm(&stack, &mut provider, &provider_log, &config_file, &config).await?;
        }
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
    let contain = std::env::args().any(|a| a == "--contained");
    let title = if contain {
        "PROVIDER CONTAINMENT"
    } else {
        "PROVIDER FLOW"
    };
    let base = std::env::temp_dir().join("mbongo_provider_harness");
    let _ = std::fs::remove_dir_all(&base);
    println!("=== {title} ===\n");
    let result = run(&base, contain).await;
    let _ = std::fs::remove_dir_all(&base);
    match result {
        Ok(()) => println!("\n{title}: PASS"),
        Err(e) => {
            eprintln!("\n{title}: FAIL\n  Error: {e}");
            std::process::exit(1);
        }
    }
}
