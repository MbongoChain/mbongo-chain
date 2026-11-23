//! Protocol negotiation across networks and versions (RFC 0006 §7, S1-9;
//! RFC 0002 Phase 4 before it).
//!
//! The sync and block-notify identifiers are bound to the chain id:
//! `/mbongo-sync/3/<c>` and `/mbongo/block_notify/0.3.0/<c>`, with `<c>` the
//! first 16 lowercase hex characters of `chain_id`. Two swarms that speak the
//! identifiers of different networks, or an identifier of an earlier version,
//! or any near miss of the right one, share no protocol: the request fails at
//! libp2p multistream-select negotiation with `UnsupportedProtocols`. Neither
//! side carries a fallback list, so there is no silent downgrade.
//!
//! Every swarm here counts its codec's calls. A failure must leave both counts
//! at zero: no payload byte was encoded or decoded, so no block was. Each
//! failure has a positive control on the same identifiers that does reach the
//! codec, so a failure comes from the identifiers, not from the harness.

use std::io;
use std::iter;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite, StreamExt};
use libp2p::request_response::{self, Codec, ProtocolSupport};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, StreamProtocol, Swarm};
use mbongo_core::{Block, BlockBody, BlockHeader, ChainId, Hash};
use mbongo_network::{
    block_notify_protocol, chain_prefix, sync_protocol, Announcement, BlockNotifyAck,
    BlockNotifyCodec, SyncCodec, SyncNotification, SyncRequest, SyncResponse,
};

/// Two networks. `A` is the one "this node" runs.
const A: ChainId = ChainId([0xa1; 32]);
const B: ChainId = ChainId([0xb2; 32]);

/// Overall per-test deadline: the failure must be bounded.
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A codec that counts every call into it, then hands the call on.
#[derive(Clone, Default)]
struct Counting<C> {
    inner: C,
    calls: Arc<AtomicUsize>,
}

impl<C> Counting<C> {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn count(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl<C> Codec for Counting<C>
where
    C: Codec<Protocol = StreamProtocol> + Clone + Send,
    C::Request: Send,
    C::Response: Send,
{
    type Protocol = StreamProtocol;
    type Request = C::Request;
    type Response = C::Response;

    async fn read_request<T>(&mut self, p: &StreamProtocol, io: &mut T) -> io::Result<C::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        self.count();
        self.inner.read_request(p, io).await
    }

    async fn read_response<T>(&mut self, p: &StreamProtocol, io: &mut T) -> io::Result<C::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        self.count();
        self.inner.read_response(p, io).await
    }

    async fn write_request<T>(
        &mut self,
        p: &StreamProtocol,
        io: &mut T,
        req: C::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self.count();
        self.inner.write_request(p, io, req).await
    }

    async fn write_response<T>(
        &mut self,
        p: &StreamProtocol,
        io: &mut T,
        res: C::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        self.count();
        self.inner.write_response(p, io, res).await
    }
}

type Peer<C> = Swarm<request_response::Behaviour<Counting<C>>>;

/// A minimal swarm speaking exactly one identifier through `codec`, on the
/// transport stack of `P2PNode` (tcp/noise/yamux).
fn make_swarm<C>(protocol: StreamProtocol, codec: Counting<C>) -> Peer<C>
where
    C: Codec<Protocol = StreamProtocol> + Clone + Send + 'static,
    C::Request: Send,
    C::Response: Send,
{
    libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .expect("tcp transport")
        .with_behaviour(|_| {
            request_response::Behaviour::with_codec(
                codec,
                iter::once((protocol, ProtocolSupport::Full)),
                request_response::Config::default(),
            )
        })
        .expect("behaviour")
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(20)))
        .build()
}

/// Drives `swarm` until it reports a listen address, returning it.
async fn listen_addr<C>(swarm: &mut Peer<C>) -> Multiaddr
where
    C: Codec<Protocol = StreamProtocol> + Clone + Send + 'static,
    C::Request: Send,
    C::Response: Send,
{
    swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap()).unwrap();
    loop {
        if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
            return address;
        }
    }
}

/// What one exchange showed.
struct Outcome<R> {
    /// The response, or the outbound failure the dialer saw.
    result: Result<R, request_response::OutboundFailure>,
    /// Whether the two swarms connected, below the protocols.
    connected: bool,
    /// Codec calls on the listener and on the dialer.
    calls: (usize, usize),
}

/// A listener speaking `listening` and a dialer speaking `dialing`: the dialer
/// connects, sends `request`, and the listener answers `response` if anything
/// reaches it.
async fn exchange<C>(
    listening: StreamProtocol,
    dialing: StreamProtocol,
    request: C::Request,
    response: C::Response,
) -> Outcome<C::Response>
where
    C: Codec<Protocol = StreamProtocol> + Clone + Default + Send + 'static,
    C::Request: Send + 'static,
    C::Response: Send + Clone + 'static,
{
    let (server_codec, client_codec) = (Counting::<C>::default(), Counting::<C>::default());
    let mut server = make_swarm(listening, server_codec.clone());
    let mut client = make_swarm(dialing, client_codec.clone());

    let server_addr = listen_addr(&mut server).await;
    client.dial(server_addr).unwrap();

    let mut request = Some(request);
    let mut connected = false;
    let result = tokio::time::timeout(TEST_TIMEOUT, async {
        loop {
            tokio::select! {
                event = client.select_next_some() => match event {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        connected = true;
                        if let Some(req) = request.take() {
                            client.behaviour_mut().send_request(&peer_id, req);
                        }
                    }
                    SwarmEvent::Behaviour(request_response::Event::Message {
                        message: request_response::Message::Response { response, .. },
                        ..
                    }) => return Ok(response),
                    SwarmEvent::Behaviour(request_response::Event::OutboundFailure {
                        error, ..
                    }) => return Err(error),
                    _ => {}
                },
                event = server.select_next_some() => {
                    if let SwarmEvent::Behaviour(request_response::Event::Message {
                        message: request_response::Message::Request { channel, .. },
                        ..
                    }) = event
                    {
                        let _ = server.behaviour_mut().send_response(channel, response.clone());
                    }
                }
            }
        }
    })
    .await
    .expect("the exchange must end within the deadline");
    Outcome {
        result,
        connected,
        calls: (server_codec.calls(), client_codec.calls()),
    }
}

/// The exchange must fail at negotiation: connected below the protocols,
/// `UnsupportedProtocols`, and neither codec ever called.
fn assert_refused_at_negotiation<R: std::fmt::Debug>(case: &str, outcome: &Outcome<R>) {
    assert!(outcome.connected, "{case}: the transport did not connect");
    assert!(
        matches!(
            outcome.result,
            Err(request_response::OutboundFailure::UnsupportedProtocols)
        ),
        "{case}: expected UnsupportedProtocols, got {:?}",
        outcome.result
    );
    assert_eq!(
        outcome.calls,
        (0, 0),
        "{case}: a codec ran, so payload bytes were exchanged"
    );
}

/// The exchange must succeed, through both codecs.
fn assert_negotiated<R: std::fmt::Debug>(case: &str, outcome: &Outcome<R>) {
    assert!(outcome.result.is_ok(), "{case}: {:?}", outcome.result);
    // Each side wrote one message and read one.
    assert_eq!(outcome.calls, (2, 2), "{case}: codec calls");
}

fn sync_exchange(
    listening: StreamProtocol,
    dialing: StreamProtocol,
) -> impl std::future::Future<Output = Outcome<SyncResponse>> {
    exchange::<SyncCodec>(
        listening,
        dialing,
        SyncRequest::GetHeight,
        SyncResponse::Height(7),
    )
}

fn notify_exchange(
    listening: StreamProtocol,
    dialing: StreamProtocol,
) -> impl std::future::Future<Output = Outcome<BlockNotifyAck>> {
    exchange::<BlockNotifyCodec>(
        listening,
        dialing,
        SyncNotification::NewBlock {
            block: empty_block(),
        }
        .into(),
        BlockNotifyAck,
    )
}

/// An empty block for block-notify payloads (content is irrelevant: in the
/// refused cases it must never be encoded at all).
fn empty_block() -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash::zero(),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: 0,
            height: 1,
            producer: mbongo_core::Address::zero(),
        },
        body: BlockBody {
            transactions: vec![],
        },
        producer_signature: [0u8; 64],
    }
}

/// Identifiers that are not `base` + the prefix of `chain`, but come close.
fn near_misses(
    base: &str,
    earlier: &[&'static str],
    chain: &ChainId,
) -> Vec<(String, StreamProtocol)> {
    let c = chain_prefix(chain);
    let full = chain.to_string();
    let mut out: Vec<(String, StreamProtocol)> =
        earlier.iter().map(|p| (p.to_string(), StreamProtocol::new(p))).collect();
    for (case, text) in [
        ("uppercase prefix", format!("{base}{}", c.to_uppercase())),
        ("15 characters", format!("{base}{}", &c[..15])),
        ("17 characters", format!("{base}{}", &full[..17])),
        ("the whole chain id", format!("{base}{full}")),
        ("no prefix", base.trim_end_matches('/').to_string()),
        ("an empty prefix", base.to_string()),
        ("0x before the prefix", format!("{base}0x{c}")),
    ] {
        out.push((
            case.to_string(),
            StreamProtocol::try_from_owned(text).unwrap(),
        ));
    }
    out
}

// ── Sync ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn sync_on_the_same_network_negotiates() {
    let outcome = sync_exchange(sync_protocol(&A), sync_protocol(&A)).await;
    assert_negotiated("same network", &outcome);
    assert!(matches!(outcome.result, Ok(SyncResponse::Height(7))));
}

#[tokio::test]
async fn sync_across_networks_fails_at_negotiation() {
    // Both directions: whichever side dials, nothing is exchanged.
    let outcome = sync_exchange(sync_protocol(&B), sync_protocol(&A)).await;
    assert_refused_at_negotiation("A dials B", &outcome);
    let outcome = sync_exchange(sync_protocol(&A), sync_protocol(&B)).await;
    assert_refused_at_negotiation("B dials A", &outcome);
}

#[tokio::test]
async fn sync_earlier_and_near_miss_identifiers_fail_at_negotiation() {
    let cases = near_misses("/mbongo-sync/3/", &["/mbongo-sync/2", "/mbongo-sync/1"], &A);
    assert_eq!(cases.len(), 9, "cases stated before they run");
    let current = sync_protocol(&A);
    for (case, other) in cases {
        assert_ne!(other, current, "{case}");
        // An old or wrong node dialing this one, and this one dialing it.
        let outcome = sync_exchange(current.clone(), other.clone()).await;
        assert_refused_at_negotiation(&format!("{case}, dialing in"), &outcome);
        let outcome = sync_exchange(other.clone(), current.clone()).await;
        assert_refused_at_negotiation(&format!("{case}, dialed"), &outcome);
    }
    // The earlier version with the right prefix is still another identifier.
    let v2_prefixed =
        StreamProtocol::try_from_owned(format!("/mbongo-sync/2/{}", chain_prefix(&A))).unwrap();
    let outcome = sync_exchange(current, v2_prefixed).await;
    assert_refused_at_negotiation("version 2 with the prefix", &outcome);
}

// ── Block notify ───────────────────────────────────────────────────────

#[tokio::test]
async fn block_notify_on_the_same_network_negotiates() {
    let outcome = notify_exchange(block_notify_protocol(&A), block_notify_protocol(&A)).await;
    assert_negotiated("same network", &outcome);
}

#[tokio::test]
async fn block_notify_across_networks_fails_at_negotiation() {
    let outcome = notify_exchange(block_notify_protocol(&B), block_notify_protocol(&A)).await;
    assert_refused_at_negotiation("A announces to B", &outcome);
    let outcome = notify_exchange(block_notify_protocol(&A), block_notify_protocol(&B)).await;
    assert_refused_at_negotiation("B announces to A", &outcome);
}

#[tokio::test]
async fn block_notify_earlier_and_near_miss_identifiers_fail_at_negotiation() {
    let cases = near_misses(
        "/mbongo/block_notify/0.3.0/",
        &["/mbongo/block_notify/0.2.0", "/mbongo/block_notify/0.1.0"],
        &A,
    );
    assert_eq!(cases.len(), 9, "cases stated before they run");
    let current = block_notify_protocol(&A);
    for (case, other) in cases {
        assert_ne!(other, current, "{case}");
        let outcome = notify_exchange(current.clone(), other.clone()).await;
        assert_refused_at_negotiation(&format!("{case}, announcing in"), &outcome);
        let outcome = notify_exchange(other.clone(), current.clone()).await;
        assert_refused_at_negotiation(&format!("{case}, announced to"), &outcome);
    }
}

/// S3 A2.2d-1: a broadcast's shared, encoded-once announcement crosses a real
/// libp2p exchange and is acknowledged like a decoded one.
#[tokio::test]
async fn block_notify_shared_encoded_announcement_negotiates() {
    let protocol = block_notify_protocol(&A);
    let outcome = exchange::<BlockNotifyCodec>(
        protocol.clone(),
        protocol,
        Announcement::new_block(&empty_block()),
        BlockNotifyAck,
    )
    .await;
    assert_negotiated("shared announcement", &outcome);
}
