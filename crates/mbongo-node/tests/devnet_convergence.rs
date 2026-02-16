//! Devnet convergence integration test.
//!
//! Spawns a 3-node devnet (1 producer + 2 followers) and validates that
//! all nodes converge to the same height and tip hash.
//!
//! This test is marked `#[ignore]` because it spawns child processes and
//! requires the `mbongo-node` binary to be built first. Run explicitly:
//!
//! ```bash
//! cargo build -p mbongo-node
//! cargo test -p mbongo-node --test devnet_convergence -- --ignored
//! ```

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::time::sleep;

// ── Configuration ───────────────────────────────────────────────────────

const BLOCK_TIME_SECS: u64 = 1;
const WAIT_SECS: u64 = 15;
const MIN_HEIGHT: u64 = 5;

// Explicit port constants — same values used for spawning AND probing.
// Uses different ports from the harness binary to avoid conflicts.
const PRODUCER_RPC: u16 = 29944;
const PRODUCER_REST: u16 = 28080;
const PRODUCER_P2P: u16 = 40333;
const FOLLOWER1_RPC: u16 = 29945;
const FOLLOWER1_REST: u16 = 28081;
const FOLLOWER1_P2P: u16 = 40334;
const FOLLOWER2_RPC: u16 = 29946;
const FOLLOWER2_REST: u16 = 28082;
const FOLLOWER2_P2P: u16 = 40335;

// RPC readiness probe: max 50 attempts × 200ms = 10s total.
const RPC_PROBE_MAX_ATTEMPTS: u32 = 50;
const RPC_PROBE_INTERVAL_MS: u64 = 200;

// Initial delay before first RPC probe to let the OS bind ports.
const RPC_STARTUP_DELAY_SECS: u64 = 2;

struct NodeConfig {
    name: &'static str,
    rpc_port: u16,
    rest_port: u16,
    p2p_port: u16,
    producer: bool,
    bootnodes: Vec<String>,
    data_dir: PathBuf,
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

    let json_val: Value =
        resp.json().await.map_err(|e| format!("failed to parse JSON response: {e}"))?;

    if let Some(err) = json_val.get("error") {
        return Err(format!("RPC error: {err}"));
    }

    json_val
        .get("result")
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
async fn wait_for_rpc(client: &Client, port: u16) -> Result<(), String> {
    // Initial delay — let the process bind its ports before we start probing.
    sleep(Duration::from_secs(RPC_STARTUP_DELAY_SECS)).await;

    for attempt in 1..=RPC_PROBE_MAX_ATTEMPTS {
        match rpc_call(client, port, "ping").await {
            Ok(_) => return Ok(()),
            Err(_) => {
                if attempt == RPC_PROBE_MAX_ATTEMPTS {
                    return Err(format!(
                        "timeout waiting for RPC on port {port} after {RPC_PROBE_MAX_ATTEMPTS} attempts"
                    ));
                }
                sleep(Duration::from_millis(RPC_PROBE_INTERVAL_MS)).await;
            }
        }
    }

    Err(format!("timeout waiting for RPC on port {port}"))
}

// ── Node process management ─────────────────────────────────────────────

fn node_binary_path() -> PathBuf {
    let self_exe = std::env::current_exe().expect("cannot determine own executable path");
    let dir = self_exe.parent().unwrap().parent().unwrap(); // tests are in deps/
    let name = if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    };
    dir.join(name)
}

fn spawn_node(config: &NodeConfig) -> Result<Child, String> {
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

    cmd.spawn().map_err(|e| format!("failed to spawn {}: {e}", config.name))
}

async fn extract_peer_id(child: &mut Child, p2p_port: u16) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let stdout = child.stdout.take().ok_or("no stdout on child process")?;
    let mut reader = BufReader::new(stdout).lines();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while let Ok(Some(line)) = tokio::time::timeout_at(deadline, reader.next_line())
        .await
        .map_err(|_| "timeout reading PeerId".to_string())?
    {
        if let Some(rest) = line.strip_prefix("  PeerId:") {
            let peer_id = rest.trim().to_string();
            return Ok(format!("/ip4/127.0.0.1/tcp/{p2p_port}/p2p/{peer_id}"));
        }
    }

    Err("could not find PeerId in node stdout".to_string())
}

fn cleanup_data_dirs(dirs: &[PathBuf]) {
    for dir in dirs {
        if dir.exists() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

// ── Test ────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore] // Requires pre-built mbongo-node binary; run with: cargo test -- --ignored
async fn devnet_three_nodes_converge() {
    let client = Client::new();
    let temp_base = std::env::temp_dir().join("mbongo_devnet_test");

    let data_dirs: Vec<PathBuf> = vec![
        temp_base.join("producer"),
        temp_base.join("follower_a"),
        temp_base.join("follower_b"),
    ];

    cleanup_data_dirs(&data_dirs);

    let producer_cfg = NodeConfig {
        name: "producer",
        rpc_port: PRODUCER_RPC,
        rest_port: PRODUCER_REST,
        p2p_port: PRODUCER_P2P,
        producer: true,
        bootnodes: vec![],
        data_dir: data_dirs[0].clone(),
    };

    let mut producer = spawn_node(&producer_cfg).unwrap();
    let producer_multiaddr = extract_peer_id(&mut producer, producer_cfg.p2p_port).await.unwrap();

    wait_for_rpc(&client, PRODUCER_RPC).await.unwrap();

    let follower_a_cfg = NodeConfig {
        name: "follower_a",
        rpc_port: FOLLOWER1_RPC,
        rest_port: FOLLOWER1_REST,
        p2p_port: FOLLOWER1_P2P,
        producer: false,
        bootnodes: vec![producer_multiaddr.clone()],
        data_dir: data_dirs[1].clone(),
    };

    let follower_b_cfg = NodeConfig {
        name: "follower_b",
        rpc_port: FOLLOWER2_RPC,
        rest_port: FOLLOWER2_REST,
        p2p_port: FOLLOWER2_P2P,
        producer: false,
        bootnodes: vec![producer_multiaddr],
        data_dir: data_dirs[2].clone(),
    };

    let mut follower_a = spawn_node(&follower_a_cfg).unwrap();
    let _fa = extract_peer_id(&mut follower_a, follower_a_cfg.p2p_port).await.unwrap();
    let mut follower_b = spawn_node(&follower_b_cfg).unwrap();
    let _fb = extract_peer_id(&mut follower_b, follower_b_cfg.p2p_port).await.unwrap();

    wait_for_rpc(&client, FOLLOWER1_RPC).await.unwrap();
    wait_for_rpc(&client, FOLLOWER2_RPC).await.unwrap();

    // Wait for blocks to be produced and broadcast.
    sleep(Duration::from_secs(WAIT_SECS)).await;

    // Check convergence.
    let ports = vec![
        ("producer", PRODUCER_RPC),
        ("follower_a", FOLLOWER1_RPC),
        ("follower_b", FOLLOWER2_RPC),
    ];

    let mut heights = Vec::new();
    let mut hashes = Vec::new();

    for (name, port) in &ports {
        let h = get_height(&client, *port).await.unwrap();
        let hash = get_tip_hash(&client, *port).await.unwrap();
        eprintln!("  {name}: height={h}, hash={hash}");
        heights.push((*name, h));
        hashes.push((*name, hash));
    }

    // All heights must be >= MIN_HEIGHT.
    for (name, h) in &heights {
        assert!(
            *h >= MIN_HEIGHT,
            "{name} height {h} < minimum expected {MIN_HEIGHT}"
        );
    }

    // All heights must be equal.
    let first_h = heights[0].1;
    for (name, h) in &heights[1..] {
        assert_eq!(
            *h, first_h,
            "height mismatch: {name} has {h}, {} has {first_h}",
            heights[0].0
        );
    }

    // All tip hashes must be equal.
    let first_hash = &hashes[0].1;
    for (name, hash) in &hashes[1..] {
        assert_eq!(
            hash, first_hash,
            "tip hash mismatch: {name} has {hash}, {} has {first_hash}",
            hashes[0].0
        );
    }

    // Cleanup.
    let _ = producer.kill().await;
    let _ = follower_a.kill().await;
    let _ = follower_b.kill().await;
    cleanup_data_dirs(&data_dirs);
}
