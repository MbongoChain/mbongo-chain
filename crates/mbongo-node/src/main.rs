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

use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;

use backend::NodeBackend;
use mbongo_network::{P2PNode, RpcBackend, SyncCommand, SyncEvent, SyncResponse, MAX_RANGE};
use mbongo_storage::RocksDbStorage;

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

    /// REST API port
    #[arg(long, default_value = "8080")]
    rest_port: u16,

    /// P2P port
    #[arg(long, default_value = "30333")]
    p2p_port: u16,

    /// Bootnodes (multiaddr format)
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let args = Args::parse();

    println!("Starting Mbongo Chain node...");
    println!("  Chain:    {}", args.chain);
    println!("  RPC:      http://127.0.0.1:{}", args.rpc_port);
    println!("  REST:     http://127.0.0.1:{}", args.rest_port);
    println!("  P2P:      0.0.0.0:{}", args.p2p_port);

    if let Some(name) = &args.name {
        println!("  Name:     {name}");
    }

    // Initialize storage
    let storage =
        RocksDbStorage::open(&args.data_dir).map_err(|e| format!("failed to open storage: {e}"))?;

    let mut backend = NodeBackend::new(storage, args.producer);

    // Ensure genesis block exists (idempotent).
    backend
        .ensure_genesis()
        .map_err(|e| format!("failed to create genesis block: {e}"))?;

    println!("  Genesis:  OK");

    // ── P2P ────────────────────────────────────────────────────────────
    let mut p2p = P2PNode::new()?;
    println!("  PeerId:   {}", p2p.peer_id);

    p2p.listen(args.p2p_port)?;

    for addr in &args.bootnodes {
        if let Err(e) = p2p.dial(addr) {
            eprintln!("  Failed to dial bootnode {addr}: {e}");
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
    let rpc_addr: SocketAddr = ([127, 0, 0, 1], args.rpc_port).into();
    let rpc_backend = backend.clone();
    let rpc_handle = tokio::spawn(async move {
        if let Err(e) = mbongo_network::serve_on_addr(rpc_addr, rpc_backend).await {
            eprintln!("RPC server error: {e}");
        }
    });

    // ── REST ───────────────────────────────────────────────────────────
    let rest_addr: SocketAddr = ([127, 0, 0, 1], args.rest_port).into();
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
// Strategy (single-peer, devnet-safe):
//   - On PeerConnected → send GetHeight
//   - On Height(h) response → if h > local, send GetBlocks for next range
//   - On Blocks([..]) response → apply each block sequentially, then
//     request next range if not yet caught up
//   - On NewBlock at height > local+1 → trigger GetHeight from known peer

/// Runs the sync orchestrator loop.  Never returns under normal operation.
#[allow(clippy::too_many_lines)]
async fn run_sync_orchestrator<S: mbongo_storage::Storage + Send + Sync + 'static>(
    backend: NodeBackend<S>,
    mut block_rx: tokio::sync::mpsc::UnboundedReceiver<mbongo_core::Block>,
    mut sync_event_rx: tokio::sync::mpsc::UnboundedReceiver<SyncEvent>,
    cmd_tx: tokio::sync::mpsc::UnboundedSender<SyncCommand>,
) {
    use libp2p::PeerId;

    /// The peer we are currently syncing from, if any.
    struct SyncState {
        /// Peer we are syncing from.
        peer: Option<PeerId>,
        /// The known remote tip height (from the last Height response).
        remote_height: u64,
        /// Whether we have an in-flight GetBlocks request.
        in_flight: bool,
    }

    let mut state = SyncState {
        peer: None,
        remote_height: 0,
        in_flight: false,
    };

    /// Request the next batch of blocks from `peer`, starting at
    /// `local_height + 1` up to `remote_height`.  Sends a `GetBlocks`
    /// command via `cmd_tx`.  Returns `true` if a request was sent.
    fn request_next_batch(
        cmd_tx: &tokio::sync::mpsc::UnboundedSender<SyncCommand>,
        peer: PeerId,
        local_height: u64,
        remote_height: u64,
    ) -> bool {
        if local_height >= remote_height {
            return false;
        }
        let start = local_height + 1;
        let end = std::cmp::min(remote_height + 1, start + MAX_RANGE);
        log::info!("Requesting blocks [{start}..{end}) from {peer}");
        let _ = cmd_tx.send(SyncCommand::GetBlocks {
            peer_id: peer,
            start_height: start,
            end_height: end,
        });
        true
    }

    loop {
        tokio::select! {
            // ── Inbound block announcements (push) ────────────────────
            Some(block) = block_rx.recv() => {
                let incoming_height = block.header.height;
                let local_height = match backend.latest_height() {
                    Ok(h) => h,
                    Err(e) => {
                        log::warn!("Failed to read local height: {e}");
                        continue;
                    }
                };

                if incoming_height == local_height + 1 {
                    // Next expected block — apply directly.
                    backend.handle_incoming_block(block);
                } else if incoming_height > local_height + 1 {
                    // Gap detected — trigger sync from a known peer.
                    log::info!(
                        "Gap detected: local={local_height}, incoming={incoming_height}; \
                         triggering sync"
                    );
                    if let Some(peer) = state.peer {
                        if !state.in_flight {
                            // Update remote height to at least incoming_height.
                            if incoming_height > state.remote_height {
                                state.remote_height = incoming_height;
                            }
                            state.in_flight = request_next_batch(
                                &cmd_tx,
                                peer,
                                local_height,
                                state.remote_height,
                            );
                        }
                    }
                    // else: incoming_height <= local_height → already have it, ignore.
                }
            }
            // ── Sync events from P2P layer ────────────────────────────
            Some(event) = sync_event_rx.recv() => {
                match event {
                    SyncEvent::PeerConnected { peer_id } => {
                        log::info!("Peer connected: {peer_id}; sending GetHeight");
                        // Remember this peer for future sync requests.
                        if state.peer.is_none() {
                            state.peer = Some(peer_id);
                        }
                        // Ask for their height.
                        let _ = cmd_tx.send(SyncCommand::GetHeight { peer_id });
                    }
                    SyncEvent::ResponseReceived { peer_id, response } => {
                        match response {
                            SyncResponse::Height(remote_h) => {
                                let local_height = match backend.latest_height() {
                                    Ok(h) => h,
                                    Err(e) => {
                                        log::warn!("Failed to read local height: {e}");
                                        continue;
                                    }
                                };
                                log::info!(
                                    "Peer {peer_id} height={remote_h}, local={local_height}"
                                );
                                state.peer = Some(peer_id);
                                state.remote_height = remote_h;

                                if remote_h > local_height && !state.in_flight {
                                    state.in_flight = request_next_batch(
                                        &cmd_tx,
                                        peer_id,
                                        local_height,
                                        remote_h,
                                    );
                                }
                            }
                            SyncResponse::Blocks(blocks) => {
                                state.in_flight = false;
                                let count = blocks.len();
                                log::info!(
                                    "Received {count} blocks from {peer_id}"
                                );
                                // Apply blocks sequentially.
                                for (_hash, block) in blocks {
                                    let h = block.header.height;
                                    match backend.apply_block(&block) {
                                        Ok(bh) => {
                                            log::info!(
                                                "Applied synced block: height={h}, hash={bh}"
                                            );
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "Failed to apply synced block at height {h}: {e}"
                                            );
                                            // Stop this batch — blocks are sequential.
                                            break;
                                        }
                                    }
                                }

                                // Check if we need more blocks.
                                let local_height = match backend.latest_height() {
                                    Ok(h) => h,
                                    Err(e) => {
                                        log::warn!("Failed to read local height: {e}");
                                        continue;
                                    }
                                };
                                if local_height < state.remote_height {
                                    if let Some(peer) = state.peer {
                                        state.in_flight = request_next_batch(
                                            &cmd_tx,
                                            peer,
                                            local_height,
                                            state.remote_height,
                                        );
                                    }
                                }
                            }
                            SyncResponse::Error(e) => {
                                state.in_flight = false;
                                log::warn!("Sync error from {peer_id}: {e}");
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
