//! F-N2 (docs/security/MVP_LAUNCH_SECURITY_REVIEW.md): mDNS discovery can be
//! switched off. The default node keeps mDNS; a node built with
//! `P2PNode::with_mdns(false)` (`mbongo-node --no-mdns`) runs no mDNS
//! behaviour, so nothing is advertised, discovered or auto-dialed, while
//! explicit dials (bootnodes) and the sync protocol keep working.
//!
//! Every listener here is on 127.0.0.1.

use std::net::TcpListener;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use mbongo_network::{P2PNode, SyncCommand, SyncEvent, SyncRequest, SyncResponse};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

// The P2P layer reports mDNS discoveries only in its log ("mDNS discovered:
// <peer> at <addr>"), so the tests read that log.
static LOG: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
struct Capture;
impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, r: &log::Record<'_>) {
        LOG.get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .push(r.args().to_string());
    }
    fn flush(&self) {}
}
fn capture_log() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let _ = log::set_logger(&Capture);
        log::set_max_level(log::LevelFilter::Info);
    });
}
fn discovered(peer: &libp2p::PeerId) -> bool {
    let needle = format!("mDNS discovered: {peer} ");
    LOG.get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.starts_with(&needle))
}

// The two tests that run an mDNS-enabled node must not overlap: an enabled
// node of one would discover the other's.
static MDNS_SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn free_loopback_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn next_connected(
    rx: &mut UnboundedReceiver<SyncEvent>,
    within: Duration,
) -> Option<libp2p::PeerId> {
    timeout(within, async {
        loop {
            match rx.recv().await {
                Some(SyncEvent::PeerConnected { peer_id }) => return Some(peer_id),
                Some(_) => {}
                None => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

#[test]
fn default_node_keeps_mdns_enabled() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    assert!(P2PNode::new().unwrap().mdns_enabled());
    assert!(P2PNode::with_mdns(true).unwrap().mdns_enabled());
}

#[test]
fn with_mdns_false_disables_mdns() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    assert!(!P2PNode::with_mdns(false).unwrap().mdns_enabled());
}

/// A node without mDNS never connects to a peer it was not told about,
/// even with an mDNS-enabled node announcing itself on the same host.
#[tokio::test(flavor = "multi_thread")]
async fn disabled_node_auto_dials_nothing() {
    let _serial = MDNS_SERIAL.lock().await;
    capture_log();
    let mut quiet = P2PNode::with_mdns(false).unwrap();
    quiet
        .listen_on_host(std::net::Ipv4Addr::LOCALHOST, free_loopback_port())
        .unwrap();
    let mut quiet_events = quiet.take_sync_event_rx().unwrap();
    assert!(!quiet.mdns_enabled());

    let mut announcer = P2PNode::with_mdns(true).unwrap();
    announcer
        .listen_on_host(std::net::Ipv4Addr::LOCALHOST, free_loopback_port())
        .unwrap();
    let mut announcer_events = announcer.take_sync_event_rx().unwrap();
    let announcer_id = announcer.peer_id;
    let quiet_id = quiet.peer_id;

    tokio::spawn(quiet.run());
    tokio::spawn(announcer.run());

    // The default mDNS query interval is 5 minutes, but the first query and
    // announcement go out at start-up; 4 s covers them with margin.
    assert_eq!(
        next_connected(&mut quiet_events, Duration::from_secs(4)).await,
        None,
        "no mDNS dial from the disabled node"
    );
    assert_eq!(
        next_connected(&mut announcer_events, Duration::from_millis(1)).await,
        None,
        "no connection either way"
    );
    // The only node that could log a discovery of the announcer is the
    // disabled one (mDNS ignores its own peer id); the announcer could log
    // the disabled one only if it answered queries.
    assert!(
        !discovered(&announcer_id),
        "the disabled node discovered nothing"
    );
    assert!(
        !discovered(&quiet_id),
        "the disabled node advertised nothing"
    );
}

/// Explicit dials (what `--bootnodes` does) and the sync protocol work with
/// mDNS disabled on both sides.
#[tokio::test(flavor = "multi_thread")]
async fn static_peers_still_connect_and_sync_without_mdns() {
    let port = free_loopback_port();
    let mut a = P2PNode::with_mdns(false).unwrap();
    a.listen_on_host(std::net::Ipv4Addr::LOCALHOST, port).unwrap();
    let a_id = a.peer_id;
    let mut a_requests = a.take_sync_rx().unwrap();
    let mut a_events = a.take_sync_event_rx().unwrap();
    let a_cmd = a.sync_commander();

    let mut b = P2PNode::with_mdns(false).unwrap();
    let b_id = b.peer_id;
    let mut b_events = b.take_sync_event_rx().unwrap();
    let b_cmd = b.sync_commander();

    tokio::spawn(a.run());
    tokio::time::sleep(Duration::from_millis(200)).await;
    b.dial(&format!("/ip4/127.0.0.1/tcp/{port}/p2p/{a_id}")).unwrap();
    tokio::spawn(b.run());

    assert_eq!(
        next_connected(&mut b_events, Duration::from_secs(10)).await,
        Some(a_id)
    );
    assert_eq!(
        next_connected(&mut a_events, Duration::from_secs(10)).await,
        Some(b_id)
    );

    // GetHeight from b, answered by a.
    b_cmd.send(SyncCommand::GetHeight { peer_id: a_id }).unwrap();
    let inbound = timeout(Duration::from_secs(10), a_requests.recv()).await.unwrap().unwrap();
    assert!(matches!(inbound.request, SyncRequest::GetHeight));
    assert_eq!(inbound.peer, b_id);
    a_cmd
        .send(SyncCommand::SendResponse {
            channel: inbound.channel,
            response: SyncResponse::Height(7),
        })
        .unwrap();
    let got = timeout(Duration::from_secs(10), async {
        loop {
            if let Some(SyncEvent::ResponseReceived { peer_id, response }) = b_events.recv().await {
                return (peer_id, response);
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(got.0, a_id);
    assert!(matches!(got.1, SyncResponse::Height(7)));
}

/// Positive control for the toggle: two mDNS-enabled nodes discover each
/// other (the dial that follows targets the advertised interface addresses,
/// which fail against loopback listeners — the F-N2 behaviour). Depends on
/// multicast being available on the host, so it is not part of CI; run with
/// `--ignored` on a workstation.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs working multicast on the host"]
async fn enabled_nodes_discover_each_other() {
    let _serial = MDNS_SERIAL.lock().await;
    capture_log();
    let mut a = P2PNode::with_mdns(true).unwrap();
    a.listen_on_host(std::net::Ipv4Addr::LOCALHOST, free_loopback_port()).unwrap();
    let a_id = a.peer_id;
    let mut b = P2PNode::with_mdns(true).unwrap();
    b.listen_on_host(std::net::Ipv4Addr::LOCALHOST, free_loopback_port()).unwrap();
    let b_id = b.peer_id;
    tokio::spawn(a.run());
    tokio::spawn(b.run());
    let both = timeout(Duration::from_secs(15), async {
        while !(discovered(&a_id) && discovered(&b_id)) {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(both.is_ok(), "a and b discover each other over mDNS");
}
