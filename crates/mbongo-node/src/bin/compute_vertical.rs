//! Mbongo Compute v0.4 Vertical Harness.
//!
//! The operational end-to-end gate for the first Compute vertical on the
//! frozen v0.4 base (`docs/specs/PROTOCOL_LOCK_v0.4.md`): a fresh three-node
//! devnet, brought to convergence, then driven through the whole flow —
//! private input registered off-chain, `ComputeTask` submitted and read back
//! **through the public TypeScript SDK**, task discovered by the reference
//! control plane, wrong executor refused, corrupted input and result-store
//! failures stopped before any receipt, worker crash after fetch recovered
//! with a fresh capability, the named executor's bound receipt anchored and
//! read back through the SDK, the private result retrieved only by its
//! owner, every chain-visible artifact scanned for the private bytes, the
//! control plane restarted, a node restarted, and every log checked for
//! payloads and secrets.
//!
//! Nothing here is consensus. The nodes are the real `mbongo-node` binary;
//! the control plane, data plane and worker are the reference
//! implementation, run in this process, and the client's chain-facing steps
//! run in `sdk/typescript/scripts/compute-vertical.mjs` against the built
//! workspace SDK. Exit code 0 on `MBONGO COMPUTE V0.4 VERTICAL: PASS`.
//!
//! See `docs/runbooks/DEVNET_V0.4_OPERATIONS.md` for the operator profile
//! this harness is the executable form of.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mbongo_compute::chain::{find_receipt, scan_tasks, ChainClient, RpcChainClient};
use mbongo_compute::clock::{Clock, SystemClock};
use mbongo_compute::control_plane::{ControlPlane, ControlPlaneConfig, FailureClass, TaskState};
use mbongo_compute::data_plane::{
    Capability, CapabilityRequest, DataPlaneFault, InMemoryDataPlane, LocalKey, ObjectState,
    Operation, Presentation,
};
use mbongo_compute::execution::{
    reference_input_commitment, reference_output_commitment, ReverseBytesProfile,
    REVERSE_BYTES_SPEC,
};
use mbongo_compute::identity::{CapabilityId, ExecutorKey, IdSource, ObjectId};
use mbongo_compute::worker::{AttemptOutcome, Fault, Worker};
use mbongo_core::{
    Address, Block, ComputeTask, Receipt, Transaction, TransactionPayload, TransactionType,
    COMPUTE_TASK_VERSION,
};
use mbongo_node::convergence::{
    await_convergence, await_endpoints_ready, get_height, NodeEndpoint,
};
use parity_scale_codec::Encode;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::sleep;

// ── Profile constants (LOCAL / DEVNET REFERENCE PROFILE) ───────────────

/// The authority the operator is running. Informational; not a protocol field.
const PROTOCOL_AUTHORITY: &str = "PROTOCOL_LOCK_v0.4";
const RPC_AUTHORITY: &str = "rpc_v0.3";
const STORAGE_SCHEMA: u32 = 3;
/// The line every node prints at start-up; the harness requires it from all
/// three, which is the operational version check (one binary, one lock).
const NODE_BANNER: &str = "  Protocol: PROTOCOL_LOCK_v0.4 (rpc_v0.3, storage schema 3)";

const BLOCK_TIME_SECS: u64 = 1;
const MIN_HEIGHT: u64 = 3;
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(300);
const WAIT_SECS: u64 = 90;

/// Lease lifetime: short, so a crashed worker's lease lapses within the run.
const LEASE_SECS: u64 = 5;

struct NodeCfg {
    name: &'static str,
    rpc: u16,
    rest: u16,
    p2p: u16,
    producer: bool,
}

const NODES: [NodeCfg; 3] = [
    NodeCfg {
        name: "producer",
        rpc: 39944,
        rest: 38080,
        p2p: 40333,
        producer: true,
    },
    NodeCfg {
        name: "follower-a",
        rpc: 39945,
        rest: 38081,
        p2p: 40334,
        producer: false,
    },
    NodeCfg {
        name: "follower-b",
        rpc: 39946,
        rest: 38082,
        p2p: 40335,
        producer: false,
    },
];

// ── Log capture for the in-process components ──────────────────────────

static COMPONENT_LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
static SINK: Sink = Sink;

struct Sink;

impl log::Log for Sink {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        if let Ok(mut buf) = COMPONENT_LOG.lock() {
            buf.push(format!(
                "{} {} {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
    }
    fn flush(&self) {}
}

// ── Nodes ──────────────────────────────────────────────────────────────

struct Node {
    child: Child,
    /// Every stdout and stderr line, kept for the log-privacy scan.
    logs: Arc<Mutex<Vec<String>>>,
    multiaddr: String,
    data_dir: PathBuf,
}

fn node_binary() -> PathBuf {
    let exe = std::env::current_exe().expect("own path");
    let dir = exe.parent().expect("parent");
    dir.join(if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    })
}

fn sdk_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("sdk")
        .join("typescript")
        .join("scripts")
        .join("compute-vertical.mjs")
}

fn sdk_dist() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("sdk")
        .join("typescript")
        .join("dist")
        .join("index.js")
}

fn port_free(port: u16, label: &str) -> Result<(), String> {
    std::net::TcpListener::bind(("127.0.0.1", port))
        .map(|_| ())
        .map_err(|e| format!("port {port} ({label}) is already in use: {e}"))
}

async fn spawn_node(
    cfg: &'static NodeCfg,
    data_dir: &Path,
    bootnodes: &[String],
) -> Result<Node, String> {
    let mut cmd = Command::new(node_binary());
    cmd.arg("--rpc-port")
        .arg(cfg.rpc.to_string())
        .arg("--rest-port")
        .arg(cfg.rest.to_string())
        .arg("--p2p-port")
        .arg(cfg.p2p.to_string())
        .arg("--data-dir")
        .arg(data_dir.to_str().ok_or("data dir path")?);
    if cfg.producer {
        cmd.arg("--producer").arg("--block-time").arg(BLOCK_TIME_SECS.to_string());
    }
    for b in bootnodes {
        cmd.arg("--bootnodes").arg(b);
    }
    cmd.env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| format!("spawn {}: {e}", cfg.name))?;
    let logs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // stderr: drain continuously.
    if let Some(stderr) = child.stderr.take() {
        let logs = Arc::clone(&logs);
        let name = cfg.name;
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                logs.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(format!("[{name}:stderr] {l}"));
            }
        });
    }
    // stdout: read until the PeerId line, then drain.
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut lines = BufReader::new(stdout).lines();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut multiaddr = None;
    let mut saw_banner = false;
    while let Ok(Ok(Some(line))) = tokio::time::timeout_at(deadline, lines.next_line()).await {
        if line == NODE_BANNER {
            saw_banner = true;
        }
        let peer = line.strip_prefix("  PeerId:").map(|r| r.trim().to_string());
        logs.lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(format!("[{}:stdout] {line}", cfg.name));
        if let Some(peer) = peer {
            multiaddr = Some(format!("/ip4/127.0.0.1/tcp/{}/p2p/{peer}", cfg.p2p));
            break;
        }
    }
    let Some(multiaddr) = multiaddr else {
        return Err(format!("{}: no PeerId line within 20s", cfg.name));
    };
    if !saw_banner {
        return Err(format!(
            "{}: the node did not announce {PROTOCOL_AUTHORITY} / {RPC_AUTHORITY} / storage schema {STORAGE_SCHEMA}; this is not the expected v0.4 binary",
            cfg.name
        ));
    }
    {
        let logs = Arc::clone(&logs);
        let name = cfg.name;
        tokio::spawn(async move {
            while let Ok(Some(l)) = lines.next_line().await {
                logs.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(format!("[{name}:stdout] {l}"));
            }
        });
    }
    Ok(Node {
        child,
        logs,
        multiaddr,
        data_dir: data_dir.to_path_buf(),
    })
}

fn endpoint(cfg: &NodeCfg) -> NodeEndpoint {
    NodeEndpoint::localhost_port(cfg.name, cfg.rpc)
}

fn chain_of(cfg: &NodeCfg) -> RpcChainClient {
    RpcChainClient::new(
        format!("http://127.0.0.1:{}/rpc", cfg.rpc),
        format!("http://127.0.0.1:{}", cfg.rest),
    )
}

// ── Chain helpers ──────────────────────────────────────────────────────

async fn wait_until<T, F, Fut>(what: &str, mut f: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Option<T>, String>>,
{
    let start = std::time::Instant::now();
    loop {
        if let Some(v) = f().await? {
            return Ok(v);
        }
        if start.elapsed() > Duration::from_secs(WAIT_SECS) {
            return Err(format!("timed out waiting for {what}"));
        }
        sleep(POLL).await;
    }
}

async fn wait_blocks<C: ChainClient>(chain: &C, n: u64) -> Result<(), String> {
    let start = chain.latest_height().await.map_err(|e| e.to_string())?;
    wait_until(&format!("{n} more blocks"), || async {
        let h = chain.latest_height().await.map_err(|e| e.to_string())?;
        Ok(if h >= start + n { Some(()) } else { None })
    })
    .await
}

async fn receipts_for<C: ChainClient>(
    chain: &C,
    task_id: [u8; 32],
) -> Result<Vec<(u64, Receipt)>, String> {
    let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for h in 0..=latest {
        if let Some(block) = chain.block_by_height(h).await.map_err(|e| e.to_string())? {
            for tx in &block.body.transactions {
                if let TransactionPayload::AnchorReceipt(r) = &tx.payload {
                    if r.task_id == task_id {
                        out.push((h, (**r).clone()));
                    }
                }
            }
        }
    }
    Ok(out)
}

fn signed_transfer(from: &LocalKey, to: Address, amount: u128, nonce: u64) -> Transaction {
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
    tx
}

fn signed_anchor(key: &ExecutorKey, receipt: Receipt, nonce: u64) -> Transaction {
    let mut tx = Transaction {
        tx_type: TransactionType::AnchorReceipt,
        sender: key.address(),
        receiver: Address::zero(),
        amount: 0,
        nonce,
        payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
        signature: [0u8; 64],
    };
    tx.signature = key.sign(&tx.signing_payload());
    tx
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Every textual and binary form a private byte string could take in a
/// block, a JSON response or a log line.
fn forms(bytes: &[u8]) -> Vec<Vec<u8>> {
    let json = serde_json::to_string(bytes).unwrap_or_default();
    vec![
        bytes.to_vec(),
        hex::encode(bytes).into_bytes(),
        json.as_bytes()[1..json.len() - 1].to_vec(),
    ]
}

// ── SDK bridge ─────────────────────────────────────────────────────────

struct SdkRun {
    out: serde_json::Value,
    stderr: String,
}

async fn run_sdk(command: &str, input: &serde_json::Value) -> Result<SdkRun, String> {
    let mut child = Command::new("node")
        .arg(sdk_script())
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run node (is Node.js installed?): {e}"))?;
    {
        let mut stdin = child.stdin.take().ok_or("sdk stdin")?;
        stdin.write_all(input.to_string().as_bytes()).await.map_err(|e| e.to_string())?;
    }
    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(format!("sdk {command} failed: {stderr}"));
    }
    let out: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("sdk {command} output is not JSON: {e}; stderr: {stderr}"))?;
    Ok(SdkRun { out, stderr })
}

fn field<'a>(v: &'a serde_json::Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(|x| x.as_str())
        .ok_or_else(|| format!("sdk output lacks {key}"))
}

fn hex32(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim_start_matches("0x")).map_err(|e| e.to_string())?;
    v.try_into().map_err(|_| "not 32 bytes".to_string())
}

// ── The trace the operator reads ───────────────────────────────────────

#[derive(Default)]
struct Trace {
    lines: Vec<String>,
}

impl Trace {
    fn record(&mut self, key: &str, value: impl std::fmt::Display) {
        self.lines.push(format!("{key}={value}"));
    }
}

// ── The vertical ───────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
async fn run(base: &Path, trace: &mut Trace) -> Result<(), String> {
    let http = reqwest::Client::new();

    // ── Phase 0: preflight ───────────────────────────────────────────
    println!("Phase 0: preflight (authority {PROTOCOL_AUTHORITY}, {RPC_AUTHORITY}, storage schema {STORAGE_SCHEMA})");
    let binary = node_binary();
    if !binary.exists() {
        return Err(format!(
            "node binary not found at {}; run `cargo build -p mbongo-node` first",
            binary.display()
        ));
    }
    let bin_bytes = std::fs::read(&binary).map_err(|e| e.to_string())?;
    let fingerprint = hex::encode(&blake3::hash(&bin_bytes).as_bytes()[..8]);
    println!(
        "  node binary {} (blake3 {fingerprint}); all three nodes run this one binary",
        binary.display()
    );
    trace.record("NODE_BINARY_BLAKE3", &fingerprint);
    if !sdk_dist().exists() {
        return Err(format!(
            "workspace SDK is not built ({} missing); run `npm ci && npm run build` in sdk/typescript. The published @mbongo/sdk 0.1.0 is not v0.4-capable and is not used.",
            sdk_dist().display()
        ));
    }
    for n in &NODES {
        port_free(n.rpc, &format!("{} rpc", n.name))?;
        port_free(n.rest, &format!("{} rest", n.name))?;
        port_free(n.p2p, &format!("{} p2p", n.name))?;
    }
    println!("  ports free: rpc 39944-39946, rest 38080-38082, p2p 40333-40335");
    if base.exists() {
        println!(
            "  removing previous harness state (only this directory): {}",
            base.display()
        );
        std::fs::remove_dir_all(base).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(base).map_err(|e| e.to_string())?;
    println!("  fresh data directories under {} — no prior chain state exists, so every node starts from genesis (schema 3)", base.display());

    // ── Phase 1: fresh three-node devnet, health gate ────────────────
    println!("Phase 1: starting a fresh three-node devnet...");
    let mut nodes: Vec<Node> = Vec::new();
    let producer_dir = base.join(NODES[0].name);
    let producer = spawn_node(&NODES[0], &producer_dir, &[]).await?;
    let boot = vec![producer.multiaddr.clone()];
    nodes.push(producer);
    await_endpoints_ready(&http, &[endpoint(&NODES[0])], POLL, READY_TIMEOUT).await?;
    for cfg in &NODES[1..] {
        let n = spawn_node(cfg, &base.join(cfg.name), &boot).await?;
        nodes.push(n);
    }
    let endpoints: Vec<NodeEndpoint> = NODES.iter().map(endpoint).collect();
    await_endpoints_ready(&http, &endpoints, POLL, READY_TIMEOUT).await?;
    println!("  all three nodes answer ping and announced {PROTOCOL_AUTHORITY}");
    let h0 = await_convergence(&http, &endpoints, MIN_HEIGHT, POLL, CONVERGENCE_TIMEOUT).await?;
    println!("  converged at height {h0}: blocks are produced and both followers receive them");
    trace.record("CONVERGED_HEIGHT_AT_START", h0);

    let chain = chain_of(&NODES[0]);
    let followers: Vec<RpcChainClient> = NODES[1..].iter().map(chain_of).collect();

    // ── Phase 2: identities and funding ──────────────────────────────
    // TEST / DEV KEYS ONLY. The client is the code-baked public devnet
    // account (seed 0xAA…, funded at genesis). The others are funded here.
    println!("Phase 2: funding the executor and a squatter from the devnet account...");
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let mut ids = IdSource::new({
        let mut s = [0u8; 32];
        s[..8].copy_from_slice(&SystemClock.now().to_le_bytes());
        s[8..12].copy_from_slice(&std::process::id().to_le_bytes());
        s
    });
    let client = LocalKey::from_seed(&[0xAAu8; 32]);
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let squatter = ExecutorKey::from_seed(&[0xE9u8; 32]);
    let stranger = ExecutorKey::from_seed(&[0x5Eu8; 32]);
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]);
    chain
        .submit_transaction(&signed_transfer(&client, executor.address(), 1_000, 0))
        .await
        .map_err(|e| e.to_string())?;
    chain
        .submit_transaction(&signed_transfer(&client, squatter.address(), 1_000, 1))
        .await
        .map_err(|e| e.to_string())?;
    wait_until("funding to be included", || async {
        Ok(
            if chain.account_nonce(&client.address()).await.map_err(|e| e.to_string())? >= 2 {
                Some(())
            } else {
                None
            },
        )
    })
    .await?;
    trace.record("CLIENT", client.address());
    trace.record("EXECUTOR", executor.address());
    trace.record("SQUATTER", squatter.address());

    // ── Phase 3: private input off-chain; ComputeTask through the SDK ─
    println!("Phase 3: client stores the private input in the reference data plane and submits the task through the SDK...");
    let private_input: Vec<u8> =
        b"vertical private input: exists only in the data plane and worker memory".to_vec();
    let mut expected_output = private_input.clone();
    expected_output.reverse();
    let input_commitment = reference_input_commitment(&private_input);
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&ids.next("salt"));
    let task = ComputeTask {
        version: COMPUTE_TASK_VERSION,
        submitter: client.address(),
        executor: executor.address(),
        salt,
        input_commitment,
        execution_spec: REVERSE_BYTES_SPEC.to_vec(),
    };
    let task_id = task.task_id();
    let mut dp = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(ids.next("dp")));
    let input_object = dp.store_input(
        &client,
        task_id,
        input_commitment,
        private_input.clone(),
        3_600,
    );
    dp.register_task(&client, task_id, executor.address())
        .map_err(|e| e.to_string())?;
    dp.delegate_issuer(&client, task_id, issuer.address())
        .map_err(|e| e.to_string())?;
    println!(
        "  input object {input_object} stored; task registered for executor {}",
        executor.address()
    );

    let nonce = chain.account_nonce(&client.address()).await.map_err(|e| e.to_string())?;
    let submit = run_sdk(
        "submit",
        &serde_json::json!({
            "rpcUrl": format!("http://127.0.0.1:{}/rpc", NODES[0].rpc),
            "submitterSeedHex": hex::encode([0xAAu8; 32]),
            "executorHex": hex::encode(executor.address().0),
            "saltHex": hex::encode(salt),
            "inputCommitmentHex": hex::encode(input_commitment),
            "executionSpecHex": hex::encode(REVERSE_BYTES_SPEC),
            "nonce": nonce,
            "timeoutSecs": WAIT_SECS,
        }),
    )
    .await?;
    let sdk_task_id = hex32(field(&submit.out, "taskId")?)?;
    if sdk_task_id != task_id {
        return Err("the SDK derived a different task_id than mbongo-core".to_string());
    }
    let task_height: u64 = field(&submit.out, "includedHeight")?.parse().map_err(|_| "height")?;
    let task_tx_hash = field(&submit.out, "txHash")?.to_string();
    let decoded = submit.out.get("decoded").ok_or("decoded")?;
    if hex32(field(decoded, "executor")?)? != executor.address().0
        || hex32(field(decoded, "inputCommitment")?)? != input_commitment
        || hex::decode(field(decoded, "executionSpec")?).map_err(|e| e.to_string())?
            != REVERSE_BYTES_SPEC
        || hex32(field(decoded, "submitter")?)? != client.address().0
    {
        return Err(
            "the task the SDK decoded from the block differs from the task it submitted"
                .to_string(),
        );
    }
    println!("  SDK: ping {}, submitted {task_tx_hash}, committed at height {task_height}, decoded back with the same task_id", field(&submit.out, "ping")?);
    trace.record("TASK_TX_HASH", &task_tx_hash);
    trace.record("TASK_ID", hex::encode(task_id));
    trace.record("TASK_HEIGHT", task_height);
    trace.record("INPUT_COMMITMENT", hex::encode(input_commitment));
    trace.record(
        "EXECUTION_SPEC",
        String::from_utf8_lossy(REVERSE_BYTES_SPEC),
    );

    // ── Phase 4: discovery, and agreement across every component ─────
    println!(
        "Phase 4: the control plane discovers the task; every node and component agrees on it..."
    );
    let mut cp = ControlPlane::new(
        Arc::clone(&clock),
        IdSource::new(ids.next("cp")),
        issuer.clone(),
        ControlPlaneConfig {
            lease_secs: LEASE_SECS,
            session_secs: 300,
            confirmation_depth: 1,
            capability_secs: 60,
        },
    );
    let start = std::time::Instant::now();
    loop {
        cp.observe(&chain).await.map_err(|e| e.to_string())?;
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        if cp.task(task_id).is_some_and(|t| t.state == TaskState::Discovered)
            && latest > task_height
        {
            break;
        }
        if start.elapsed() > Duration::from_secs(WAIT_SECS) {
            return Err("control plane never discovered the confirmed task".to_string());
        }
        sleep(POLL).await;
    }
    cp.register_input(task_id, input_object).map_err(|e| e.to_string())?;
    let rec = cp.task(task_id).ok_or("record")?;
    if rec.task.executor != executor.address()
        || rec.task.input_commitment != input_commitment
        || rec.observed_height != task_height
    {
        return Err("control plane's view of the task disagrees with the SDK's".to_string());
    }
    for (i, f) in followers.iter().enumerate() {
        let latest = f.latest_height().await.map_err(|e| e.to_string())?;
        let seen = scan_tasks(f, task_height, latest.max(task_height))
            .await
            .map_err(|e| e.to_string())?;
        let same = seen.iter().filter(|t| t.task_id == task_id).count();
        if same != 1
            || seen
                .iter()
                .any(|t| t.task_id == task_id && (t.task != task || t.height != task_height))
        {
            return Err(format!(
                "follower {} sees {same} copies of the task or a different envelope",
                NODES[i + 1].name
            ));
        }
    }
    println!("  task_id, executor, input_commitment and height agree: SDK, producer, follower-a, follower-b, control plane");

    // ── Phase 5: wrong executor ──────────────────────────────────────
    println!("Phase 5: the wrong executor is not offered the task and cannot anchor...");
    let mut squat = Worker::new(&mut ids, squatter.clone(), Box::new(ReverseBytesProfile));
    let o = squat.run_once(&mut cp, &mut dp, &chain).await.map_err(|e| e.to_string())?;
    if o != AttemptOutcome::Idle {
        return Err(format!(
            "the control plane offered the task to the squatter: {o:?}"
        ));
    }
    let mut squat_receipt = Receipt {
        version: 1,
        task_id,
        input_commitment,
        output_commitment: [0x44u8; 32],
        executor: squatter.address(),
        metadata: Vec::new(),
        signature: [0u8; 64],
    };
    squat_receipt.signature = squatter.sign(&squat_receipt.receipt_hash().0);
    match chain.submit_transaction(&signed_anchor(&squatter, squat_receipt, 0)).await {
        Err(e) if e.to_string().contains("not authorised") => {
            println!("  node refused the squatter's receipt (rule s): {e}")
        }
        other => {
            return Err(format!(
                "squatter receipt not refused on rule (s): {other:?}"
            ))
        }
    }
    trace.record(
        "WRONG_EXECUTOR",
        "not offered; receipt refused by the node on rule (s)",
    );

    // ── Phase 6: corrupted input, then a result-store failure ────────
    println!("Phase 6: corrupted input and a result-store failure stop the attempt before any receipt...");
    let mut w = Worker::new(&mut ids, executor.clone(), Box::new(ReverseBytesProfile));
    w.inject(Fault::CorruptInput);
    let o = w.run_once(&mut cp, &mut dp, &chain).await.map_err(|e| e.to_string())?;
    if !matches!(
        o,
        AttemptOutcome::Failed {
            class: FailureClass::Input,
            ..
        }
    ) {
        return Err(format!(
            "corrupted input did not fail at the input stage: {o:?}"
        ));
    }
    if dp.result_ref(task_id, executor.address()).is_some() {
        return Err("a result exists after a commitment mismatch".to_string());
    }
    let mut w = Worker::new(&mut ids, executor.clone(), Box::new(ReverseBytesProfile));
    dp.inject(DataPlaneFault::PutResultFails);
    let o = w.run_once(&mut cp, &mut dp, &chain).await.map_err(|e| e.to_string())?;
    if !matches!(
        o,
        AttemptOutcome::Failed {
            class: FailureClass::Persistence,
            ..
        }
    ) {
        return Err(format!(
            "a failed result store did not fail the attempt: {o:?}"
        ));
    }
    if dp.result_ref(task_id, executor.address()).is_some() {
        return Err("a result exists although the store failed".to_string());
    }
    wait_blocks(&chain, 2).await?;
    if !receipts_for(&chain, task_id).await?.is_empty() {
        return Err("a receipt was anchored after a failed attempt".to_string());
    }
    println!("  Failed(Input) then Failed(Persistence): no result, no receipt on-chain");
    trace.record(
        "COMMITMENT_MISMATCH",
        "Failed(Input); no execution, no result, no receipt",
    );
    trace.record(
        "PERSISTENCE_FAILURE",
        "Failed(Persistence); no result, no receipt",
    );

    // ── Phase 7: worker crash after fetch, recovered by a new instance ─
    println!("Phase 7: a worker crashes after fetching; a new instance recovers with a fresh capability...");
    let caps_before = cp.task(task_id).map(|t| t.capabilities.len()).unwrap_or(0);
    let attempts_before = cp.task(task_id).map(|t| t.attempt_count).unwrap_or(0);
    let mut w1 = Worker::new(&mut ids, executor.clone(), Box::new(ReverseBytesProfile));
    w1.inject(Fault::CrashAfterFetch);
    let o = w1.run_once(&mut cp, &mut dp, &chain).await.map_err(|e| e.to_string())?;
    if !matches!(o, AttemptOutcome::Crashed { .. }) {
        return Err(format!("expected a crash after fetch, got {o:?}"));
    }
    if dp.object_state(&input_object) != Some(ObjectState::Consumed) {
        return Err("the input was not marked consumed after the crashed fetch".to_string());
    }
    drop(w1);
    println!("  crashed instance gone; its capability is spent; waiting {LEASE_SECS}s for its lease to lapse...");
    sleep(Duration::from_secs(LEASE_SECS + 1)).await;
    let mut worker = Worker::new(&mut ids, executor.clone(), Box::new(ReverseBytesProfile));
    let o = worker.run_once(&mut cp, &mut dp, &chain).await.map_err(|e| e.to_string())?;
    let AttemptOutcome::Submitted {
        tx_hash: anchor_tx_hash,
        ..
    } = o
    else {
        return Err(format!(
            "the recovering instance did not submit a receipt: {o:?}"
        ));
    };
    let result_ref = dp
        .result_ref(task_id, executor.address())
        .ok_or("no durable result at submission time")?;
    let rec = cp.task(task_id).ok_or("record")?.clone();
    if rec.attempt_count <= attempts_before || rec.capabilities.len() <= caps_before + 1 {
        return Err("the recovery did not use a new attempt with fresh capabilities".to_string());
    }
    println!("  new attempt #{} with fresh capabilities; result persisted ({}); anchor submitted {anchor_tx_hash}", rec.attempt_count, result_ref.object);
    trace.record("ATTEMPTS", rec.attempt_count);
    trace.record("RESULT_OBJECT", result_ref.object);
    trace.record(
        "OUTPUT_COMMITMENT",
        hex::encode(result_ref.output_commitment),
    );
    trace.record("ANCHOR_TX_HASH", anchor_tx_hash);

    // ── Phase 8: receipt anchored, observed through the SDK ──────────
    println!("Phase 8: the bound receipt is anchored; the SDK reads and verifies it...");
    let observe = run_sdk(
        "observe",
        &serde_json::json!({
            "rpcUrl": format!("http://127.0.0.1:{}/rpc", NODES[0].rpc),
            "task": {
                "version": 1,
                "submitterHex": hex::encode(client.address().0),
                "executorHex": hex::encode(executor.address().0),
                "saltHex": hex::encode(salt),
                "inputCommitmentHex": hex::encode(input_commitment),
                "executionSpecHex": hex::encode(REVERSE_BYTES_SPEC),
            },
            "executorSeedHex": hex::encode([0xE1u8; 32]),
            "outputCommitmentHex": hex::encode(result_ref.output_commitment),
            "fromHeight": task_height,
            "timeoutSecs": WAIT_SECS,
        }),
    )
    .await?;
    let anchor_height: u64 =
        field(&observe.out, "anchorHeight")?.parse().map_err(|_| "anchor height")?;
    let ok = |k: &str| observe.out.get(k).and_then(serde_json::Value::as_bool) == Some(true);
    if !ok("signatureValid") || !ok("bound") || !ok("identicalToSdkBoundReceipt") {
        return Err(format!(
            "the SDK could not verify the anchored receipt: {}",
            observe.out
        ));
    }
    if hex32(field(&observe.out, "anchorTxSender")?)? != executor.address().0 {
        return Err("the anchoring transaction's sender is not the executor".to_string());
    }
    let (h, receipt) = find_receipt(&chain, task_id, task_height, anchor_height)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("receipt not on producer")?;
    if h != anchor_height
        || receipt.executor != executor.address()
        || receipt.input_commitment != input_commitment
        || receipt.output_commitment != result_ref.output_commitment
        || !receipt.metadata.is_empty()
    {
        return Err("the anchored receipt is not the bound receipt".to_string());
    }
    if anchor_height <= task_height {
        return Err("receipt anchored before the task".to_string());
    }
    println!("  anchored at height {anchor_height}; signature valid; bound to task_id and input_commitment; executor and anchor sender are the named executor; byte-identical to signBoundReceipt");
    trace.record("ANCHOR_HEIGHT", anchor_height);
    trace.record("RECEIPT_METADATA_BYTES", receipt.metadata.len());
    cp.observe(&chain).await.map_err(|e| e.to_string())?;
    if cp.task(task_id).map(|t| t.state) != Some(TaskState::Completed) {
        return Err("control plane did not derive completion from the chain".to_string());
    }
    let again = worker.run_once(&mut cp, &mut dp, &chain).await.map_err(|e| e.to_string())?;
    if again != AttemptOutcome::Idle {
        return Err(format!("worker re-attempted a completed task: {again:?}"));
    }

    // ── Phase 9: private result retrieval ────────────────────────────
    println!("Phase 9: the owner retrieves the private result; nobody else can...");
    let get = dp
        .issue_capability(
            &client,
            &CapabilityRequest {
                task_id,
                operation: Operation::GetResult,
                resource: Some(result_ref.object),
                ttl_secs: 60,
                max_uses: 2,
            },
        )
        .map_err(|e| e.to_string())?;
    let ch = dp.issue_challenge(stranger.address());
    if dp.get_result(&Presentation::sign(get.clone(), ch, &stranger)).is_ok() {
        return Err("a stranger read the result with the owner's grant".to_string());
    }
    let forged = Capability {
        capability_id: CapabilityId(task_id),
        task_id,
        presenter: stranger.address(),
        operation: Operation::GetResult,
        resource: ObjectId(task_id),
        not_before: 0,
        not_after: u64::MAX,
        max_uses: 1,
        issuer: issuer.address(),
        issuer_signature: [0u8; 64],
    };
    let ch = dp.issue_challenge(stranger.address());
    if dp.get_result(&Presentation::sign(forged.clone(), ch, &stranger)).is_ok() {
        return Err("task_id alone retrieved the result".to_string());
    }
    let forged_fetch = Capability {
        operation: Operation::FetchInput,
        resource: input_object,
        ..forged
    };
    let ch = dp.issue_challenge(stranger.address());
    if dp.fetch_input(&Presentation::sign(forged_fetch, ch, &stranger)).is_ok() {
        return Err("task_id alone retrieved the input".to_string());
    }
    let ch = dp.issue_challenge(client.address());
    let result = dp
        .get_result(&Presentation::sign(get, ch, &client))
        .map_err(|e| e.to_string())?;
    if result.as_bytes() != expected_output.as_slice()
        || reference_output_commitment(result.as_bytes()) != receipt.output_commitment
    {
        return Err(
            "the retrieved result is not the reference transform committed in the receipt"
                .to_string(),
        );
    }
    println!("  owner retrieved {} bytes matching the anchored output_commitment; stranger and task_id-only refused", result.len());
    trace.record("AUTHORIZED_RETRIEVAL", "owner ok");
    trace.record(
        "UNAUTHORIZED_RETRIEVAL",
        "stranger refused; task_id-only refused (input and result)",
    );

    // ── Phase 10: privacy scan of everything chain-visible ───────────
    println!("Phase 10: scanning every block on every node, as SCALE bytes and as RPC JSON...");
    let needles: Vec<Vec<u8>> =
        forms(&private_input).into_iter().chain(forms(&expected_output)).collect();
    let mut scanned = 0u64;
    for (i, c) in std::iter::once(&chain).chain(followers.iter()).enumerate() {
        let latest = c.latest_height().await.map_err(|e| e.to_string())?;
        for h in 0..=latest {
            let Some(block): Option<Block> =
                c.block_by_height(h).await.map_err(|e| e.to_string())?
            else {
                continue;
            };
            let scale = block.encode();
            let json = serde_json::to_vec(&block).map_err(|e| e.to_string())?;
            for n in &needles {
                if contains(&scale, n) || contains(&json, n) {
                    return Err(format!(
                        "private bytes found in block {h} on {}",
                        NODES[i].name
                    ));
                }
            }
            scanned += 1;
        }
    }
    if contains(&task.execution_spec, &private_input)
        || contains(&receipt.metadata, &private_input)
        || contains(&receipt.metadata, &expected_output)
    {
        return Err("private bytes in execution_spec or receipt metadata".to_string());
    }
    println!("  {scanned} blocks scanned across 3 nodes: no private input or output bytes in any form; execution_spec is the public profile tag; receipt metadata is empty");
    trace.record("BLOCKS_SCANNED", scanned);

    // ── Phase 11: control-plane restart ──────────────────────────────
    println!("Phase 11: the control plane restarts from its durable snapshot...");
    let snap = cp.snapshot();
    let mut cp2 = ControlPlane::restore(
        snap,
        Arc::clone(&clock),
        IdSource::new(ids.next("cp2")),
        issuer.clone(),
        ControlPlaneConfig {
            lease_secs: LEASE_SECS,
            session_secs: 300,
            confirmation_depth: 1,
            capability_secs: 60,
        },
    );
    cp2.observe(&chain).await.map_err(|e| e.to_string())?;
    let r2 = cp2.task(task_id).ok_or("task lost across restart")?;
    if r2.task.executor != executor.address()
        || r2.state != TaskState::Completed
        || r2.attempt_count != rec.attempt_count
    {
        return Err(
            "restart changed the executor, the completion or the attempt count".to_string(),
        );
    }
    let mut w3 = Worker::new(&mut ids, executor.clone(), Box::new(ReverseBytesProfile));
    if w3.run_once(&mut cp2, &mut dp, &chain).await.map_err(|e| e.to_string())?
        != AttemptOutcome::Idle
    {
        return Err("a restarted control plane re-offered a completed task".to_string());
    }
    wait_blocks(&chain, 2).await?;
    let n = receipts_for(&chain, task_id).await?.len();
    if n != 1 {
        return Err(format!("{n} receipts on-chain for the task"));
    }
    println!("  executor unchanged, completion reconstructed from the chain, no reassignment, still exactly one receipt");

    // ── Phase 12: node restart ───────────────────────────────────────
    println!("Phase 12: follower-a restarts on its existing schema-3 directory...");
    let idx = 1;
    nodes[idx].child.kill().await.map_err(|e| e.to_string())?;
    sleep(Duration::from_secs(1)).await;
    let dir = nodes[idx].data_dir.clone();
    let old_logs = Arc::clone(&nodes[idx].logs);
    let restarted = spawn_node(&NODES[idx], &dir, &boot).await?;
    old_logs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .extend(restarted.logs.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned());
    nodes[idx] = restarted;
    await_endpoints_ready(&http, &[endpoint(&NODES[idx])], POLL, READY_TIMEOUT).await?;
    let baseline = get_height(&http, &endpoints[0]).await?;
    let h_after =
        await_convergence(&http, &endpoints, baseline + 1, POLL, CONVERGENCE_TIMEOUT).await?;
    let fa = chain_of(&NODES[idx]);
    for h in [task_height, anchor_height] {
        let a = chain.block_by_height(h).await.map_err(|e| e.to_string())?;
        let b = fa.block_by_height(h).await.map_err(|e| e.to_string())?;
        if a.is_none() || a != b {
            return Err(format!(
                "block {h} differs between the producer and the restarted follower"
            ));
        }
    }
    println!("  reopened its data directory, rejoined, converged at {h_after}; task block and anchor block identical to the producer's");
    trace.record(
        "NODE_RESTART",
        format!("follower-a reopened, converged at {h_after}"),
    );

    // ── Phase 13: data-plane restart limitation ──────────────────────
    println!(
        "Phase 13: the in-memory data plane does not survive a restart (documented limitation)..."
    );
    let dp2 = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(ids.next("dp2")));
    if dp2.result_ref(task_id, executor.address()).is_some()
        || dp2.object_state(&input_object).is_some()
    {
        return Err("a fresh in-memory data plane unexpectedly holds state".to_string());
    }
    println!("  DATA_PLANE_RESTART_SURVIVAL=NO: chain state (task, receipt) survives; private objects in the in-memory backend do not");
    trace.record("DATA_PLANE_RESTART_SURVIVAL", "NO");

    // ── Phase 14: log privacy ────────────────────────────────────────
    println!("Phase 14: scanning node, control-plane, worker, data-plane and SDK logs for payloads and secrets...");
    let mut all: Vec<u8> = Vec::new();
    let mut lines = 0usize;
    for n in &nodes {
        for l in n.logs.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            all.extend_from_slice(l.as_bytes());
            all.push(b'\n');
            lines += 1;
        }
    }
    for l in COMPONENT_LOG.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        all.extend_from_slice(l.as_bytes());
        all.push(b'\n');
        lines += 1;
    }
    for s in [&submit.stderr, &observe.stderr] {
        all.extend_from_slice(s.as_bytes());
    }
    all.extend_from_slice(submit.out.to_string().as_bytes());
    all.extend_from_slice(observe.out.to_string().as_bytes());
    let mut secrets: Vec<Vec<u8>> = needles.clone();
    for seed in [
        [0xAAu8; 32],
        [0xE1u8; 32],
        [0xE9u8; 32],
        [0xC0u8; 32],
        [0x5Eu8; 32],
    ] {
        secrets.extend(forms(&seed));
    }
    for (i, s) in secrets.iter().enumerate() {
        if contains(&all, s) {
            return Err(format!("logs carry private material (needle #{i})"));
        }
    }
    println!("  {lines} log lines plus SDK output: no private input, private output, or key seed in raw, hex or JSON form");
    trace.record("LOG_LINES_SCANNED", lines);

    for n in &mut nodes {
        let _ = n.child.kill().await;
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    if log::set_logger(&SINK).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let base = std::env::temp_dir().join("mbongo_compute_vertical");
    println!("=== Mbongo Compute v0.4 Vertical Harness ===");
    println!("authority: {PROTOCOL_AUTHORITY} · {RPC_AUTHORITY} · storage schema {STORAGE_SCHEMA} · reference worker and profile are non-consensus\n");
    let mut trace = Trace::default();
    let result = run(&base, &mut trace).await;
    let _ = std::fs::remove_dir_all(&base);
    println!("\nVERTICAL_TRACE");
    for l in &trace.lines {
        println!("  {l}");
    }
    match result {
        Ok(()) => {
            println!("\nMBONGO COMPUTE V0.4 VERTICAL: PASS");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("\nMBONGO COMPUTE V0.4 VERTICAL: FAIL\n  Error: {e}");
            std::process::exit(1);
        }
    }
}
