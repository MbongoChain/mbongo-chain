//! Minimal libp2p networking node for Phase 2 devnet.
//!
//! Provides peer-to-peer connectivity with:
//! - **Ping** – connection liveness checks
//! - **Identify** – protocol/agent version exchange
//! - **mDNS** – automatic local peer discovery (on by default; off with
//!   [`P2PNode::with_mdns`]`(false)`, e.g. `mbongo-node --no-mdns`)
//! - **Request/Response** – block sync protocol
//!
//! Inbound sync requests are forwarded over an mpsc channel so that
//! the node binary can answer them with data from storage.

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::BuildHasher;
use std::iter;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use libp2p::{
    connection_limits, identify, mdns, noise, ping,
    request_response::{self, ProtocolSupport, ResponseChannel},
    swarm::{
        behaviour::toggle::Toggle, dial_opts::DialOpts, ConnectionError, DialError, ListenError,
        NetworkBehaviour, SwarmEvent,
    },
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use log::{debug, info, warn};
use tokio::sync::mpsc::{self, error::TrySendError};

use mbongo_core::Block;

use crate::bootnodes::BootnodeSet;
use crate::frame_budget::FrameBudget;
use crate::limits::{
    Admission, IpLimits, P2PCounters, P2PCountersSnapshot, Permit, Refusal, BLOCK_QUEUE_CAPACITY,
    BLOCK_QUOTA_PER_PEER, BROADCAST_QUEUE_CAPACITY, MAX_ESTABLISHED_INCOMING,
    MAX_ESTABLISHED_OUTGOING, MAX_ESTABLISHED_PER_PEER, MAX_NEGOTIATING_INBOUND_STREAMS,
    MAX_PENDING_INCOMING, MAX_PENDING_OUTGOING, MAX_YAMUX_STREAMS_PER_CONNECTION, MDNS_DIAL_BUDGET,
    NOTIFY_STREAMS_PER_CONNECTION, SYNC_QUEUE_CAPACITY, SYNC_QUOTA_PER_PEER,
    SYNC_STREAMS_PER_CONNECTION,
};
use crate::rate_limit::{ip_of, InboundRateLimit, RateRefusal};
use crate::request_rate::{
    announcement_cost, sync_request_cost, Cost, RequestLimiter, RequestRateConfig, RequestRefusal,
};

/// How often the counters are summarised in the log, when they changed.
const SUMMARY_INTERVAL: Duration = Duration::from_secs(60);
use crate::p2p_protocol::{
    Announcement, BlockNotifyAck, BlockNotifyCodec, SyncCodec, SyncNotification, SyncRequest,
    SyncResponse, BLOCK_NOTIFY_PROTOCOL, SYNC_PROTOCOL,
};

// ── Sync event / command types ─────────────────────────────────────────

/// Which outbound sync request a failure belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncRequestKind {
    /// A `GetHeight` request.
    Height,
    /// A `GetBlocks` request.
    Blocks,
}

/// Whether a response is of the kind the request calls for. An error answers
/// either.
fn answers(kind: SyncRequestKind, response: &SyncResponse) -> bool {
    match response {
        SyncResponse::Height(_) => kind == SyncRequestKind::Height,
        SyncResponse::Blocks(_) => kind == SyncRequestKind::Blocks,
        SyncResponse::Error(_) => true,
    }
}

/// A value from 0 to 1000, for spreading retries out. It decides nothing else,
/// so the standard library's per-instance hash seed is random enough.
/// The yamux configuration: the defaults, with at most
/// [`MAX_YAMUX_STREAMS_PER_CONNECTION`] streams per connection (S3 A2.3).
pub(crate) fn yamux_config() -> yamux::Config {
    let mut config = yamux::Config::default();
    config.set_max_num_streams(MAX_YAMUX_STREAMS_PER_CONNECTION);
    config
}

fn jitter() -> u16 {
    u16::try_from(RandomState::new().hash_one(0u8) % 1001).unwrap_or(500)
}

/// Whether a closed connection counts against the peer. A local close and an
/// idle timeout do not. libp2p reports a remote close as I/O, so a graceful one
/// counts too, which costs that peer one short backoff.
fn is_abnormal_close(cause: Option<&ConnectionError>) -> bool {
    !matches!(cause, None | Some(ConnectionError::KeepAliveTimeout))
}

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
    /// An outbound sync request failed before a response arrived.
    RequestFailed {
        /// The peer the failed request targeted.
        peer_id: PeerId,
        /// Which request failed.
        kind: SyncRequestKind,
    },
    /// The last connection to a peer closed.
    PeerDisconnected {
        /// The remote peer identity.
        peer_id: PeerId,
        /// True when the connection failed, rather than closing normally or
        /// idling out.
        abnormal: bool,
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
    /// Answer an inbound sync request; built by [`InboundSyncRequest::respond`].
    SendResponse(SyncReply),
}

/// The answer to an inbound sync request, on its way to the swarm. It keeps
/// the request's queue slot until the swarm hands it to libp2p, so an answer
/// waiting here is counted like a request waiting to be answered.
pub struct SyncReply {
    channel: ResponseChannel<SyncResponse>,
    response: SyncResponse,
    _permit: Permit,
}

/// Trait for broadcasting blocks to connected peers.
///
/// Abstraction over the P2P layer so that [`NodeBackend`] can broadcast
/// blocks without depending on networking internals directly.
pub trait BlockBroadcaster: Send + Sync {
    /// Broadcast a block to all connected peers.
    fn broadcast(&self, block: Block);
}

/// Protocol version string exchanged during the libp2p Identify
/// handshake.
///
/// v0.3: this is **informational metadata only** — peers advertise it but
/// nothing gates on it, and it must never grow fallback or admission
/// logic. The authoritative compatibility gates are the Sync and Block
/// Notify protocol negotiations ([`SYNC_PROTOCOL`],
/// [`BLOCK_NOTIFY_PROTOCOL`]), which fail deterministically across
/// versions.
const IDENTIFY_PROTOCOL_VERSION: &str = "/mbongo/0.3.0";

/// Agent version string exchanged during identify handshake.
const AGENT_VERSION: &str = "mbongo-node/0.1.0";

/// Composite libp2p behaviour for the Mbongo devnet.
#[derive(NetworkBehaviour)]
struct Behaviour {
    /// Inbound connection attempts per source and in all, decided before the
    /// handshake (S3 A2.3-a). First, so every attempt is charged.
    rate_limit: InboundRateLimit,
    /// Connection counts: pending, established, per direction and per peer.
    limits: connection_limits::Behaviour,
    /// Inbound connections per IP address.
    ip_limits: IpLimits,
    ping: ping::Behaviour,
    identify: identify::Behaviour,
    /// Disabled: no mDNS socket, no advertisement, no discovery event, so
    /// nothing is ever auto-dialed from mDNS.
    mdns: Toggle<mdns::tokio::Behaviour>,
    sync: request_response::Behaviour<SyncCodec>,
    block_notify: request_response::Behaviour<BlockNotifyCodec>,
}

/// An inbound sync request delivered to the node for processing. It holds one
/// slot of the bounded sync queue until it is answered or dropped.
pub struct InboundSyncRequest {
    /// The request payload.
    pub request: SyncRequest,
    /// The peer that sent the request.
    pub peer: PeerId,
    channel: ResponseChannel<SyncResponse>,
    permit: Permit,
    counters: Arc<P2PCounters>,
}

impl InboundSyncRequest {
    /// Whether the requester is gone: timed out, disconnected or reset. A
    /// canceled request is not worth answering.
    #[must_use]
    pub fn is_canceled(&self) -> bool {
        !self.channel.is_open()
    }

    /// Drops a canceled request, unanswered, and counts it.
    pub fn discard_canceled(self) {
        P2PCounters::add(&self.counters.sync_requests_canceled);
    }

    /// The command that sends `response` back to the requester.
    #[must_use]
    pub fn respond(self, response: SyncResponse) -> SyncCommand {
        SyncCommand::SendResponse(SyncReply {
            channel: self.channel,
            response,
            _permit: self.permit,
        })
    }
}

/// An inbound block announcement delivered to the node for processing. It
/// holds one slot of the bounded announcement queue until it is dropped, which
/// is after the node has handled the block.
pub struct InboundBlock {
    /// The peer that announced the block.
    pub peer: PeerId,
    /// The announced block, not yet validated.
    pub block: Block,
    _permit: Option<Permit>,
}

impl InboundBlock {
    /// An announcement that holds no queue slot: for in-process sources and
    /// tests, never for anything a peer sent.
    #[must_use]
    pub fn without_permit(peer: PeerId, block: Block) -> Self {
        Self {
            peer,
            block,
            _permit: None,
        }
    }
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
    sync_tx: mpsc::Sender<InboundSyncRequest>,
    /// Receive-half; taken by the caller before `run()`.
    sync_rx: Option<mpsc::Receiver<InboundSyncRequest>>,
    /// Slots of the sync queue, per peer and in all.
    sync_admission: Admission,
    /// Send-half for forwarding inbound block announcements to the node.
    block_tx: mpsc::Sender<InboundBlock>,
    /// Receive-half for inbound block announcements; taken via [`P2PNode::take_block_rx`].
    block_rx: Option<mpsc::Receiver<InboundBlock>>,
    /// Slots of the announcement queue, per peer and in all.
    block_admission: Admission,
    /// Receive-half for outbound broadcast requests from the node backend.
    broadcast_rx: mpsc::Receiver<Block>,
    /// Cloneable sender for [`BlockBroadcaster`] implementation.
    broadcast_tx: mpsc::Sender<Block>,
    /// What was admitted, refused and dropped.
    counters: Arc<P2PCounters>,
    /// The inbound frame budget the codecs reserve from.
    frame_budget: Arc<FrameBudget>,
    /// Rate limits on decoded inbound requests, per peer, per source and in
    /// all (S3 A2.3-b-1).
    request_limiter: RequestLimiter,
    /// The counters as last logged.
    last_summary: P2PCountersSnapshot,
    /// The capacity-warning count as last logged.
    last_dropped: Option<u64>,
    /// The yamux stream-limit count as last logged (S3 A2.3).
    last_yamux_limit: Option<u64>,
    /// Events pushed to the sync orchestrator (peer connected, sync responses).
    sync_event_tx: mpsc::UnboundedSender<SyncEvent>,
    /// Receive-half for sync events; taken via [`P2PNode::take_sync_event_rx`].
    sync_event_rx: Option<mpsc::UnboundedReceiver<SyncEvent>>,
    /// Receive-half for sync commands from the orchestrator.
    sync_cmd_rx: mpsc::UnboundedReceiver<SyncCommand>,
    /// Cloneable sender for sync commands.
    sync_cmd_tx: mpsc::UnboundedSender<SyncCommand>,
    /// Outbound sync requests whose failures release orchestrator state.
    outbound_sync_requests: HashMap<request_response::OutboundRequestId, SyncRequestKind>,
    /// The configured bootnodes, redialed whenever they are not connected.
    bootnodes: BootnodeSet,
}

impl P2PNode {
    /// Creates a new P2P node with a fresh Ed25519 identity and mDNS
    /// discovery enabled (the default behaviour).
    ///
    /// # Errors
    ///
    /// Returns an error if the transport or mDNS initialisation fails.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_mdns(true)
    }

    /// Creates a new P2P node with a fresh Ed25519 identity; `mdns` chooses
    /// whether mDNS local discovery runs. With `false` the mDNS behaviour is
    /// never constructed: it opens no socket, advertises nothing and
    /// discovers (so auto-dials) no peer. Explicit dials and bootnodes are
    /// unaffected.
    ///
    /// # Errors
    ///
    /// Returns an error if the transport or mDNS initialisation fails.
    pub fn with_mdns(mdns: bool) -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_identity(mdns, None)
    }

    /// Creates a new P2P node. With no `keypair`, a fresh identity is generated, so
    /// the peer id changes at each start. See [`Self::with_mdns`].
    ///
    /// # Errors
    ///
    /// Returns an error if the swarm cannot be built.
    pub fn with_identity(
        mdns: bool,
        keypair: Option<libp2p::identity::Keypair>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let keypair = keypair.unwrap_or_else(libp2p::identity::Keypair::generate_ed25519);
        // One inbound frame budget, shared by both codecs and every clone
        // libp2p makes of them (S3 A2.2b-lite).
        let frame_budget = Arc::new(FrameBudget::default());
        let codec_budget = Arc::clone(&frame_budget);
        let swarm = libp2p::SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux_config)?
            .with_behaviour(|key| {
                let identify = identify::Behaviour::new(
                    identify::Config::new(IDENTIFY_PROTOCOL_VERSION.to_string(), key.public())
                        .with_agent_version(AGENT_VERSION.to_string()),
                );
                let mdns = if mdns {
                    Some(mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        key.public().to_peer_id(),
                    )?)
                } else {
                    None
                };
                let ping = ping::Behaviour::default();

                // Block-sync request/response behaviour. The stream cap is per
                // connection, inbound and outbound together; a stream past it
                // is dropped (inbound) or fails (outbound), never queued.
                let sync = request_response::Behaviour::with_codec(
                    SyncCodec::with_budget(Arc::clone(&codec_budget)),
                    iter::once((SYNC_PROTOCOL, ProtocolSupport::Full)),
                    request_response::Config::default()
                        .with_max_concurrent_streams(SYNC_STREAMS_PER_CONNECTION),
                );

                // Block notification push behaviour.
                let block_notify = request_response::Behaviour::with_codec(
                    BlockNotifyCodec::with_budget(Arc::clone(&codec_budget)),
                    iter::once((BLOCK_NOTIFY_PROTOCOL, ProtocolSupport::Full)),
                    request_response::Config::default()
                        .with_max_concurrent_streams(NOTIFY_STREAMS_PER_CONNECTION),
                );

                let limits = connection_limits::Behaviour::new(
                    connection_limits::ConnectionLimits::default()
                        .with_max_pending_incoming(Some(MAX_PENDING_INCOMING))
                        .with_max_pending_outgoing(Some(MAX_PENDING_OUTGOING))
                        .with_max_established_incoming(Some(MAX_ESTABLISHED_INCOMING))
                        .with_max_established_outgoing(Some(MAX_ESTABLISHED_OUTGOING))
                        .with_max_established_per_peer(Some(MAX_ESTABLISHED_PER_PEER)),
                );

                Ok(Behaviour {
                    rate_limit: InboundRateLimit::default(),
                    limits,
                    ip_limits: IpLimits::default(),
                    ping,
                    identify,
                    mdns: Toggle::from(mdns),
                    sync,
                    block_notify,
                })
            })?
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
                    .with_max_negotiating_inbound_streams(MAX_NEGOTIATING_INBOUND_STREAMS)
            })
            .build();

        let peer_id = *swarm.local_peer_id();
        // The admission slots, not the channel, decide what is accepted: a
        // message is sent only with a permit, so these never fill first.
        let (sync_tx, sync_rx) = mpsc::channel(SYNC_QUEUE_CAPACITY);
        let (block_tx, block_rx) = mpsc::channel(BLOCK_QUEUE_CAPACITY);
        let (broadcast_tx, broadcast_rx) = mpsc::channel(BROADCAST_QUEUE_CAPACITY);
        let (sync_event_tx, sync_event_rx) = mpsc::unbounded_channel();
        let (sync_cmd_tx, sync_cmd_rx) = mpsc::unbounded_channel();

        Ok(Self {
            peer_id,
            swarm,
            sync_tx,
            sync_rx: Some(sync_rx),
            sync_admission: Admission::new(SYNC_QUOTA_PER_PEER, SYNC_QUEUE_CAPACITY),
            block_tx,
            block_rx: Some(block_rx),
            block_admission: Admission::new(BLOCK_QUOTA_PER_PEER, BLOCK_QUEUE_CAPACITY),
            broadcast_rx,
            broadcast_tx,
            counters: Arc::new(P2PCounters::default()),
            frame_budget,
            request_limiter: RequestLimiter::new(
                RequestRateConfig::default(),
                std::time::Instant::now(),
            ),
            last_summary: P2PCountersSnapshot::default(),
            last_dropped: None,
            last_yamux_limit: None,
            sync_event_tx,
            sync_event_rx: Some(sync_event_rx),
            sync_cmd_rx,
            sync_cmd_tx,
            outbound_sync_requests: HashMap::new(),
            bootnodes: BootnodeSet::default(),
        })
    }

    /// Whether mDNS local discovery is running on this node.
    pub fn mdns_enabled(&self) -> bool {
        self.swarm.behaviour().mdns.is_enabled()
    }

    /// Takes ownership of the inbound-sync receiver.
    ///
    /// Must be called exactly once before [`P2PNode::run`]. Returns `None`
    /// on subsequent calls.
    pub fn take_sync_rx(&mut self) -> Option<mpsc::Receiver<InboundSyncRequest>> {
        self.sync_rx.take()
    }

    /// Takes ownership of the inbound block-announcement receiver.
    ///
    /// Must be called exactly once before [`P2PNode::run`]. Returns `None`
    /// on subsequent calls.
    pub fn take_block_rx(&mut self) -> Option<mpsc::Receiver<InboundBlock>> {
        self.block_rx.take()
    }

    /// The counters of what this node admitted, refused and dropped.
    pub fn counters(&self) -> Arc<P2PCounters> {
        Arc::clone(&self.counters)
    }

    /// Takes ownership of the sync event receiver.
    ///
    /// The orchestrator task listens on this for [`SyncEvent::PeerConnected`],
    /// [`SyncEvent::ResponseReceived`], and [`SyncEvent::RequestFailed`] events.
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
            counters: Arc::clone(&self.counters),
        }
    }

    /// Broadcast a newly produced block to all connected peers.
    ///
    /// Iterates over all connected peers and sends a [`SyncNotification::NewBlock`]
    /// to each. Peers that fail to receive are logged and skipped.
    ///
    /// The announcement is encoded once and its bytes shared by every peer's
    /// request (S3 A2.2d-1); each peer holds a pointer, not a copy.
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
        let announcement = Announcement::new_block(block);
        for peer in peers {
            self.swarm
                .behaviour_mut()
                .block_notify
                .send_request(&peer, announcement.clone());
        }
    }

    /// Start listening on the given port on all interfaces.
    ///
    /// # Errors
    ///
    /// Returns an error if the listen address cannot be parsed or bound.
    pub fn listen(&mut self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        self.listen_on_host(std::net::Ipv4Addr::UNSPECIFIED, port)
    }

    /// Start listening on the given port on one IPv4 interface
    /// (`127.0.0.1` keeps a single-host devnet off the network).
    ///
    /// # Errors
    ///
    /// Returns an error if the listen address cannot be parsed or bound.
    pub fn listen_on_host(
        &mut self,
        host: std::net::Ipv4Addr,
        port: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let listen_addr: Multiaddr = format!("/ip4/{host}/tcp/{port}").parse()?;
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

    /// Adds a bootnode. It is dialed once the event loop runs, and redialed
    /// with a bounded backoff whenever it is not connected.
    ///
    /// # Errors
    ///
    /// Returns an error if the address cannot be parsed.
    pub fn add_bootnode(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let remote: Multiaddr = addr.parse()?;
        self.bootnodes.add(remote, tokio::time::Instant::now());
        Ok(())
    }

    /// Dials every bootnode that is due.
    fn dial_due_bootnodes(&mut self, now: tokio::time::Instant) {
        for (index, addr) in self.bootnodes.dials_due(now) {
            let opts = DialOpts::unknown_peer_id().address(addr.clone()).build();
            let connection = opts.connection_id();
            info!("Dialing bootnode {addr}");
            match self.swarm.dial(opts) {
                Ok(()) => self.bootnodes.dial_started(index, connection),
                Err(e) => {
                    warn!("Dial of bootnode {addr} refused: {e}");
                    self.bootnodes.dial_refused(index, now, jitter());
                }
            }
        }
    }

    /// Send a `GetHeight` request to a specific peer.
    ///
    /// The response will arrive as a sync event in the event loop.
    pub fn send_get_height(&mut self, peer: PeerId) -> request_response::OutboundRequestId {
        let request_id =
            self.swarm.behaviour_mut().sync.send_request(&peer, SyncRequest::GetHeight);
        self.outbound_sync_requests.insert(request_id, SyncRequestKind::Height);
        request_id
    }

    /// Send a `GetBlocks` request to a specific peer.
    pub fn send_get_blocks(
        &mut self,
        peer: PeerId,
        start_height: u64,
        end_height: u64,
    ) -> request_response::OutboundRequestId {
        let request_id = self.swarm.behaviour_mut().sync.send_request(
            &peer,
            SyncRequest::GetBlocks {
                start_height,
                end_height,
            },
        );
        self.outbound_sync_requests.insert(request_id, SyncRequestKind::Blocks);
        request_id
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
        let mut summary = tokio::time::interval(SUMMARY_INTERVAL);
        summary.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            // One reading of the clock: a retry is either dialed now or waited
            // for, and cannot fall between the two.
            let now = tokio::time::Instant::now();
            self.dial_due_bootnodes(now);
            let wake = self.bootnodes.next_wake(now);
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
                // A bootnode's backoff ended, with nothing else to wake us.
                () = tokio::time::sleep_until(wake.unwrap_or_else(tokio::time::Instant::now)),
                    if wake.is_some() => {}
                _ = summary.tick() => self.log_summary(),
            }
        }
    }

    /// Logs the counters in one line, if they changed since the last line.
    fn log_summary(&mut self) {
        let now = self.counters.snapshot();
        // Process-wide, and present only when the capacity logger is installed.
        let dropped = crate::capacity_log::dropped_at_capacity();
        let yamux_limit = crate::capacity_log::yamux_stream_limit_reached();
        if now != self.last_summary
            || dropped != self.last_dropped
            || yamux_limit != self.last_yamux_limit
        {
            match dropped {
                Some(n) if n > 0 => info!(
                    "P2P counters since start: {now} \
                     inbound_streams_dropped_at_capacity_warnings={n}"
                ),
                _ => info!("P2P counters since start: {now}"),
            }
            match yamux_limit {
                Some(n) => info!(
                    "P2P resources: {} yamux_stream_limit_errors={n} yamux_max_streams={}",
                    self.resources(),
                    MAX_YAMUX_STREAMS_PER_CONNECTION
                ),
                None => info!("P2P resources: {}", self.resources()),
            }
            self.last_summary = now;
            self.last_dropped = dropped;
            self.last_yamux_limit = yamux_limit;
        }
    }

    /// Process memory, bounded-queue occupancy and connection counts, read
    /// from supported APIs only (S3 A2.2a). Streams per connection are not
    /// exposed by libp2p 0.53 and are not reported.
    fn resources(&self) -> String {
        let memory = crate::proc_memory::ProcessMemory::read().unwrap_or_default();
        let (sync_held, sync_peak) = self.sync_admission.occupancy();
        let (blocks_held, blocks_peak) = self.block_admission.occupancy();
        let info = self.swarm.network_info();
        let c = info.connection_counters();
        format!(
            "{memory} sync_queue={sync_held} sync_queue_peak={sync_peak} \
             block_queue={blocks_held} block_queue_peak={blocks_peak} \
             connections_in={} connections_out={} pending_in={} pending_out={} {} {} {}",
            c.num_established_incoming(),
            c.num_established_outgoing(),
            c.num_pending_incoming(),
            c.num_pending_outgoing(),
            self.frame_budget,
            self.swarm.behaviour().rate_limit.stats(),
            self.request_limiter.stats()
        )
    }

    /// Whether a decoded request from `peer` costing `cost` is within the
    /// request rates. A refusal is logged at DEBUG only; it is counted by the
    /// caller and in the limiter's own statistics.
    fn within_request_rate(&mut self, peer: PeerId, cost: Cost) -> bool {
        match self.request_limiter.try_admit(peer, cost, std::time::Instant::now()) {
            Ok(()) => true,
            Err(
                refusal @ (RequestRefusal::Peer | RequestRefusal::Source | RequestRefusal::Global),
            ) => {
                debug!(
                    "{:?} from {peer} refused by the {refusal:?} rate",
                    cost.kind
                );
                false
            }
        }
    }

    /// Admits an inbound sync request into the bounded queue, or refuses it.
    /// A refused request is dropped unanswered: the requester sees its
    /// request fail, never a reply that was not computed.
    fn admit_sync_request(
        &mut self,
        peer: PeerId,
        request: SyncRequest,
        channel: ResponseChannel<SyncResponse>,
    ) {
        // Rate first, before any queue slot or storage read (S3 A2.3-b-1).
        if !self.within_request_rate(peer, sync_request_cost(&request)) {
            P2PCounters::add(&self.counters.sync_requests_refused_rate);
            return;
        }
        let permit = match self.sync_admission.try_admit(peer) {
            Ok(permit) => permit,
            Err(refusal) => {
                debug!("Sync request from {peer} refused: {refusal:?}");
                P2PCounters::add(match refusal {
                    Refusal::PeerQuota => &self.counters.sync_requests_refused_peer_quota,
                    Refusal::Full => &self.counters.sync_requests_refused_full,
                });
                return;
            }
        };
        let inbound = InboundSyncRequest {
            request,
            peer,
            channel,
            permit,
            counters: Arc::clone(&self.counters),
        };
        match self.sync_tx.try_send(inbound) {
            Ok(()) => P2PCounters::add(&self.counters.sync_requests_admitted),
            Err(TrySendError::Full(_)) => {
                P2PCounters::add(&self.counters.sync_requests_refused_full);
            }
            Err(TrySendError::Closed(_)) => {
                warn!("Sync receiver dropped; cannot forward inbound request");
            }
        }
    }

    /// Admits an inbound block announcement into the bounded queue and only
    /// then acknowledges it. A refused announcement gets no acknowledgement:
    /// the announcer sees a failure.
    fn admit_announcement(
        &mut self,
        peer: PeerId,
        block: Block,
        channel: ResponseChannel<BlockNotifyAck>,
    ) {
        debug!(
            "Block announcement from {peer}: height {}",
            block.header.height
        );
        // Rate first, before any queue slot (S3 A2.3-b-1). Unacknowledged, as
        // for the other refusals.
        if !self.within_request_rate(
            peer,
            announcement_cost(parity_scale_codec::Encode::encoded_size(&block)),
        ) {
            P2PCounters::add(&self.counters.announcements_refused_rate);
            return;
        }
        let permit = match self.block_admission.try_admit(peer) {
            Ok(permit) => permit,
            Err(refusal) => {
                debug!("Block announcement from {peer} refused: {refusal:?}");
                P2PCounters::add(match refusal {
                    Refusal::PeerQuota => &self.counters.announcements_refused_peer_quota,
                    Refusal::Full => &self.counters.announcements_refused_full,
                });
                return;
            }
        };
        let inbound = InboundBlock {
            peer,
            block,
            _permit: Some(permit),
        };
        match self.block_tx.try_send(inbound) {
            Ok(()) => {
                P2PCounters::add(&self.counters.announcements_admitted);
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
            Err(TrySendError::Full(_)) => {
                P2PCounters::add(&self.counters.announcements_refused_full);
            }
            Err(TrySendError::Closed(_)) => {
                warn!("Block receiver dropped; cannot forward announcement");
            }
        }
    }

    /// Dials an mDNS discovery, unless it is already connected or the
    /// outbound budget is spent: bootnodes keep the outbound room they need.
    fn dial_mdns_discovery(&mut self, peer_id: PeerId, addr: Multiaddr) {
        if self.swarm.is_connected(&peer_id) {
            return;
        }
        let info = self.swarm.network_info();
        let outbound = info.connection_counters().num_established_outgoing()
            + info.connection_counters().num_pending_outgoing();
        if outbound >= MDNS_DIAL_BUDGET {
            P2PCounters::add(&self.counters.mdns_dials_skipped);
            return;
        }
        info!("mDNS discovered: {peer_id} at {addr}");
        if self.swarm.dial(addr).is_err() {
            debug!("Failed to dial mDNS peer {peer_id}");
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
            SyncCommand::SendResponse(reply) => {
                // The permit is released once the answer is handed to libp2p.
                let SyncReply {
                    channel,
                    response,
                    _permit,
                } = reply;
                if self.send_response(channel, response).is_err() {
                    debug!("Failed to send sync response (channel closed)");
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
                peer_id,
                connection_id,
                endpoint,
                ..
            } => {
                info!("Peer connected: {peer_id} via {endpoint:?}");
                self.request_limiter.connection_established(
                    connection_id,
                    peer_id,
                    ip_of(endpoint.get_remote_address()),
                );
                self.bootnodes.connected(connection_id, peer_id, tokio::time::Instant::now());
                // Notify the sync orchestrator so it can trigger an initial sync.
                let _ = self.sync_event_tx.send(SyncEvent::PeerConnected { peer_id });
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                connection_id,
                cause,
                num_established,
                ..
            } => {
                self.request_limiter.connection_closed(connection_id);
                info!("Peer disconnected: {peer_id} (cause: {cause:?})");
                if num_established == 0 {
                    self.bootnodes.disconnected(peer_id, tokio::time::Instant::now(), jitter());
                    let abnormal = is_abnormal_close(cause.as_ref());
                    let _ =
                        self.sync_event_tx.send(SyncEvent::PeerDisconnected { peer_id, abnormal });
                }
            }
            SwarmEvent::OutgoingConnectionError {
                connection_id,
                error,
                ..
            } => {
                if matches!(error, DialError::WrongPeerId { .. }) {
                    // Visible at the default log level: an operator has to act.
                    log::error!("A bootnode's identity changed: {error}");
                } else {
                    debug!("Outgoing connection failed: {error}");
                }
                self.bootnodes.dial_failed(connection_id, tokio::time::Instant::now(), jitter());
            }
            SwarmEvent::IncomingConnectionError {
                send_back_addr,
                error,
                ..
            } => {
                if let ListenError::Denied { cause } = &error {
                    P2PCounters::add(&self.counters.connections_denied);
                    match cause.downcast_ref::<RateRefusal>() {
                        Some(RateRefusal::Source) => {
                            P2PCounters::add(&self.counters.inbound_rate_refused_source);
                        }
                        Some(RateRefusal::Global) => {
                            P2PCounters::add(&self.counters.inbound_rate_refused_global);
                        }
                        None => {}
                    }
                }
                debug!("Incoming connection from {send_back_addr} failed: {error}");
            }
            SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, addr) in list {
                    self.dial_mdns_discovery(peer_id, addr);
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
                self.admit_sync_request(peer, request, channel);
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Response {
                        request_id,
                        response,
                    },
            })) => {
                debug!("Sync response from {peer}: {response:?}");
                let kind = self.outbound_sync_requests.remove(&request_id);
                // An answer of the wrong kind leaves the request unanswered.
                let event = match kind {
                    Some(kind) if !answers(kind, &response) => {
                        warn!("Sync response from {peer} does not answer a {kind:?} request");
                        SyncEvent::RequestFailed {
                            peer_id: peer,
                            kind,
                        }
                    }
                    _ => SyncEvent::ResponseReceived {
                        peer_id: peer,
                        response,
                    },
                };
                // Forward it to the sync orchestrator.
                let _ = self.sync_event_tx.send(event);
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(
                request_response::Event::OutboundFailure {
                    peer,
                    request_id,
                    error,
                },
            )) => {
                debug!("Sync outbound failure to {peer}: {error}");
                P2PCounters::add(&self.counters.outbound_failures);
                if let Some(kind) = self.outbound_sync_requests.remove(&request_id) {
                    let _ = self.sync_event_tx.send(SyncEvent::RequestFailed {
                        peer_id: peer,
                        kind,
                    });
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Sync(
                request_response::Event::InboundFailure { peer, error, .. },
            )) => {
                debug!("Sync inbound failure from {peer}: {error}");
                P2PCounters::add(&self.counters.inbound_failures);
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
                    // Acknowledged only once admitted to the bounded queue.
                    Announcement::Decoded(SyncNotification::NewBlock { block }) => {
                        self.admit_announcement(peer, block, channel);
                    }
                    // The codec decodes everything it reads; an encoded form
                    // only ever leaves this node. Not acknowledged.
                    Announcement::Encoded(_) => {
                        debug!("Ignoring an undecoded announcement from {peer}");
                    }
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::Message {
                    peer,
                    message: request_response::Message::Response { .. },
                },
            )) => {
                debug!("Block notify ACK from {peer}");
                P2PCounters::add(&self.counters.announcements_acknowledged);
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::OutboundFailure { peer, error, .. },
            )) => {
                debug!("Block notify outbound failure to {peer}: {error}");
                P2PCounters::add(&self.counters.outbound_failures);
            }
            SwarmEvent::Behaviour(BehaviourEvent::BlockNotify(
                request_response::Event::InboundFailure { peer, error, .. },
            )) => {
                debug!("Block notify inbound failure from {peer}: {error}");
                P2PCounters::add(&self.counters.inbound_failures);
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
    tx: mpsc::Sender<Block>,
    counters: Arc<P2PCounters>,
}

impl BlockBroadcaster for ChannelBroadcaster {
    /// Queues the block for announcement. When the queue is full the
    /// announcement is dropped and counted; peers still learn the block from
    /// their periodic height request.
    fn broadcast(&self, block: Block) {
        match self.tx.try_send(block) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                debug!("P2P broadcast queue full; block not announced");
                P2PCounters::add(&self.counters.broadcasts_dropped);
            }
            Err(TrySendError::Closed(_)) => {
                warn!("P2P broadcast channel closed; block not broadcast");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use libp2p::request_response;

    use super::{BehaviourEvent, P2PNode, SyncEvent, SyncRequestKind, IDENTIFY_PROTOCOL_VERSION};

    #[test]
    fn identify_protocol_version_pinned() {
        // Locked identifier (PROTOCOL_LOCK_v0.3): informational Identify
        // metadata only — never a negotiation gate. Changing it is a
        // protocol version bump.
        assert_eq!(IDENTIFY_PROTOCOL_VERSION, "/mbongo/0.3.0");
    }

    fn fail(
        node: &mut P2PNode,
        peer: libp2p::PeerId,
        request_id: request_response::OutboundRequestId,
    ) {
        node.handle_swarm_event(libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Sync(
            request_response::Event::OutboundFailure {
                peer,
                request_id,
                error: request_response::OutboundFailure::DialFailure,
            },
        )));
    }

    #[test]
    fn sync_outbound_failure_is_forwarded_to_orchestrator() {
        let mut node = P2PNode::with_mdns(false).unwrap();
        let mut sync_events = node.take_sync_event_rx().unwrap();
        let peer = libp2p::PeerId::random();

        let height_request_id = node.send_get_height(peer);
        fail(&mut node, peer, height_request_id);
        assert!(matches!(
            sync_events.try_recv().unwrap(),
            SyncEvent::RequestFailed { peer_id, kind: SyncRequestKind::Height } if peer_id == peer
        ));

        let block_request_id = node.send_get_blocks(peer, 1, 2);
        fail(&mut node, peer, block_request_id);
        assert!(matches!(
            sync_events.try_recv().unwrap(),
            SyncEvent::RequestFailed { peer_id, kind: SyncRequestKind::Blocks } if peer_id == peer
        ));
    }

    fn respond(
        node: &mut P2PNode,
        peer: libp2p::PeerId,
        request_id: request_response::OutboundRequestId,
        response: crate::p2p_protocol::SyncResponse,
    ) {
        node.handle_swarm_event(libp2p::swarm::SwarmEvent::Behaviour(BehaviourEvent::Sync(
            request_response::Event::Message {
                peer,
                message: request_response::Message::Response {
                    request_id,
                    response,
                },
            },
        )));
    }

    #[test]
    fn a_response_of_the_wrong_kind_fails_the_request() {
        use crate::p2p_protocol::SyncResponse;
        let mut node = P2PNode::with_mdns(false).unwrap();
        let mut sync_events = node.take_sync_event_rx().unwrap();
        let peer = libp2p::PeerId::random();

        let blocks = node.send_get_blocks(peer, 1, 2);
        respond(&mut node, peer, blocks, SyncResponse::Height(9));
        assert!(matches!(
            sync_events.try_recv().unwrap(),
            SyncEvent::RequestFailed {
                kind: SyncRequestKind::Blocks,
                ..
            }
        ));

        let height = node.send_get_height(peer);
        respond(&mut node, peer, height, SyncResponse::Blocks(Vec::new()));
        assert!(matches!(
            sync_events.try_recv().unwrap(),
            SyncEvent::RequestFailed {
                kind: SyncRequestKind::Height,
                ..
            }
        ));
    }

    #[test]
    fn a_response_of_the_right_kind_or_an_error_is_forwarded() {
        use crate::p2p_protocol::SyncResponse;
        let mut node = P2PNode::with_mdns(false).unwrap();
        let mut sync_events = node.take_sync_event_rx().unwrap();
        let peer = libp2p::PeerId::random();

        let height = node.send_get_height(peer);
        respond(&mut node, peer, height, SyncResponse::Height(9));
        assert!(matches!(
            sync_events.try_recv().unwrap(),
            SyncEvent::ResponseReceived {
                response: SyncResponse::Height(9),
                ..
            }
        ));

        let blocks = node.send_get_blocks(peer, 1, 2);
        respond(
            &mut node,
            peer,
            blocks,
            SyncResponse::Error("no".to_string()),
        );
        assert!(matches!(
            sync_events.try_recv().unwrap(),
            SyncEvent::ResponseReceived {
                response: SyncResponse::Error(_),
                ..
            }
        ));
    }

    #[test]
    fn only_a_failed_connection_counts_against_a_peer() {
        use libp2p::swarm::ConnectionError;
        assert!(!super::is_abnormal_close(None));
        assert!(!super::is_abnormal_close(Some(
            &ConnectionError::KeepAliveTimeout
        )));
        let reset = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        assert!(super::is_abnormal_close(Some(&ConnectionError::IO(reset))));
    }

    #[test]
    fn a_failure_with_no_tracked_request_is_not_forwarded() {
        let mut node = P2PNode::with_mdns(false).unwrap();
        let mut sync_events = node.take_sync_event_rx().unwrap();
        let peer = libp2p::PeerId::random();
        let request_id = node.send_get_height(peer);
        fail(&mut node, peer, request_id);
        sync_events.try_recv().unwrap();

        fail(&mut node, peer, request_id);
        assert!(matches!(
            sync_events.try_recv(),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty)
        ));
    }
}
