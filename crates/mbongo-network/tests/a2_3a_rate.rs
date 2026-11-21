//! S3 A2.3-a between a real node and real sockets, over loopback.
//!
//! A burst of raw TCP connections from one address exhausts that source's
//! bucket. Refused sockets are closed at once, before any Noise handshake (an
//! admitted socket would instead wait for the initiator's first Noise
//! message). A peer already connected keeps working, and a new legitimate
//! peer from the same address gets in once the bucket refills.

use std::io::Read;
use std::net::{Ipv4Addr, TcpStream};
use std::time::Duration;

use libp2p::PeerId;
use mbongo_network::{P2PNode, SyncCommand, SyncEvent, SyncResponse};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{timeout, Instant};

/// Far above any local round trip, far below a CI job's patience.
const WAIT: Duration = Duration::from_secs(30);

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers it.
const RATE_P2P: u16 = 27121;

/// Raw connection attempts, from 127.0.0.1 like the honest peers.
const BURST_ATTEMPTS: usize = 30;

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

async fn height_from(events: &mut UnboundedReceiver<SyncEvent>) -> u64 {
    timeout(WAIT, async {
        loop {
            if let Some(SyncEvent::ResponseReceived {
                response: SyncResponse::Height(h),
                ..
            }) = events.recv().await
            {
                return h;
            }
        }
    })
    .await
    .expect("no height answer")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_burst_is_refused_before_the_handshake_without_harming_peers() {
    let mut victim = P2PNode::with_mdns(false).unwrap();
    victim.listen_on_host(Ipv4Addr::LOCALHOST, RATE_P2P).unwrap();
    let victim_id = victim.peer_id;
    let counters = victim.counters();
    let mut requests = victim.take_sync_rx().unwrap();
    let answers = victim.sync_commander();
    tokio::spawn(victim.run());
    // The victim answers every height request with 7.
    tokio::spawn(async move {
        while let Some(inbound) = requests.recv().await {
            let _ = answers.send(inbound.respond(SyncResponse::Height(7)));
        }
    });

    // An honest peer, connected before the burst.
    let mut honest = P2PNode::with_mdns(false).unwrap();
    let mut honest_events = honest.take_sync_event_rx().unwrap();
    let honest_cmd = honest.sync_commander();
    honest.dial(&format!("/ip4/127.0.0.1/tcp/{RATE_P2P}/p2p/{victim_id}")).unwrap();
    tokio::spawn(honest.run());
    connected(&mut honest_events, victim_id).await;
    honest_cmd.send(SyncCommand::GetHeight { peer_id: victim_id }).unwrap();
    assert_eq!(height_from(&mut honest_events).await, 7);

    // The burst: raw sockets that never speak.
    let started = Instant::now();
    let mut sockets = Vec::new();
    for _ in 0..BURST_ATTEMPTS {
        if let Ok(s) = TcpStream::connect((Ipv4Addr::LOCALHOST, RATE_P2P)) {
            sockets.push(s);
        }
    }
    assert_eq!(
        sockets.len(),
        BURST_ATTEMPTS,
        "the kernel accepted every socket"
    );
    let deadline = Instant::now() + WAIT;
    while counters.snapshot().inbound_rate_refused_source < 10 {
        assert!(
            Instant::now() < deadline,
            "no rate refusal: {}",
            counters.snapshot()
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // A refused socket is closed at once; an admitted one waits for Noise.
    let mut closed_at_once = 0;
    for mut s in sockets {
        s.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        let mut byte = [0u8; 1];
        match s.read(&mut byte) {
            Ok(0) => closed_at_once += 1,
            Err(e)
                if e.kind() != std::io::ErrorKind::WouldBlock
                    && e.kind() != std::io::ErrorKind::TimedOut =>
            {
                closed_at_once += 1
            }
            _ => {}
        }
    }
    let snapshot = counters.snapshot();
    let elapsed = started.elapsed().as_secs() + 1;
    println!("burst of {BURST_ATTEMPTS}: {closed_at_once} closed at once; counters {snapshot}");
    // The honest peer took one of the source's 10 tokens; at most `elapsed`
    // more refilled during the burst.
    let refused = usize::try_from(snapshot.inbound_rate_refused_source).unwrap();
    assert!(
        refused >= BURST_ATTEMPTS - 9 - usize::try_from(elapsed).unwrap(),
        "refused {refused}"
    );
    assert!(refused <= BURST_ATTEMPTS - 9, "refused {refused}");
    assert!(
        closed_at_once >= refused,
        "every refused socket closed without a handshake"
    );
    assert_eq!(snapshot.inbound_rate_refused_global, 0);

    // The established peer is unaffected.
    honest_cmd.send(SyncCommand::GetHeight { peer_id: victim_id }).unwrap();
    assert_eq!(height_from(&mut honest_events).await, 7);

    // A new legitimate peer from the same address gets in once tokens refill:
    // as a bootnode it is redialed with backoff until admitted.
    let mut late = P2PNode::with_mdns(false).unwrap();
    let mut late_events = late.take_sync_event_rx().unwrap();
    late.add_bootnode(&format!("/ip4/127.0.0.1/tcp/{RATE_P2P}/p2p/{victim_id}"))
        .unwrap();
    tokio::spawn(late.run());
    connected(&mut late_events, victim_id).await;
}
