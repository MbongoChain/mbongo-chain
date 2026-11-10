//! S3 A2.3-b-1 between real nodes, over loopback.
//!
//! A peer floods full-range `GetBlocks`. Only what its rate allows reaches the
//! node, where the storage reads would happen; the rest is refused unanswered
//! and fails at the requester. Another peer is still served, and the flooder
//! is served again once its bucket refills.

use std::net::Ipv4Addr;
use std::time::Duration;

use libp2p::PeerId;
use mbongo_network::{P2PNode, SyncCommand, SyncEvent, SyncRequest, SyncResponse};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::{timeout, Instant};

const WAIT: Duration = Duration::from_secs(30);

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers it.
const RATE_P2P: u16 = 27122;

/// Full ranges asked back to back by the flooding peer, each as soon as the
/// previous one settled: one stream at a time, so the A1 stream caps never
/// apply and only the request rate decides.
const FLOOD: u64 = 40;

/// One full range costs 257 units; the default peer bucket holds 1024 and
/// refills 512 per second.
const RANGE_COST: f64 = 257.0;

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

/// Answers and failures seen by a requester, until `n` in all.
async fn outcomes(events: &mut UnboundedReceiver<SyncEvent>, n: u64) -> (u64, u64) {
    let (mut answered, mut failed) = (0, 0);
    timeout(WAIT, async {
        while answered + failed < n {
            match events.recv().await {
                Some(SyncEvent::ResponseReceived { .. }) => answered += 1,
                Some(SyncEvent::RequestFailed { .. }) => failed += 1,
                _ => {}
            }
        }
    })
    .await
    .expect("not every request settled");
    (answered, failed)
}

async fn peer_of(victim: PeerId) -> (UnboundedReceiver<SyncEvent>, UnboundedSender<SyncCommand>) {
    let mut node = P2PNode::with_mdns(false).unwrap();
    let mut events = node.take_sync_event_rx().unwrap();
    let cmd = node.sync_commander();
    node.dial(&format!("/ip4/127.0.0.1/tcp/{RATE_P2P}/p2p/{victim}")).unwrap();
    tokio::spawn(node.run());
    connected(&mut events, victim).await;
    (events, cmd)
}

fn full_range(victim: PeerId) -> SyncCommand {
    SyncCommand::GetBlocks {
        peer_id: victim,
        start_height: 0,
        end_height: 256,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_get_blocks_flood_reaches_the_node_only_at_its_rate() {
    let mut victim = P2PNode::with_mdns(false).unwrap();
    victim.listen_on_host(Ipv4Addr::LOCALHOST, RATE_P2P).unwrap();
    let victim_id = victim.peer_id;
    let counters = victim.counters();
    let mut requests = victim.take_sync_rx().unwrap();
    let answers = victim.sync_commander();
    tokio::spawn(victim.run());
    // The node: counts the ranges that reach it and answers each at once.
    let (seen_tx, mut seen) = tokio::sync::mpsc::unbounded_channel::<()>();
    tokio::spawn(async move {
        while let Some(inbound) = requests.recv().await {
            if matches!(inbound.request, SyncRequest::GetBlocks { .. }) {
                let _ = seen_tx.send(());
            }
            let _ = answers.send(inbound.respond(SyncResponse::Blocks(Vec::new())));
        }
    });

    let (mut flood_events, flood_cmd) = peer_of(victim_id).await;
    let (mut honest_events, honest_cmd) = peer_of(victim_id).await;

    // The flood: back to back.
    let started = Instant::now();
    let (mut answered, mut failed) = (0, 0);
    for _ in 0..FLOOD {
        flood_cmd.send(full_range(victim_id)).unwrap();
        let (a, f) = outcomes(&mut flood_events, 1).await;
        answered += a;
        failed += f;
    }
    let elapsed = started.elapsed().as_secs_f64();
    let mut reached = 0u64;
    while seen.try_recv().is_ok() {
        reached += 1;
    }
    let snapshot = counters.snapshot();
    println!(
        "flood of {FLOOD} full ranges in {elapsed:.2} s: {answered} answered, {failed} failed, \
         {reached} reached the node; counters {snapshot}"
    );
    // At most the burst plus what refilled while the flood lasted.
    let allowed = (1024.0 + 512.0 * elapsed) / RANGE_COST;
    #[allow(clippy::cast_precision_loss)]
    let reached_f = reached as f64;
    assert!(
        reached_f <= allowed.floor() + 1.0,
        "{reached} reached, {allowed:.1} allowed"
    );
    assert!(reached >= 3, "the burst is served: {reached}");
    assert_eq!(
        answered, reached,
        "every request that reached the node was answered"
    );
    assert!(snapshot.sync_requests_refused_rate > 0);
    assert_eq!(
        snapshot.sync_requests_admitted
            + snapshot.sync_requests_refused_rate
            + snapshot.sync_requests_refused_peer_quota
            + snapshot.sync_requests_refused_full,
        FLOOD,
        "every request admitted or refused once"
    );

    // Another peer, from the same address, is served during the aftermath.
    honest_cmd.send(full_range(victim_id)).unwrap();
    assert_eq!(
        outcomes(&mut honest_events, 1).await,
        (1, 0),
        "the other peer is served"
    );

    // The flooder is served again once its bucket has refilled.
    tokio::time::sleep(Duration::from_secs(1)).await;
    flood_cmd.send(full_range(victim_id)).unwrap();
    assert_eq!(
        outcomes(&mut flood_events, 1).await,
        (1, 0),
        "served after refill"
    );
}
