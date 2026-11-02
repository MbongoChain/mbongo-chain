//! Reference compute harness: the full lifecycle against a live local node.
//!
//! Spawns a producer node (as the replay harness does), then plays every
//! role in process: a client that stores a private input in the reference
//! data plane and commits a `ComputeTask` naming an executor; a control
//! plane that observes the task in a block and leases it; a worker holding
//! the executor key that fetches the input under a scoped capability,
//! verifies its commitment, runs the reference profile, persists the
//! result, and anchors the bound receipt; and the client again, retrieving
//! the private result under a get-result grant.
//!
//! The chain is real: admission and rules (k)–(s) are the node's. Two
//! negatives are driven through it — an unbound receipt and a squatter's
//! receipt — and one proof is taken from it: the private input bytes never
//! appear in any block.
//!
//! Exit code 0 on PASS. Never prints private bytes or keys.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use mbongo_compute::chain::{find_receipt, scan_tasks, ChainClient, RpcChainClient};
use mbongo_compute::clock::{Clock, SystemClock};
use mbongo_compute::control_plane::{ControlPlane, ControlPlaneConfig, TaskState};
use mbongo_compute::data_plane::{
    CapabilityRequest, InMemoryDataPlane, LocalKey, Operation, Presentation,
};
use mbongo_compute::execution::{
    reference_input_commitment, ReverseBytesProfile, REVERSE_BYTES_SPEC,
};
use mbongo_compute::identity::{ExecutorKey, IdSource};
use mbongo_compute::worker::{AttemptOutcome, Worker};
use mbongo_core::{
    Address, ComputeTask, Receipt, Transaction, TransactionPayload, TransactionType,
    COMPUTE_TASK_VERSION,
};
use tokio::process::{Child, Command};
use tokio::time::sleep;

const RPC_PORT: u16 = 41944;
const REST_PORT: u16 = 41080;
const P2P_PORT: u16 = 51333;
const BLOCK_TIME_SECS: u64 = 1;
const WAIT_SECS: u64 = 90;

fn node_binary_path() -> PathBuf {
    let self_exe = std::env::current_exe().expect("cannot determine own executable path");
    let dir = self_exe.parent().expect("executable has no parent dir");
    dir.join(if cfg!(windows) {
        "mbongo-node.exe"
    } else {
        "mbongo-node"
    })
}

fn spawn_producer(data_dir: &std::path::Path) -> Result<Child, String> {
    let binary = node_binary_path();
    if !binary.exists() {
        return Err(format!(
            "node binary not found at {}; build mbongo-node first",
            binary.display()
        ));
    }
    Command::new(&binary)
        .arg("--producer")
        .arg("--block-time")
        .arg(BLOCK_TIME_SECS.to_string())
        .arg("--rpc-port")
        .arg(RPC_PORT.to_string())
        .arg("--rest-port")
        .arg(REST_PORT.to_string())
        .arg("--p2p-port")
        .arg(P2P_PORT.to_string())
        .arg("--data-dir")
        .arg(data_dir.to_str().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("failed to spawn producer: {e}"))
}

async fn wait_for_rpc(chain: &RpcChainClient) -> Result<(), String> {
    sleep(Duration::from_secs(2)).await;
    for _ in 0..50 {
        if chain.latest_height().await.is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err("timeout waiting for the node's RPC".to_string())
}

/// Polls until `pred` over the chain returns `Some`.
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
        sleep(Duration::from_millis(300)).await;
    }
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

fn signed_task(client: &LocalKey, task: ComputeTask, nonce: u64) -> Transaction {
    let mut tx = Transaction {
        tx_type: TransactionType::ComputeTask,
        sender: client.address(),
        receiver: Address::zero(),
        amount: 0,
        nonce,
        payload: TransactionPayload::ComputeTask(Box::new(task)),
        signature: [0u8; 64],
    };
    tx.signature = client.sign(&tx.signing_payload());
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

fn short(id: &[u8; 32]) -> String {
    hex::encode(&id[..8])
}

#[allow(clippy::too_many_lines)]
async fn run(chain: &RpcChainClient) -> Result<(), String> {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let seed = {
        let mut s = [0u8; 32];
        let t = SystemClock.now().to_le_bytes();
        s[..8].copy_from_slice(&t);
        s[8..12].copy_from_slice(&std::process::id().to_le_bytes());
        s
    };
    let mut ids = IdSource::new(seed);

    // Roles. The dev key is the only funded account: it plays the client.
    // The executor is a separate key, funded by a transfer so it can anchor.
    let client = LocalKey::from_seed(&[0xAAu8; 32]);
    let executor = ExecutorKey::from_seed(&[0xE1u8; 32]);
    let squatter = ExecutorKey::from_seed(&[0xE9u8; 32]);
    let issuer = LocalKey::from_seed(&[0xC0u8; 32]); // control-plane service key

    println!("Phase 1: funding executor and squatter accounts...");
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

    // ── Client: private input stays here, in the data plane.
    println!("Phase 2: client stores private input off-chain and commits the task...");
    let private_input: Vec<u8> = b"the quick brown fox jumps over the lazy dog".to_vec();
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
    let mut dp = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(ids.next("dp-seed")));
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
    let task_tx_hash = chain
        .submit_transaction(&signed_task(&client, task.clone(), 2))
        .await
        .map_err(|e| e.to_string())?;
    println!("  task {} submitted ({task_tx_hash})", short(&task_id));

    // ── Control plane observes the task in a block.
    let mut cp = ControlPlane::new(
        Arc::clone(&clock),
        IdSource::new(ids.next("cp-seed")),
        issuer,
        ControlPlaneConfig {
            lease_secs: 30,
            session_secs: 300,
            confirmation_depth: 1,
            capability_secs: 60,
        },
    );
    let observed_height = wait_until("the task to be observed in a block", || async {
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        let tasks = scan_tasks(chain, 1, latest).await.map_err(|e| e.to_string())?;
        Ok(tasks.into_iter().find(|t| t.task_id == task_id).map(|t| t.height))
    })
    .await?;
    println!("Phase 3: task observed at height {observed_height}; control plane leases it to the named executor...");
    let start = std::time::Instant::now();
    loop {
        cp.observe(chain).await.map_err(|e| e.to_string())?;
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        if cp.task(task_id).is_some_and(|t| t.state == TaskState::Discovered)
            && latest > observed_height
        {
            break;
        }
        if start.elapsed() > Duration::from_secs(WAIT_SECS) {
            return Err(
                "timed out waiting for the control plane to observe the confirmed task".to_string(),
            );
        }
        sleep(Duration::from_millis(300)).await;
    }
    cp.register_input(task_id, input_object).map_err(|e| e.to_string())?;

    // ── Negative first: a squatter (funded, wrong executor) cannot anchor.
    println!("Phase 4: squatter and unbound receipts are refused by the node...");
    let mut squat_worker = Worker::new(&mut ids, squatter.clone(), Box::new(ReverseBytesProfile));
    let squat_outcome = squat_worker
        .run_once(&mut cp, &mut dp, chain)
        .await
        .map_err(|e| e.to_string())?;
    if squat_outcome != AttemptOutcome::Idle {
        return Err(format!(
            "control plane offered the task to the wrong executor: {squat_outcome:?}"
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
        Err(e) if e.to_string().contains("not authorised") => println!("  squatter refused: {e}"),
        other => {
            return Err(format!(
                "squatter receipt was not refused on rule (s): {other:?}"
            ))
        }
    }
    let mut unbound = Receipt {
        version: 1,
        task_id: [0x7Au8; 32],
        input_commitment,
        output_commitment: [0x44u8; 32],
        executor: executor.address(),
        metadata: Vec::new(),
        signature: [0u8; 64],
    };
    unbound.signature = executor.sign(&unbound.receipt_hash().0);
    match chain.submit_transaction(&signed_anchor(&executor, unbound, 0)).await {
        Err(e) if e.to_string().contains("not a registered task") => {
            println!("  unbound receipt refused: {e}")
        }
        other => {
            return Err(format!(
                "unbound receipt was not refused on rule (q): {other:?}"
            ))
        }
    }

    // ── The named executor's worker runs the lifecycle.
    println!("Phase 5: worker fetches, verifies, executes, persists, anchors...");
    let mut worker = Worker::new(&mut ids, executor.clone(), Box::new(ReverseBytesProfile));
    let outcome = worker.run_once(&mut cp, &mut dp, chain).await.map_err(|e| e.to_string())?;
    let AttemptOutcome::Submitted { tx_hash, .. } = outcome else {
        return Err(format!("expected a submitted receipt, got {outcome:?}"));
    };
    println!("  anchor submitted ({tx_hash})");
    let result_ref = dp
        .result_ref(task_id, executor.address())
        .ok_or("result was not persisted before the receipt")?;

    // ── Receipt observed on-chain, and the control plane sees completion.
    let (anchor_height, receipt) = wait_until("the receipt to be anchored", || async {
        let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
        find_receipt(chain, task_id, observed_height, latest)
            .await
            .map_err(|e| e.to_string())
    })
    .await?;
    println!("Phase 6: receipt anchored at height {anchor_height}");
    if receipt.executor != executor.address() || receipt.input_commitment != input_commitment {
        return Err("anchored receipt is not bound to the task".to_string());
    }
    if receipt.output_commitment != result_ref.output_commitment {
        return Err(
            "anchored receipt commits to a different result than the one persisted".to_string(),
        );
    }
    cp.observe(chain).await.map_err(|e| e.to_string())?;
    if cp.task(task_id).map(|t| t.state) != Some(TaskState::Completed) {
        return Err("control plane did not derive completion from the chain".to_string());
    }
    // A second run finds the task completed and does nothing.
    let again = worker.run_once(&mut cp, &mut dp, chain).await.map_err(|e| e.to_string())?;
    if again != AttemptOutcome::Idle {
        return Err(format!("worker re-attempted a completed task: {again:?}"));
    }

    // ── Client retrieves the private result under its own grant.
    println!("Phase 7: client retrieves the private result off-chain...");
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
    let challenge = dp.issue_challenge(client.address());
    let result = dp
        .get_result(&Presentation::sign(get, challenge, &client))
        .map_err(|e| e.to_string())?;
    let mut expected = private_input.clone();
    expected.reverse();
    if result.as_bytes() != expected.as_slice() {
        return Err("retrieved result is not the reference transform of the input".to_string());
    }
    if mbongo_compute::execution::reference_output_commitment(result.as_bytes())
        != receipt.output_commitment
    {
        return Err("retrieved result does not match the anchored output_commitment".to_string());
    }

    // ── Public-chain exclusion: the private bytes are in no block.
    println!("Phase 8: proving the private input and result appear in no block...");
    let latest = chain.latest_height().await.map_err(|e| e.to_string())?;
    for h in 0..=latest {
        if let Some(block) = chain.block_by_height(h).await.map_err(|e| e.to_string())? {
            let json = serde_json::to_string(&block).map_err(|e| e.to_string())?;
            let bytes = serde_json::to_string(&private_input).unwrap();
            let out_bytes = serde_json::to_string(&expected).unwrap();
            if json.contains(&bytes[1..bytes.len() - 1])
                || json.contains(&out_bytes[1..out_bytes.len() - 1])
            {
                return Err(format!("private bytes found in block {h}"));
            }
            if json.contains(&hex::encode(&private_input)) {
                return Err(format!("private input hex found in block {h}"));
            }
        }
    }
    println!("  no block carries the private input or result");
    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let temp_base = std::env::temp_dir().join("mbongo_compute_harness");
    let _ = std::fs::remove_dir_all(&temp_base);
    let data_dir = temp_base.join("producer");

    println!("=== Reference Compute Harness ===\n");
    let chain = RpcChainClient::new(
        format!("http://127.0.0.1:{RPC_PORT}/rpc"),
        format!("http://127.0.0.1:{REST_PORT}"),
    );
    let result = match spawn_producer(&data_dir) {
        Err(e) => Err(e),
        Ok(mut producer) => {
            let r = match wait_for_rpc(&chain).await {
                Ok(()) => run(&chain).await,
                Err(e) => Err(e),
            };
            let _ = producer.kill().await;
            r
        }
    };
    let _ = std::fs::remove_dir_all(&temp_base);
    match result {
        Ok(()) => {
            println!("\nREFERENCE COMPUTE FLOW: PASS");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("\nREFERENCE COMPUTE FLOW: FAIL\n  Error: {e}");
            std::process::exit(1);
        }
    }
}
