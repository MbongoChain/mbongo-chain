//! S3 A1 admission limits between real nodes, over loopback.
//!
//! Each test drives a victim node and a peer that misbehaves, and checks what
//! the victim admitted, what it refused, and what the peer was told. Every
//! wait is bounded and waits for an event or a counter, not for a duration,
//! except where a libp2p timeout is the thing under test.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use libp2p::PeerId;
use mbongo_core::{Block, BlockBody, BlockHeader, Hash};
use mbongo_network::limits::{BLOCK_QUOTA_PER_PEER, SYNC_QUOTA_PER_PEER};
use mbongo_network::{
    BlockBroadcaster, P2PCounters, P2PCountersSnapshot, P2PNode, SyncCommand, SyncEvent,
    SyncRequest, SyncResponse,
};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{timeout, Instant};

/// Far above any local round trip, far below a CI job's patience.
const WAIT: Duration = Duration::from_secs(30);

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers them.
const QUOTA_P2P: u16 = 27111;
const SYNC_P2P: u16 = 27112;

fn block(height: u64) -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash::zero(),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: 1_700_000_000,
            height,
        },
        body: BlockBody {
            transactions: vec![],
        },
    }
}

/// Waits until `check` holds for the counters, or fails with the last reading.
async fn until(
    counters: &Arc<P2PCounters>,
    what: &str,
    check: impl Fn(&P2PCountersSnapshot) -> bool,
) -> P2PCountersSnapshot {
    let deadline = Instant::now() + WAIT;
    loop {
        let now = counters.snapshot();
        if check(&now) {
            return now;
        }
        assert!(
            Instant::now() < deadline,
            "{what}: never reached; counters: {now}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn connected(events: &mut UnboundedReceiver<SyncEvent>, peer: PeerId) {
    timeout(WAIT, async {
        loop {
            if let Some(SyncEvent::PeerConnected { peer_id }) = events.recv().await {
                if peer_id == peer {
                    return;
                }
            }
        }
    })
    .await
    .expect("never connected");
}

/// A victim listening on `port`, and a second node connected to it.
struct Pair {
    victim: P2PNode,
    victim_id: PeerId,
    peer: P2PNode,
}

fn pair(port: u16) -> Pair {
    let mut victim = P2PNode::with_mdns(false).unwrap();
    victim.listen_on_host(Ipv4Addr::LOCALHOST, port).unwrap();
    let victim_id = victim.peer_id;
    let mut peer = P2PNode::with_mdns(false).unwrap();
    peer.dial(&format!("/ip4/127.0.0.1/tcp/{port}/p2p/{victim_id}")).unwrap();
    Pair {
        victim,
        victim_id,
        peer,
    }
}

/// Announcements past the per-peer quota are refused and not acknowledged;
/// every acknowledgement the sender received matches an admission; slots come
/// back once the node has handled what it admitted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn announcements_past_the_quota_are_refused_unacknowledged_until_handled() {
    let Pair {
        mut victim,
        victim_id,
        mut peer,
        ..
    } = pair(QUOTA_P2P);
    // The victim's node never reads its queue until told to below.
    let mut queue = victim.take_block_rx().unwrap();
    let victim_counters = victim.counters();
    let mut peer_events = peer.take_sync_event_rx().unwrap();
    let peer_counters = peer.counters();
    let announce = peer.broadcaster();
    let quota = u64::try_from(BLOCK_QUOTA_PER_PEER).unwrap();
    tokio::spawn(victim.run());
    tokio::spawn(peer.run());
    connected(&mut peer_events, victim_id).await;

    // One at a time, so each outcome is known before the next is sent.
    let mut sent = 0u64;
    for _ in 0..quota {
        sent += 1;
        announce.broadcast(block(sent));
        until(&peer_counters, "acknowledged", |c| {
            c.announcements_acknowledged == sent
        })
        .await;
    }
    for refused in 1..=3 {
        sent += 1;
        announce.broadcast(block(sent));
        until(&peer_counters, "failure seen by the sender", |c| {
            c.outbound_failures == refused
        })
        .await;
        until(&victim_counters, "refusal counted", |c| {
            c.announcements_refused_peer_quota == refused
        })
        .await;
    }
    let victim_now = victim_counters.snapshot();
    let peer_now = peer_counters.snapshot();
    assert_eq!(victim_now.announcements_admitted, quota);
    assert_eq!(
        peer_now.announcements_acknowledged, quota,
        "no acknowledgement without admission"
    );
    assert_eq!(
        peer_now.announcements_acknowledged + peer_now.outbound_failures,
        sent,
        "every announcement acknowledged or failed"
    );

    // The node handles what it admitted: the slots come back.
    for _ in 0..quota {
        let inbound = timeout(WAIT, queue.recv()).await.unwrap().unwrap();
        drop(inbound);
    }
    sent += 1;
    announce.broadcast(block(sent));
    until(&peer_counters, "acknowledged after release", |c| {
        c.announcements_acknowledged == quota + 1
    })
    .await;
    assert_eq!(victim_counters.snapshot().announcements_admitted, quota + 1);
}

/// Sync requests past the per-peer quota fail at the requester at once. The
/// admitted ones, left unanswered, time out; the node then finds them canceled,
/// drops them unanswered, and their slots serve the next request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canceled_sync_requests_release_their_slots() {
    let Pair {
        mut victim,
        victim_id,
        mut peer,
        ..
    } = pair(SYNC_P2P);
    let mut queue = victim.take_sync_rx().unwrap();
    let victim_counters = victim.counters();
    let victim_cmd = victim.sync_commander();
    let mut peer_events = peer.take_sync_event_rx().unwrap();
    let peer_cmd = peer.sync_commander();
    let quota = u64::try_from(SYNC_QUOTA_PER_PEER).unwrap();
    tokio::spawn(victim.run());
    tokio::spawn(peer.run());
    connected(&mut peer_events, victim_id).await;

    let ask = || peer_cmd.send(SyncCommand::GetHeight { peer_id: victim_id }).unwrap();
    for _ in 0..quota {
        ask();
    }
    until(&victim_counters, "admitted", |c| {
        c.sync_requests_admitted == quota
    })
    .await;
    ask();
    until(&victim_counters, "refused over quota", |c| {
        c.sync_requests_refused_peer_quota == 1
    })
    .await;
    let failed = timeout(WAIT, async {
        loop {
            if let Some(SyncEvent::RequestFailed { peer_id, .. }) = peer_events.recv().await {
                return peer_id;
            }
        }
    })
    .await
    .expect("the refused request never failed at the requester");
    assert_eq!(failed, victim_id);

    // Nothing answers the admitted requests; libp2p's 10 s request timeout
    // cancels them at both ends.
    let started = Instant::now();
    let mut canceled = 0;
    for _ in 0..quota {
        let inbound = timeout(WAIT, queue.recv()).await.unwrap().unwrap();
        while !inbound.is_canceled() {
            assert!(
                started.elapsed() < WAIT,
                "an unanswered request was never canceled"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        inbound.discard_canceled();
        canceled += 1;
    }
    assert_eq!(canceled, quota);
    assert_eq!(victim_counters.snapshot().sync_requests_canceled, quota);

    // The slots are back: a new request is admitted and answered.
    ask();
    let inbound = timeout(WAIT, queue.recv()).await.unwrap().unwrap();
    assert!(matches!(inbound.request, SyncRequest::GetHeight));
    victim_cmd.send(inbound.respond(SyncResponse::Height(42))).unwrap();
    let answer = timeout(WAIT, async {
        loop {
            if let Some(SyncEvent::ResponseReceived { response, .. }) = peer_events.recv().await {
                return response;
            }
        }
    })
    .await
    .expect("no answer after the slots were released");
    assert!(matches!(answer, SyncResponse::Height(42)));
    assert_eq!(victim_counters.snapshot().sync_requests_admitted, quota + 1);
}
