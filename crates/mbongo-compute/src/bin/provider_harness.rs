//! End-to-end proof on a real node, with every role in its own process: a
//! producer node, a provider service, a standalone `job_client`, and a runner
//! written independently of the core. Synthetic fixtures only.
//!
//! With `--contained` (Linux, root) the same flow runs under the reference
//! launcher, then attacks the boundary from inside a runner. Root is the
//! harness's, to set up and to observe: the service and the runner never have it.
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
/// Every way a contained start must be refused.
const REFUSALS: usize = 13;
/// Two key files, two directories, the journal, the lock, two `/proc` entries.
const PROBED_PATHS: u64 = 8;
const NO_CAPABILITIES: &str = "0000000000000000";
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
    }
    let base = c.base.display();
    let root = c.cgroup.display();
    let (provider, runner) = (c.provider_uid, c.runner_gid);
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
    Ok(())
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

/// Under the limit it runs. Over it, the kernel kills the job and says so.
/// A spec cannot lift the provider's ceiling.
fn prove_memory(c: &Contained, stack: &Stack) -> Result<(), String> {
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
    println!("  64 MiB under a 256 MiB limit: verified at T1, no OOM kill");
    refused_job(stack, "over-limit", &job(128, 512)?, "JOB_RUNTIME_FAILED")?;
    let after = c.oom_kills()?;
    if after <= before {
        return Err("the job failed, but not by the memory limit".to_owned());
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
                "max_pids": 256,
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
            prove_memory(c, &stack)?;
        }
        prove_vectors(&stack, &chain).await?;
        prove_failure(&stack)?;
        prove_exclusive_state(&stack, &config_file)?;
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
