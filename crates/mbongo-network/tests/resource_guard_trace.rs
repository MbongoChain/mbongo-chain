//! The guard's per-stream records on the real node (Phase 1, T3).
//!
//! At `TRACE` the guard writes one record when an inbound stream is observed
//! and one when its handle is dropped, each with the connection's number, a
//! monotonic timestamp and the active counts. This test is alone in its
//! binary because the logger is process-wide: every record it captures comes
//! from the one node it starts.

use std::collections::HashMap;
use std::iter;
use std::net::Ipv4Addr;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use futures::StreamExt;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr};
use mbongo_network::{P2PNode, SyncCodec, SyncRequest, SyncResponse, SYNC_PROTOCOL};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(30);

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers it.
const TRACE_P2P: u16 = 27139;

const GUARD_TARGET: &str = "mbongo_network::resource_guard";

/// Keeps the guard's records and nothing else.
struct Capture;

fn records() -> &'static Mutex<Vec<String>> {
    static RECORDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    RECORDS.get_or_init(|| Mutex::new(Vec::new()))
}

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == GUARD_TARGET
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            records().lock().unwrap().push(format!("{} {}", record.level(), record.args()));
        }
    }
    fn flush(&self) {}
}

/// The `name=value` fields of one record.
fn fields(record: &str) -> HashMap<&str, &str> {
    record.split_whitespace().filter_map(|part| part.split_once('=')).collect()
}

fn number(fields: &HashMap<&str, &str>, name: &str) -> u64 {
    fields
        .get(name)
        .unwrap_or_else(|| panic!("no {name} in {fields:?}"))
        .parse()
        .unwrap_or_else(|_| panic!("{name} is not a number in {fields:?}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_inbound_stream_leaves_an_opening_and_a_closing_record() {
    const REQUESTS: u64 = 4;
    log::set_logger(&Capture).unwrap();
    log::set_max_level(log::LevelFilter::Trace);

    let mut node = P2PNode::with_mdns(false).unwrap();
    node.listen_on_host(Ipv4Addr::LOCALHOST, TRACE_P2P).unwrap();
    let node_id = node.peer_id;
    let guard = node.resource_guard();
    let mut requests = node.take_sync_rx().unwrap();
    let answers = node.sync_commander();
    tokio::spawn(node.run());
    tokio::spawn(async move {
        while let Some(inbound) = requests.recv().await {
            let _ = answers.send(inbound.respond(SyncResponse::Height(42)));
        }
    });

    // A peer built with `with_tcp`, running the sync protocol only: each of
    // its streams is one request.
    let mut peer = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .unwrap()
        .with_behaviour(|_| {
            request_response::Behaviour::<SyncCodec>::new(
                iter::once((SYNC_PROTOCOL, ProtocolSupport::Full)),
                request_response::Config::default(),
            )
        })
        .unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();
    let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{TRACE_P2P}").parse().unwrap();
    peer.dial(addr).unwrap();
    timeout(WAIT, async {
        let mut answered = 0;
        loop {
            match peer.select_next_some().await {
                SwarmEvent::ConnectionEstablished { .. } => {
                    peer.behaviour_mut().send_request(&node_id, SyncRequest::GetHeight);
                }
                SwarmEvent::Behaviour(request_response::Event::Message {
                    message: request_response::Message::Response { .. },
                    ..
                }) => {
                    answered += 1;
                    if answered == REQUESTS {
                        return;
                    }
                    // The next request leaves once the node has let go of
                    // this one's stream, so the records do not interleave.
                    while guard.snapshot().inbound_streams_closed_total < answered {
                        tokio::time::sleep(Duration::from_millis(2)).await;
                    }
                    peer.behaviour_mut().send_request(&node_id, SyncRequest::GetHeight);
                }
                SwarmEvent::Behaviour(request_response::Event::OutboundFailure {
                    error, ..
                }) => panic!("a request failed: {error:?}"),
                _ => {}
            }
        }
    })
    .await
    .expect("the requests were not all answered");
    timeout(WAIT, async {
        while guard.snapshot().inbound_streams_closed_total < REQUESTS {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the streams never closed");

    let captured = records().lock().unwrap().clone();
    for record in &captured {
        println!("TRACE | {record}");
    }
    let opened: Vec<_> = captured.iter().filter(|r| r.contains(" stream_opened ")).collect();
    let closed: Vec<_> = captured.iter().filter(|r| r.contains(" stream_closed ")).collect();
    assert_eq!(
        opened.len(),
        usize::try_from(REQUESTS).unwrap(),
        "{captured:?}"
    );
    assert_eq!(
        closed.len(),
        usize::try_from(REQUESTS).unwrap(),
        "{captured:?}"
    );
    assert!(opened.iter().chain(&closed).all(|r| r.starts_with("TRACE ")));

    // One request at a time on one connection: stream n opens, then closes,
    // then stream n + 1 opens. Timestamps never go back.
    let mut last = 0;
    for (index, (open, close)) in opened.iter().zip(&closed).enumerate() {
        let (open, close) = (fields(open), fields(close));
        let stream = u64::try_from(index).unwrap() + 1;
        for record in [&open, &close] {
            assert_eq!(number(record, "conn"), 1, "{record:?}");
            assert_eq!(number(record, "stream"), stream, "{record:?}");
        }
        assert_eq!(number(&open, "conn_active"), 1);
        assert_eq!(number(&open, "active"), 1);
        assert_eq!(number(&close, "conn_active"), 0);
        assert_eq!(number(&close, "active"), 0);
        assert_eq!(close["negotiation_reached"], "true");
        assert_eq!(close["error"], "false");
        let (opened_at, closed_at) = (number(&open, "t_us"), number(&close, "t_us"));
        assert!(
            opened_at >= last,
            "the clock went back: {opened_at} after {last}"
        );
        assert!(closed_at >= opened_at);
        // The lifetime is measured from just before the opening record.
        let lifetime = number(&close, "lifetime_us");
        assert!(lifetime <= closed_at - opened_at + 1_000, "{close:?}");
        last = closed_at;
    }
    // The connection itself is recorded at DEBUG.
    assert_eq!(
        captured
            .iter()
            .filter(|r| r.starts_with("DEBUG ") && r.contains("connection 1 observed (inbound)"))
            .count(),
        1,
        "{captured:?}"
    );
}
