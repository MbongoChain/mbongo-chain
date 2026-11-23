//! P2P chain binding between real `mbongo-node` processes (RFC 0006 §7, S1-9).
//!
//! A producer and a follower run the dev network; a third node runs another
//! network (its own genesis file) and names the producer as its bootnode.
//!
//! - **Same network:** the follower reaches the producer's height and tip.
//! - **Another network:** the third node connects to the producer, and both
//!   chain-bound protocols fail at negotiation, in both directions; it never
//!   receives a block, so its height stays at its genesis. Its log shows the
//!   negotiation failures and none of the lines a received block produces
//!   (no gap, no synced block, no rejected block): the rejection happens
//!   before any block is decoded, not later in consensus.
//! - **Restarts:** the follower restarted on its own network catches up; the
//!   same node identity restarted on the other network's genesis is refused at
//!   negotiation; after a producer outage the follower catches up again.
//!
//! Nodes log at `info`, and `mbongo_network` at `debug`, where libp2p's
//! failure reasons are printed.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::time::{sleep, Instant};

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers them.
const PRODUCER_RPC: u16 = 25944;
const PRODUCER_REST: u16 = 25080;
const PRODUCER_P2P: u16 = 25333;
const FOLLOWER_RPC: u16 = 25945;
const FOLLOWER_REST: u16 = 25081;
const FOLLOWER_P2P: u16 = 25334;
const FOREIGN_RPC: u16 = 25946;
const FOREIGN_REST: u16 = 25082;
const FOREIGN_P2P: u16 = 25335;

/// Another network: same authority as the dev network (the dev key), its own
/// allocation, name and timestamp, so its own chain id.
const OTHER_GENESIS: &str = "{\"allocations\":[{\"address\":\"7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e\",\"amount\":\"5\"}],\"authorities\":[\"b1c2c08e2e372f831ca40e2a6e442ddc18f1dc1b6e0b39a0a32b41d6e5541c15\"],\"name\":\"private-1\",\"schema\":\"mbongo-genesis:v1\",\"timestamp\":7}";

const WAIT: Duration = Duration::from_secs(60);

/// libp2p's text for an outbound request the remote cannot negotiate.
const NEGOTIATION_REFUSED: &str = "The remote supports none of the requested protocols";

#[derive(Clone, Copy)]
struct Ports {
    rpc: u16,
    rest: u16,
    p2p: u16,
}

const PRODUCER: Ports = Ports {
    rpc: PRODUCER_RPC,
    rest: PRODUCER_REST,
    p2p: PRODUCER_P2P,
};
const FOLLOWER: Ports = Ports {
    rpc: FOLLOWER_RPC,
    rest: FOLLOWER_REST,
    p2p: FOLLOWER_P2P,
};
const FOREIGN: Ports = Ports {
    rpc: FOREIGN_RPC,
    rest: FOREIGN_REST,
    p2p: FOREIGN_P2P,
};

enum Network<'a> {
    Dev,
    File(&'a Path),
}

/// A running node, with every line it has written so far.
struct Running {
    child: Child,
    peer: String,
    ports: Ports,
    lines: Arc<Mutex<Vec<String>>>,
}

impl Running {
    fn address(&self) -> String {
        format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", self.ports.p2p, self.peer)
    }

    fn logged(&self, text: &str) -> bool {
        self.lines.lock().unwrap().iter().any(|l| l.contains(text))
    }

    fn lines_with(&self, text: &str) -> Vec<String> {
        let lines = self.lines.lock().unwrap();
        lines.iter().filter(|l| l.contains(text)).cloned().collect()
    }

    async fn until_logged(&self, text: &str) {
        let deadline = Instant::now() + WAIT;
        while !self.logged(text) {
            assert!(Instant::now() < deadline, "never logged: {text}");
            sleep(Duration::from_millis(100)).await;
        }
    }

    async fn stop(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

fn collect<R: AsyncRead + Unpin + Send + 'static>(stream: R, lines: Arc<Mutex<Vec<String>>>) {
    tokio::spawn(async move {
        // Read for the node's whole life: a node whose output is closed
        // panics on its next line and exits.
        let mut reader = BufReader::new(stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            lines.lock().unwrap().push(line);
        }
    });
}

fn write_key(dir: &Path, name: &str, seed: u8) -> PathBuf {
    let path = dir.join(format!("{name}.key"));
    std::fs::write(&path, hex::encode([seed; 32])).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    path
}

async fn launch(
    network: &Network<'_>,
    ports: Ports,
    data: &Path,
    key: &Path,
    producer: bool,
    bootnode: Option<&str>,
) -> Running {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mbongo-node"));
    match network {
        Network::Dev => cmd.arg("--dev"),
        Network::File(path) => cmd.arg("--genesis").arg(path),
    };
    cmd.args(["--rpc-port", &ports.rpc.to_string()])
        .args(["--rest-port", &ports.rest.to_string()])
        .args(["--p2p-port", &ports.p2p.to_string()])
        .args(["--p2p-host", "127.0.0.1", "--no-mdns"])
        .arg("--node-key-file")
        .arg(key)
        .arg("--data-dir")
        .arg(data)
        .env("RUST_LOG", "info,mbongo_network=debug")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if producer {
        cmd.args(["--producer", "--block-time", "1"]);
    }
    if let Some(addr) = bootnode {
        cmd.args(["--bootnodes", addr]);
    }
    let mut child = cmd.spawn().expect("spawn mbongo-node");
    let lines = Arc::new(Mutex::new(Vec::new()));
    collect(child.stdout.take().unwrap(), Arc::clone(&lines));
    collect(child.stderr.take().unwrap(), Arc::clone(&lines));
    let deadline = Instant::now() + WAIT;
    let peer = loop {
        let found = lines
            .lock()
            .unwrap()
            .iter()
            .find_map(|l| l.strip_prefix("  PeerId:").map(|p| p.trim().to_string()));
        if let Some(peer) = found {
            break peer;
        }
        assert!(
            Instant::now() < deadline,
            "no PeerId: {:?}",
            lines.lock().unwrap()
        );
        sleep(Duration::from_millis(50)).await;
    };
    let running = Running {
        child,
        peer,
        ports,
        lines,
    };
    let client = Client::new();
    while rpc(&client, ports.rpc, "ping").await.is_err() {
        assert!(
            Instant::now() < deadline,
            "RPC on {} never came up",
            ports.rpc
        );
        sleep(Duration::from_millis(100)).await;
    }
    running
}

async fn rpc(client: &Client, port: u16, method: &str) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{port}/rpc");
    let body = json!({ "jsonrpc": "2.0", "method": method, "id": 1 });
    let resp = client
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("{url}: {e}"))?;
    let value: Value = resp.json().await.map_err(|e| format!("{url}: {e}"))?;
    if let Some(err) = value.get("error") {
        return Err(format!("RPC error: {err}"));
    }
    value.get("result").cloned().ok_or_else(|| "no result".to_string())
}

/// Height and tip hash.
async fn tip(client: &Client, port: u16) -> (u64, String) {
    let height = rpc(client, port, "get_block_height").await.unwrap().as_u64().unwrap();
    let hash = rpc(client, port, "get_latest_block_hash").await.unwrap();
    (height, hash.as_str().unwrap().to_string())
}

/// Waits until the producer is past `height`.
async fn produced_past(client: &Client, height: u64) -> u64 {
    let deadline = Instant::now() + WAIT;
    loop {
        let (h, _) = tip(client, PRODUCER_RPC).await;
        if h > height {
            return h;
        }
        assert!(Instant::now() < deadline, "the producer stopped at {h}");
        sleep(Duration::from_millis(100)).await;
    }
}

/// Waits until the node on `port` has the producer's height and tip, past
/// `above`.
async fn converged(client: &Client, port: u16, above: u64, phase: &str) -> u64 {
    let deadline = Instant::now() + WAIT;
    loop {
        let (p, f) = (tip(client, PRODUCER_RPC).await, tip(client, port).await);
        if p == f && p.0 > above {
            println!("{phase}: converged at height {} ({})", p.0, p.1);
            return p.0;
        }
        let last = format!("producer {} follower {}", p.0, f.0);
        assert!(Instant::now() < deadline, "{phase}: no convergence; {last}");
        sleep(Duration::from_millis(100)).await;
    }
}

/// The node on another network: connected to the producer, refused at
/// negotiation, and never handed a block.
async fn refused_at_negotiation(
    node: &Running,
    producer: &Running,
    client: &Client,
    genesis: &(u64, String),
    phase: &str,
) {
    node.until_logged(&format!("Peer connected: {}", producer.peer)).await;
    node.until_logged(&format!(
        "Sync outbound failure to {}: {NEGOTIATION_REFUSED}",
        producer.peer
    ))
    .await;
    // Blocks keep coming on the producer's network meanwhile.
    let (h, _) = tip(client, PRODUCER_RPC).await;
    produced_past(client, h + 2).await;
    assert_eq!(
        &tip(client, node.ports.rpc).await,
        genesis,
        "{phase}: height moved"
    );
    for line in [
        "Gap detected",
        "Received ",
        "Applied synced block",
        "Failed to apply synced block",
        &format!("Peer {} height=", producer.peer),
    ] {
        assert!(
            !node.logged(line),
            "{phase}: a block or an answer crossed networks: {:?}",
            node.lines_with(line)
        );
    }
    println!(
        "{phase}: refused at negotiation, height still {}: {:?}",
        genesis.0,
        node.lines_with(NEGOTIATION_REFUSED).first()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nodes_sync_only_within_their_network() {
    let dir = std::env::temp_dir().join(format!("mbongo-s1-9-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let other = dir.join("other.json");
    std::fs::write(&other, OTHER_GENESIS).unwrap();
    let dev_id = mbongo_core::Genesis::dev().chain_id;
    let other_id = mbongo_core::chain_id(OTHER_GENESIS.as_bytes());
    assert_ne!(
        mbongo_network::chain_prefix(&dev_id),
        mbongo_network::chain_prefix(&other_id),
        "the two networks must differ in their identifiers"
    );
    let (producer_key, follower_key, foreign_key) = (
        write_key(&dir, "producer", 1),
        write_key(&dir, "follower", 2),
        write_key(&dir, "foreign", 3),
    );
    let client = Client::new();

    // ── Same network, and another one ───────────────────────────────────
    let mut producer = launch(
        &Network::Dev,
        PRODUCER,
        &dir.join("producer"),
        &producer_key,
        true,
        None,
    )
    .await;
    let boot = producer.address();
    let follower = launch(
        &Network::Dev,
        FOLLOWER,
        &dir.join("follower"),
        &follower_key,
        false,
        Some(&boot),
    )
    .await;
    let foreign = launch(
        &Network::File(&other),
        FOREIGN,
        &dir.join("foreign"),
        &foreign_key,
        false,
        Some(&boot),
    )
    .await;
    let foreign_genesis = tip(&client, FOREIGN_RPC).await;
    assert_eq!(foreign_genesis.0, 0);

    let height = converged(&client, FOLLOWER_RPC, 2, "same network").await;
    refused_at_negotiation(
        &foreign,
        &producer,
        &client,
        &foreign_genesis,
        "another network",
    )
    .await;
    // The producer's announcements to it fail the same way.
    producer
        .until_logged(&format!(
            "Block notify outbound failure to {}: {NEGOTIATION_REFUSED}",
            foreign.peer
        ))
        .await;

    // ── The follower restarts on its own network ────────────────────────
    follower.stop().await;
    produced_past(&client, height + 2).await;
    let follower = launch(
        &Network::Dev,
        FOLLOWER,
        &dir.join("follower"),
        &follower_key,
        false,
        Some(&boot),
    )
    .await;
    let height = converged(&client, FOLLOWER_RPC, height + 2, "follower restarted").await;

    // ── The same identity restarts on the other network ─────────────────
    follower.stop().await;
    let moved = launch(
        &Network::File(&other),
        FOLLOWER,
        &dir.join("follower-other"),
        &follower_key,
        false,
        Some(&boot),
    )
    .await;
    let moved_genesis = tip(&client, FOLLOWER_RPC).await;
    assert_eq!(
        moved_genesis, foreign_genesis,
        "the other network's genesis"
    );
    refused_at_negotiation(
        &moved,
        &producer,
        &client,
        &moved_genesis,
        "restarted on another genesis",
    )
    .await;
    moved.stop().await;
    let follower = launch(
        &Network::Dev,
        FOLLOWER,
        &dir.join("follower"),
        &follower_key,
        false,
        Some(&boot),
    )
    .await;
    let height = converged(&client, FOLLOWER_RPC, height, "back on its own network").await;

    // ── Producer outage ─────────────────────────────────────────────────
    producer.stop().await;
    sleep(Duration::from_secs(3)).await;
    let (stalled, _) = tip(&client, FOLLOWER_RPC).await;
    assert!(
        stalled >= height,
        "the follower lost blocks: {stalled} < {height}"
    );
    producer = launch(
        &Network::Dev,
        PRODUCER,
        &dir.join("producer"),
        &producer_key,
        true,
        None,
    )
    .await;
    assert_eq!(producer.address(), boot, "same identity, same address");
    converged(
        &client,
        FOLLOWER_RPC,
        stalled + 1,
        "after the producer outage",
    )
    .await;

    // The node on the other network never moved.
    assert_eq!(tip(&client, FOREIGN_RPC).await, foreign_genesis);

    follower.stop().await;
    foreign.stop().await;
    producer.stop().await;
    let _ = std::fs::remove_dir_all(&dir);
}
