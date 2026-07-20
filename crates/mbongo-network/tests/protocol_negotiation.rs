//! Mixed-version protocol negotiation tests (RFC 0002 Phase 4).
//!
//! Proves that a peer advertising a legacy v0.2 protocol string and a
//! v0.3 node share no common protocol on either negotiated channel
//! (Sync and Block Notify): the request fails at libp2p
//! multistream-select negotiation with `UnsupportedProtocols` —
//! deterministic, bounded, before any SCALE payload byte is exchanged
//! (the codec is never invoked), with no silent downgrade (neither side
//! carries a fallback protocol list). Matching-version positive
//! controls prove the failures are caused by the version mismatch and
//! not by the test harness.

use std::iter;
use std::time::Duration;

use futures::StreamExt;
use libp2p::request_response::{self, Codec, ProtocolSupport};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, Swarm};
use mbongo_core::{Block, BlockBody, BlockHeader, Hash};
use mbongo_network::{
    BlockNotifyCodec, SyncCodec, SyncNotification, SyncRequest, SyncResponse,
    BLOCK_NOTIFY_PROTOCOL, SYNC_PROTOCOL,
};

/// The v0.2 protocol strings, reproduced here as literals: the v0.3
/// codebase intentionally no longer defines them anywhere.
const LEGACY_SYNC_PROTOCOL: &str = "/mbongo-sync/1";
const LEGACY_BLOCK_NOTIFY_PROTOCOL: &str = "/mbongo/block_notify/0.1.0";

/// Overall per-test deadline: the failure must be bounded.
const TEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Builds a minimal swarm speaking exactly one protocol string with the
/// given codec, mirroring the transport stack used by `P2PNode`
/// (tcp/noise/yamux).
fn make_swarm<C>(protocol: &'static str) -> Swarm<request_response::Behaviour<C>>
where
    C: Codec<Protocol = &'static str> + Default + Clone + Send + 'static,
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
            request_response::Behaviour::<C>::new(
                iter::once((protocol, ProtocolSupport::Full)),
                request_response::Config::default(),
            )
        })
        .expect("behaviour")
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(20)))
        .build()
}

/// Drives `swarm` until it reports a listen address, returning it.
async fn listen_addr<C>(swarm: &mut Swarm<request_response::Behaviour<C>>) -> Multiaddr
where
    C: Codec<Protocol = &'static str> + Default + Clone + Send + 'static,
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

/// Dials a listener speaking `legacy` from a node speaking `current`,
/// sends `request`, and returns the outbound failure. Panics if the
/// request succeeds.
async fn negotiation_failure<C>(
    legacy: &'static str,
    current: &'static str,
    request: C::Request,
) -> request_response::OutboundFailure
where
    C: Codec<Protocol = &'static str> + Default + Clone + Send + 'static,
    C::Request: Send + 'static,
    C::Response: Send + 'static,
{
    let mut legacy_swarm = make_swarm::<C>(legacy);
    let mut current_swarm = make_swarm::<C>(current);

    let legacy_addr = listen_addr(&mut legacy_swarm).await;
    current_swarm.dial(legacy_addr).unwrap();

    let mut request = Some(request);
    tokio::time::timeout(TEST_TIMEOUT, async {
        loop {
            tokio::select! {
                event = current_swarm.select_next_some() => match event {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        if let Some(req) = request.take() {
                            current_swarm.behaviour_mut().send_request(&peer_id, req);
                        }
                    }
                    SwarmEvent::Behaviour(request_response::Event::OutboundFailure {
                        error, ..
                    }) => {
                        return error;
                    }
                    SwarmEvent::Behaviour(request_response::Event::Message { .. }) => {
                        panic!("request must not succeed across protocol versions");
                    }
                    _ => {}
                },
                _ = legacy_swarm.select_next_some() => {}
            }
        }
    })
    .await
    .expect("negotiation failure must occur within the bounded deadline")
}

/// Matching-version positive control: completes one request/response
/// round trip over `protocol` and returns the response received.
async fn roundtrip<C>(
    protocol: &'static str,
    request: C::Request,
    response: C::Response,
) -> C::Response
where
    C: Codec<Protocol = &'static str> + Default + Clone + Send + 'static,
    C::Request: Send + 'static,
    C::Response: Send + Clone + 'static,
{
    let mut server = make_swarm::<C>(protocol);
    let mut client = make_swarm::<C>(protocol);

    let server_addr = listen_addr(&mut server).await;
    client.dial(server_addr).unwrap();

    let mut request = Some(request);
    tokio::time::timeout(TEST_TIMEOUT, async {
        loop {
            tokio::select! {
                event = client.select_next_some() => match event {
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        if let Some(req) = request.take() {
                            client.behaviour_mut().send_request(&peer_id, req);
                        }
                    }
                    SwarmEvent::Behaviour(request_response::Event::Message {
                        message: request_response::Message::Response { response, .. },
                        ..
                    }) => {
                        return response;
                    }
                    SwarmEvent::Behaviour(request_response::Event::OutboundFailure {
                        error, ..
                    }) => {
                        panic!("matching protocols must negotiate, got: {error:?}");
                    }
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
    .expect("round trip must complete within the deadline")
}

/// An empty block for block-notify payloads (content is irrelevant: in
/// the mismatch test it must never be encoded at all).
fn empty_block() -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash::zero(),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: 0,
            height: 1,
        },
        body: BlockBody {
            transactions: vec![],
        },
    }
}

// ── Sync protocol ───────────────────────────────────────────────────────

#[tokio::test]
async fn sync_mismatched_versions_fail_cleanly() {
    // Deterministic, negotiation-level failure: UnsupportedProtocols means
    // multistream-select found no common protocol — the SyncCodec was
    // never invoked, so no SCALE bytes were exchanged and no decode
    // error path was reachable. Anything else (timeout, io, decode)
    // would indicate a half-connection or downgrade.
    let outcome = negotiation_failure::<SyncCodec>(
        LEGACY_SYNC_PROTOCOL,
        SYNC_PROTOCOL,
        SyncRequest::GetHeight,
    )
    .await;
    assert!(
        matches!(
            outcome,
            request_response::OutboundFailure::UnsupportedProtocols
        ),
        "expected UnsupportedProtocols, got: {outcome:?}"
    );
}

#[tokio::test]
async fn sync_matching_versions_negotiate() {
    let response = roundtrip::<SyncCodec>(
        SYNC_PROTOCOL,
        SyncRequest::GetHeight,
        SyncResponse::Height(7),
    )
    .await;
    assert!(matches!(response, SyncResponse::Height(7)));
}

// ── Block notify protocol ───────────────────────────────────────────────

#[tokio::test]
async fn block_notify_mismatched_versions_fail_cleanly() {
    // Same guarantee as the sync test: no notification payload is ever
    // encoded or decoded — negotiation fails before the BlockNotifyCodec
    // runs, and there is no fallback to the legacy string.
    let outcome = negotiation_failure::<BlockNotifyCodec>(
        LEGACY_BLOCK_NOTIFY_PROTOCOL,
        BLOCK_NOTIFY_PROTOCOL,
        SyncNotification::NewBlock {
            block: empty_block(),
        },
    )
    .await;
    assert!(
        matches!(
            outcome,
            request_response::OutboundFailure::UnsupportedProtocols
        ),
        "expected UnsupportedProtocols, got: {outcome:?}"
    );
}

#[tokio::test]
async fn block_notify_matching_versions_negotiate() {
    use mbongo_network::BlockNotifyAck;
    let _ack: BlockNotifyAck = roundtrip::<BlockNotifyCodec>(
        BLOCK_NOTIFY_PROTOCOL,
        SyncNotification::NewBlock {
            block: empty_block(),
        },
        BlockNotifyAck,
    )
    .await;
}
