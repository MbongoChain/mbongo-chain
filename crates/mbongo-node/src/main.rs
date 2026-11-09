//! Mbongo Chain full node binary.
//!
//! This is the main entry point for running a Mbongo Chain node.
//! Supports multiple modes:
//! - Full node (sync entire blockchain)
//! - Validator node (participate in consensus)
//! - Compute provider node (execute AI/ML workloads)
//!
//! # Usage
//!
//! ```bash
//! # Run development node
//! mbongo-node --dev
//!
//! # Run full node on testnet
//! mbongo-node --chain testnet --bootnodes /ip4/.../p2p/...
//!
//! # Run validator node
//! mbongo-node --chain mainnet --validator --name "My Validator"
//!
//! # Run compute provider
//! mbongo-node --chain mainnet --provider --gpu nvidia-rtx-4090
//! ```

mod backend;
mod mempool;
mod sync_service;
mod sync_tracker;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use clap::Parser;

use backend::NodeBackend;
use mbongo_network::{
    InboundBlock, P2PNode, RpcBackend, SyncCommand, SyncEvent, SyncResponse, MAX_RANGE,
};
use mbongo_storage::RocksDbStorage;
use sync_tracker::{BatchOutcome, SyncTracker};

#[derive(Parser, Debug)]
#[command(name = "mbongo-node")]
#[command(author = "Mbongo Chain Contributors")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Mbongo Chain full node", long_about = None)]
struct Args {
    /// Run in development mode (single validator, instant finality)
    #[arg(long)]
    dev: bool,

    /// Chain specification (dev, testnet, mainnet)
    #[arg(long, default_value = "dev")]
    chain: String,

    /// Enable validator mode
    #[arg(long)]
    validator: bool,

    /// Enable compute provider mode
    #[arg(long)]
    provider: bool,

    /// Validator name (if running as validator)
    #[arg(long)]
    name: Option<String>,

    /// RPC port
    #[arg(long, default_value = "9944")]
    rpc_port: u16,

    /// RPC bind host (IP literal; 0.0.0.0 exposes it on all interfaces)
    #[arg(long, default_value = "127.0.0.1")]
    rpc_host: String,

    /// REST API port
    #[arg(long, default_value = "8080")]
    rest_port: u16,

    /// REST bind host (IP literal; 0.0.0.0 exposes it on all interfaces)
    #[arg(long, default_value = "127.0.0.1")]
    rest_host: String,

    /// P2P port
    #[arg(long, default_value = "30333")]
    p2p_port: u16,

    /// P2P bind host (IPv4 literal). The default listens on all interfaces,
    /// as multi-host and container devnets need; a single-host deployment
    /// passes 127.0.0.1 so no other host can reach the P2P port.
    #[arg(long, default_value = "0.0.0.0")]
    p2p_host: std::net::Ipv4Addr,

    /// Disable mDNS local peer discovery. By default the node advertises
    /// itself and auto-dials peers found by mDNS on every interface, whatever
    /// --p2p-host says; a loopback-only deployment passes this flag and
    /// reaches its peers through --bootnodes only.
    #[arg(long)]
    no_mdns: bool,

    /// Most transactions this node holds pending (1 to 65536)
    #[arg(long, default_value_t = mempool::DEFAULT_MAX_PENDING)]
    mempool_max_pending: usize,

    /// Seconds a transaction may stay pending before it is evicted
    #[arg(long, default_value_t = mempool::DEFAULT_TTL_SECS)]
    mempool_ttl_secs: u64,

    /// File holding this node's P2P identity: a 32-byte seed as 64 hex
    /// characters. Without it, the peer id changes at each start. Not an
    /// authority or account key.
    #[arg(long)]
    node_key_file: Option<std::path::PathBuf>,

    /// Bootnodes (multiaddr format). They are redialed whenever they are not
    /// connected, with a bounded backoff
    #[arg(long)]
    bootnodes: Vec<String>,

    /// Enable automatic block production (single-producer devnet mode)
    #[arg(long)]
    producer: bool,

    /// Block production interval in seconds (only used with --producer)
    #[arg(long, default_value = "5")]
    block_time: u64,

    /// Data directory for storage (default: "data")
    #[arg(long, default_value = "data")]
    data_dir: String,
}

/// Resolves a bind address from a host string and a port.
///
/// The host must be a literal IP address: `127.0.0.1` keeps the service
/// loopback-only (the default, unchanged behaviour), `0.0.0.0` accepts
/// connections from other hosts, which is what running inside a container
/// requires. Names are deliberately not resolved: a bind address must be
/// unambiguous.
fn resolve_bind_addr(host: &str, port: u16, flag: &str) -> Result<SocketAddr, String> {
    let ip: IpAddr = host.parse().map_err(|_| {
        format!("invalid {flag} value {host:?}: expected an IP literal, e.g. 127.0.0.1 or 0.0.0.0")
    })?;
    Ok(SocketAddr::new(ip, port))
}

/// The protocol lock this binary implements (informational; see the lock).
const PROTOCOL_AUTHORITY: &str = "PROTOCOL_LOCK_v0.4";

/// How often expired transactions are swept when no block commits.
const MEMPOOL_SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(30);
/// The RPC contract this binary serves (informational).
const RPC_AUTHORITY: &str = "rpc_v0.3";
/// The on-disk schema this binary writes (informational; the storage layer
/// is the authority and stamps it itself).
const STORAGE_SCHEMA: u32 = 3;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The same RUST_LOG configuration as `env_logger::init()`, wrapped so that
    // libp2p's per-stream capacity warning is counted instead of written once
    // per refused stream (S3 A2.5). Every other record is unchanged.
    let logger = env_logger::Builder::from_default_env().build();
    let max = logger.filter();
    mbongo_network::capacity_log::install(logger, max)?;

    let args = Args::parse();

    // Resolve bind addresses up front so an invalid host fails before any
    // storage or network side effect happens.
    let rpc_addr = resolve_bind_addr(&args.rpc_host, args.rpc_port, "--rpc-host")?;
    let rest_addr = resolve_bind_addr(&args.rest_host, args.rest_port, "--rest-host")?;

    println!("Starting Mbongo Chain node...");
    // Operator version visibility (not a protocol field, not negotiated):
    // the authority this binary implements. See
    // docs/specs/PROTOCOL_LOCK_v0.4.md and docs/runbooks/DEVNET_V0.4_OPERATIONS.md.
    println!("  Protocol: {PROTOCOL_AUTHORITY} ({RPC_AUTHORITY}, storage schema {STORAGE_SCHEMA})");
    println!("  Chain:    {}", args.chain);
    println!("  RPC:      http://{rpc_addr}");
    println!("  REST:     http://{rest_addr}");
    println!("  P2P:      0.0.0.0:{}", args.p2p_port);

    if let Some(name) = &args.name {
        println!("  Name:     {name}");
    }

    let mempool_config =
        mempool::MempoolConfig::new(args.mempool_max_pending, args.mempool_ttl_secs)?;

    let node_identity =
        args.node_key_file.as_deref().map(mbongo_network::load_node_key).transpose()?;

    // Initialize storage
    let storage =
        RocksDbStorage::open(&args.data_dir).map_err(|e| format!("failed to open storage: {e}"))?;

    let mut backend = NodeBackend::with_mempool_config(storage, args.producer, mempool_config);

    // Ensure genesis block exists (idempotent).
    backend
        .ensure_genesis()
        .map_err(|e| format!("failed to create genesis block: {e}"))?;

    println!("  Genesis:  OK");

    // ── P2P ────────────────────────────────────────────────────────────
    let mut p2p = P2PNode::with_identity(!args.no_mdns, node_identity)?;
    println!("  PeerId:   {}", p2p.peer_id);
    // Printed only with --no-mdns: the default node's output is unchanged.
    if !p2p.mdns_enabled() {
        println!("  mDNS:     disabled (--no-mdns)");
    }

    p2p.listen_on_host(args.p2p_host, args.p2p_port)?;

    for addr in &args.bootnodes {
        if let Err(e) = p2p.add_bootnode(addr) {
            eprintln!("  Invalid bootnode {addr}: {e}");
        }
    }

    // Inject block broadcaster into backend so produce_block can push to peers.
    backend.set_broadcaster(Arc::new(p2p.broadcaster()));

    // Take channels before moving p2p into its event loop.
    let sync_rx = p2p.take_sync_rx().expect("sync_rx should be available exactly once");
    let block_rx = p2p.take_block_rx().expect("block_rx should be available exactly once");
    let sync_event_rx = p2p
        .take_sync_event_rx()
        .expect("sync_event_rx should be available exactly once");
    let sync_cmd_tx = p2p.sync_commander();

    // Spawn the sync service (reads inbound requests, queries storage,
    // sends responses back via SyncCommand::SendResponse).
    let sync_storage = Arc::clone(&backend.storage);
    let sync_respond_tx = sync_cmd_tx.clone();
    let sync_handle = tokio::spawn(async move {
        sync_service::run_sync_service(sync_storage, sync_rx, sync_respond_tx).await;
    });

    // Spawn the sync orchestrator (handles block announcements, peer
    // connect events, sync responses, and drives catch-up).
    let orch_backend = backend.clone();
    let orch_cmd_tx = sync_cmd_tx;
    let orchestrator_handle = tokio::spawn(async move {
        run_sync_orchestrator(orch_backend, block_rx, sync_event_rx, orch_cmd_tx).await;
    });

    // Spawn the P2P event loop (swarm + broadcast + sync command drain).
    let p2p_handle = tokio::spawn(async move {
        p2p.run().await;
    });

    // ── RPC ────────────────────────────────────────────────────────────
    let rpc_backend = backend.clone();
    let rpc_handle = tokio::spawn(async move {
        if let Err(e) = mbongo_network::serve_on_addr(rpc_addr, rpc_backend).await {
            eprintln!("RPC server error: {e}");
        }
    });

    // ── REST ───────────────────────────────────────────────────────────
    let rest_backend = backend.clone();
    let rest_handle = tokio::spawn(async move {
        if let Err(e) = mbongo_api::rest::serve_on_addr(rest_addr, rest_backend).await {
            eprintln!("REST server error: {e}");
        }
    });

    // ── Timed block production ──────────────────────────────────────────
    let producer_handle = if args.producer {
        let block_time = args.block_time;
        println!("  Producer: ON (block time: {block_time}s)");
        log::info!("Timed block production enabled. Block time: {block_time} seconds.");
        let producer_backend = backend.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(block_time));
            loop {
                interval.tick().await;
                match producer_backend.produce_block().await {
                    Ok(hash) => {
                        let height = producer_backend.get_block_height().await.unwrap_or(0);
                        log::info!("Produced block at height {height} ({hash})");
                    }
                    Err(e) => {
                        log::warn!("Block production failed: {e}");
                    }
                }
            }
        }))
    } else {
        None
    };

    // Expiry also runs after each commit. This covers a node with no blocks.
    let sweep_backend = backend.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(MEMPOOL_SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            sweep_backend.sweep_mempool().await;
        }
    });

    println!("Node started.");

    // Wait for any task to finish (normally they run forever).
    tokio::select! {
        _ = rpc_handle => eprintln!("RPC server exited"),
        _ = rest_handle => eprintln!("REST server exited"),
        _ = p2p_handle => eprintln!("P2P event loop exited"),
        _ = sync_handle => eprintln!("Sync service exited"),
        _ = orchestrator_handle => eprintln!("Sync orchestrator exited"),
        _ = async { if let Some(h) = producer_handle { h.await.ok(); } else { std::future::pending::<()>().await; } } => {
            eprintln!("Producer loop exited");
        },
    }

    Ok(())
}

// ── Sync orchestrator ─────────────────────────────────────────────────
//
// Unified task that drives auto-sync on peer connect and gap recovery
// on NewBlock announcements.  Listens on three sources:
//   1. `block_rx`      – inbound block announcements (SyncNotification::NewBlock)
//   2. `sync_event_rx` – PeerConnected + sync responses from the P2P layer
//   3. Internal state  – tracks which peer we are syncing from
//
// Strategy, with the per-peer rules in `sync_tracker`:
//   - On PeerConnected → send GetHeight
//   - On Height(h) response → record it for that peer only
//   - On Blocks([..]) response → apply each block sequentially
//   - On NewBlock at height > local+1 → ask that peer its height
//   - Every HEIGHT_POLL_INTERVAL → ask every connected peer its height again,
//     so a lost announcement (the last one in particular) cannot stall sync
//   - On a failed request, an error or a refused block → back that peer off,
//     and move to another
//   - After every event, and when a backoff ends → send what is now due

/// Request the next batch of blocks from `peer`, starting at
/// `local_height + 1` up to `remote_height`. Returns `true` only when the
/// command was accepted by the P2P event loop's channel.
fn request_next_batch(
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<SyncCommand>,
    peer: libp2p::PeerId,
    local_height: u64,
    remote_height: u64,
) -> bool {
    if local_height >= remote_height {
        return false;
    }
    let start = local_height.saturating_add(1);
    let end = std::cmp::min(
        remote_height.saturating_add(1),
        start.saturating_add(MAX_RANGE),
    );
    log::info!("Requesting blocks [{start}..{end}) from {peer}");
    cmd_tx
        .send(SyncCommand::GetBlocks {
            peer_id: peer,
            start_height: start,
            end_height: end,
        })
        .is_ok()
}

/// Runs the sync orchestrator loop.  Never returns under normal operation.
#[allow(clippy::too_many_lines)]
async fn run_sync_orchestrator<S: mbongo_storage::Storage + Send + Sync + 'static>(
    backend: NodeBackend<S>,
    mut block_rx: tokio::sync::mpsc::Receiver<InboundBlock>,
    mut sync_event_rx: tokio::sync::mpsc::UnboundedReceiver<SyncEvent>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<SyncCommand>,
) {
    let mut tracker = SyncTracker::default();

    loop {
        // One reading of the clock: a retry is either sent now or waited for,
        // and cannot fall between the two.
        let now = tokio::time::Instant::now();
        tracker.release_expired(now);
        tracker.poll_heights(now);
        send_due_requests(&backend, &mut tracker, &cmd_tx, now);
        let wake = tracker.next_wake(now);
        tokio::select! {
            // ── Inbound block announcements (push) ──────────
            // The announcement keeps its queue slot until this branch ends.
            Some(inbound) = block_rx.recv() => {
                let peer_id = inbound.peer;
                let block = inbound.block;
                let incoming_height = block.header.height;
                let local_height = match backend.latest_height() {
                    Ok(h) => h,
                    Err(e) => {
                        log::warn!("Failed to read local height: {e}");
                        continue;
                    }
                };

                if incoming_height == local_height.saturating_add(1) {
                    // Next expected block — apply directly.
                    if let Some(applied) = backend.handle_incoming_block(block) {
                        backend.after_block_committed(&applied).await;
                        tracker.peer_progressed(peer_id);
                    } else {
                        tracker.peer_failed(peer_id, tokio::time::Instant::now());
                    }
                } else if incoming_height > local_height {
                    // Gap: the peer may be ahead, so ask it. The announced
                    // height itself proves nothing.
                    // Logged at INFO only when it changes what is wanted of the
                    // peer; further gaps while a request is already wanted add
                    // nothing (S3 A2.5). Sync failures keep their own WARN lines.
                    if tracker.ask_height(peer_id) {
                        log::info!(
                            "Gap detected: local={local_height}, incoming={incoming_height}; \
                             asking {peer_id}"
                        );
                    } else {
                        log::debug!(
                            "Gap detected: local={local_height}, incoming={incoming_height}; \
                             {peer_id} already being asked"
                        );
                    }
                }
                // else: incoming_height <= local_height → already have it, ignore.
            }
            // ── Sync events from P2P layer ──────────────
            Some(event) = sync_event_rx.recv() => {
                let now = tokio::time::Instant::now();
                match event {
                    SyncEvent::PeerConnected { peer_id } => {
                        log::info!("Peer connected: {peer_id}");
                        tracker.peer_connected(peer_id);
                    }
                    SyncEvent::PeerDisconnected { peer_id, abnormal } => {
                        tracker.peer_disconnected(peer_id, abnormal, now);
                    }
                    SyncEvent::RequestFailed { peer_id, kind } => {
                        log::warn!("Sync request to {peer_id} failed: {kind:?}");
                        tracker.request_failed(peer_id, kind, now);
                    }
                    SyncEvent::ResponseReceived { peer_id, response } => match response {
                        SyncResponse::Height(remote_h) => {
                            log::info!("Peer {peer_id} height={remote_h}");
                            tracker.height_answered(peer_id, remote_h);
                        }
                        SyncResponse::Blocks(blocks) => {
                            if let Some(start) = tracker.expected_start(&peer_id) {
                                log::info!("Received {} blocks from {peer_id}", blocks.len());
                                let outcome = apply_synced_blocks(&backend, blocks, start).await;
                                tracker.blocks_answered(peer_id, outcome, now);
                            } else {
                                log::warn!("Ignoring blocks from {peer_id}: none were asked");
                            }
                        }
                        SyncResponse::Error(e) => {
                            log::warn!("Sync error from {peer_id}: {e}");
                            tracker.error_answered(peer_id, now);
                        }
                    },
                }
            }
            // ── A backoff ended, with nothing else to wake us ──────
            () = tokio::time::sleep_until(wake.unwrap_or_else(tokio::time::Instant::now)),
                if wake.is_some() => {}
            else => break,
        }
    }
}

/// Applies a block answer in order, and says what it amounted to. Blocks at or
/// below the local height are skipped: a pushed block may have arrived first.
/// That also moves the local height to `start` or past it, so an answer that
/// applies nothing and leaves the node below `start` did not answer the request.
async fn apply_synced_blocks<S: mbongo_storage::Storage + Send + Sync + 'static>(
    backend: &NodeBackend<S>,
    blocks: Vec<(mbongo_core::Hash, mbongo_core::Block)>,
    start: u64,
) -> BatchOutcome {
    if blocks.is_empty() {
        return BatchOutcome::Empty;
    }
    let mut applied = 0usize;
    for (_hash, block) in blocks {
        let h = block.header.height;
        match backend.latest_height() {
            Ok(local) if h <= local => continue,
            Ok(_) => {}
            Err(e) => {
                log::warn!("Failed to read local height: {e}");
                return BatchOutcome::Refused;
            }
        }
        match backend.apply_block(&block) {
            Ok(bh) => {
                log::info!("Applied synced block: height={h}, hash={bh}");
                backend.after_block_committed(&block).await;
                applied += 1;
            }
            Err(e) => {
                log::warn!("Failed to apply synced block at height {h}: {e}");
                // Stop this batch — blocks are sequential.
                return BatchOutcome::Refused;
            }
        }
    }
    if applied > 0 {
        return BatchOutcome::Progress;
    }
    match backend.latest_height() {
        Ok(local) if local >= start => BatchOutcome::AlreadyHeld,
        _ => BatchOutcome::Refused,
    }
}

/// Sends what is due now: height requests, and at most one block request.
fn send_due_requests<S: mbongo_storage::Storage + Send + Sync + 'static>(
    backend: &NodeBackend<S>,
    tracker: &mut SyncTracker,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<SyncCommand>,
    now: tokio::time::Instant,
) {
    for peer_id in tracker.heights_due(now) {
        if cmd_tx.send(SyncCommand::GetHeight { peer_id }).is_ok() {
            tracker.height_requested(peer_id);
        }
    }
    let local_height = match backend.latest_height() {
        Ok(h) => h,
        Err(e) => {
            log::warn!("Failed to read local height: {e}");
            return;
        }
    };
    if let Some((peer, remote_height)) = tracker.block_source(local_height, now) {
        if request_next_batch(cmd_tx, peer, local_height, remote_height) {
            tracker.blocks_requested(peer, local_height.saturating_add(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbongo_storage::InMemoryStorage;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn rpc_and_rest_bind_to_loopback_by_default() {
        // Backward compatibility: with no new flags the node must bind
        // exactly where it always has.
        let args = Args::parse_from(["mbongo-node"]);
        assert_eq!(args.rpc_host, "127.0.0.1");
        assert_eq!(args.rest_host, "127.0.0.1");
        assert_eq!(
            resolve_bind_addr(&args.rpc_host, args.rpc_port, "--rpc-host").unwrap(),
            "127.0.0.1:9944".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            resolve_bind_addr(&args.rest_host, args.rest_port, "--rest-host").unwrap(),
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn explicit_wildcard_host_is_accepted() {
        let args = Args::parse_from([
            "mbongo-node",
            "--rpc-host",
            "0.0.0.0",
            "--rest-host",
            "0.0.0.0",
        ]);
        assert_eq!(
            resolve_bind_addr(&args.rpc_host, args.rpc_port, "--rpc-host").unwrap(),
            "0.0.0.0:9944".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            resolve_bind_addr(&args.rest_host, args.rest_port, "--rest-host").unwrap(),
            "0.0.0.0:8080".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn invalid_host_is_rejected_with_an_actionable_error() {
        let err = resolve_bind_addr("not-an-ip", 9944, "--rpc-host").unwrap_err();
        assert!(err.contains("--rpc-host"), "error names the flag: {err}");
        assert!(err.contains("not-an-ip"), "error quotes the value: {err}");
        assert!(resolve_bind_addr("", 9944, "--rpc-host").is_err());
        assert!(resolve_bind_addr("127.0.0.1:9944", 9944, "--rpc-host").is_err());
    }

    #[test]
    fn hosts_and_ports_stay_independent() {
        let args = Args::parse_from([
            "mbongo-node",
            "--rpc-host",
            "0.0.0.0",
            "--rpc-port",
            "19944",
        ]);
        assert_eq!(args.rpc_port, 19944);
        assert_eq!(args.rpc_host, "0.0.0.0");
        // --rpc-host must not leak into the REST side.
        assert_eq!(args.rest_host, "127.0.0.1");
        assert_eq!(args.rest_port, 8080);
    }

    #[test]
    fn p2p_behaviour_is_untouched() {
        // P2P has no host flag: it keeps its own listen semantics.
        let args = Args::parse_from(["mbongo-node"]);
        assert_eq!(args.p2p_port, 30333);
    }

    #[test]
    fn mdns_stays_enabled_by_default() {
        // F-N2: the default node keeps mDNS discovery exactly as before.
        let args = Args::parse_from(["mbongo-node"]);
        assert!(!args.no_mdns);
        assert_eq!(args.p2p_host, std::net::Ipv4Addr::UNSPECIFIED);
    }

    #[test]
    fn no_mdns_flag_disables_mdns() {
        let args = Args::parse_from(["mbongo-node", "--p2p-host", "127.0.0.1", "--no-mdns"]);
        assert!(args.no_mdns);
        assert_eq!(args.p2p_host, std::net::Ipv4Addr::LOCALHOST);
        // The flag takes no value.
        assert!(Args::try_parse_from(["mbongo-node", "--no-mdns", "true"]).is_err());
    }

    #[test]
    fn mempool_flags_default_to_the_stated_bounds() {
        let args = Args::parse_from(["mbongo-node"]);
        assert_eq!(args.mempool_max_pending, 8192);
        assert_eq!(args.mempool_ttl_secs, 3600);
    }

    /// A node holding one pending transaction, and a block built on a second
    /// node, which the first has to import. With `pending_amount` 5 the block
    /// includes that transaction. With another amount it takes its nonce.
    async fn node_with_a_pending_transaction_and_a_block(
        pending_amount: u128,
    ) -> (
        NodeBackend<mbongo_storage::InMemoryStorage>,
        mbongo_core::Hash,
        mbongo_core::Block,
    ) {
        use ed25519_dalek::{Signer, SigningKey};
        use mbongo_core::{Account, Address, Transaction, TransactionPayload, TransactionType};
        use mbongo_storage::Storage;

        let sk = SigningKey::from_bytes(&[0x71u8; 32]);
        let sender = Address(sk.verifying_key().to_bytes());
        let transfer = |amount| {
            let mut tx = Transaction {
                tx_type: TransactionType::Transfer,
                sender,
                receiver: Address([1u8; 32]),
                amount,
                nonce: 0,
                payload: TransactionPayload::None,
                signature: [0u8; 64],
            };
            tx.signature = sk.sign(&tx.signing_payload()).to_bytes();
            tx
        };
        let mut account = Account::new(sender);
        account.balance = 100;

        let build = || {
            let node = NodeBackend::new(mbongo_storage::InMemoryStorage::new(), true);
            node.ensure_genesis().unwrap();
            node.storage.put_account(&sender, &account).unwrap();
            node
        };
        let producer = build();
        producer.submit_transaction(transfer(5)).await.unwrap();
        producer.produce_block().await.unwrap();
        let block = producer.storage.get_block_by_height(1).unwrap().unwrap();

        let follower = build();
        let tx = transfer(pending_amount);
        follower.submit_transaction(tx.clone()).await.unwrap();
        let hash = backend::compute_tx_hash(&tx);
        assert!(follower.is_pending(&hash).await);
        (follower, hash, block)
    }

    async fn wait_until_not_pending(
        node: &NodeBackend<mbongo_storage::InMemoryStorage>,
        hash: &mbongo_core::Hash,
    ) {
        for _ in 0..500 {
            if !node.is_pending(hash).await {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("the transaction stayed pending");
    }

    async fn pushed_block_clears(pending_amount: u128) {
        let (node, hash, block) = node_with_a_pending_transaction_and_a_block(pending_amount).await;
        let (block_tx, block_rx) = tokio::sync::mpsc::channel(1);
        let (_event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(run_sync_orchestrator(
            node.clone(),
            block_rx,
            event_rx,
            cmd_tx,
        ));
        block_tx
            .try_send(InboundBlock::without_permit(
                libp2p::PeerId::random(),
                block,
            ))
            .unwrap();
        wait_until_not_pending(&node, &hash).await;
        assert_eq!(node.latest_height().unwrap(), 1);
        task.abort();
    }

    async fn synced_block_clears(pending_amount: u128) {
        let (node, hash, block) = node_with_a_pending_transaction_and_a_block(pending_amount).await;
        let (_block_tx, block_rx) = tokio::sync::mpsc::channel(1);
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(run_sync_orchestrator(
            node.clone(),
            block_rx,
            event_rx,
            cmd_tx,
        ));
        let peer_id = libp2p::PeerId::random();
        event_tx.send(SyncEvent::PeerConnected { peer_id }).unwrap();
        next_command(&mut cmd_rx).await;
        event_tx
            .send(SyncEvent::ResponseReceived {
                peer_id,
                response: SyncResponse::Height(1),
            })
            .unwrap();
        next_command(&mut cmd_rx).await;
        let block_hash = backend::compute_block_hash(&block);
        event_tx
            .send(SyncEvent::ResponseReceived {
                peer_id,
                response: mbongo_network::SyncResponse::Blocks(vec![(block_hash, block)]),
            })
            .unwrap();
        wait_until_not_pending(&node, &hash).await;
        assert_eq!(node.latest_height().unwrap(), 1);
        task.abort();
    }

    #[tokio::test]
    async fn the_orchestrator_removes_an_included_transaction_on_a_pushed_block() {
        pushed_block_clears(5).await;
    }

    #[tokio::test]
    async fn the_orchestrator_removes_a_stale_transaction_on_a_pushed_block() {
        pushed_block_clears(6).await;
    }

    #[tokio::test]
    async fn the_orchestrator_removes_an_included_transaction_on_a_synced_block() {
        synced_block_clears(5).await;
    }

    #[tokio::test]
    async fn the_orchestrator_removes_a_stale_transaction_on_a_synced_block() {
        synced_block_clears(6).await;
    }

    #[test]
    fn next_batch_is_not_in_flight_when_command_channel_is_closed() {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        drop(cmd_rx);

        assert!(!request_next_batch(&cmd_tx, libp2p::PeerId::random(), 0, 1));
    }

    type Commands = tokio::sync::mpsc::UnboundedReceiver<SyncCommand>;

    async fn next_command(commands: &mut Commands) -> SyncCommand {
        tokio::time::timeout(std::time::Duration::from_secs(1), commands.recv())
            .await
            .expect("no command in time")
            .expect("channel open")
    }

    /// The next command, allowing for one height-poll interval.
    async fn next_poll(commands: &mut Commands) -> SyncCommand {
        let wait = sync_tracker::HEIGHT_POLL_INTERVAL + std::time::Duration::from_secs(1);
        tokio::time::timeout(wait, commands.recv())
            .await
            .expect("no poll in time")
            .expect("channel open")
    }

    /// Lets the orchestrator run, then proves it sent nothing.
    async fn assert_silent(commands: &mut Commands, wait: std::time::Duration) {
        assert!(
            tokio::time::timeout(wait, commands.recv()).await.is_err(),
            "unexpected command"
        );
    }

    struct Harness {
        events: tokio::sync::mpsc::UnboundedSender<SyncEvent>,
        blocks: tokio::sync::mpsc::Sender<InboundBlock>,
        commands: Commands,
        task: tokio::task::JoinHandle<()>,
        node: NodeBackend<InMemoryStorage>,
    }

    fn harness() -> Harness {
        let node = NodeBackend::new(InMemoryStorage::new(), false);
        node.ensure_genesis().unwrap();
        let (blocks, block_rx) = tokio::sync::mpsc::channel(8);
        let (events, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (cmd_tx, commands) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(run_sync_orchestrator(
            node.clone(),
            block_rx,
            event_rx,
            cmd_tx,
        ));
        Harness {
            events,
            blocks,
            commands,
            task,
            node,
        }
    }

    impl Harness {
        /// A peer connects, is asked its height, and answers.
        async fn report(&mut self, peer_id: libp2p::PeerId, height: u64) {
            self.events.send(SyncEvent::PeerConnected { peer_id }).unwrap();
            assert!(is_height_to(
                &next_command(&mut self.commands).await,
                peer_id
            ));
            self.answer(peer_id, SyncResponse::Height(height));
        }

        /// Waits for a pushed block: the two channels have no order between them.
        async fn applied(&self, height: u64) {
            for _ in 0..1000 {
                if self.node.latest_height().unwrap() == height {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("height {height} was not reached");
        }

        fn answer(&self, peer_id: libp2p::PeerId, response: SyncResponse) {
            self.events.send(SyncEvent::ResponseReceived { peer_id, response }).unwrap();
        }

        fn failed(&self, peer_id: libp2p::PeerId, kind: mbongo_network::SyncRequestKind) {
            self.events.send(SyncEvent::RequestFailed { peer_id, kind }).unwrap();
        }
    }

    fn is_blocks_to(command: &SyncCommand, peer: libp2p::PeerId) -> bool {
        matches!(command, SyncCommand::GetBlocks { peer_id, .. } if *peer_id == peer)
    }

    fn is_height_to(command: &SyncCommand, peer: libp2p::PeerId) -> bool {
        matches!(command, SyncCommand::GetHeight { peer_id } if *peer_id == peer)
    }

    /// A single peer, whose request fails, is asked again with no other event.
    #[tokio::test(start_paused = true)]
    async fn a_failed_block_request_is_retried_on_its_own_after_the_backoff() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        h.report(peer, 1).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));

        h.failed(peer, mbongo_network::SyncRequestKind::Blocks);
        assert_silent(&mut h.commands, std::time::Duration::from_millis(900)).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));
        h.task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_height_request_is_retried_on_its_own_after_the_backoff() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        h.events.send(SyncEvent::PeerConnected { peer_id: peer }).unwrap();
        assert!(is_height_to(&next_command(&mut h.commands).await, peer));

        h.failed(peer, mbongo_network::SyncRequestKind::Height);
        assert_silent(&mut h.commands, std::time::Duration::from_millis(900)).await;
        assert!(is_height_to(&next_command(&mut h.commands).await, peer));
        h.task.abort();
    }

    /// Repeated failures slow down, up to the cap, and never spin. The height
    /// poll adds height requests, and never more than one per backoff end.
    #[tokio::test(start_paused = true)]
    async fn repeated_failures_do_not_hot_loop() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        h.report(peer, 1).await;
        let (mut requests, mut polls) = (0u32, 0u32);
        let start = tokio::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(300) {
            // A poll goes out at the same instant as the block request it
            // precedes, so it is answered within the same iteration.
            let command = loop {
                let Ok(Some(command)) =
                    tokio::time::timeout(std::time::Duration::from_secs(70), h.commands.recv())
                        .await
                else {
                    panic!("the retries stopped");
                };
                if !is_height_to(&command, peer) {
                    break command;
                }
                polls += 1;
                h.answer(peer, SyncResponse::Height(1));
            };
            assert!(is_blocks_to(&command, peer));
            requests += 1;
            assert!(requests <= 11, "too many requests");
            h.answer(peer, SyncResponse::Blocks(Vec::new()));
        }
        // At 0, 1, 3, 7, 15, 31, 63, then every 60 seconds: the last at 303.
        assert_eq!(requests, 11);
        // A poll falls due every 15 s but waits for the backoff to end, so
        // it goes out with a block request: at most one per block request.
        assert!(polls >= 1 && polls <= requests, "{polls} polls");
        h.task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_lost_peer_releases_the_request_and_another_takes_over() {
        let mut h = harness();
        let (gone, stays) = (libp2p::PeerId::random(), libp2p::PeerId::random());
        h.report(gone, 5).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, gone));
        h.report(stays, 4).await;
        assert_silent(&mut h.commands, std::time::Duration::from_millis(50)).await;

        h.events
            .send(SyncEvent::PeerDisconnected {
                peer_id: gone,
                abnormal: true,
            })
            .unwrap();
        assert!(is_height_to(&next_command(&mut h.commands).await, stays));
        assert!(is_blocks_to(&next_command(&mut h.commands).await, stays));
        h.task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn blocks_from_a_peer_not_asked_change_nothing() {
        let mut h = harness();
        let (asked, other) = (libp2p::PeerId::random(), libp2p::PeerId::random());
        h.report(asked, 1).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, asked));

        h.answer(other, SyncResponse::Blocks(Vec::new()));
        assert_silent(&mut h.commands, std::time::Duration::from_secs(5)).await;
        h.failed(asked, mbongo_network::SyncRequestKind::Blocks);
        assert_silent(&mut h.commands, std::time::Duration::from_millis(900)).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, asked));
        h.task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_gap_asks_the_announcing_peer_and_does_not_believe_it() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        h.report(peer, 0).await;
        h.blocks.try_send(InboundBlock::without_permit(peer, sample_block(50))).unwrap();
        assert!(is_height_to(&next_command(&mut h.commands).await, peer));
        assert_silent(&mut h.commands, std::time::Duration::from_secs(5)).await;
        assert_eq!(h.node.latest_height().unwrap(), 0);
        h.task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn a_refused_block_backs_off_instead_of_asking_again_at_once() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        h.report(peer, 1).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));

        let bad = sample_block(1);
        let hash = backend::compute_block_hash(&bad);
        h.answer(peer, SyncResponse::Blocks(vec![(hash, bad)]));
        assert_silent(&mut h.commands, std::time::Duration::from_millis(900)).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));
        assert_eq!(h.node.latest_height().unwrap(), 0);
        h.task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn the_largest_possible_height_does_not_panic() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        h.report(peer, u64::MAX).await;
        let SyncCommand::GetBlocks {
            start_height,
            end_height,
            ..
        } = next_command(&mut h.commands).await
        else {
            panic!("expected a block request");
        };
        assert_eq!(start_height, 1);
        assert_eq!(end_height, 1 + MAX_RANGE);
        assert!(!h.task.is_finished());
        h.task.abort();
    }

    /// Valid empty blocks 1 to `count`, from a chain with the same genesis.
    async fn chain(count: u64) -> Vec<(mbongo_core::Hash, mbongo_core::Block)> {
        use mbongo_storage::Storage;
        let producer = NodeBackend::new(InMemoryStorage::new(), true);
        producer.ensure_genesis().unwrap();
        let mut blocks = Vec::new();
        for height in 1..=count {
            producer.produce_block().await.unwrap();
            let block = producer.storage.get_block_by_height(height).unwrap().unwrap();
            blocks.push((backend::compute_block_hash(&block), block));
        }
        blocks
    }

    #[tokio::test(start_paused = true)]
    async fn the_next_batch_is_asked_after_progress() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        let mut blocks = chain(2).await;
        let second = blocks.pop().unwrap();
        h.report(peer, 2).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));

        h.answer(peer, SyncResponse::Blocks(blocks));
        let SyncCommand::GetBlocks { start_height, .. } = next_command(&mut h.commands).await
        else {
            panic!("expected a block request");
        };
        assert_eq!(start_height, 2);
        h.answer(peer, SyncResponse::Blocks(vec![second]));
        assert_silent(&mut h.commands, std::time::Duration::from_secs(5)).await;
        assert_eq!(h.node.latest_height().unwrap(), 2);
        h.task.abort();
    }

    /// The peer's last announcement never arrives, and nothing else happens:
    /// the height poll alone finds the block and applies it (S3 A1).
    #[tokio::test(start_paused = true)]
    async fn a_lost_final_announcement_is_recovered_by_the_height_poll() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        let blocks = chain(1).await;
        h.report(peer, 0).await;
        // The peer produces block 1; its announcement is lost.
        assert_silent(
            &mut h.commands,
            sync_tracker::HEIGHT_POLL_INTERVAL - std::time::Duration::from_millis(100),
        )
        .await;
        assert_eq!(h.node.latest_height().unwrap(), 0, "nothing told it yet");

        assert!(is_height_to(&next_command(&mut h.commands).await, peer));
        h.answer(peer, SyncResponse::Height(1));
        let SyncCommand::GetBlocks { start_height, .. } = next_command(&mut h.commands).await
        else {
            panic!("expected a block request");
        };
        assert_eq!(start_height, 1);
        h.answer(peer, SyncResponse::Blocks(blocks));
        h.applied(1).await;
        h.task.abort();
    }

    /// Announcements refused by a full queue cost nothing but time: the height
    /// poll brings the node to the peer's height all the same.
    #[tokio::test(start_paused = true)]
    async fn a_node_whose_announcements_were_all_refused_still_converges() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        let mut blocks = chain(3).await;
        h.report(peer, 0).await;
        for _ in 0..2 {
            // Each poll finds the peer one block higher; no announcement arrives.
            let height = h.node.latest_height().unwrap() + 1;
            assert!(is_height_to(&next_poll(&mut h.commands).await, peer));
            h.answer(peer, SyncResponse::Height(height));
            assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));
            h.answer(peer, SyncResponse::Blocks(vec![blocks.remove(0)]));
            h.applied(height).await;
        }
        assert!(is_height_to(&next_poll(&mut h.commands).await, peer));
        h.answer(peer, SyncResponse::Height(3));
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));
        h.answer(peer, SyncResponse::Blocks(blocks));
        h.applied(3).await;
        h.task.abort();
    }

    /// A pushed block overtakes the answer: no fault of the peer.
    #[tokio::test(start_paused = true)]
    async fn a_batch_overlapping_a_pushed_block_is_no_failure() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        let blocks = chain(1).await;
        h.report(peer, 2).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));

        h.blocks
            .try_send(InboundBlock::without_permit(peer, blocks[0].1.clone()))
            .unwrap();
        h.applied(1).await;
        h.answer(peer, SyncResponse::Blocks(blocks));
        let start = tokio::time::Instant::now();
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));
        assert_eq!(start.elapsed(), std::time::Duration::ZERO, "no backoff");
        h.task.abort();
    }

    /// Old blocks in answer to a request for new ones are not an overlap.
    #[tokio::test(start_paused = true)]
    async fn old_blocks_in_place_of_those_asked_back_the_peer_off() {
        let mut h = harness();
        let peer = libp2p::PeerId::random();
        let blocks = chain(1).await;
        h.report(peer, 0).await;
        h.blocks
            .try_send(InboundBlock::without_permit(peer, blocks[0].1.clone()))
            .unwrap();
        h.applied(1).await;
        h.answer(peer, SyncResponse::Height(9));
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));

        h.answer(peer, SyncResponse::Blocks(blocks));
        assert_silent(&mut h.commands, std::time::Duration::from_millis(900)).await;
        assert!(is_blocks_to(&next_command(&mut h.commands).await, peer));
        h.task.abort();
    }

    /// A block with a parent this node does not have: refused by `apply_block`.
    fn sample_block(height: u64) -> mbongo_core::Block {
        mbongo_core::Block {
            header: mbongo_core::BlockHeader {
                parent_hash: mbongo_core::Hash([9u8; 32]),
                state_root: mbongo_core::Hash::zero(),
                transactions_root: mbongo_core::Hash::zero(),
                timestamp: 1,
                height,
            },
            body: mbongo_core::BlockBody {
                transactions: Vec::new(),
            },
        }
    }
}
