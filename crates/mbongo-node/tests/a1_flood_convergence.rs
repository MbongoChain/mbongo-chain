//! S3 A1: an honest follower keeps up with the producer while a peer floods it.
//!
//! A producer and a follower run as real `mbongo-node` processes. A third peer,
//! in this process, connects to the follower and, for `FLOOD_SECS`, sends it
//! block announcements as fast as its queue allows (invalid blocks, some at
//! the next height, some far ahead) and full-range block requests, and never
//! answers anything. The follower must still reach the producer's height and
//! tip, during the flood and after it.
//!
//! The flooding peer uses only the P2P API that `dev` already had, so the same
//! test runs against a `dev` build for comparison.
//!
//! Ignored by default: it needs the `mbongo-node` binary built first.
//!
//! ```bash
//! cargo build -p mbongo-node
//! cargo test -p mbongo-node --test a1_flood_convergence -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use mbongo_core::{Block, BlockBody, BlockHeader, Hash};
use mbongo_network::{BlockBroadcaster, P2PNode, SyncCommand};
use reqwest::Client;
use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::time::{sleep, Instant};

const BLOCK_TIME_SECS: u64 = 1;
const FLOOD_SECS: u64 = 30;
/// Longest wait for the follower to match the producer's height and tip.
const CONVERGE_SECS: u64 = 30;

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers them.
const PRODUCER_RPC: u16 = 29954;
const PRODUCER_REST: u16 = 28090;
const PRODUCER_P2P: u16 = 28343;
const FOLLOWER_RPC: u16 = 29955;
const FOLLOWER_REST: u16 = 28091;
const FOLLOWER_P2P: u16 = 28344;

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

/// Height and tip hash, read in that order.
async fn tip(client: &Client, port: u16) -> Result<(u64, String), String> {
    let height = rpc(client, port, "get_block_height")
        .await?
        .as_u64()
        .ok_or("height is not a number")?;
    let hash = rpc(client, port, "get_latest_block_hash")
        .await?
        .as_str()
        .ok_or("hash is not a string")?
        .to_string();
    Ok((height, hash))
}

async fn wait_for_rpc(client: &Client, port: u16) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while rpc(client, port, "ping").await.is_err() {
        assert!(Instant::now() < deadline, "RPC on {port} never came up");
        sleep(Duration::from_millis(200)).await;
    }
}

fn node_binary() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap();
    dir.join(if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    })
}

fn spawn(
    rpc_port: u16,
    rest: u16,
    p2p: u16,
    data_dir: &std::path::Path,
    producer: bool,
    bootnode: Option<&str>,
) -> Child {
    let mut cmd = Command::new(node_binary());
    cmd.args(["--rpc-port", &rpc_port.to_string()])
        .args(["--rest-port", &rest.to_string()])
        .args(["--p2p-port", &p2p.to_string()])
        .args(["--data-dir", data_dir.to_str().unwrap()])
        .arg("--no-mdns");
    if producer {
        cmd.args(["--producer", "--block-time", &BLOCK_TIME_SECS.to_string()]);
    }
    if let Some(addr) = bootnode {
        cmd.args(["--bootnodes", addr]);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
    cmd.spawn().expect("spawn mbongo-node")
}

async fn peer_address(child: &mut Child, p2p: u16) -> String {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let line = tokio::time::timeout_at(deadline, lines.next_line())
            .await
            .expect("no PeerId in time")
            .unwrap()
            .expect("stdout closed");
        if let Some(rest) = line.strip_prefix("  PeerId:") {
            // Keep reading for the node's whole life: a node whose stdout is
            // closed panics on its next line of output and exits.
            tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
            return format!("/ip4/127.0.0.1/tcp/{p2p}/p2p/{}", rest.trim());
        }
    }
}

/// A process's resident memory in KiB, as the operating system reports it.
async fn resident_kib(pid: u32) -> Option<u64> {
    if cfg!(windows) {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .output()
            .await
            .ok()?;
        // "name","pid","session","number","12,345 K": the last field, in KiB.
        let text = String::from_utf8_lossy(&out.stdout);
        let field = text.trim().rsplit("\",\"").next()?;
        field.chars().filter(char::is_ascii_digit).collect::<String>().parse().ok()
    } else {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        status
            .lines()
            .find_map(|line| line.strip_prefix("VmRSS:"))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    }
}

/// Stops a node and waits for it, so its ports and files are released.
async fn stop(mut child: Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

fn junk(height: u64, salt: u64) -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash([u8::try_from(salt % 251).unwrap(); 32]),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: salt,
            height,
        },
        body: BlockBody {
            transactions: vec![],
        },
    }
}

/// Waits until the follower has the producer's height and tip.
async fn converged(client: &Client, phase: &str) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(CONVERGE_SECS);
    let mut last = String::new();
    loop {
        if let (Ok(p), Ok(f)) = (
            tip(client, PRODUCER_RPC).await,
            tip(client, FOLLOWER_RPC).await,
        ) {
            if p == f && p.0 > 0 {
                println!("{phase}: converged at height {} ({})", p.0, p.1);
                return p.0;
            }
            last = format!("producer {} follower {}", p.0, f.0);
        }
        assert!(Instant::now() < deadline, "{phase}: no convergence; {last}");
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore] // Needs the mbongo-node binary: cargo build -p mbongo-node
async fn a_follower_converges_while_a_peer_floods_it() {
    let client = Client::new();
    let base = std::env::temp_dir().join(format!("mbongo_a1_flood_{}", std::process::id()));
    let (producer_dir, follower_dir) = (base.join("producer"), base.join("follower"));

    let mut producer = spawn(
        PRODUCER_RPC,
        PRODUCER_REST,
        PRODUCER_P2P,
        &producer_dir,
        true,
        None,
    );
    let producer_addr = peer_address(&mut producer, PRODUCER_P2P).await;
    wait_for_rpc(&client, PRODUCER_RPC).await;
    let mut follower = spawn(
        FOLLOWER_RPC,
        FOLLOWER_REST,
        FOLLOWER_P2P,
        &follower_dir,
        false,
        Some(&producer_addr),
    );
    let follower_addr = peer_address(&mut follower, FOLLOWER_P2P).await;
    wait_for_rpc(&client, FOLLOWER_RPC).await;
    let before = converged(&client, "before the flood").await;

    // The flooding peer: answers nothing, never reads what it is sent.
    let mut flooder = P2PNode::with_mdns(false).unwrap();
    let _unread_requests = flooder.take_sync_rx().unwrap();
    let _unread_blocks = flooder.take_block_rx().unwrap();
    let _unread_events = flooder.take_sync_event_rx().unwrap();
    let target: libp2p::PeerId = follower_addr.rsplit('/').next().unwrap().parse().unwrap();
    flooder.dial(&follower_addr).unwrap();
    let announce = flooder.broadcaster();
    let ask = flooder.sync_commander();
    let flood_counters = flooder.counters();
    tokio::spawn(flooder.run());

    // The flood runs on its own task, so it goes on while convergence is read.
    let flood = tokio::spawn(async move {
        let started = Instant::now();
        let (mut announced, mut asked) = (0u64, 0u64);
        while started.elapsed() < Duration::from_secs(FLOOD_SECS) {
            for _ in 0..64 {
                announced += 1;
                let height = if announced % 2 == 0 {
                    before + 1 + announced / 64
                } else {
                    u64::MAX - announced
                };
                announce.broadcast(junk(height, announced));
                asked += 1;
                let start = announced % 1000;
                let _ = ask.send(SyncCommand::GetBlocks {
                    peer_id: target,
                    start_height: start,
                    end_height: start + 256,
                });
            }
            sleep(Duration::from_millis(5)).await;
        }
        (announced, asked)
    });

    // The follower's resident memory, sampled while the flood runs: an
    // observation printed for comparison, not a bound and not asserted.
    let pid = follower.id().expect("the follower has a pid");
    let resident_before = resident_kib(pid).await;
    let sampler = tokio::spawn(async move {
        let mut peak = 0;
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(FLOOD_SECS) {
            peak = peak.max(resident_kib(pid).await.unwrap_or(0));
            sleep(Duration::from_millis(250)).await;
        }
        peak
    });

    sleep(Duration::from_secs(FLOOD_SECS / 2)).await;
    assert!(!flood.is_finished(), "the flood is still running");
    let during = converged(&client, "during the flood").await;
    assert!(!flood.is_finished(), "converged while the flood still ran");
    let (announced, asked) = flood.await.unwrap();
    println!("flood: {announced} announcements and {asked} block requests queued for sending");
    let resident_peak = sampler.await.unwrap();
    println!(
        "follower resident memory (observed): before {resident_before:?} KiB, \
         peak during the flood {resident_peak} KiB, after {:?} KiB",
        resident_kib(pid).await
    );
    // A1 only (not on dev): proof the flood reached the follower. Each
    // acknowledgement is an announcement the follower received and admitted.
    // Failures mix the follower's refusals with the flooder's own stream cap,
    // so they prove nothing about the follower and are only printed;
    // refusals by the receiving node are proven in `a1_limits`.
    let reached = flood_counters.snapshot();
    println!("flooder counters: {reached}");
    assert!(
        reached.announcements_acknowledged > 100,
        "the flood did not reach the follower: {reached}"
    );
    assert!(
        during > before,
        "the follower made progress during the flood"
    );
    assert!(
        follower.try_wait().unwrap().is_none(),
        "the follower is still running"
    );
    let after = converged(&client, "after the flood").await;
    assert!(after > during);

    stop(follower).await;
    stop(producer).await;
    let _ = std::fs::remove_dir_all(&base);
}
