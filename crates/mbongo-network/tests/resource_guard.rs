//! The P2P resource guard on the real node, over loopback (Phase 1).
//!
//! The node under test is a `P2PNode`, whose transport wraps every muxer in
//! the guard. Its peers are raw swarms built with `SwarmBuilder::with_tcp`:
//! the construction the node itself used before the guard, and the one a node
//! built from `dev` still uses. They run the sync protocol only, so each
//! stream they open carries one request and the node's inbound streams can be
//! counted to the unit.
//!
//! T1 to T5 and T8 run by default. The tests marked `#[ignore]` are
//! measurements: they take an external node, a release build or a Linux host,
//! and print `PHASE1-…` lines instead of asserting on timings.

use std::iter;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use futures::{AsyncRead, AsyncWrite, StreamExt};
use libp2p::core::muxing::{StreamMuxer, StreamMuxerEvent};
use libp2p::core::ConnectedPoint;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::dial_opts::{DialOpts, PeerCondition};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identity, noise, tcp, yamux, Multiaddr, PeerId, Swarm};
use mbongo_core::{
    Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionPayload, TransactionType,
};
use mbongo_network::{
    GuardMode, GuardSnapshot, P2PCounters, P2PNode, P2pResourceGuard, SyncCodec, SyncCommand,
    SyncEvent, SyncRequest, SyncResponse, SYNC_PROTOCOL,
};
use parity_scale_codec::Encode;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;
use tokio::time::{timeout, Instant};

const WAIT: Duration = Duration::from_secs(30);
const ORD: Ordering = Ordering::SeqCst;

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers them.
const T1_P2P: u16 = 27131;
const T2_P2P: u16 = 27132;
const T2_NODES_P2P: u16 = 27133;
const T3_P2P: u16 = 27134;
const T4_P2P: u16 = 27135;
const T5_P2P: u16 = 27136;
const T8_P2P: u16 = 27137;
const NODELAY_P2P: u16 = 27138;
const NEGOTIATION_P2P: u16 = 27140;

/// A protocol the node does not speak: negotiation on it fails.
const UNKNOWN_PROTOCOL: &str = "/mbongo-phase1/unknown/1";

/// The height the node under test answers.
const NODE_HEIGHT: u64 = 42;
/// The height a raw peer answers.
const PEER_HEIGHT: u64 = 77;

// ───────────────────────────── payload ────────────────────────────────────

fn transfer(nonce: u64) -> Transaction {
    Transaction {
        tx_type: TransactionType::Transfer,
        sender: Address([1u8; 32]),
        receiver: Address([2u8; 32]),
        amount: 1,
        nonce,
        payload: TransactionPayload::None,
        signature: [0u8; 64],
    }
}

/// What a block-range answer carries.
type Batch = Vec<(Hash, Block)>;

/// `count` blocks of `transactions` transfers each. Not valid blocks: the
/// network layer carries them, it does not judge them.
fn blocks(count: u64, transactions: u64) -> Batch {
    (0..count)
        .map(|height| {
            let block = Block {
                header: BlockHeader {
                    parent_hash: Hash::zero(),
                    state_root: Hash::zero(),
                    transactions_root: Hash::zero(),
                    timestamp: 1_700_000_000,
                    height,
                    producer: mbongo_core::Address::zero(),
                },
                body: BlockBody {
                    transactions: (0..transactions).map(transfer).collect(),
                },
                producer_signature: [0u8; 64],
            };
            (Hash::zero(), block)
        })
        .collect()
}

/// About 1 MiB of blocks, in 8 blocks.
fn payload() -> Batch {
    blocks(8, 900)
}

fn digest(blocks: &[(Hash, Block)]) -> String {
    blake3::hash(&blocks.encode()).to_hex()[..16].to_string()
}

/// A response, short enough for a transcript.
fn describe(response: &SyncResponse) -> String {
    match response {
        SyncResponse::Height(h) => format!("height {h}"),
        SyncResponse::Blocks(b) => format!("blocks n={} digest={}", b.len(), digest(b)),
        SyncResponse::Error(e) => format!("error {e}"),
    }
}

// ───────────────────────────── the node under test ────────────────────────

/// A real node, answering heights with [`NODE_HEIGHT`] and block ranges with
/// [`payload`]. While `hold` is set it keeps requests unanswered, which keeps
/// their streams open, until `release` is notified.
struct Victim {
    peer: PeerId,
    addr: Multiaddr,
    guard: Arc<P2pResourceGuard>,
    counters: Arc<P2PCounters>,
    commands: UnboundedSender<SyncCommand>,
    events: UnboundedReceiver<SyncEvent>,
    hold: Arc<AtomicBool>,
    held: Arc<AtomicU64>,
    release: Arc<Notify>,
}

fn victim(port: u16) -> Victim {
    let mut node = P2PNode::with_mdns(false).unwrap();
    node.listen_on_host(Ipv4Addr::LOCALHOST, port).unwrap();
    let peer = node.peer_id;
    let guard = node.resource_guard();
    let counters = node.counters();
    let commands = node.sync_commander();
    let answers = node.sync_commander();
    let events = node.take_sync_event_rx().unwrap();
    let mut requests = node.take_sync_rx().unwrap();
    tokio::spawn(node.run());

    let hold = Arc::new(AtomicBool::new(false));
    let held = Arc::new(AtomicU64::new(0));
    let release = Arc::new(Notify::new());
    let (hold_flag, held_count, released) =
        (Arc::clone(&hold), Arc::clone(&held), Arc::clone(&release));
    tokio::spawn(async move {
        let answer = |request: &SyncRequest| match request {
            SyncRequest::GetHeight => SyncResponse::Height(NODE_HEIGHT),
            SyncRequest::GetBlocks { .. } => SyncResponse::Blocks(payload()),
        };
        let mut waiting = Vec::new();
        loop {
            tokio::select! {
                inbound = requests.recv() => {
                    let Some(inbound) = inbound else { break };
                    if hold_flag.load(ORD) {
                        waiting.push(inbound);
                        held_count.fetch_add(1, ORD);
                    } else {
                        let response = answer(&inbound.request);
                        let _ = answers.send(inbound.respond(response));
                    }
                }
                () = released.notified() => {
                    for inbound in waiting.drain(..) {
                        let response = answer(&inbound.request);
                        let _ = answers.send(inbound.respond(response));
                    }
                }
            }
        }
    });

    Victim {
        peer,
        addr: format!("/ip4/127.0.0.1/tcp/{port}/p2p/{peer}").parse().unwrap(),
        guard,
        counters,
        commands,
        events,
        hold,
        held,
        release,
    }
}

impl Victim {
    /// Asks `peer` through the node's own commander and returns the answer.
    async fn ask(&mut self, command: SyncCommand) -> Result<SyncResponse, &'static str> {
        self.commands.send(command).unwrap();
        timeout(WAIT, async {
            loop {
                match self.events.recv().await {
                    Some(SyncEvent::ResponseReceived { response, .. }) => return Ok(response),
                    Some(SyncEvent::RequestFailed { .. }) => return Err("request failed"),
                    Some(_) => {}
                    None => return Err("node stopped"),
                }
            }
        })
        .await
        .expect("no outcome for the node's request")
    }

    /// Waits for the node to report that `peer` is gone.
    async fn disconnected(&mut self, peer: PeerId) -> bool {
        timeout(WAIT, async {
            loop {
                if let Some(SyncEvent::PeerDisconnected { peer_id, abnormal }) =
                    self.events.recv().await
                {
                    if peer_id == peer {
                        return abnormal;
                    }
                }
            }
        })
        .await
        .expect("the node never reported the disconnection")
    }
}

/// Waits until the guard's counts satisfy `check`, or fails with the last
/// reading.
async fn until(guard: &P2pResourceGuard, what: &str, check: impl Fn(&GuardSnapshot) -> bool) {
    let deadline = Instant::now() + WAIT;
    loop {
        let seen = guard.snapshot();
        if check(&seen) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {seen}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// What holds once every stream is closed and nothing went wrong in the
/// guard.
fn assert_settled(seen: &GuardSnapshot) {
    assert_eq!(seen.mode, GuardMode::ObserveOnly);
    assert_eq!(seen.inbound_streams_active, 0, "{seen}");
    assert_eq!(
        seen.inbound_streams_closed_total, seen.inbound_streams_total,
        "{seen}"
    );
    assert_eq!(
        seen.streams_observation_only_total, seen.inbound_streams_total,
        "{seen}"
    );
    assert_eq!(
        seen.lifetime_buckets.iter().sum::<u64>(),
        seen.inbound_streams_total,
        "{seen}"
    );
    assert_eq!(seen.observation_errors_total, 0, "{seen}");
}

// ───────────────────────────── raw peers ──────────────────────────────────

#[derive(NetworkBehaviour)]
struct Raw {
    sync: request_response::Behaviour<SyncCodec>,
    /// Outbound only: a protocol the node does not have.
    unknown: request_response::Behaviour<SyncCodec>,
}

enum Cmd {
    Dial(Multiaddr),
    DialAlways(PeerId, Multiaddr),
    Request(PeerId, SyncRequest),
    Unknown(PeerId),
    Disconnect(PeerId),
}

#[derive(Debug, Clone)]
enum Ev {
    Listening(Multiaddr),
    Connected(PeerId),
    Closed(Option<String>),
    Inbound(SyncRequest),
    Response(SyncResponse),
    Failure(String),
    DialError(String),
}

fn raw_swarm(nodelay: Option<bool>) -> Swarm<Raw> {
    let config = || {
        request_response::Config::default()
            .with_request_timeout(Duration::from_secs(20))
            .with_max_concurrent_streams(10_000)
    };
    let tcp_config = match nodelay {
        None => tcp::Config::default(),
        Some(flag) => tcp::Config::default().nodelay(flag),
    };
    libp2p::SwarmBuilder::with_existing_identity(identity::Keypair::generate_ed25519())
        .with_tokio()
        .with_tcp(tcp_config, noise::Config::new, yamux::Config::default)
        .expect("tcp")
        .with_behaviour(|_| Raw {
            sync: request_response::Behaviour::new(
                iter::once((SYNC_PROTOCOL, ProtocolSupport::Full)),
                config(),
            ),
            unknown: request_response::Behaviour::new(
                iter::once((UNKNOWN_PROTOCOL, ProtocolSupport::Outbound)),
                config(),
            ),
        })
        .expect("behaviour")
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
        .build()
}

/// What a raw peer answers to a request from the node.
fn peer_answer(request: &SyncRequest, height: u64, bulk: &[(Hash, Block)]) -> SyncResponse {
    match request {
        SyncRequest::GetHeight => SyncResponse::Height(height),
        SyncRequest::GetBlocks { .. } => SyncResponse::Blocks(bulk.to_vec()),
    }
}

async fn run_raw(
    mut swarm: Swarm<Raw>,
    height: u64,
    bulk: Batch,
    mut commands: UnboundedReceiver<Cmd>,
    events: UnboundedSender<Ev>,
) {
    use request_response::{Event, Message};
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                None => break,
                Some(Cmd::Dial(addr)) => {
                    if let Err(e) = swarm.dial(addr) {
                        let _ = events.send(Ev::DialError(e.to_string()));
                    }
                }
                Some(Cmd::DialAlways(peer, addr)) => {
                    let opts = DialOpts::peer_id(peer)
                        .condition(PeerCondition::Always)
                        .addresses(vec![addr])
                        .build();
                    if let Err(e) = swarm.dial(opts) {
                        let _ = events.send(Ev::DialError(e.to_string()));
                    }
                }
                Some(Cmd::Request(peer, request)) => {
                    swarm.behaviour_mut().sync.send_request(&peer, request);
                }
                Some(Cmd::Unknown(peer)) => {
                    swarm.behaviour_mut().unknown.send_request(&peer, SyncRequest::GetHeight);
                }
                Some(Cmd::Disconnect(peer)) => {
                    let _ = swarm.disconnect_peer_id(peer);
                }
            },
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    let _ = events.send(Ev::Listening(address));
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    let _ = events.send(Ev::Connected(peer_id));
                }
                SwarmEvent::ConnectionClosed { cause, .. } => {
                    let _ = events.send(Ev::Closed(cause.map(|c| c.to_string())));
                }
                SwarmEvent::OutgoingConnectionError { error, .. } => {
                    let _ = events.send(Ev::DialError(error.to_string()));
                }
                SwarmEvent::Behaviour(RawEvent::Sync(Event::Message { message, .. })) => {
                    match message {
                        Message::Request { request, channel, .. } => {
                            let response = peer_answer(&request, height, &bulk);
                            let _ = events.send(Ev::Inbound(request));
                            let _ = swarm.behaviour_mut().sync.send_response(channel, response);
                        }
                        Message::Response { response, .. } => {
                            let _ = events.send(Ev::Response(response));
                        }
                    }
                }
                SwarmEvent::Behaviour(
                    RawEvent::Sync(Event::OutboundFailure { error, .. })
                    | RawEvent::Unknown(Event::OutboundFailure { error, .. }),
                ) => {
                    let _ = events.send(Ev::Failure(format!("{error:?}")));
                }
                _ => {}
            },
        }
    }
}

/// A raw peer and what it has seen.
struct RawPeer {
    peer: PeerId,
    commands: UnboundedSender<Cmd>,
    events: UnboundedReceiver<Ev>,
    /// Requests the node sent to this peer.
    inbound: Vec<SyncRequest>,
    closed: Vec<Option<String>>,
}

impl RawPeer {
    fn spawn() -> Self {
        Self::spawn_full(None, None, PEER_HEIGHT, payload())
    }

    fn spawn_with(nodelay: Option<bool>, listen: Option<&str>, bulk: Batch) -> Self {
        Self::spawn_full(nodelay, listen, PEER_HEIGHT, bulk)
    }

    /// `nodelay`: `None` leaves libp2p's default, as the node does. `height`
    /// and `bulk` are what this peer answers to the node's own requests.
    fn spawn_full(nodelay: Option<bool>, listen: Option<&str>, height: u64, bulk: Batch) -> Self {
        let mut swarm = raw_swarm(nodelay);
        if let Some(addr) = listen {
            swarm.listen_on(addr.parse().unwrap()).unwrap();
        }
        let peer = *swarm.local_peer_id();
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (event_tx, events) = mpsc::unbounded_channel();
        tokio::spawn(run_raw(swarm, height, bulk, command_rx, event_tx));
        Self {
            peer,
            commands,
            events,
            inbound: Vec::new(),
            closed: Vec::new(),
        }
    }

    fn send(&self, command: Cmd) {
        self.commands.send(command).expect("raw peer stopped");
    }

    /// The next event matching `wanted`. Requests from the node and closures
    /// seen on the way are kept.
    async fn next(&mut self, what: &str, wanted: impl Fn(&Ev) -> bool) -> Ev {
        let deadline = Instant::now() + WAIT;
        loop {
            let event = tokio::time::timeout_at(deadline, self.events.recv())
                .await
                .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
                .unwrap_or_else(|| panic!("raw peer stopped while waiting for {what}"));
            match &event {
                Ev::Inbound(request) => self.inbound.push(request.clone()),
                Ev::Closed(cause) => self.closed.push(cause.clone()),
                _ => {}
            }
            if wanted(&event) {
                return event;
            }
        }
    }

    async fn connect(&mut self, addr: &Multiaddr) -> PeerId {
        self.send(Cmd::Dial(addr.clone()));
        match self
            .next("a connection", |e| {
                matches!(e, Ev::Connected(_) | Ev::DialError(_))
            })
            .await
        {
            Ev::Connected(peer) => peer,
            Ev::DialError(error) => panic!("dial failed: {error}"),
            _ => unreachable!(),
        }
    }

    /// The outcome of the next settled request: the response, or the failure.
    async fn outcome(&mut self) -> Result<SyncResponse, String> {
        match self
            .next("a request outcome", |e| {
                matches!(e, Ev::Response(_) | Ev::Failure(_))
            })
            .await
        {
            Ev::Response(response) => Ok(response),
            Ev::Failure(error) => Err(error),
            _ => unreachable!(),
        }
    }

    async fn request(
        &mut self,
        target: PeerId,
        request: SyncRequest,
    ) -> Result<SyncResponse, String> {
        self.send(Cmd::Request(target, request));
        self.outcome().await
    }

    /// Sends `total` height requests, at most `in_flight` unsettled at once.
    /// Returns (answered, failed).
    async fn burst(&mut self, target: PeerId, total: u64, in_flight: u64) -> (u64, u64) {
        let (mut sent, mut answered, mut failed) = (0u64, 0u64, 0u64);
        while answered + failed < total {
            while sent < total && sent - answered - failed < in_flight {
                self.send(Cmd::Request(target, SyncRequest::GetHeight));
                sent += 1;
            }
            match self.outcome().await {
                Ok(_) => answered += 1,
                Err(_) => failed += 1,
            }
        }
        (answered, failed)
    }
}

// ───────────────────────────── T1 — non-intervention ──────────────────────

/// T1. Requests that the node's existing rules admit are all served: the
/// guard adds no refusal. Three peers, eight heights each, one at a time:
/// inside the per-peer burst (8) and the per-source burst (32).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t1_every_admissible_stream_is_served() {
    const PEERS: u64 = 3;
    const EACH: u64 = 8;
    let mut node = victim(T1_P2P);
    let mut answered = 0u64;
    let mut peers = Vec::new();
    for _ in 0..PEERS {
        let mut peer = RawPeer::spawn();
        peer.connect(&node.addr).await;
        for _ in 0..EACH {
            let response = peer.request(node.peer, SyncRequest::GetHeight).await;
            assert!(
                matches!(response, Ok(SyncResponse::Height(NODE_HEIGHT))),
                "{response:?}"
            );
            answered += 1;
        }
        peers.push(peer);
    }
    // The node's own requests go out through the wrapper too.
    for peer in &peers {
        let answer = node.ask(SyncCommand::GetHeight { peer_id: peer.peer }).await;
        assert!(
            matches!(answer, Ok(SyncResponse::Height(PEER_HEIGHT))),
            "{answer:?}"
        );
    }

    let total = PEERS * EACH;
    until(&node.guard, "every stream closed", |s| {
        s.inbound_streams_active == 0 && s.inbound_streams_closed_total == total
    })
    .await;
    let seen = node.guard.snapshot();
    let counters = node.counters.snapshot();
    println!("T1: {answered}/{total} answered; guard: {seen}; node: {counters}");
    assert_eq!(answered, total);
    assert_eq!(
        seen.inbound_streams_total, total,
        "one stream per request, no other"
    );
    assert_eq!(seen.stream_negotiations_started_total, total);
    assert_eq!(seen.connections_total, PEERS);
    assert!(seen.outbound_streams_total >= PEERS, "{seen}");
    assert_settled(&seen);
    // The node's own account: all admitted, none refused by any rule.
    assert_eq!(counters.sync_requests_admitted, total);
    assert_eq!(counters.sync_requests_refused_rate, 0);
    assert_eq!(counters.sync_requests_refused_peer_quota, 0);
    assert_eq!(counters.sync_requests_refused_full, 0);
    assert_eq!(counters.inbound_failures, 0);
    for peer in &peers {
        assert!(
            peer.closed.is_empty(),
            "a connection closed: {:?}",
            peer.closed
        );
    }
}

// ───────────────────────────── T2 — transparency ──────────────────────────

/// T2. A whole session between the node and a peer built the old way:
/// connection, streams in both directions, negotiation that succeeds and
/// negotiation that fails, about 1 MiB each way, closure and reconnection.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t2_a_session_with_a_with_tcp_peer_is_unchanged() {
    let mut node = victim(T2_P2P);
    let mut peer = RawPeer::spawn();
    let expected = digest(&payload());
    let mut transcript = Vec::new();
    let mut say = |line: String| transcript.push(line);

    // Establishment.
    let connected_to = peer.connect(&node.addr).await;
    say(format!(
        "connected to the node: {}",
        connected_to == node.peer
    ));

    // Peer to node: a stream, a negotiation, a small exchange, a large one.
    let height = peer.request(node.peer, SyncRequest::GetHeight).await;
    say(format!("peer asks height: {}", describe(&height.unwrap())));
    let range = SyncRequest::GetBlocks {
        start_height: 0,
        end_height: 8,
    };
    let bulk = peer.request(node.peer, range).await;
    say(format!("peer asks blocks: {}", describe(&bulk.unwrap())));

    // Node to peer: the node's outbound streams.
    let answer = node.ask(SyncCommand::GetHeight { peer_id: peer.peer }).await;
    say(format!("node asks height: {}", describe(&answer.unwrap())));
    let answer = node
        .ask(SyncCommand::GetBlocks {
            peer_id: peer.peer,
            start_height: 0,
            end_height: 8,
        })
        .await;
    say(format!("node asks blocks: {}", describe(&answer.unwrap())));

    // A negotiation that fails leaves the connection as it was.
    peer.send(Cmd::Unknown(node.peer));
    let failure = peer.outcome().await.unwrap_err();
    say(format!("unknown protocol: {failure}"));
    let height = peer.request(node.peer, SyncRequest::GetHeight).await;
    say(format!(
        "peer asks height again: {}",
        describe(&height.unwrap())
    ));
    say(format!("closures so far: {}", peer.closed.len()));

    // Closure, then reconnection.
    peer.send(Cmd::Disconnect(node.peer));
    let closed = peer.next("the closure", |e| matches!(e, Ev::Closed(_))).await;
    say(format!("peer closes: {closed:?}"));
    let abnormal = node.disconnected(peer.peer).await;
    say(format!("node sees the peer gone: abnormal={abnormal}"));
    let connected_to = peer.connect(&node.addr).await;
    say(format!(
        "reconnected to the node: {}",
        connected_to == node.peer
    ));
    let height = peer.request(node.peer, SyncRequest::GetHeight).await;
    say(format!(
        "peer asks height after reconnecting: {}",
        describe(&height.unwrap())
    ));

    for line in &transcript {
        println!("T2 | {line}");
    }
    assert_eq!(
        transcript,
        [
            "connected to the node: true".to_string(),
            format!("peer asks height: height {NODE_HEIGHT}"),
            format!("peer asks blocks: blocks n=8 digest={expected}"),
            format!("node asks height: height {PEER_HEIGHT}"),
            format!("node asks blocks: blocks n=8 digest={expected}"),
            "unknown protocol: UnsupportedProtocols".to_string(),
            format!("peer asks height again: height {NODE_HEIGHT}"),
            "closures so far: 0".to_string(),
            "peer closes: Closed(None)".to_string(),
            "node sees the peer gone: abnormal=true".to_string(),
            "reconnected to the node: true".to_string(),
            format!("peer asks height after reconnecting: height {NODE_HEIGHT}"),
        ]
    );
    assert_eq!(
        peer.inbound.len(),
        2,
        "the node's two requests reached the peer"
    );

    // Five streams from the peer: four requests and the unknown protocol.
    until(&node.guard, "every stream closed", |s| {
        s.inbound_streams_active == 0 && s.inbound_streams_closed_total == 5
    })
    .await;
    let seen = node.guard.snapshot();
    println!("T2: guard: {seen}");
    assert_eq!(seen.inbound_streams_total, 5);
    assert_eq!(seen.stream_negotiations_started_total, 5);
    assert_eq!(seen.connections_total, 2);
    assert_settled(&seen);
}

/// T2, between two real nodes: each is guarded, each asks the other. A node
/// peer also runs ping and identify, so the guard sees those streams as well.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t2_two_guarded_nodes_talk_both_ways() {
    let mut server = victim(T2_NODES_P2P);

    let mut client = P2PNode::with_mdns(false).unwrap();
    let client_id = client.peer_id;
    let client_guard = client.resource_guard();
    let mut client_events = client.take_sync_event_rx().unwrap();
    let client_commands = client.sync_commander();
    let client_answers = client.sync_commander();
    let mut client_requests = client.take_sync_rx().unwrap();
    client.dial(&server.addr.to_string()).unwrap();
    tokio::spawn(client.run());
    tokio::spawn(async move {
        while let Some(inbound) = client_requests.recv().await {
            let _ = client_answers.send(inbound.respond(SyncResponse::Height(7)));
        }
    });
    timeout(WAIT, async {
        loop {
            if let Some(SyncEvent::PeerConnected { peer_id }) = client_events.recv().await {
                if peer_id == server.peer {
                    return;
                }
            }
        }
    })
    .await
    .expect("the nodes never connected");

    for _ in 0..3 {
        client_commands
            .send(SyncCommand::GetHeight {
                peer_id: server.peer,
            })
            .unwrap();
        let answer = timeout(WAIT, async {
            loop {
                match client_events.recv().await {
                    Some(SyncEvent::ResponseReceived { response, .. }) => return Ok(response),
                    Some(SyncEvent::RequestFailed { .. }) => return Err("request failed"),
                    Some(_) => {}
                    None => return Err("node stopped"),
                }
            }
        })
        .await
        .expect("no answer");
        assert!(
            matches!(answer, Ok(SyncResponse::Height(NODE_HEIGHT))),
            "{answer:?}"
        );
        let answer = server.ask(SyncCommand::GetHeight { peer_id: client_id }).await;
        assert!(matches!(answer, Ok(SyncResponse::Height(7))), "{answer:?}");
    }

    // Each side saw the other's three requests, and at least its ping stream.
    for (name, guard) in [("server", &server.guard), ("client", &client_guard)] {
        until(guard, "the requests' streams closed", |s| {
            s.inbound_streams_total >= 4 && s.inbound_streams_closed_total >= 3
        })
        .await;
        let seen = guard.snapshot();
        println!("T2 nodes: {name} guard: {seen}");
        assert_eq!(
            seen.streams_observation_only_total, seen.inbound_streams_total,
            "{seen}"
        );
        assert_eq!(seen.observation_errors_total, 0, "{seen}");
        assert_eq!(seen.connections_active, 1, "{seen}");
        assert_eq!(seen.muxer_errors_total, 0, "{seen}");
        assert!(seen.outbound_streams_total >= 4, "{seen}");
    }
    assert_eq!(server.counters.snapshot().sync_requests_admitted, 3);
}

// ───────────────────────────── T3 — real counting ─────────────────────────

/// T3. Openings and closings agree at every step, and the active count
/// returns to zero: after answers, after failed negotiations, and after a
/// peer leaves with streams still open.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t3_openings_and_closings_agree_and_active_returns_to_zero() {
    let node = victim(T3_P2P);
    let mut peer = RawPeer::spawn();
    peer.connect(&node.addr).await;

    // Two requests held by the node: two streams open.
    node.hold.store(true, ORD);
    peer.send(Cmd::Request(node.peer, SyncRequest::GetHeight));
    peer.send(Cmd::Request(node.peer, SyncRequest::GetHeight));
    until(&node.guard, "two streams open", |s| {
        s.inbound_streams_active == 2
    })
    .await;
    timeout(WAIT, async {
        while node.held.load(ORD) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the node never held the two requests");
    let seen = node.guard.snapshot();
    println!("T3 held: {seen}");
    assert_eq!(seen.inbound_streams_total, 2);
    assert_eq!(seen.inbound_streams_closed_total, 0);
    assert_eq!(seen.streams_per_connection_max, 2);
    assert_eq!(seen.stream_negotiations_started_total, 2);

    // Three failed negotiations come and go beside them.
    for _ in 0..3 {
        peer.send(Cmd::Unknown(node.peer));
        assert_eq!(peer.outcome().await.unwrap_err(), "UnsupportedProtocols");
    }
    until(&node.guard, "the failed streams closed", |s| {
        s.inbound_streams_closed_total == 3
    })
    .await;
    let seen = node.guard.snapshot();
    println!("T3 after failed negotiations: {seen}");
    assert_eq!(
        (seen.inbound_streams_total, seen.inbound_streams_active),
        (5, 2)
    );

    // Answered: both streams close, having lived at least the pause.
    tokio::time::sleep(Duration::from_millis(300)).await;
    node.hold.store(false, ORD);
    node.release.notify_one();
    for _ in 0..2 {
        assert!(matches!(
            peer.outcome().await,
            Ok(SyncResponse::Height(NODE_HEIGHT))
        ));
    }
    until(&node.guard, "every stream closed", |s| {
        s.inbound_streams_active == 0
    })
    .await;
    let seen = node.guard.snapshot();
    println!("T3 after answers: {seen}");
    assert_eq!(seen.inbound_streams_total, 5);
    assert_settled(&seen);
    assert_eq!(seen.streams_per_connection_max, 0);
    // The two held streams, and one failed negotiation beside them.
    assert_eq!(seen.streams_per_connection_peak, 3);
    assert!(
        seen.lifetime_sum_micros >= 600_000,
        "two streams lived 300 ms: {seen}"
    );
    assert!(
        seen.lifetime_buckets[3] + seen.lifetime_buckets[4] >= 2,
        "two lifetimes above 100 ms: {seen}"
    );

    // The peer leaves with two streams open: they are released with it.
    node.hold.store(true, ORD);
    peer.send(Cmd::Request(node.peer, SyncRequest::GetHeight));
    peer.send(Cmd::Request(node.peer, SyncRequest::GetHeight));
    until(&node.guard, "two more streams open", |s| {
        s.inbound_streams_active == 2
    })
    .await;
    peer.send(Cmd::Disconnect(node.peer));
    until(&node.guard, "the connection gone", |s| {
        s.connections_active == 0 && s.inbound_streams_active == 0
    })
    .await;
    node.hold.store(false, ORD);
    node.release.notify_one();
    let seen = node.guard.snapshot();
    println!("T3 after the peer left: {seen}");
    assert_eq!(seen.inbound_streams_total, 7);
    assert_settled(&seen);
    assert_eq!(seen.connections_total, 1);
}

// ───────────────────────────── T4 — concurrent load ───────────────────────

/// T4. Four peers, two connections each, 400 requests each with 32 unsettled
/// at once. Most are refused by the node's existing limits, which is not the
/// point: every stream opened is counted once, closed once, and nothing is
/// left over.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t4_counts_are_exact_under_concurrent_peers_and_connections() {
    const PEERS: u64 = 4;
    const EACH: u64 = 400;
    const IN_FLIGHT: u64 = 32;
    let node = victim(T4_P2P);
    let bare: Multiaddr = format!("/ip4/127.0.0.1/tcp/{T4_P2P}").parse().unwrap();

    let mut tasks = Vec::new();
    for _ in 0..PEERS {
        let (addr, bare, target) = (node.addr.clone(), bare.clone(), node.peer);
        tasks.push(tokio::spawn(async move {
            let mut peer = RawPeer::spawn();
            peer.connect(&addr).await;
            peer.send(Cmd::DialAlways(target, bare));
            peer.next("a second connection", |e| matches!(e, Ev::Connected(_))).await;
            let outcome = peer.burst(target, EACH, IN_FLIGHT).await;
            (outcome, peer)
        }));
    }
    let (mut answered, mut failed, mut closures) = (0, 0, 0);
    let mut peers = Vec::new();
    for task in tasks {
        let ((a, f), peer) = task.await.unwrap();
        answered += a;
        failed += f;
        closures += peer.closed.len();
        peers.push(peer);
    }

    let total = PEERS * EACH;
    until(&node.guard, "every stream closed", |s| {
        s.inbound_streams_active == 0 && s.inbound_streams_closed_total == total
    })
    .await;
    let seen = node.guard.snapshot();
    let counters = node.counters.snapshot();
    println!(
        "T4: {total} streams from {PEERS} peers on {} connections: {answered} answered, \
         {failed} failed at the requester, {closures} connections closed; guard: {seen}; \
         node: {counters}",
        seen.connections_total
    );
    assert_eq!(answered + failed, total);
    assert_eq!(seen.inbound_streams_total, total, "counted to the unit");
    assert_eq!(seen.connections_total, PEERS * 2);
    assert_eq!(
        seen.connections_active,
        PEERS * 2,
        "no connection was closed"
    );
    assert_eq!(closures, 0);
    assert!(
        seen.inbound_streams_active_peak >= 2,
        "streams did overlap: {seen}"
    );
    assert!(seen.streams_per_connection_peak >= 2, "{seen}");
    assert_settled(&seen);
    // Whatever was decoded was admitted or refused by an existing rule, once.
    let decoded = counters.sync_requests_admitted
        + counters.sync_requests_refused_rate
        + counters.sync_requests_refused_peer_quota
        + counters.sync_requests_refused_full;
    assert!(decoded <= total, "{counters}");
    assert_eq!(answered, counters.sync_requests_admitted, "{counters}");
}

// ───────────────────────────── T5 — persistent connection ─────────────────

/// T5. One connection kept for a quarter of a minute, a stream opened and
/// closed on it again and again. After each round the counts are exact and
/// the connection is the same one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t5_a_long_connection_keeps_exact_counts_round_after_round() {
    const ROUNDS: u64 = 24;
    let node = victim(T5_P2P);
    let mut peer = RawPeer::spawn();
    peer.connect(&node.addr).await;
    let started = Instant::now();
    for round in 1..=ROUNDS {
        let response = peer.request(node.peer, SyncRequest::GetHeight).await;
        assert!(
            matches!(response, Ok(SyncResponse::Height(NODE_HEIGHT))),
            "round {round}: {response:?}"
        );
        until(&node.guard, "the round's stream closed", |s| {
            s.inbound_streams_closed_total == round
        })
        .await;
        let seen = node.guard.snapshot();
        assert_eq!(seen.inbound_streams_total, round, "round {round}: {seen}");
        assert_eq!(seen.inbound_streams_active, 0, "round {round}: {seen}");
        assert_eq!(
            (seen.connections_active, seen.connections_total),
            (1, 1),
            "round {round}: {seen}"
        );
        // Under the per-peer height rate of 2 a second.
        tokio::time::sleep(Duration::from_millis(550)).await;
    }
    let seen = node.guard.snapshot();
    println!(
        "T5: {ROUNDS} rounds on one connection in {:.1} s; guard: {seen}",
        started.elapsed().as_secs_f64()
    );
    assert!(peer.closed.is_empty(), "{:?}", peer.closed);
    assert_eq!(seen.streams_per_connection_peak, 1);
    assert_eq!(seen.muxer_errors_total, 0);
    assert_settled(&seen);
    assert_eq!(node.counters.snapshot().sync_requests_admitted, ROUNDS);
}

// ───────────────────────────── T8 — the mode cannot change ────────────────

/// T8, on the real node. With every plausible switch set in the environment
/// and a burst far past the node's limits, the guard still passes every
/// stream on: refusals come from the rules that already existed, after the
/// guard, never from it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t8_nothing_makes_the_node_guard_refuse_a_stream() {
    const TOTAL: u64 = 300;
    for (name, value) in [
        ("MBONGO_P2P_GUARD_MODE", "enforce"),
        ("MBONGO_GUARD_MODE", "enforce"),
        ("P2P_GUARD_MODE", "refuse"),
        ("MBONGO_P2P_GUARD_ENFORCE", "1"),
        ("MBONGO_P2P_STREAM_LIMIT", "1"),
        ("OBSERVE_ONLY", "0"),
    ] {
        std::env::set_var(name, value);
    }
    let node = victim(T8_P2P);
    match node.guard.mode() {
        GuardMode::ObserveOnly => {}
    }
    let mut peer = RawPeer::spawn();
    peer.connect(&node.addr).await;
    let (answered, failed) = peer.burst(node.peer, TOTAL, 64).await;

    until(&node.guard, "every stream closed", |s| {
        s.inbound_streams_active == 0 && s.inbound_streams_closed_total == TOTAL
    })
    .await;
    let seen = node.guard.snapshot();
    let counters = node.counters.snapshot();
    println!("T8: {answered} answered, {failed} failed; guard: {seen}; node: {counters}");
    assert_eq!(seen.inbound_streams_total, TOTAL);
    assert_eq!(
        seen.streams_observation_only_total, TOTAL,
        "every stream was passed on"
    );
    assert_eq!(seen.mode, GuardMode::ObserveOnly);
    assert!(
        peer.closed.is_empty(),
        "the connection was closed: {:?}",
        peer.closed
    );
    assert!(
        failed > 0,
        "the existing limits did refuse some of the burst"
    );
    assert_settled(&seen);
}

// ───────────────────────────── negotiations started, not succeeded ────────

/// The negotiation counter moves when a negotiation starts, and says nothing
/// of how it ends. On the real node: one request on the sync protocol, whose
/// negotiation succeeds, and two on a protocol the node does not have, whose
/// negotiation fails. Three negotiations started; the node's own counters
/// know of one request.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn negotiations_started_include_those_that_fail() {
    let node = victim(NEGOTIATION_P2P);
    let mut peer = RawPeer::spawn();
    peer.connect(&node.addr).await;

    let answered = peer.request(node.peer, SyncRequest::GetHeight).await;
    assert!(
        matches!(answered, Ok(SyncResponse::Height(NODE_HEIGHT))),
        "{answered:?}"
    );
    for _ in 0..2 {
        peer.send(Cmd::Unknown(node.peer));
        assert_eq!(peer.outcome().await.unwrap_err(), "UnsupportedProtocols");
    }

    until(&node.guard, "every stream closed", |s| {
        s.inbound_streams_closed_total == 3
    })
    .await;
    let seen = node.guard.snapshot();
    let counters = node.counters.snapshot();
    println!("negotiations: guard: {seen}; node: {counters}");
    assert_eq!(seen.inbound_streams_total, 3);
    assert_eq!(
        seen.stream_negotiations_started_total, 3,
        "started, whatever the outcome"
    );
    assert_eq!(
        counters.sync_requests_admitted, 1,
        "one of the three negotiations led to a request"
    );
    assert_settled(&seen);
    assert!(peer.closed.is_empty(), "{:?}", peer.closed);
}

// ───────────────────────────── measurements (ignored) ─────────────────────

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let rank = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn summary(mut micros: Vec<u64>) -> String {
    micros.sort_unstable();
    format!(
        "n={} p50_us={} p95_us={} p99_us={} max_us={}",
        micros.len(),
        percentile(&micros, 0.50),
        percentile(&micros, 0.95),
        percentile(&micros, 0.99),
        micros.last().copied().unwrap_or(0)
    )
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn env_nodelay() -> Option<bool> {
    match std::env::var("PHASE1_NODELAY").as_deref() {
        Ok("1") => Some(true),
        Ok("0") => Some(false),
        _ => None,
    }
}

/// T7. Height requests to an external node, from several identities, each one
/// request every `PHASE1_INTERVAL_MS` (default 600, under the node's per-peer
/// rate), for `PHASE1_SECONDS`. Prints the latency distribution.
///
/// `PHASE1_TARGET` is the node's address, e.g. `/ip4/172.31.250.20/tcp/30333`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "measurement against an external node; set PHASE1_TARGET"]
async fn probe_latency() {
    let target: Multiaddr = std::env::var("PHASE1_TARGET").expect("PHASE1_TARGET").parse().unwrap();
    let seconds = env_u64("PHASE1_SECONDS", 30);
    let interval = Duration::from_millis(env_u64("PHASE1_INTERVAL_MS", 600));
    let identities = env_u64("PHASE1_IDENTITIES", 4);
    let label = std::env::var("PHASE1_LABEL").unwrap_or_default();
    let nodelay = env_nodelay();

    let mut tasks = Vec::new();
    for _ in 0..identities {
        let target = target.clone();
        tasks.push(tokio::spawn(async move {
            // Height 0: the node never asks this peer for blocks.
            let mut peer = RawPeer::spawn_full(nodelay, None, 0, Vec::new());
            let node = peer.connect(&target).await;
            let (mut micros, mut failed) = (Vec::new(), 0u64);
            let end = Instant::now() + Duration::from_secs(seconds);
            let mut tick = tokio::time::interval(interval);
            while Instant::now() < end {
                tick.tick().await;
                let sent = Instant::now();
                match peer.request(node, SyncRequest::GetHeight).await {
                    Ok(_) => micros.push(u64::try_from(sent.elapsed().as_micros()).unwrap()),
                    Err(_) => failed += 1,
                }
            }
            (micros, failed, peer.closed.len())
        }));
    }
    let (mut micros, mut failed, mut closures) = (Vec::new(), 0, 0);
    for task in tasks {
        let (m, f, c) = task.await.unwrap();
        micros.extend(m);
        failed += f;
        closures += c;
    }
    let answered = micros.len();
    let stats = summary(micros);
    println!(
        "PHASE1-LATENCY label={label} client_nodelay={nodelay:?} identities={identities} \
         seconds={seconds} interval_ms={} answered={answered} failed={failed} \
         closures={closures} {stats}",
        interval.as_millis(),
    );
}

/// T2, against an external node: the same session on any build, printed as a
/// transcript so that two builds can be compared line by line.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "session against an external node; set PHASE1_TARGET"]
async fn probe_session() {
    let target: Multiaddr = std::env::var("PHASE1_TARGET").expect("PHASE1_TARGET").parse().unwrap();
    // Answers height 0 to the node, so that it never asks this peer for blocks.
    let mut peer = RawPeer::spawn_full(None, None, 0, Vec::new());
    let pause = Duration::from_millis(600);
    let say = |line: String| println!("PHASE1-SESSION {line}");

    let node = peer.connect(&target).await;
    say("connected".to_string());
    for i in 0..3 {
        let response = peer.request(node, SyncRequest::GetHeight).await;
        // The height moves on a producing node; only the kind is compared.
        say(format!(
            "height request {i}: answered={}",
            matches!(response, Ok(SyncResponse::Height(_)))
        ));
        tokio::time::sleep(pause).await;
    }
    let range = SyncRequest::GetBlocks {
        start_height: 0,
        end_height: 1,
    };
    let response = peer.request(node, range).await;
    say(format!(
        "genesis range: {}",
        response.as_ref().map_or_else(Clone::clone, describe)
    ));
    peer.send(Cmd::Unknown(node));
    say(format!("unknown protocol: {:?}", peer.outcome().await));
    tokio::time::sleep(pause).await;
    let response = peer.request(node, SyncRequest::GetHeight).await;
    say(format!(
        "height after a failed negotiation: answered={}",
        response.is_ok()
    ));
    say(format!(
        "closures before disconnecting: {}",
        peer.closed.len()
    ));
    peer.send(Cmd::Disconnect(node));
    let closed = peer.next("the closure", |e| matches!(e, Ev::Closed(_))).await;
    say(format!("closed by this peer: {closed:?}"));
    tokio::time::sleep(pause).await;
    let again = peer.connect(&target).await;
    say(format!("reconnected to the same node: {}", again == node));
    tokio::time::sleep(pause).await;
    let response = peer.request(node, SyncRequest::GetHeight).await;
    say(format!(
        "height after reconnecting: answered={}",
        response.is_ok()
    ));
    // The node asks a new peer its height once per connection.
    tokio::time::sleep(Duration::from_secs(1)).await;
    while let Ok(event) = peer.events.try_recv() {
        if let Ev::Inbound(request) = event {
            peer.inbound.push(request);
        }
    }
    let heights = peer.inbound.iter().filter(|r| matches!(r, SyncRequest::GetHeight)).count();
    say(format!(
        "requests from the node: {} of which height requests: {heights}",
        peer.inbound.len()
    ));
}

/// A muxer that always has a stream ready, and a stream that does nothing:
/// what is left when measured is the wrapper.
struct NullMuxer;
struct NullStream;

impl AsyncRead for NullStream {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Ok(0))
    }
}

impl AsyncWrite for NullStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(Ok(buf.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl StreamMuxer for NullMuxer {
    type Substream = NullStream;
    type Error = std::io::Error;

    fn poll_inbound(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<NullStream, std::io::Error>> {
        Poll::Ready(Ok(NullStream))
    }
    fn poll_outbound(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<NullStream, std::io::Error>> {
        Poll::Ready(Ok(NullStream))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<StreamMuxerEvent, std::io::Error>> {
        Poll::Pending
    }
}

/// One stream's life as the swarm drives it: taken from the muxer, read once,
/// written once, dropped. Returns nanoseconds per stream.
fn cycle<M>(muxer: &mut M, streams: u64) -> f64
where
    M: StreamMuxer + Unpin,
    M::Substream: AsyncRead + AsyncWrite + Unpin,
    M::Error: std::fmt::Debug,
{
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut buf = [0u8; 32];
    let started = std::time::Instant::now();
    for _ in 0..streams {
        let Poll::Ready(Ok(mut stream)) = Pin::new(&mut *muxer).poll_inbound(&mut cx) else {
            panic!("no stream");
        };
        let _ = std::hint::black_box(Pin::new(&mut stream).poll_read(&mut cx, &mut buf));
        let _ = std::hint::black_box(Pin::new(&mut stream).poll_write(&mut cx, &buf));
        drop(std::hint::black_box(stream));
    }
    #[allow(clippy::cast_precision_loss)]
    let per_stream = started.elapsed().as_nanos() as f64 / streams as f64;
    per_stream
}

fn listener() -> ConnectedPoint {
    let addr: Multiaddr = "/ip4/127.0.0.1/tcp/1".parse().unwrap();
    ConnectedPoint::Listener {
        local_addr: addr.clone(),
        send_back_addr: addr,
    }
}

/// T7. The cost of observing one stream, with the network taken away: the
/// same loop over a bare muxer and over the guarded one, alone and with four
/// threads sharing one guard. Run with `--release`.
#[test]
#[ignore = "micro-benchmark; run with --release"]
fn bench_observation_cost() {
    let streams = env_u64("PHASE1_STREAMS", 2_000_000);
    let runs = env_u64("PHASE1_RUNS", 5);
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };

    let (mut bare, mut guarded) = (Vec::new(), Vec::new());
    for _ in 0..runs {
        bare.push(cycle(&mut NullMuxer, streams));
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(NullMuxer, &listener());
        guarded.push(cycle(&mut muxer, streams));
        assert_eq!(guard.snapshot().inbound_streams_total, streams);
    }
    let (bare, guarded) = (median(bare), median(guarded));
    println!(
        "PHASE1-BENCH threads=1 streams={streams} runs={runs} bare_ns={bare:.1} \
         guarded_ns={guarded:.1} observation_ns={:.1}",
        guarded - bare
    );

    // Four connections on four threads, one guard: the shared counters are
    // contended, as they are when several connections flood at once.
    const THREADS: u64 = 4;
    let mut contended = Vec::new();
    for _ in 0..runs {
        let guard = P2pResourceGuard::new();
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let guard = Arc::clone(&guard);
                std::thread::spawn(move || {
                    let mut muxer = guard.observe(NullMuxer, &listener());
                    cycle(&mut muxer, streams)
                })
            })
            .collect();
        let per_thread: Vec<f64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        #[allow(clippy::cast_precision_loss)]
        contended.push(per_thread.iter().sum::<f64>() / THREADS as f64);
        let seen = guard.snapshot();
        assert_eq!(seen.inbound_streams_total, streams * THREADS);
        assert_eq!(seen.inbound_streams_active, 0);
        assert_eq!(seen.observation_errors_total, 0);
    }
    let contended = median(contended);
    println!(
        "PHASE1-BENCH threads={THREADS} streams={streams} runs={runs} guarded_ns={contended:.1} \
         observation_ns={:.1}",
        contended - bare
    );
}

// ───────────────────────────── TCP_NODELAY investigation (ignored) ────────

/// Sequential height requests from a raw peer; microseconds each.
async fn small_exchanges(peer: &mut RawPeer, target: PeerId, n: usize, gap: Duration) -> Vec<u64> {
    let mut micros = Vec::new();
    for _ in 0..n {
        let sent = Instant::now();
        let response = peer.request(target, SyncRequest::GetHeight).await;
        assert!(response.is_ok(), "{response:?}");
        micros.push(u64::try_from(sent.elapsed().as_micros()).unwrap());
        tokio::time::sleep(gap).await;
    }
    micros
}

/// Block ranges fetched one after the other; MiB per second of answer.
async fn bulk_exchanges(peer: &mut RawPeer, target: PeerId, n: usize, gap: Duration) -> f64 {
    let (mut bytes, mut busy) = (0usize, Duration::ZERO);
    for _ in 0..n {
        let range = SyncRequest::GetBlocks {
            start_height: 0,
            end_height: 8,
        };
        let sent = Instant::now();
        let response = peer.request(target, range).await;
        busy += sent.elapsed();
        match response {
            Ok(SyncResponse::Blocks(blocks)) => bytes += blocks.encode().len(),
            other => panic!("no blocks: {other:?}"),
        }
        tokio::time::sleep(gap).await;
    }
    #[allow(clippy::cast_precision_loss)]
    let rate = bytes as f64 / (1024.0 * 1024.0) / busy.as_secs_f64();
    rate
}

/// The TCP_NODELAY question, on loopback. The node is used as it is built
/// (libp2p's default TCP configuration); only the raw peers, which are test
/// code, change the option.
///
/// Cases: the real node as server with a default client, a `nodelay` client
/// and a real node as client; the real node as client of a raw server with
/// and without `nodelay`; and the four raw combinations, with small and
/// large exchanges. Meaningful on Linux; run with `--release`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "TCP_NODELAY measurement; run with --release, on Linux"]
async fn nodelay_matrix() {
    let small = usize::try_from(env_u64("PHASE1_SMALL", 300)).unwrap();
    let large = usize::try_from(env_u64("PHASE1_LARGE", 20)).unwrap();
    let label = |flag: Option<bool>| match flag {
        None => "default",
        Some(true) => "nodelay",
        Some(false) => "nagle",
    };

    // The real node as server. Its request rates bound the sample: one
    // height every 600 ms, one small range every second.
    let node = victim(NODELAY_P2P);
    for client in [None, Some(true)] {
        let mut peer = RawPeer::spawn_with(client, None, Vec::new());
        peer.connect(&node.addr).await;
        let micros = small_exchanges(&mut peer, node.peer, 30, Duration::from_millis(600)).await;
        let rate = bulk_exchanges(&mut peer, node.peer, 5, Duration::from_secs(1)).await;
        println!(
            "PHASE1-NODELAY server=node(default) client=raw({}) small: {} bulk_mib_s={rate:.1}",
            label(client),
            summary(micros)
        );
    }

    // The real node as client, of raw servers.
    for server in [None, Some(true)] {
        let mut raw = RawPeer::spawn_with(server, Some("/ip4/127.0.0.1/tcp/0"), payload());
        let Ev::Listening(addr) = raw.next("listening", |e| matches!(e, Ev::Listening(_))).await
        else {
            unreachable!()
        };
        let mut client = P2PNode::with_mdns(false).unwrap();
        let mut events = client.take_sync_event_rx().unwrap();
        let commands = client.sync_commander();
        client.dial(&addr.to_string()).unwrap();
        tokio::spawn(client.run());
        let ask = |command: SyncCommand| {
            commands.send(command).unwrap();
        };
        let mut micros = Vec::new();
        timeout(WAIT, async {
            loop {
                if let Some(SyncEvent::PeerConnected { .. }) = events.recv().await {
                    return;
                }
            }
        })
        .await
        .expect("the node never connected to the raw server");
        for _ in 0..30 {
            let sent = Instant::now();
            ask(SyncCommand::GetHeight { peer_id: raw.peer });
            timeout(WAIT, async {
                loop {
                    if let Some(SyncEvent::ResponseReceived { .. }) = events.recv().await {
                        return;
                    }
                }
            })
            .await
            .expect("no answer to the node");
            micros.push(u64::try_from(sent.elapsed().as_micros()).unwrap());
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        println!(
            "PHASE1-NODELAY server=raw({}) client=node(default) small: {}",
            label(server),
            summary(micros)
        );
    }

    // Real node to real node: both as built.
    {
        let mut client = P2PNode::with_mdns(false).unwrap();
        let mut events = client.take_sync_event_rx().unwrap();
        let commands = client.sync_commander();
        client.dial(&node.addr.to_string()).unwrap();
        tokio::spawn(client.run());
        timeout(WAIT, async {
            loop {
                if let Some(SyncEvent::PeerConnected { .. }) = events.recv().await {
                    return;
                }
            }
        })
        .await
        .expect("the nodes never connected");
        let mut micros = Vec::new();
        for _ in 0..30 {
            let sent = Instant::now();
            commands.send(SyncCommand::GetHeight { peer_id: node.peer }).unwrap();
            timeout(WAIT, async {
                loop {
                    if let Some(SyncEvent::ResponseReceived { .. }) = events.recv().await {
                        return;
                    }
                }
            })
            .await
            .expect("no answer between nodes");
            micros.push(u64::try_from(sent.elapsed().as_micros()).unwrap());
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
        println!(
            "PHASE1-NODELAY server=node(default) client=node(default) small: {}",
            summary(micros)
        );
    }

    // Raw to raw: every combination, small and large.
    for server in [None, Some(true)] {
        for client in [None, Some(true)] {
            let mut raw =
                RawPeer::spawn_with(server, Some("/ip4/127.0.0.1/tcp/0"), blocks(32, 900));
            let Ev::Listening(addr) =
                raw.next("listening", |e| matches!(e, Ev::Listening(_))).await
            else {
                unreachable!()
            };
            let mut peer = RawPeer::spawn_with(client, None, Vec::new());
            let target = peer.connect(&addr).await;
            let micros = small_exchanges(&mut peer, target, small, Duration::ZERO).await;
            let rate = bulk_exchanges(&mut peer, target, large, Duration::ZERO).await;
            println!(
                "PHASE1-NODELAY server=raw({}) client=raw({}) small: {} bulk_mib_s={rate:.1}",
                label(server),
                label(client),
                summary(micros)
            );
        }
    }
}
