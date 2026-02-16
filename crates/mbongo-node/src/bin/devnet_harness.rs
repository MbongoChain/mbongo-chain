//! Devnet convergence harness.
//!
//! Spawns a 3-node devnet (1 producer + 2 followers), waits for block
//! production and broadcast, then validates that all nodes converge to the
//! same height and tip hash. Also tests restart scenarios.
//!
//! This is an **external orchestration tool** — it does NOT embed any
//! consensus, storage, or protocol logic. It launches `mbongo-node` child
//! processes and queries them via JSON-RPC over HTTP.

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::time::sleep;

// ── Configuration ───────────────────────────────────────────────────────

const BLOCK_TIME_SECS: u64 = 1;
const WAIT_INITIAL_SECS: u64 = 15;
const WAIT_RESTART_SECS: u64 = 8;
const MIN_HEIGHT: u64 = 5;

// Explicit port constants — same values used for spawning AND probing.
// CLI flags verified against main.rs Args struct:
//   --rpc-port   (clap field: rpc_port,  default 9944)
//   --rest-port  (clap field: rest_port, default 8080)
//   --p2p-port   (clap field: p2p_port,  default 30333)
//   --block-time (clap field: block_time, default 5)
//   --data-dir   (clap field: data_dir,  default "data")
//   --producer   (clap field: producer,  flag)
//   --bootnodes  (clap field: bootnodes, multi-value)
const PRODUCER_RPC: u16 = 19944;
const PRODUCER_REST: u16 = 18080;
const PRODUCER_P2P: u16 = 30333;
const FOLLOWER1_RPC: u16 = 19945;
const FOLLOWER1_REST: u16 = 18081;
const FOLLOWER1_P2P: u16 = 30334;
const FOLLOWER2_RPC: u16 = 19946;
const FOLLOWER2_REST: u16 = 18082;
const FOLLOWER2_P2P: u16 = 30335;

// RPC readiness probe: max 50 attempts × 200ms = 10s total.
const RPC_PROBE_MAX_ATTEMPTS: u32 = 50;
const RPC_PROBE_INTERVAL_MS: u64 = 200;

// Initial delay before first RPC probe to let the OS bind ports.
const RPC_STARTUP_DELAY_SECS: u64 = 2;

/// Shared ring-buffer size for stdout/stderr capture.  Kept behind
/// `Arc<Mutex<_>>` so drain tasks and the main harness can both access it.
const OUTPUT_RING_SIZE: usize = 50;

struct NodeConfig {
    name: &'static str,
    rpc_port: u16,
    rest_port: u16,
    p2p_port: u16,
    producer: bool,
    bootnodes: Vec<String>,
    data_dir: PathBuf,
}

/// A running child process together with its captured stdout/stderr ring
/// buffers and the command-line that was used to spawn it (for diagnostics).
struct ManagedChild {
    child: Child,
    stdout_ring: Arc<Mutex<Vec<String>>>,
    stderr_ring: Arc<Mutex<Vec<String>>>,
    cmdline: String,
}

// ── Pre-flight port check ───────────────────────────────────────────────

/// Try to bind a TCP listener on `127.0.0.1:port`.  If it succeeds the
/// port is free (the listener is dropped immediately).  If it fails, the
/// port is already in use and we return an error with a clear message.
fn assert_port_free(port: u16, label: &str) -> Result<(), String> {
    match TcpListener::bind(("127.0.0.1", port)) {
        Ok(_listener) => Ok(()), // dropped → port released
        Err(e) => Err(format!("port {port} ({label}) is already in use: {e}")),
    }
}

/// Verify that every port used by a `NodeConfig` is free.
fn preflight_ports(config: &NodeConfig) -> Result<(), String> {
    assert_port_free(config.rpc_port, &format!("{} rpc", config.name))?;
    assert_port_free(config.rest_port, &format!("{} rest", config.name))?;
    assert_port_free(config.p2p_port, &format!("{} p2p", config.name))?;
    Ok(())
}

// ── RPC helpers ─────────────────────────────────────────────────────────

async fn rpc_call(client: &Client, port: u16, method: &str) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}/rpc");
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "id": 1
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP request to {url} failed: {e}"))?;

    let json: Value =
        resp.json().await.map_err(|e| format!("failed to parse JSON response: {e}"))?;

    if let Some(err) = json.get("error") {
        return Err(format!("RPC error: {err}"));
    }

    json.get("result")
        .cloned()
        .ok_or_else(|| "missing 'result' in RPC response".to_string())
}

async fn get_height(client: &Client, port: u16) -> Result<u64, String> {
    let result = rpc_call(client, port, "get_block_height").await?;
    result.as_u64().ok_or_else(|| format!("expected u64 height, got: {result}"))
}

async fn get_tip_hash(client: &Client, port: u16) -> Result<String, String> {
    let result = rpc_call(client, port, "get_latest_block_hash").await?;
    result
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("expected string hash, got: {result}"))
}

/// Wait until the node's RPC port is reachable.
///
/// Uses a fixed retry loop: an initial delay to let the OS bind ports,
/// then up to `RPC_PROBE_MAX_ATTEMPTS` pings with `RPC_PROBE_INTERVAL_MS`
/// between each attempt. Total wait ≈ startup delay + 10 seconds.
///
/// On every failed attempt, checks whether the child process has already
/// exited.  If it has, fails immediately with exit code + last stderr
/// lines instead of waiting for the remaining attempts.
async fn wait_for_rpc(client: &Client, managed: &mut ManagedChild) -> Result<(), String> {
    let port = extract_rpc_port_from_cmdline(&managed.cmdline);

    // Initial delay — let the process bind its ports before we start probing.
    sleep(Duration::from_secs(RPC_STARTUP_DELAY_SECS)).await;

    for attempt in 1..=RPC_PROBE_MAX_ATTEMPTS {
        // Check if the child already exited before we even try.
        if let Some(status) = managed.child.try_wait().map_err(|e| format!("try_wait: {e}"))? {
            let stderr_tail = dump_stderr_ring(&managed.stderr_ring);
            let stdout_tail = dump_stdout_ring(&managed.stdout_ring);
            return Err(format!(
                "node exited early (status: {status}) before RPC was ready on port {port}\n\
                 cmdline: {}\n\
                 last stdout:\n{stdout_tail}\n\
                 last stderr:\n{stderr_tail}",
                managed.cmdline
            ));
        }

        match rpc_call(client, port, "ping").await {
            Ok(_) => return Ok(()),
            Err(_) => {
                if attempt == RPC_PROBE_MAX_ATTEMPTS {
                    let stderr_tail = dump_stderr_ring(&managed.stderr_ring);
                    let stdout_tail = dump_stdout_ring(&managed.stdout_ring);
                    return Err(format!(
                        "timeout waiting for RPC on port {port} after \
                         {RPC_PROBE_MAX_ATTEMPTS} attempts\n\
                         cmdline: {}\n\
                         last stdout:\n{stdout_tail}\n\
                         last stderr:\n{stderr_tail}",
                        managed.cmdline
                    ));
                }
                sleep(Duration::from_millis(RPC_PROBE_INTERVAL_MS)).await;
            }
        }
    }

    Err(format!("timeout waiting for RPC on port {port}"))
}

/// Extract the `--rpc-port` value from a stored command line string.
/// Falls back to 0 if parsing fails (should never happen).
fn extract_rpc_port_from_cmdline(cmdline: &str) -> u16 {
    cmdline
        .split_whitespace()
        .zip(cmdline.split_whitespace().skip(1))
        .find(|(flag, _)| *flag == "--rpc-port")
        .and_then(|(_, val)| val.parse().ok())
        .unwrap_or(0)
}

/// Return the last N lines from a ring buffer, formatted for display.
fn dump_ring(ring: &Arc<Mutex<Vec<String>>>, empty_msg: &str) -> String {
    let lines = ring.lock().unwrap_or_else(|e| e.into_inner());
    if lines.is_empty() {
        return format!("  ({empty_msg})");
    }
    lines.iter().map(|l| format!("  | {l}")).collect::<Vec<_>>().join("\n")
}

fn dump_stderr_ring(ring: &Arc<Mutex<Vec<String>>>) -> String {
    dump_ring(ring, "no stderr captured")
}

fn dump_stdout_ring(ring: &Arc<Mutex<Vec<String>>>) -> String {
    dump_ring(ring, "no stdout captured")
}

// ── Node process management ─────────────────────────────────────────────

fn node_binary_path() -> PathBuf {
    // Find the mbongo-node binary next to our own executable.
    let self_exe = std::env::current_exe().expect("cannot determine own executable path");
    let dir = self_exe.parent().expect("executable has no parent dir");
    let name = if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    };
    dir.join(name)
}

/// Spawn a node child process and start a background task that drains
/// its stderr into a ring buffer.  Returns a `ManagedChild` that owns
/// the child handle, the ring buffer, and the exact command line used.
fn spawn_node(config: &NodeConfig) -> Result<ManagedChild, String> {
    let binary = node_binary_path();

    let mut cmd = Command::new(&binary);
    cmd.arg("--rpc-port")
        .arg(config.rpc_port.to_string())
        .arg("--rest-port")
        .arg(config.rest_port.to_string())
        .arg("--p2p-port")
        .arg(config.p2p_port.to_string())
        .arg("--data-dir")
        .arg(config.data_dir.to_str().unwrap());

    if config.producer {
        cmd.arg("--producer").arg("--block-time").arg(BLOCK_TIME_SECS.to_string());
    }

    for bootnode in &config.bootnodes {
        cmd.arg("--bootnodes").arg(bootnode);
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);

    // Build the cmdline string for diagnostics BEFORE spawning.
    let cmdline = format_cmdline(&binary, config);

    let mut child = cmd.spawn().map_err(|e| format!("failed to spawn {}: {e}", config.name))?;

    // Drain stderr in a background task so the pipe buffer never fills up
    // (on Windows, a full pipe buffer blocks the child process).
    let stderr_ring: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let ring_clone = Arc::clone(&stderr_ring);
    let label = config.name.to_string();

    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[{label}] {line}");
                let mut ring = ring_clone.lock().unwrap_or_else(|e| e.into_inner());
                if ring.len() >= OUTPUT_RING_SIZE {
                    ring.remove(0);
                }
                ring.push(line);
            }
        });
    }

    // stdout is NOT drained here — it stays piped so that `extract_peer_id`
    // can read lines from it.  After PeerId extraction, `extract_peer_id`
    // spawns a background drain task (same pattern as stderr above) so
    // the pipe buffer never fills and the child never gets OS error 232.
    let stdout_ring: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    Ok(ManagedChild {
        child,
        stdout_ring,
        stderr_ring,
        cmdline,
    })
}

/// Build a human-readable command line string from binary path + config.
fn format_cmdline(binary: &std::path::Path, config: &NodeConfig) -> String {
    let mut parts = vec![
        binary.display().to_string(),
        "--rpc-port".to_string(),
        config.rpc_port.to_string(),
        "--rest-port".to_string(),
        config.rest_port.to_string(),
        "--p2p-port".to_string(),
        config.p2p_port.to_string(),
        "--data-dir".to_string(),
        config.data_dir.display().to_string(),
    ];
    if config.producer {
        parts.push("--producer".to_string());
        parts.push("--block-time".to_string());
        parts.push(BLOCK_TIME_SECS.to_string());
    }
    for bootnode in &config.bootnodes {
        parts.push("--bootnodes".to_string());
        parts.push(bootnode.clone());
    }
    parts.join(" ")
}

/// Read stdout lines from a child process until we find the PeerId line.
/// Returns the extracted multiaddr `/ip4/127.0.0.1/tcp/{p2p_port}/p2p/{peer_id}`.
///
/// After finding the PeerId, spawns a background task to continuously
/// drain remaining stdout into `managed.stdout_ring`.  This prevents the
/// stdout pipe buffer from filling up on Windows (which would cause the
/// child to fail with "os error 232" on the next `println!`).
async fn extract_peer_id(managed: &mut ManagedChild, p2p_port: u16) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let stdout = managed.child.stdout.take().ok_or("no stdout on child process")?;
    let mut lines_reader = BufReader::new(stdout).lines();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while let Ok(Some(line)) =
        tokio::time::timeout_at(deadline, lines_reader.next_line()).await.map_err(|_| {
            let stderr_tail = dump_stderr_ring(&managed.stderr_ring);
            format!(
                "timeout reading PeerId\n\
                 cmdline: {}\n\
                 last stderr:\n{stderr_tail}",
                managed.cmdline
            )
        })?
    {
        // The node prints: "  PeerId:   12D3KooW..."
        if let Some(rest) = line.strip_prefix("  PeerId:") {
            let peer_id = rest.trim().to_string();

            // Spawn a background task to drain remaining stdout so the
            // pipe buffer never fills up (prevents OS error 232 on Windows).
            let ring_clone = Arc::clone(&managed.stdout_ring);
            let label = extract_node_name_from_cmdline(&managed.cmdline);
            tokio::spawn(async move {
                while let Ok(Some(stdout_line)) = lines_reader.next_line().await {
                    eprintln!("[{label}:stdout] {stdout_line}");
                    let mut ring = ring_clone.lock().unwrap_or_else(|e| e.into_inner());
                    if ring.len() >= OUTPUT_RING_SIZE {
                        ring.remove(0);
                    }
                    ring.push(stdout_line);
                }
            });

            return Ok(format!("/ip4/127.0.0.1/tcp/{p2p_port}/p2p/{peer_id}"));
        }
    }

    let stderr_tail = dump_stderr_ring(&managed.stderr_ring);
    Err(format!(
        "could not find PeerId in node stdout\n\
         cmdline: {}\n\
         last stderr:\n{stderr_tail}",
        managed.cmdline
    ))
}

/// Extract the node label (e.g. "producer") from a stored cmdline for
/// use in drain task log prefixes.  Falls back to "node" if not parseable.
fn extract_node_name_from_cmdline(cmdline: &str) -> String {
    // The data-dir ends with the node name (e.g. .../producer, .../follower_a).
    cmdline
        .split_whitespace()
        .zip(cmdline.split_whitespace().skip(1))
        .find(|(flag, _)| *flag == "--data-dir")
        .and_then(|(_, val)| val.rsplit(['/', '\\']).next())
        .unwrap_or("node")
        .to_string()
}

// ── Convergence validation ──────────────────────────────────────────────

struct ConvergenceResult {
    heights: Vec<(String, u64)>,
    hashes: Vec<(String, String)>,
}

async fn check_convergence(
    client: &Client,
    nodes: &[(&str, u16)],
) -> Result<ConvergenceResult, String> {
    let mut heights = Vec::new();
    let mut hashes = Vec::new();

    for (name, port) in nodes {
        let h = get_height(client, *port).await?;
        let hash = get_tip_hash(client, *port).await?;
        heights.push((name.to_string(), h));
        hashes.push((name.to_string(), hash));
    }

    Ok(ConvergenceResult { heights, hashes })
}

fn validate_convergence(result: &ConvergenceResult, min_height: u64) -> Result<(), String> {
    println!("  Heights:");
    for (name, h) in &result.heights {
        println!("    {name}: {h}");
    }
    println!("  Tip hashes:");
    for (name, hash) in &result.hashes {
        println!("    {name}: {hash}");
    }

    // Check minimum height.
    for (name, h) in &result.heights {
        if *h < min_height {
            return Err(format!("{name} height {h} < minimum expected {min_height}"));
        }
    }

    // Check all heights equal.
    let first_height = result.heights[0].1;
    for (name, h) in &result.heights[1..] {
        if *h != first_height {
            return Err(format!(
                "height mismatch: {} has {}, {} has {first_height}",
                name, h, result.heights[0].0
            ));
        }
    }

    // Check all hashes equal.
    let first_hash = &result.hashes[0].1;
    for (name, hash) in &result.hashes[1..] {
        if hash != first_hash {
            return Err(format!(
                "tip hash mismatch: {} has {}, {} has {first_hash}",
                name, hash, result.hashes[0].0
            ));
        }
    }

    println!("  Converged: height={first_height}, hash={first_hash}");
    Ok(())
}

// ── Cleanup ─────────────────────────────────────────────────────────────

fn cleanup_data_dirs(dirs: &[PathBuf]) {
    for dir in dirs {
        if dir.exists() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let client = Client::new();
    let temp_base = std::env::temp_dir().join("mbongo_devnet_harness");

    let data_dirs: Vec<PathBuf> = vec![
        temp_base.join("producer"),
        temp_base.join("follower_a"),
        temp_base.join("follower_b"),
    ];

    // Clean up from any previous run.
    cleanup_data_dirs(&data_dirs);

    println!("=== M2.4 Devnet Convergence Harness ===\n");

    let result = run_harness(&client, &data_dirs).await;

    // Always clean up.
    cleanup_data_dirs(&data_dirs);

    match result {
        Ok(()) => {
            println!("\nDEVNET CONVERGENCE: PASS");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("\nDEVNET CONVERGENCE: FAIL");
            eprintln!("  Error: {e}");
            std::process::exit(1);
        }
    }
}

#[allow(clippy::too_many_lines)]
async fn run_harness(client: &Client, data_dirs: &[PathBuf]) -> Result<(), String> {
    // ── Phase 1: Spawn all 3 nodes ──────────────────────────────────────

    println!("Phase 1: Spawning nodes...");

    let producer_cfg = NodeConfig {
        name: "producer",
        rpc_port: PRODUCER_RPC,
        rest_port: PRODUCER_REST,
        p2p_port: PRODUCER_P2P,
        producer: true,
        bootnodes: vec![],
        data_dir: data_dirs[0].clone(),
    };

    let follower_a_cfg = NodeConfig {
        name: "follower_a",
        rpc_port: FOLLOWER1_RPC,
        rest_port: FOLLOWER1_REST,
        p2p_port: FOLLOWER1_P2P,
        producer: false,
        bootnodes: vec![], // set after producer starts
        data_dir: data_dirs[1].clone(),
    };

    let follower_b_cfg = NodeConfig {
        name: "follower_b",
        rpc_port: FOLLOWER2_RPC,
        rest_port: FOLLOWER2_REST,
        p2p_port: FOLLOWER2_P2P,
        producer: false,
        bootnodes: vec![], // set after producer starts
        data_dir: data_dirs[2].clone(),
    };

    // Pre-flight: verify every port is free before spawning anything.
    println!("  Pre-flight port check...");
    preflight_ports(&producer_cfg)?;
    preflight_ports(&follower_a_cfg)?;
    preflight_ports(&follower_b_cfg)?;
    println!("  All ports free");

    // Spawn producer.
    let mut producer = spawn_node(&producer_cfg)?;
    let producer_multiaddr = extract_peer_id(&mut producer, producer_cfg.p2p_port).await?;
    println!("  Producer started: {producer_multiaddr}");

    // Wait for producer RPC (includes startup delay + early-exit detection).
    wait_for_rpc(client, &mut producer).await?;
    println!("  Producer RPC ready on port {PRODUCER_RPC}");

    // Now create follower configs with bootnodes pointing at producer.
    let follower_a_cfg = NodeConfig {
        bootnodes: vec![producer_multiaddr.clone()],
        ..follower_a_cfg
    };
    let follower_b_cfg = NodeConfig {
        bootnodes: vec![producer_multiaddr.clone()],
        ..follower_b_cfg
    };

    let mut follower_a = spawn_node(&follower_a_cfg)?;
    let _fa_multiaddr = extract_peer_id(&mut follower_a, follower_a_cfg.p2p_port).await?;
    println!("  Follower A started");

    let mut follower_b = spawn_node(&follower_b_cfg)?;
    let _fb_multiaddr = extract_peer_id(&mut follower_b, follower_b_cfg.p2p_port).await?;
    println!("  Follower B started");

    wait_for_rpc(client, &mut follower_a).await?;
    println!("  Follower A RPC ready on port {FOLLOWER1_RPC}");
    wait_for_rpc(client, &mut follower_b).await?;
    println!("  Follower B RPC ready on port {FOLLOWER2_RPC}");
    println!("  All RPCs ready\n");

    // ── Phase 2: Wait for convergence ───────────────────────────────────

    println!("Phase 2: Waiting {WAIT_INITIAL_SECS}s for block production...");
    sleep(Duration::from_secs(WAIT_INITIAL_SECS)).await;

    let nodes = vec![
        ("producer", PRODUCER_RPC),
        ("follower_a", FOLLOWER1_RPC),
        ("follower_b", FOLLOWER2_RPC),
    ];

    println!("Phase 2: Checking convergence...");
    let conv = check_convergence(client, &nodes).await?;
    validate_convergence(&conv, MIN_HEIGHT)?;
    println!("  Phase 2: PASS\n");

    // ── Phase 3: Restart producer ───────────────────────────────────────

    println!("Phase 3: Restart producer scenario...");
    producer.child.kill().await.map_err(|e| format!("kill producer: {e}"))?;
    println!("  Producer killed");
    sleep(Duration::from_secs(2)).await;

    let mut producer = spawn_node(&producer_cfg)?;
    let _new_multiaddr = extract_peer_id(&mut producer, producer_cfg.p2p_port).await?;
    wait_for_rpc(client, &mut producer).await?;
    println!("  Producer restarted, RPC ready on port {PRODUCER_RPC}");

    sleep(Duration::from_secs(WAIT_RESTART_SECS)).await;
    println!("Phase 3: Checking convergence...");
    let conv = check_convergence(client, &nodes).await?;
    validate_convergence(&conv, MIN_HEIGHT)?;
    println!("  Phase 3: PASS\n");

    // ── Phase 4: Restart follower ───────────────────────────────────────

    println!("Phase 4: Restart follower A scenario...");
    follower_a.child.kill().await.map_err(|e| format!("kill follower_a: {e}"))?;
    println!("  Follower A killed");
    sleep(Duration::from_secs(2)).await;

    let mut follower_a = spawn_node(&follower_a_cfg)?;
    let _fa_new = extract_peer_id(&mut follower_a, follower_a_cfg.p2p_port).await?;
    wait_for_rpc(client, &mut follower_a).await?;
    println!("  Follower A restarted, RPC ready on port {FOLLOWER1_RPC}");

    sleep(Duration::from_secs(WAIT_RESTART_SECS)).await;
    println!("Phase 4: Checking convergence...");
    let conv = check_convergence(client, &nodes).await?;
    validate_convergence(&conv, MIN_HEIGHT)?;
    println!("  Phase 4: PASS\n");

    // ── Cleanup: kill all remaining ─────────────────────────────────────

    let _ = producer.child.kill().await;
    let _ = follower_a.child.kill().await;
    let _ = follower_b.child.kill().await;

    Ok(())
}
