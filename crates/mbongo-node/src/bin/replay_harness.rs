//! Deterministic replay harness.
//!
//! Proves that chain state is deterministically reproducible by:
//! 1. Running a producer node to create a chain of blocks holding a
//!    Transfer, a ComputeTask and an AnchorReceipt.
//! 2. Exporting all blocks via RPC (`get_block_by_height`), and the
//!    producer's account state via REST.
//! 3. Replaying them through the node's own `apply_block`, starting from
//!    the node's own `ensure_genesis`.
//! 4. Asserting that every block hash, the watched accounts, the task and the
//!    receipt equal the producer's; that a second replay is identical; that a
//!    RocksDB replay survives a reopen; that an altered block is refused; and
//!    that a diverged starting state is detected.
//!
//! Block production happens in a real node process. The replay runs the
//! node's block-application code, compiled into this harness from the same
//! source files (see `mod backend` below).
#![cfg_attr(test, allow(dead_code))]

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::time::sleep;

// ── The node's block-application path ───────────────────────────────────
//
// `backend.rs` and `mempool.rs` are compiled into this harness from the same
// source files as the node binary. The replay below therefore runs the code
// that applies blocks in the node, with no copy and with nothing exported
// from `mbongo-node` for it. The harness's own unit tests leave these
// modules out, so the node's unit tests still run once, in the node.
#[cfg(not(test))]
#[allow(dead_code)]
#[path = "../backend.rs"]
mod backend;
#[cfg(not(test))]
#[allow(dead_code)]
#[path = "../mempool.rs"]
mod mempool;

#[cfg(not(test))]
mod replay {
    use super::backend::{compute_block_hash as node_block_hash, NodeBackend};
    use super::mempool::MempoolConfig;
    use mbongo_core::{Address, Block, Hash};
    use mbongo_storage::{InMemoryStorage, RocksDbStorage, Storage};

    /// What a chain holds, for comparison: every block hash by height, the
    /// watched accounts, and the watched task and receipt bytes.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ChainState {
        pub block_hashes: Vec<Hash>,
        pub accounts: Vec<(Address, Option<(u128, u64)>)>,
        pub task: Option<Vec<u8>>,
        pub receipt: Option<Vec<u8>>,
    }

    /// The state the comparison reads.
    pub struct Watched {
        pub accounts: Vec<Address>,
        pub task_id: [u8; 32],
        pub receipt_task_id: [u8; 32],
    }

    /// What the replay proved, for the report.
    pub struct Summary {
        pub blocks: usize,
        pub transactions: usize,
        pub runs: usize,
    }

    fn fresh<S: Storage>(storage: S) -> NodeBackend<S> {
        NodeBackend::with_mempool_config(storage, false, MempoolConfig::default())
    }

    /// Writes genesis as the node does, and checks it is the exported one.
    fn genesis<S: Storage>(backend: &NodeBackend<S>, blocks: &[Block]) -> Result<(), String> {
        backend
            .ensure_genesis(&mbongo_core::Genesis::dev())
            .map_err(|e| format!("ensure_genesis: {e}"))?;
        let local = backend
            .storage
            .get_block_by_height(0)
            .map_err(|e| format!("storage error: {e}"))?
            .ok_or("no local genesis block")?;
        if node_block_hash(&local) != node_block_hash(&blocks[0]) {
            return Err("the local genesis differs from the exported genesis".to_string());
        }
        Ok(())
    }

    /// Applies `blocks` in order through `apply_block`.
    fn apply_all<S: Storage>(backend: &NodeBackend<S>, blocks: &[Block]) -> Result<(), String> {
        for block in blocks {
            backend.apply_block(block).map_err(|e| {
                format!(
                    "apply_block refused the block at height {}: {e}",
                    block.header.height
                )
            })?;
        }
        Ok(())
    }

    /// Replays the exported chain onto a backend: genesis, then every block.
    fn replay<S: Storage>(backend: &NodeBackend<S>, blocks: &[Block]) -> Result<(), String> {
        genesis(backend, blocks)?;
        apply_all(backend, &blocks[1..])
    }

    pub fn state<S: Storage>(backend: &NodeBackend<S>, w: &Watched) -> Result<ChainState, String> {
        let err = |e: mbongo_storage::StorageError| format!("storage error: {e}");
        let tip = backend.latest_height().map_err(|e| e.to_string())?;
        let mut block_hashes = Vec::new();
        for h in 0..=tip {
            let block = backend
                .storage
                .get_block_by_height(h)
                .map_err(err)?
                .ok_or_else(|| format!("no block at height {h}"))?;
            block_hashes.push(node_block_hash(&block));
        }
        let mut accounts = Vec::new();
        for a in &w.accounts {
            let held = backend.storage.get_account(a).map_err(err)?;
            accounts.push((*a, held.map(|acc| (acc.balance, acc.nonce))));
        }
        Ok(ChainState {
            block_hashes,
            accounts,
            task: backend.storage.get_task(&w.task_id).map_err(err)?,
            receipt: backend.storage.get_receipt(&w.receipt_task_id).map_err(err)?,
        })
    }

    /// Names the first difference between two states, if any.
    pub fn compare(replayed: &ChainState, reference: &ChainState) -> Result<(), String> {
        let (r, s) = (&replayed.block_hashes, &reference.block_hashes);
        if let Some(h) = r.iter().zip(s.iter()).position(|(a, b)| a != b) {
            return Err(format!("block hashes differ first at height {h}"));
        }
        if r.len() != s.len() {
            return Err(format!(
                "heights differ: replay {}, reference {}",
                r.len() - 1,
                s.len() - 1
            ));
        }
        for ((addr, got), (_, want)) in replayed.accounts.iter().zip(reference.accounts.iter()) {
            if got != want {
                return Err(format!(
                    "account {addr} differs: replay {got:?}, reference {want:?}"
                ));
            }
        }
        if replayed.task != reference.task {
            return Err("the committed task differs".to_string());
        }
        if replayed.receipt != reference.receipt {
            return Err("the anchored receipt differs".to_string());
        }
        Ok(())
    }

    /// Runs every replay check against the producer's reference state.
    pub fn run(
        blocks: &[Block],
        reference: &ChainState,
        watched: &Watched,
        rocks_dir: &std::path::Path,
        harness_hash: fn(&Block) -> Hash,
    ) -> Result<Summary, String> {
        // The harness's own hash function must be the node's.
        for b in blocks {
            if harness_hash(b) != node_block_hash(b) {
                return Err(format!(
                    "hash functions disagree at height {}",
                    b.header.height
                ));
            }
        }
        let transactions: usize = blocks.iter().map(|b| b.body.transactions.len()).sum();

        // 1. Replay through apply_block, then compare with the producer.
        let first = fresh(InMemoryStorage::new());
        replay(&first, blocks)?;
        let replayed = state(&first, watched)?;
        compare(&replayed, reference)?;
        println!("  Replay 1 (in memory): every block applied; state equals the producer's");

        // 2. Reproducibility: a second replay from nothing gives the same state.
        let second = fresh(InMemoryStorage::new());
        replay(&second, blocks)?;
        if state(&second, watched)? != replayed {
            return Err("two replays of the same blocks gave different states".to_string());
        }
        println!("  Replay 2 (in memory): identical to replay 1");

        // 3. Persistence: replay into RocksDB, close, reopen.
        {
            let rocks =
                fresh(RocksDbStorage::open(rocks_dir).map_err(|e| format!("rocksdb: {e}"))?);
            replay(&rocks, blocks)?;
            if state(&rocks, watched)? != replayed {
                return Err("the RocksDB replay differs from the in-memory replay".to_string());
            }
        }
        let reopened =
            fresh(RocksDbStorage::open(rocks_dir).map_err(|e| format!("rocksdb reopen: {e}"))?);
        genesis(&reopened, blocks)?;
        if state(&reopened, watched)? != replayed {
            return Err("the reopened RocksDB state differs from the replay".to_string());
        }
        let tip = blocks.last().ok_or("no blocks")?;
        if reopened.apply_block(tip).is_ok() {
            return Err("a stored block was applied a second time after reopening".to_string());
        }
        if state(&reopened, watched)? != replayed {
            return Err("refusing a stored block changed the reopened state".to_string());
        }
        drop(reopened);
        println!(
            "  Replay 3 (RocksDB): persisted, reopened, state unchanged; a stored block is refused"
        );

        // 4. An altered block is refused, and leaves no trace.
        let i = blocks
            .iter()
            .position(|b| !b.body.transactions.is_empty())
            .ok_or("no exported block holds a transaction")?;
        let partial = fresh(InMemoryStorage::new());
        genesis(&partial, blocks)?;
        apply_all(&partial, &blocks[1..i])?;
        let before = state(&partial, watched)?;
        let mut altered = blocks[i].clone();
        altered.body.transactions[0].amount ^= 1;
        if partial.apply_block(&altered).is_ok() {
            return Err(format!("an altered block at height {i} was applied"));
        }
        altered.header.transactions_root =
            mbongo_core::compute_transactions_root(&altered.body.transactions);
        if partial.apply_block(&altered).is_ok() {
            return Err(format!(
                "an altered block with a matching transactions_root was applied at height {i}"
            ));
        }
        if state(&partial, watched)? != before {
            return Err("a refused block changed state".to_string());
        }
        apply_all(&partial, &blocks[i..])?;
        if state(&partial, watched)? != replayed {
            return Err("after the refusals, the original blocks gave another state".to_string());
        }
        println!("  Altered block at height {i}: refused twice (content, then signature); no state written");

        // 5. Divergence is detected: replay onto a different starting state.
        let diverged = fresh(InMemoryStorage::new());
        genesis(&diverged, blocks)?;
        let dev = watched.accounts[0];
        let mut acc = diverged
            .storage
            .get_account(&dev)
            .map_err(|e| format!("storage error: {e}"))?
            .ok_or("the dev account is missing from genesis")?;
        acc.balance -= 1;
        diverged
            .storage
            .put_account(&dev, &acc)
            .map_err(|e| format!("storage error: {e}"))?;
        let detected = match apply_all(&diverged, &blocks[1..]) {
            Err(_) => true,
            Ok(()) => compare(&state(&diverged, watched)?, reference).is_err(),
        };
        if !detected {
            return Err("a diverged starting state was not detected".to_string());
        }
        println!("  Divergence: a starting state one unit lower is detected");

        Ok(Summary {
            blocks: blocks.len(),
            transactions,
            runs: 3,
        })
    }
}

/// Builds a valid `Transfer` from the dev account, as RPC JSON.
fn build_transfer_tx(nonce: u64, receiver: mbongo_core::Address, amount: u128) -> Value {
    use ed25519_dalek::{Signer, SigningKey};
    use mbongo_core::{Address, Transaction, TransactionPayload, TransactionType};

    let sk = SigningKey::from_bytes(&[0xAAu8; 32]);
    let mut tx = Transaction {
        tx_type: TransactionType::Transfer,
        sender: Address(sk.verifying_key().to_bytes()),
        receiver,
        amount,
        nonce,
        payload: TransactionPayload::None,
        signature: [0u8; 64],
    };
    tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
    serde_json::to_value(&tx).expect("transaction serializes")
}

/// The producer's view of an account, over REST: `None` when absent.
async fn producer_account(
    client: &Client,
    addr: mbongo_core::Address,
) -> Result<Option<(u128, u64)>, String> {
    let url = format!("http://127.0.0.1:{REST_PORT}/accounts/{addr}");
    let resp = client.get(&url).send().await.map_err(|e| format!("REST request failed: {e}"))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let body: Value = resp.json().await.map_err(|e| format!("REST body: {e}"))?;
    let balance = body["balance"]
        .as_str()
        .and_then(|b| b.parse::<u128>().ok())
        .ok_or_else(|| format!("REST account {addr}: no balance in {body}"))?;
    let nonce = body["nonce"].as_u64().ok_or_else(|| format!("REST account {addr}: no nonce"))?;
    Ok(Some((balance, nonce)))
}

// ── Configuration ───────────────────────────────────────────────────────

const BLOCK_TIME_SECS: u64 = 1;
const MIN_HEIGHT: u64 = 10;

// Production polling: wait for the chain to reach MIN_HEIGHT with a
// bounded poll instead of a fixed sleep (slow runners produce slower
// than one block per second).
const PRODUCTION_POLL_INTERVAL_MS: u64 = 500;
const PRODUCTION_TIMEOUT_SECS: u64 = 60;
const RPC_PORT: u16 = 21944;
const REST_PORT: u16 = 21080;
const P2P_PORT: u16 = 21333;

// RPC readiness probe: max 50 attempts × 200ms = 10s total.
const RPC_PROBE_MAX_ATTEMPTS: u32 = 50;
const RPC_PROBE_INTERVAL_MS: u64 = 200;

// Initial delay before first RPC probe to let the OS bind ports.
const RPC_STARTUP_DELAY_SECS: u64 = 2;

// Lines of the node's output kept for a failure report.
const NODE_LOG_TAIL: usize = 40;

/// The last lines the node wrote, stdout and stderr interleaved.
type NodeLog = Arc<Mutex<VecDeque<String>>>;

// ── RPC helpers ─────────────────────────────────────────────────────────

async fn rpc_call(client: &Client, method: &str, params: Option<Value>) -> Result<Value, String> {
    let url = format!("http://127.0.0.1:{RPC_PORT}/rpc");
    let mut body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "id": 1
    });
    if let Some(p) = params {
        body["params"] = p;
    }

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let json_val: Value = resp.json().await.map_err(|e| format!("failed to parse JSON: {e}"))?;

    if let Some(err) = json_val.get("error") {
        return Err(format!("RPC error: {err}"));
    }

    json_val
        .get("result")
        .cloned()
        .ok_or_else(|| "missing 'result' in RPC response".to_string())
}

/// Wait until the producer's RPC port is reachable.
///
/// Uses a fixed retry loop: an initial delay to let the OS bind ports,
/// then up to `RPC_PROBE_MAX_ATTEMPTS` pings with `RPC_PROBE_INTERVAL_MS`
/// between each attempt. Total wait ≈ startup delay + 10 seconds.
///
/// A node that exits fails at once: no amount of waiting brings it back.
async fn wait_for_rpc(client: &Client, producer: &mut Child) -> Result<(), String> {
    // Initial delay — let the process bind its ports before we start probing.
    sleep(Duration::from_secs(RPC_STARTUP_DELAY_SECS)).await;

    for attempt in 1..=RPC_PROBE_MAX_ATTEMPTS {
        if let Some(status) = producer.try_wait().map_err(|e| e.to_string())? {
            return Err(format!("the node exited before its RPC answered: {status}"));
        }
        match rpc_call(client, "ping", None).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                if attempt == RPC_PROBE_MAX_ATTEMPTS {
                    return Err(format!(
                        "timeout waiting for RPC on port {RPC_PORT} after {RPC_PROBE_MAX_ATTEMPTS} attempts; \
                         node still running; last error: {e}"
                    ));
                }
                sleep(Duration::from_millis(RPC_PROBE_INTERVAL_MS)).await;
            }
        }
    }

    Err(format!("timeout waiting for RPC on port {RPC_PORT}"))
}

// ── Node process management ─────────────────────────────────────────────

fn node_binary_path() -> PathBuf {
    let self_exe = std::env::current_exe().expect("cannot determine own executable path");
    let dir = self_exe.parent().expect("executable has no parent dir");
    let name = if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    };
    dir.join(name)
}

fn spawn_producer(data_dir: &std::path::Path) -> Result<Child, String> {
    let binary = node_binary_path();

    let child = Command::new(&binary)
        .arg("--producer")
        .arg("--block-time")
        .arg(BLOCK_TIME_SECS.to_string())
        .arg("--rpc-port")
        .arg(RPC_PORT.to_string())
        .arg("--rest-port")
        .arg(REST_PORT.to_string())
        .arg("--p2p-port")
        .arg(P2P_PORT.to_string())
        .arg("--dev")
        .arg("--data-dir")
        .arg(data_dir.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn producer: {e}"))?;

    Ok(child)
}

/// Reads `stream` to its end, keeping the last lines in `log`.
fn keep_tail(stream: impl AsyncRead + Unpin + Send + 'static, name: &'static str, log: NodeLog) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut log = log.lock().expect("node log lock");
            if log.len() == NODE_LOG_TAIL {
                log.pop_front();
            }
            log.push_back(format!("{name}: {line}"));
        }
    });
}

/// Starts reading the node's output. The pipes must be read: unread, they
/// fill and block the node, and the reason a node died is lost.
fn read_node_output(producer: &mut Child) -> NodeLog {
    let log = NodeLog::default();
    if let Some(stdout) = producer.stdout.take() {
        keep_tail(stdout, "stdout", Arc::clone(&log));
    }
    if let Some(stderr) = producer.stderr.take() {
        keep_tail(stderr, "stderr", Arc::clone(&log));
    }
    log
}

/// A producer failure with what the node said.
async fn with_node_output(error: String, log: &NodeLog) -> String {
    // The readers may still hold the last lines of a node that just exited.
    sleep(Duration::from_millis(200)).await;
    let log = log.lock().expect("node log lock");
    let lines: Vec<&str> = log.iter().map(String::as_str).collect();
    format!(
        "{error}\n  Node output (last {} lines):\n    {}",
        lines.len(),
        lines.join("\n    ")
    )
}

fn cleanup(dir: &std::path::Path) {
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ── Main ────────────────────────────────────────────────────────────────

#[cfg(not(test))]
#[tokio::main]
async fn main() {
    let client = Client::new();
    let temp_base = std::env::temp_dir().join("mbongo_replay_harness");
    let producer_dir = temp_base.join("producer");

    cleanup(&producer_dir);
    cleanup(&temp_base);

    println!("=== M2.5 Deterministic Replay Harness ===\n");

    let result = run_replay(&client, &producer_dir).await;

    // Always clean up.
    cleanup(&producer_dir);
    cleanup(&temp_base);

    match result {
        Ok(()) => {
            println!("\nDETERMINISTIC REPLAY: PASS");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("\nDETERMINISTIC REPLAY: FAIL");
            eprintln!("  Error: {e}");
            std::process::exit(1);
        }
    }
}

/// Builds a fully valid `AnchorReceipt` transaction from the dev account
/// (the pre-funded key `[0xAA; 32]`, matching `ensure_genesis`) answering
/// the task `task_id` that `build_task_tx` committed: same
/// `input_commitment`, and the dev key as the executor the task named
/// (RFC 0005 rules q–s). Returns the transaction as RPC JSON, the task id,
/// and the canonical SCALE receipt bytes for the replay comparison.
fn build_anchor_tx(nonce: u64, task_id: [u8; 32]) -> (Value, [u8; 32], Vec<u8>) {
    use ed25519_dalek::{Signer, SigningKey};
    use mbongo_core::{Address, Receipt, Transaction, TransactionPayload, TransactionType};
    use parity_scale_codec::Encode;

    let sk = SigningKey::from_bytes(&[0xAAu8; 32]);
    let sender = Address(sk.verifying_key().to_bytes());

    let mut receipt = Receipt {
        version: 1,
        task_id,
        input_commitment: [0x10u8; 32],
        output_commitment: [0x20u8; 32],
        executor: sender,
        metadata: b"replay-harness".to_vec(),
        signature: [0u8; 64],
    };
    receipt.signature = sk.sign(&receipt.receipt_hash().0).to_bytes();
    let receipt_bytes = receipt.encode();

    let mut tx = Transaction {
        tx_type: TransactionType::AnchorReceipt,
        sender,
        receiver: Address::zero(),
        amount: 0,
        nonce,
        payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
        signature: [0u8; 64],
    };
    tx.signature = sk.sign(&tx.signing_payload()).to_bytes();

    let tx_json = serde_json::to_value(&tx).expect("transaction serializes");
    (tx_json, task_id, receipt_bytes)
}

/// Builds a fully valid `ComputeTask` transaction from the dev account
/// (RFC 0005 §2.8: amount 0, zero receiver, `submitter == sender`).
/// Returns the transaction as RPC JSON, the derived `task_id`, and the
/// canonical SCALE task bytes for the replay comparison.
fn build_task_tx(nonce: u64, salt: [u8; 32]) -> (Value, [u8; 32], Vec<u8>) {
    use ed25519_dalek::{Signer, SigningKey};
    use mbongo_core::{
        Address, ComputeTask, Transaction, TransactionPayload, TransactionType,
        COMPUTE_TASK_VERSION,
    };
    use parity_scale_codec::Encode;

    let sk = SigningKey::from_bytes(&[0xAAu8; 32]);
    let sender = Address(sk.verifying_key().to_bytes());

    let task = ComputeTask {
        version: COMPUTE_TASK_VERSION,
        submitter: sender,
        // The dev key answers its own task: it is the only pre-funded
        // account, and the anchor below must come from the named executor.
        executor: sender,
        salt,
        input_commitment: [0x10u8; 32],
        execution_spec: b"replay-harness".to_vec(),
    };
    let task_id = task.task_id();
    let task_bytes = task.encode();

    let mut tx = Transaction {
        tx_type: TransactionType::ComputeTask,
        sender,
        receiver: Address::zero(),
        amount: 0,
        nonce,
        payload: TransactionPayload::ComputeTask(Box::new(task)),
        signature: [0u8; 64],
    };
    tx.signature = sk.sign(&tx.signing_payload()).to_bytes();

    let tx_json = serde_json::to_value(&tx).expect("transaction serializes");
    (tx_json, task_id, task_bytes)
}

#[cfg(not(test))]
async fn run_replay(client: &Client, producer_dir: &std::path::Path) -> Result<(), String> {
    // ── Phase A: Produce chain ──────────────────────────────────────────

    println!("Phase A: Producing chain...");
    let mut producer = spawn_producer(producer_dir)?;
    let node_log = read_node_output(&mut producer);

    if let Err(e) = wait_for_rpc(client, &mut producer).await {
        return Err(with_node_output(e, &node_log).await);
    }
    println!("  Producer RPC ready");

    // Submit one ComputeTask from the dev account (nonce 0) so the chain
    // carries task state (RFC 0005 §4 replay coverage: the tasks index is
    // derived from blocks), then the AnchorReceipt answering it (nonce 1,
    // behind the task: rules q–s need the task registered first).
    let (task_tx_json, compute_task_id, compute_task_bytes) = build_task_tx(0, [0x5Fu8; 32]);
    rpc_call(client, "submit_transaction", Some(task_tx_json))
        .await
        .map_err(|e| format!("compute task submission failed: {e}"))?;
    println!("  ComputeTask submitted (salt 0x5f..5f)");

    let (anchor_tx_json, anchor_task_id, anchor_receipt_bytes) =
        build_anchor_tx(1, compute_task_id);
    rpc_call(client, "submit_transaction", Some(anchor_tx_json))
        .await
        .map_err(|e| format!("anchor submission failed: {e}"))?;
    println!("  AnchorReceipt submitted (bound to the committed task)");

    // A Transfer from the dev account (nonce 2) to a fresh address, so the
    // replayed state includes moved balances and a created account.
    let dev_addr = mbongo_core::Address(
        ed25519_dalek::SigningKey::from_bytes(&[0xAAu8; 32]).verifying_key().to_bytes(),
    );
    let transfer_receiver = mbongo_core::Address([0x7Eu8; 32]);
    rpc_call(
        client,
        "submit_transaction",
        Some(build_transfer_tx(2, transfer_receiver, 12_345)),
    )
    .await
    .map_err(|e| format!("transfer submission failed: {e}"))?;
    println!("  Transfer submitted (12345 to a fresh address)");

    // Bounded height polling: succeed once the chain reaches MIN_HEIGHT.
    println!(
        "  Polling for height >= {MIN_HEIGHT} \
         (every {PRODUCTION_POLL_INTERVAL_MS}ms, up to {PRODUCTION_TIMEOUT_SECS}s)..."
    );
    let poll_start = std::time::Instant::now();
    let snapshot_height = loop {
        let h = rpc_call(client, "get_block_height", None)
            .await?
            .as_u64()
            .ok_or("height is not u64")?;
        if h >= MIN_HEIGHT {
            break h;
        }
        if poll_start.elapsed() >= Duration::from_secs(PRODUCTION_TIMEOUT_SECS) {
            let e = format!(
                "height {h} did not reach minimum {MIN_HEIGHT} within \
                 {PRODUCTION_TIMEOUT_SECS}s ({:.1}s elapsed): producer is stalled \
                 or producing too slowly",
                poll_start.elapsed().as_secs_f64()
            );
            return Err(with_node_output(e, &node_log).await);
        }
        sleep(Duration::from_millis(PRODUCTION_POLL_INTERVAL_MS)).await;
    };

    // ONE immutable snapshot: everything below — export range, tip
    // reference, and replay comparison — derives from this single value.
    // The producer keeps running, but blocks at heights <= snapshot are
    // immutable, so no later production can affect the snapshot.
    println!("  Snapshot height: {snapshot_height}");

    // Export exactly blocks 0..=snapshot_height, deserialized eagerly.
    use mbongo_core::Block;
    println!("  Exporting {} blocks...", snapshot_height + 1);
    let mut exported_blocks: Vec<Block> = Vec::new();
    for h in 0..=snapshot_height {
        let value = rpc_call(client, "get_block_by_height", Some(json!({"height": h}))).await?;
        let block: Block = serde_json::from_value(value)
            .map_err(|e| format!("failed to deserialize exported block {h}: {e}"))?;
        exported_blocks.push(block);
    }
    println!("  Exported {} blocks", exported_blocks.len());

    // The replay reference is derived LOCALLY from the exported snapshot
    // tip — never from a separate moving-tip RPC query, which can race
    // with ongoing production (a block minted between two tip reads made
    // the reference hash belong to a later height than the export).
    let (ref_height, original_tip_hash) =
        derive_snapshot_reference(&exported_blocks).ok_or("no blocks were exported")?;
    if ref_height != snapshot_height {
        return Err(format!(
            "exported tip height {ref_height} != snapshot height {snapshot_height}"
        ));
    }
    println!("  Snapshot tip hash (derived from exported block {ref_height}): {original_tip_hash}");

    // Consistency cross-check via the immutable by-height query: the
    // chain must still serve the identical block at the snapshot height.
    let refetched = rpc_call(
        client,
        "get_block_by_height",
        Some(json!({"height": snapshot_height})),
    )
    .await?;
    let refetched: Block = serde_json::from_value(refetched)
        .map_err(|e| format!("failed to deserialize refetched snapshot block: {e}"))?;
    if compute_block_hash(&refetched) != original_tip_hash {
        return Err(format!(
            "block at height {snapshot_height} changed between export and re-fetch: \
             immutable-by-height guarantee violated"
        ));
    }

    // The producer's state, read after the export. Every block it made after
    // the snapshot must be empty, so this is also its state at the snapshot.
    let watched = replay::Watched {
        accounts: vec![dev_addr, transfer_receiver],
        task_id: compute_task_id,
        receipt_task_id: anchor_task_id,
    };
    let mut accounts = Vec::new();
    for a in &watched.accounts {
        accounts.push((*a, producer_account(client, *a).await?));
    }
    let later = rpc_call(client, "get_block_height", None)
        .await?
        .as_u64()
        .ok_or("height is not u64")?;
    for h in snapshot_height + 1..=later {
        let value = rpc_call(client, "get_block_by_height", Some(json!({"height": h}))).await?;
        let block: Block = serde_json::from_value(value)
            .map_err(|e| format!("failed to deserialize block {h}: {e}"))?;
        if !block.body.transactions.is_empty() {
            return Err(format!("block {h}, after the snapshot, holds transactions"));
        }
    }
    let reference = replay::ChainState {
        block_hashes: exported_blocks.iter().map(compute_block_hash).collect(),
        accounts,
        task: Some(compute_task_bytes.clone()),
        receipt: Some(anchor_receipt_bytes.clone()),
    };
    println!(
        "  Producer state read: {} accounts; no block after the snapshot holds a transaction (checked to height {later})",
        reference.accounts.len()
    );

    // Kill producer — we're done with it.
    let _ = producer.kill().await;
    println!("  Producer stopped");
    println!("  Phase A: DONE\n");

    // ── Phase B: Replay through the node's apply_block ──────────────────

    println!(
        "Phase B: Replaying {} blocks through apply_block...",
        exported_blocks.len()
    );
    let rocks_dir = producer_dir.with_file_name("replay-rocksdb");
    cleanup(&rocks_dir);
    let outcome = replay::run(
        &exported_blocks,
        &reference,
        &watched,
        &rocks_dir,
        compute_block_hash,
    );
    cleanup(&rocks_dir);
    let summary = outcome?;
    if summary.transactions < 3 {
        return Err(format!(
            "expected the 3 submitted transactions in the exported chain, found {}",
            summary.transactions
        ));
    }
    println!(
        "  {} blocks, {} transactions, {} full replays",
        summary.blocks, summary.transactions, summary.runs
    );
    println!("  Phase B: PASS");
    Ok(())
}

// ── Snapshot reference ──────────────────────────────────────────────────

/// Returns `(height, canonical hash)` of the FINAL exported block — the
/// immutable snapshot reference for the replay comparison. Derived
/// locally from exported data only, so no later producer tip can affect
/// it.
fn derive_snapshot_reference(exported: &[mbongo_core::Block]) -> Option<(u64, mbongo_core::Hash)> {
    exported.last().map(|block| (block.header.height, compute_block_hash(block)))
}

// ── Hash computation (same as backend.rs) ───────────────────────────────

fn compute_block_hash(block: &mbongo_core::Block) -> mbongo_core::Hash {
    use parity_scale_codec::Encode;
    let encoded = block.encode();
    let digest = blake3::hash(&encoded);
    let mut out = [0u8; 32];
    out.copy_from_slice(digest.as_bytes());
    mbongo_core::Hash(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbongo_core::{Block, BlockBody, BlockHeader, Hash};

    fn block_at(height: u64, timestamp: u64) -> Block {
        Block {
            header: BlockHeader {
                parent_hash: Hash::zero(),
                state_root: Hash::zero(),
                transactions_root: Hash::zero(),
                timestamp,
                height,
            },
            body: BlockBody {
                transactions: vec![],
            },
        }
    }

    #[test]
    fn snapshot_reference_is_final_exported_block() {
        let exported = vec![block_at(0, 0), block_at(1, 100), block_at(2, 200)];
        let (height, hash) = derive_snapshot_reference(&exported).unwrap();
        assert_eq!(height, 2);
        assert_eq!(hash, compute_block_hash(&exported[2]));
    }

    #[test]
    fn later_producer_tip_cannot_change_snapshot_reference() {
        // The reference is a pure function of the exported slice: a block
        // the producer mints AFTER the export (height 3) is not in the
        // slice and therefore cannot influence the reference.
        let exported = vec![block_at(0, 0), block_at(1, 100), block_at(2, 200)];
        let reference = derive_snapshot_reference(&exported).unwrap();

        let later_tip = block_at(3, 300);
        assert_ne!(reference.1, compute_block_hash(&later_tip));
        assert_eq!(reference, derive_snapshot_reference(&exported).unwrap());
    }

    #[test]
    fn snapshot_reference_empty_export_is_none() {
        assert!(derive_snapshot_reference(&[]).is_none());
    }
}
