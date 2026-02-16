//! Minimal libp2p networking node for Phase 2 devnet.
//!
//! Provides peer-to-peer connectivity with:
//! - **Ping** – connection liveness checks
//! - **Identify** – protocol/agent version exchange
//! - **mDNS** – automatic local peer discovery
//! - **Request/Response** – block sync protocol
//!
//! Inbound sync requests are forwarded over an mpsc channel so that
//! the node binary can answer them with data from storage.

use std::iter;
use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    identify, mdns, noise, ping,
    request_response::{self, ProtocolSupport, ResponseChannel},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use log::{debug, info, warn};
use tokio::sync::mpsc;

use mbongo_core::Block;

use crate::p2p_protocol::{
    BlockNotifyAck, BlockNotifyCodec, SyncCodec, SyncNotification, SyncRequest, SyncResponse,
    BLOCK_NOTIFY_PROTOCOL, SYNC_PROTOCOL,
};

// ── Sync event / command types ─────────────────────────────────────────

/// Events pushed out of the P2P layer to the sync orchestrator.
#[derive(Debug)]
pub enum SyncEvent {
    /// A new peer connection was established.
    PeerConnected {
        /// The remote peer identity.
        peer_id: PeerId,
    },
    /// A sync response arrived for an outbound request we sent.
    ResponseReceived {
        /// The peer that sent the response.
        peer_id: PeerId,
        /// The response payload.
        response: SyncResponse,
    },
}

/// Commands sent into the P2P swarm from the sync orchestrator.
pub enum SyncCommand {
    /// Ask a peer for its latest height.
    GetHeight {
        /// Target peer.
        peer_id: PeerId,
    },
    /// Ask a peer for a range of blocks.
    GetBlocks {
        /// Target peer.
        peer_id: PeerId,
        /// First height to request (inclusive).
        start_height: u64,
        /// End height (exclusive).
        end_height: u64,
    },
    /// Send a sync response on a previously received inbound request channel.
    SendResponse {
        /// The response channel from the inbound request.
        channel: ResponseChannel<SyncResponse>,
        /// The response payload to send.
        response: SyncResponse,
    },
}

/// Trait for broadcasting blocks to connected peers.
///
/// Abstraction over the P2P layer so that [`NodeBackend`] can broadcast
/// blocks without depending on networking internals directly.
pub trait BlockBroadcaster: Send + Sync {
    /// Broadcast a block to all connected peers.
    fn broadcast(&self, block: Block);
}

/// Protocol version string exchanged during identify handshake.
const PROTOCOL_VERSION: &str = "/mbongo/0.1.0";

/// Agent version string exchanged during identify handshake.
const AGENT_VERSION: &str = "mbongo-node/0.1.0";

/// Composite libp2p behaviour for the Mbongo devnet.
#[derive(NetworkBehaviour)]
struct Behaviour {
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    mdns: mdns::tokio::Behaviour,
    sync: request_response::Behaviour<SyncCodec>,
    block_notify: request_response::Behaviour<BlockNotifyCodec>,
}

/// An inbound sync request delivered to the node for processing.
pub struct InboundSyncRequest {
    /// The request payload.
    pub request: SyncRequest,
    /// The response channel; send exactly one [`SyncResponse`] back.
    pub channel: ResponseChannel<SyncResponse>,
    /// The peer that sent the request.
    pub peer: PeerId,
}

/// Minimal libp2p node with block-sync and block-announce support.
///
/// Holds the swarm and exposes the local [`PeerId`]. Call [`P2PNode::run`]
/// to start the event loop (non-blocking when spawned on a tokio task).
pub struct P2PNode {
    /// The local peer identity.
    pub peer_id: PeerId,
    swarm: Swarm<Behaviour>,
    /// Send-half kept inside the node; receives are handed out via [`P2PNode::take_sync_rx`].
    sync_tx: mpsc::UnboundedSender<InboundSyncRequest>,
    /// Receive-half; taken by the caller before `run()`.
    sync_rx: Option<mpsc::UnboundedReceiver<InboundSyncRequest>>,
    /// Send-half for forwarding inbound block announcements to the node.
    block_tx: mpsc::UnboundedSender<Block>,
    /// Receive-half for inbound block announcements; taken via [`P2PNode::take_block_rx`].
    block_rx: Option<mpsc::UnboundedReceiver<Block>>,
    /// Receive-half for outbound broadcast requests from the node backend.
    broadcast_rx: mpsc::UnboundedReceiver<Block>,
    /// Cloneable sender for [`BlockBroadcaster`] implementation.
    broadcast_tx: mpsc::UnboundedSender<Block>,
    /// Events pushed to the sync orchestrator (peer connected, sync responses).
    sync_event_tx: mpsc::UnboundedSender<SyncEvent>,
    /// Receive-half for sync events; taken via [`P2PNode::take_sync_event_rx`].
    sync_event_rx: Option<mpsc::UnboundedReceiver<SyncEvent>>,
    /// Receive-half for sync commands from the orchestrator.
    sync_cmd_rx: mpsc::UnboundedReceiver<SyncCommand>,
    /// Cloneable sender for sync commands.
    sync_cmd_tx: mpsc::UnboundedSender<SyncCommand>,
}

impl P2PNode {
    /// Creates a new P2P node with a fresh Ed25519 identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport or mDNS initialisation fails.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let identify = identify::Behaviour::new(
                    identify::Config::new(PROTOCOL_VERSION.to_string(), key.public())
                        .with_agent_version(AGENT_VERSION.to_string()),
                );
                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                )?;
                let ping = ping::Behaviour::default();

                // Block-sync request/response behaviour.
                let sync = request_response::Behaviour::new(
                    iter::once((SYNC_PROTOCOL, ProtocolSupport::Full)),
                    request_response::Config::default(),
                );

                // Block notification push behaviour.
                let block_notify = request_response::Behaviour::new(
                    iter::once((BLOCK_NOTIFY_PROTOCOL, ProtocolSupport::Full)),
                    request_response::Config::default(),
                );

                Ok(Behaviour {
                    ping,
                    identify,
                    mdns,
                    sync,
                    block_notify,
                })
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let peer_id = *swarm.local_peer_id();
        let (sync_tx, sync_rx) = mpsc::unbounded_channel();
        let (block_tx, block_rx) = mpsc::unbounded_channel();
        let (broadcast_tx, broadcast_rx) = mpsc::unbounded_channel();
        let (sync_event_tx, sync_event_rx) = mpsc::unbounded_channel();
        let (sync_cmd_tx, sync_cmd_rx) = mpsc::unbounded_channel();

        Ok(Self {
            peer_id,
            swarm,
            sync_tx,
            sync_rx: Some(sync_rx),
            block_tx,
            block_rx: Some(block_rx),
            broadcast_rx,
            broadcast_tx,
            sync_event_tx,
            sync_event_rx: Some(sync_event_rx),
            sync_cmd_rx,
            sync_cmd_tx,
        })
    }

    /// Takes ownership of the inbound-sync receiver.
    ///
    /// Must be called exactly once before [`P2PNode::run`]. Returns `None`
    /// on subsequent calls.
    pub fn take_sync_rx(&mut self) -> Option<mpsc::UnboundedReceiver<InboundSyncRequest>> {
        self.sync_rx.take()
    }

    /// Takes ownership of the inbound block-announcement receiver.
    ///
    /// Must be called exactly once before [`P2PNode::run`]. Returns `None`
    /// on subsequent calls.
    pub fn take_block_rx(&mut self) -> Option<mpsc::UnboundedReceiver<Block>> {
        self.block_rx.take()
    }

    /// Takes ownership of the sync event receiver.
    ///
    /// The orchestrator task listens on this for [`SyncEvent::PeerConnected`]
    /// and [`SyncEvent::ResponseReceived`] events.
    /// Must be called exactly once before [`P2PNode::run`].
    pub fn take_sync_event_rx(&mut self) -> Option<mpsc::UnboundedReceiver<SyncEvent>> {
        self.sync_event_rx.take()
    }

    /// Returns a cloneable sender for [`SyncCommand`] messages.
    ///
    /// The sync orchestrator uses this to send `GetHeight`, `GetBlocks`,
    /// and `SendResponse` commands into the swarm event loop.
    pub fn sync_commander(&self) -> mpsc::UnboundedSender<SyncCommand> {
        self.sync_cmd_tx.clone()
    }

    /// Returns a cloneable [`BlockBroadcaster`] handle that sends blocks
    /// to this node's event loop for broadcasting to peers.
    pub fn broadcaster(&self) -> ChannelBroadcaster {
        ChannelBroadcaster {
            tx: self.broadcast_tx.clone(),
        }
    }

    /// Broadcast a newly produced block to all connected peers.
    ///
    /// Iterates over all connected peers and sends a [`SyncNotification::NewBlock`]
    /// to each. Peers that fail to receive are logged and skipped.
    fn broadcast_block(&mut self, block: &Block) {
        let peers: Vec<PeerId> = self.swarm.connected_peers().copied().collect();
        if peers.is_empty() {
            debug!("No connected peers to broadcast block to");
            return;
        }
        info!(
            "Broadcasting block (height {}) to {} peers",
            block.header.height,
            peers.len()
        );
        for peer in peers {
            self.swarm.behaviour_mut().block_notify.send_request(
                &peer,
                SyncNotification::NewBlock {
                    block: block.clone(),
                },
            );
        }
    }

    /// Start listening on the given port on all interfaces.
    ///
    /// # Errors
    ///
    /// Returns an error if the listen address cannot be parsed or bound.
    pub fn listen(&mut self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{port}").parse()?;
        self.swarm.listen_on(listen_addr)?;
        Ok(())
    }

    /// Dial a remote peer by multiaddr.
    ///
    /// # Errors
    ///
    /// Returns an error if the address cannot be parsed or the dial fails.
    pub fn dial(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let remote: Multiaddr = addr.parse()?;
        self.swarm.dial(remote)?;
        info!("Dialing {addr}");
        Ok(())
    }

    /// Send a `GetHeight` request to a specific peer.
    ///
    /// The response will arrive as a sync event in the event loop.
    pub fn send_get_height(&mut self, peer: PeerId) -> request_response::OutboundRequestId {
        self.swarm.behaviour_mut().sync.send_request(&peer, SyncRequest::GetHeight)
    }

    /// Send a `GetBlocks` request to a specific peer.
    pub fn send_get_blocks(
        &mut self,
        peer: PeerId,
        start_height: u64,
        end_height: u64,
    ) -> request_response::OutboundRequestId {
        self.swarm.behaviour_mut().sync.send_request(
            &peer,
            SyncRequest::GetBlocks {
                start_height,
                end_height,
            },
        )
    }

    /// Send a response on a previously received inbound request channel.
    ///
    /// # Errors
    ///
    /// Returns `Err(response)` if the channel was already closed.
    pub fn send_response(
        &mut self,
        channel: ResponseChannel<SyncResponse>,
        response: SyncResponse,
    ) -> Result<(), SyncResponse> {
        self.swarm.behaviour_mut().sync.send_response(channel, response)
    }

    /// Run the swarm event loop. This future never completes under
    /// normal operation — spawn it on a tokio task.
    ///
    /// The event loop drains three sources:
    /// 1. Outbound broadcast requests from the backend.
    /// 2. Sync commands from the orchestrator (`GetHeight`, `GetBlocks`, `SendResponse`).
    /// 3. Swarm events (connections, sync messages, block announcements).
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // Drain outbound broadcast requests from the backend.
                Some(block) = self.broadcast_rx.recv() => {
                    self.broadcast_block(&block);
                }
                // Drain sync commands from the orchestrator.
                Some(cmd) = self.sync_cmd_rx.recv() => {
                    self.handle_sync_command(cmd);
                }
                // Process swarm events.
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event);
                }
            }
        }
    }

    /// Execute a sync command received from the orchestrator.
    fn handle_sync_command(&mut self, cmd: SyncCommand) {
        match cmd {
            SyncCommand::GetHeight { peer_id } => {
                debug!("Sending GetHeight to {peer_id}");
                self.send_get_height(peer_id);
            }
            SyncCommand::GetBlocks {
                peer_id,
                start_height,
                end_height,
            } => {
                debug!("Sending GetBlocks [{start_height}..{end_height}) to {peer_id}");
                self.send_get_blocks(peer_id, start_height, end_height);
            }
            SyncCommand::SendResponse { channel, response } => {
                if self.send_response(channel, response).is_err() {
                    warn!("Failed to send sync response (channel closed)");
                }
            }
        }
    }

    /// Handle a single swarm event.
    #[allow(clippy::too_many_lines)]
    fn handle_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("P2P listening on {}/p2p/{}", address, self.peer_id);
            }
            SwarmEvent::ConnectionEstablished {
                peer_id, endpoint, ..
            } => {
                info!("Peer connected: {peer_id} via {endpoint:?}");
                // Notify the sync orchestrator so it can trigger an initial sync.
                let _ = self.sync_event_tx.send(SyncEvent::PeerConnected { peer_id });
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("Peer disconnected: {peer_id} (cause: {cause:?})");
            }
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, addr) in list {
                    info!("mDNS discovered: {peer_id} at {addr}");
                    // Auto-dial discovered peers.
                    if self.swarm.dial(addr.clone()).is_err() {
                        warn!("Failed to dial mDNS peer {peer_id}");
                    }
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                for (peer_id, addr) in list {
                    debug!("mDNS expired: {peer_id} at {addr}");
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
                peer_id,
                info: id_info,
                ..
            })) => {
                info!(
                    "Identify: {peer_id} running {} ({})",
                    id_info.protocol_version, id_info.agent_version
                );
            }
            SwarmEvent::Behaviour(BehaviourEvent::Ping(ping::Event { peer, result, .. })) => {
                debug!("Ping {peer}: {result:?}");
            }
            // ── Sync protocol events ──────────────────────────────
            SwarmEvent::Behaviour(BehaviourEvent::Sync(request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
            })) => {
                debug!("Sync request from {peer}: {request:?}");
                let inbound = InboundSyncRequest {
                    request,
                    channel,
                    peer,
                };
                if self.sync_tx.send(inbound).is_err() {
                    warn!("Sync receiver dropped; cannot forward inbound request");
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(request_response::Event::Message {
                peer,
                message: request_response::Message::Response { response, .. },
            })) => {
                debug!("Sync response from {peer}: {response:?}");
                // Forward the response to the sync orchestrator.
                let _ = self.sync_event_tx.send(SyncEvent::ResponseReceived {
                    peer_id: peer,
                    response,
                });
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(
                request_response::Event::OutboundFailure { peer, error, .. },
            )) => {
                warn!("Sync outbound failure to {peer}: {error}");
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(
                request_response::Event::InboundFailure { peer, error, .. },
            )) => {
                warn!("Sync inbound failure from {peer}: {error}");
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(
                request_response::Event::ResponseSent { peer, .. },
            )) => {
                debug!("Sync response sent to {peer}");
            }
            // ── Block notification events ──────────────────────────
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::Message {
                    peer,
                    message:
                        request_response::Message::Request {
                            request, channel, ..
                        },
                },
            )) => {
                match request {
                    SyncNotification::NewBlock { block } => {
                        info!(
                            "Block announcement from {peer}: height {}",
                            block.header.height
                        );
                        if self.block_tx.send(block).is_err() {
                            warn!("Block receiver dropped; cannot forward announcement");
                        }
                    }
                }
                // Send empty ACK back.
                if self
                    .swarm
                    .behaviour_mut()
                    .block_notify
                    .send_response(channel, BlockNotifyAck)
                    .is_err()
                {
                    debug!("Failed to send block notify ACK to {peer}");
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::Message {
                    peer,
                    message: request_response::Message::Response { .. },
                },
            )) => {
                debug!("Block notify ACK from {peer}");
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::OutboundFailure { peer, error, .. },
            )) => {
                warn!("Block notify outbound failure to {peer}: {error}");
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::InboundFailure { peer, error, .. },
            )) => {
                warn!("Block notify inbound failure from {peer}: {error}");
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::ResponseSent { peer, .. },
            )) => {
                debug!("Block notify ACK sent to {peer}");
            }
            other => {
                debug!("Swarm event: {other:?}");
            }
        }
    }
}

/// Channel-backed [`BlockBroadcaster`] that sends blocks to the P2P event loop.
///
/// Created via [`P2PNode::broadcaster`]. Cheaply cloneable.
#[derive(Clone)]
pub struct ChannelBroadcaster {
    tx: mpsc::UnboundedSender<Block>,
}

impl BlockBroadcaster for ChannelBroadcaster {
    fn broadcast(&self, block: Block) {
        if self.tx.send(block).is_err() {
            warn!("P2P broadcast channel closed; block not broadcast");
        }
    }
}
