//! The chain binding on the real `P2PNode` (RFC 0006 §7, S1-9).
//!
//! A node speaks `/mbongo-sync/3/<c>` and `/mbongo/block_notify/0.3.0/<c>` for
//! the chain id it was built with. Nodes of one network exchange requests and
//! blocks; nodes of two networks connect, below the protocols, and exchange
//! nothing: every request and every announcement between them fails, and
//! neither side's queues ever see the other's traffic. Identify stays
//! informational: a peer that advertises another version is still served.
//!
//! `protocol_negotiation.rs` proves, codec by codec, that such failures happen
//! at negotiation with no payload byte exchanged; here the identifiers are
//! the ones the node itself builds.

use std::iter;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{identify, identity, noise, tcp, yamux, Multiaddr, PeerId};
use mbongo_core::{Address, Block, BlockBody, BlockHeader, ChainId, Hash};
use mbongo_network::{
    sync_protocol, BlockBroadcaster, ChannelBroadcaster, InboundBlock, P2PCounters, P2PNode,
    SyncCodec, SyncCommand, SyncEvent, SyncRequest, SyncRequestKind, SyncResponse,
};
use parity_scale_codec::Encode;
use tokio::sync::mpsc::{Receiver, UnboundedReceiver, UnboundedSender};
use tokio::time::timeout;

/// Two networks.
const A: ChainId = ChainId([0xa1; 32]);
const B: ChainId = ChainId([0xb2; 32]);

/// What a node answers to a height request.
const HEIGHT: u64 = 42;

const WAIT: Duration = Duration::from_secs(30);
/// How long nothing more may arrive, once the failures that prove each
/// attempt was made have been seen.
const QUIET: Duration = Duration::from_secs(2);

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers them.
const SAME_NETWORK_P2P: u16 = 27151;
const OTHER_NETWORK_P2P: u16 = 27152;
const IDENTIFY_P2P: u16 = 27153;

/// A running `P2PNode`, with the handles the tests use.
struct Node {
    peer: PeerId,
    events: UnboundedReceiver<SyncEvent>,
    commands: UnboundedSender<SyncCommand>,
    blocks: Receiver<InboundBlock>,
    broadcaster: ChannelBroadcaster,
    counters: Arc<P2PCounters>,
    /// Sync requests that reached this node. It answers each one `HEIGHT`.
    requests: Arc<AtomicU64>,
}

/// Starts a node of `chain`, listening on `port` if given, with `bootnode`
/// (dialed, and redialed until it answers) if given.
fn start(chain: &ChainId, port: Option<u16>, bootnode: Option<&str>) -> Node {
    let mut node = P2PNode::with_mdns(chain, false).unwrap();
    if let Some(port) = port {
        node.listen_on_host(Ipv4Addr::LOCALHOST, port).unwrap();
    }
    if let Some(addr) = bootnode {
        node.add_bootnode(addr).unwrap();
    }
    let peer = node.peer_id;
    let events = node.take_sync_event_rx().unwrap();
    let commands = node.sync_commander();
    let answers = node.sync_commander();
    let blocks = node.take_block_rx().unwrap();
    let mut inbound = node.take_sync_rx().unwrap();
    let broadcaster = node.broadcaster();
    let counters = node.counters();
    let requests = Arc::new(AtomicU64::new(0));
    let seen = Arc::clone(&requests);
    tokio::spawn(node.run());
    tokio::spawn(async move {
        while let Some(request) = inbound.recv().await {
            seen.fetch_add(1, Ordering::SeqCst);
            let _ = answers.send(request.respond(SyncResponse::Height(HEIGHT)));
        }
    });
    Node {
        peer,
        events,
        commands,
        blocks,
        broadcaster,
        counters,
        requests,
    }
}

fn address(port: u16, peer: PeerId) -> String {
    format!("/ip4/127.0.0.1/tcp/{port}/p2p/{peer}")
}

impl Node {
    fn requests(&self) -> u64 {
        self.requests.load(Ordering::SeqCst)
    }

    /// Waits for the next event that `wanted` accepts, skipping others.
    async fn event(&mut self, what: &str, wanted: impl Fn(&SyncEvent) -> bool) -> SyncEvent {
        timeout(WAIT, async {
            loop {
                let event = self.events.recv().await.expect("node stopped");
                if wanted(&event) {
                    return event;
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no {what} in time"))
    }

    async fn connected_to(&mut self, peer: PeerId) {
        self.event(
            "connection",
            |e| matches!(e, SyncEvent::PeerConnected { peer_id } if *peer_id == peer),
        )
        .await;
    }

    async fn ask_height(&mut self, peer: PeerId) -> SyncEvent {
        self.commands.send(SyncCommand::GetHeight { peer_id: peer }).unwrap();
        self.event("answer or failure", |e| {
            matches!(
                e,
                SyncEvent::ResponseReceived { peer_id, .. }
                    | SyncEvent::RequestFailed { peer_id, .. } if *peer_id == peer
            )
        })
        .await
    }

    async fn until_outbound_failures(&self, at_least: u64) {
        timeout(WAIT, async {
            while self.counters.snapshot().outbound_failures < at_least {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("the outbound failures were not reported in time");
    }
}

fn block(height: u64) -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash([9; 32]),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: 1,
            height,
            producer: Address::zero(),
        },
        body: BlockBody {
            transactions: vec![],
        },
        producer_signature: [0u8; 64],
    }
}

/// Positive control for the next test: two nodes of one network answer each
/// other's requests and receive each other's blocks.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nodes_of_one_network_exchange_requests_and_blocks() {
    let mut server = start(&A, Some(SAME_NETWORK_P2P), None);
    let mut client = start(&A, None, Some(&address(SAME_NETWORK_P2P, server.peer)));
    client.connected_to(server.peer).await;
    server.connected_to(client.peer).await;

    let answer = client.ask_height(server.peer).await;
    assert!(
        matches!(
            answer,
            SyncEvent::ResponseReceived {
                response: SyncResponse::Height(HEIGHT),
                ..
            }
        ),
        "{answer:?}"
    );
    assert_eq!(server.requests(), 1);

    let sent = block(7);
    server.broadcaster.broadcast(sent.clone());
    let received = timeout(WAIT, client.blocks.recv())
        .await
        .expect("the block did not arrive in time")
        .expect("block queue closed");
    assert_eq!(received.peer, server.peer);
    assert_eq!(received.block.encode(), sent.encode());

    assert_eq!(server.counters.snapshot().outbound_failures, 0);
    assert_eq!(client.counters.snapshot().outbound_failures, 0);
}

/// Two nodes of different networks connect, and exchange nothing: the
/// request fails, the announcement fails, and neither node's queues see the
/// other's traffic.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nodes_of_two_networks_connect_but_exchange_nothing() {
    let mut server = start(&A, Some(OTHER_NETWORK_P2P), None);
    let mut client = start(&B, None, Some(&address(OTHER_NETWORK_P2P, server.peer)));
    // Connectivity below the protocols is allowed (refusing the peer itself
    // is D-7, not S1-9).
    client.connected_to(server.peer).await;
    server.connected_to(client.peer).await;

    // B asks A: the request fails.
    let answer = client.ask_height(server.peer).await;
    assert!(
        matches!(
            answer,
            SyncEvent::RequestFailed {
                kind: SyncRequestKind::Height,
                ..
            }
        ),
        "{answer:?}"
    );
    client.until_outbound_failures(1).await;

    // A announces a block to B: the announcement fails.
    server.broadcaster.broadcast(block(7));
    server.until_outbound_failures(1).await;

    // Nothing reached either side, then or since.
    tokio::time::sleep(QUIET).await;
    assert_eq!(server.requests(), 0, "a request crossed networks");
    assert_eq!(client.requests(), 0, "a request crossed networks");
    assert!(
        client.blocks.try_recv().is_err(),
        "a block crossed networks"
    );
    assert!(
        server.blocks.try_recv().is_err(),
        "a block crossed networks"
    );
    for (side, node) in [("A", &server), ("B", &client)] {
        let seen = node.counters.snapshot();
        assert_eq!(seen.sync_requests_admitted, 0, "{side}: {seen}");
        assert_eq!(seen.announcements_admitted, 0, "{side}: {seen}");
        assert_eq!(seen.announcements_acknowledged, 0, "{side}: {seen}");
    }
}

#[derive(NetworkBehaviour)]
struct Probe {
    identify: identify::Behaviour,
    /// The node's own network.
    own: request_response::Behaviour<SyncCodec>,
    /// Another network.
    foreign: request_response::Behaviour<SyncCodec>,
}

/// Identify is informational. A peer that advertises another protocol version
/// is served on the node's network and refused at negotiation on another one,
/// exactly as any peer; the node itself advertises `/mbongo/0.5.0`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn identify_is_informational() {
    let server = start(&A, Some(IDENTIFY_P2P), None);
    let mut probe =
        libp2p::SwarmBuilder::with_existing_identity(identity::Keypair::generate_ed25519())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .unwrap()
            .with_behaviour(|key| Probe {
                identify: identify::Behaviour::new(identify::Config::new(
                    "/mbongo/0.4.0".to_string(),
                    key.public(),
                )),
                own: request_response::Behaviour::new(
                    iter::once((sync_protocol(&A), ProtocolSupport::Full)),
                    request_response::Config::default(),
                ),
                foreign: request_response::Behaviour::new(
                    iter::once((sync_protocol(&B), ProtocolSupport::Full)),
                    request_response::Config::default(),
                ),
            })
            .unwrap()
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(30)))
            .build();

    let target: Multiaddr = address(IDENTIFY_P2P, server.peer).parse().unwrap();
    let (mut own, mut foreign, mut advertised) = (None, None, None);
    timeout(WAIT, async {
        probe.dial(target.clone()).unwrap();
        while own.is_none() || foreign.is_none() || advertised.is_none() {
            match probe.select_next_some().await {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    probe.behaviour_mut().own.send_request(&peer_id, SyncRequest::GetHeight);
                    probe.behaviour_mut().foreign.send_request(&peer_id, SyncRequest::GetHeight);
                }
                // The node may not listen yet: dial again.
                SwarmEvent::OutgoingConnectionError { .. } => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    probe.dial(target.clone()).unwrap();
                }
                SwarmEvent::Behaviour(ProbeEvent::Identify(identify::Event::Received {
                    info,
                    ..
                })) => advertised = Some(info.protocol_version),
                SwarmEvent::Behaviour(ProbeEvent::Own(request_response::Event::Message {
                    message: request_response::Message::Response { response, .. },
                    ..
                })) => own = Some(Ok(response)),
                SwarmEvent::Behaviour(ProbeEvent::Own(
                    request_response::Event::OutboundFailure { error, .. },
                )) => own = Some(Err(error)),
                SwarmEvent::Behaviour(ProbeEvent::Foreign(
                    request_response::Event::OutboundFailure { error, .. },
                )) => foreign = Some(Err(error)),
                SwarmEvent::Behaviour(ProbeEvent::Foreign(request_response::Event::Message {
                    message: request_response::Message::Response { response, .. },
                    ..
                })) => foreign = Some(Ok(response)),
                _ => {}
            }
        }
    })
    .await
    .expect("the probe did not finish in time");

    assert_eq!(advertised.as_deref(), Some("/mbongo/0.5.0"));
    assert!(
        matches!(own, Some(Ok(SyncResponse::Height(HEIGHT)))),
        "own network: {own:?}"
    );
    assert!(
        matches!(
            foreign,
            Some(Err(request_response::OutboundFailure::UnsupportedProtocols))
        ),
        "another network: {foreign:?}"
    );
    assert_eq!(server.requests(), 1, "only the own-network request arrived");
}
