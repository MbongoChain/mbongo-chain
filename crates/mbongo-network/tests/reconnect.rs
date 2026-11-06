//! Reconnection between real nodes, over loopback.
//!
//! The required gate for bootnode redial: it runs in `cargo test`, on every pull
//! request. Each wait is bounded, and is for an event, not for an amount of time.

use std::net::{Ipv4Addr, TcpListener};
use std::time::Duration;

use libp2p::identity::Keypair;
use libp2p::PeerId;
use mbongo_network::{P2PNode, SyncEvent};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::JoinHandle;
use tokio::time::timeout;

/// Far above the first retry delays, far below a CI job's patience.
const WAIT: Duration = Duration::from_secs(30);

fn free_port() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    listener.local_addr().unwrap().port()
}

/// A node listening on `port`, running in its own task.
fn listener(port: u16, key: Option<Keypair>) -> (PeerId, JoinHandle<()>) {
    let mut node = P2PNode::with_identity(false, key).unwrap();
    node.listen_on_host(Ipv4Addr::LOCALHOST, port).unwrap();
    let peer = node.peer_id;
    (peer, tokio::spawn(node.run()))
}

/// A node holding one bootnode, and its sync events.
fn follower(bootnode: &str) -> (UnboundedReceiver<SyncEvent>, JoinHandle<()>) {
    let mut node = P2PNode::with_identity(false, None).unwrap();
    node.add_bootnode(bootnode).unwrap();
    let events = node.take_sync_event_rx().unwrap();
    (events, tokio::spawn(node.run()))
}

async fn connected_to(events: &mut UnboundedReceiver<SyncEvent>, peer: PeerId) {
    timeout(WAIT, async {
        loop {
            match events.recv().await.expect("events open") {
                SyncEvent::PeerConnected { peer_id } if peer_id == peer => return,
                _ => {}
            }
        }
    })
    .await
    .expect("no connection in time");
}

async fn disconnected_from(events: &mut UnboundedReceiver<SyncEvent>, peer: PeerId) {
    timeout(WAIT, async {
        loop {
            match events.recv().await.expect("events open") {
                SyncEvent::PeerDisconnected { peer_id, .. } if peer_id == peer => return,
                _ => {}
            }
        }
    })
    .await
    .expect("no disconnection in time");
}

async fn stop(task: JoinHandle<()>) {
    task.abort();
    let _ = task.await;
}

fn bare(port: u16) -> String {
    format!("/ip4/127.0.0.1/tcp/{port}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bootnode_with_a_bare_address_is_rejoined_after_it_restarts() {
    let port = free_port();
    let (first, server) = listener(port, None);
    let (mut events, client) = follower(&bare(port));
    connected_to(&mut events, first).await;

    stop(server).await;
    disconnected_from(&mut events, first).await;

    // A new identity on the same address: a bare address does not mind.
    let (second, server) = listener(port, None);
    assert_ne!(first, second);
    connected_to(&mut events, second).await;

    stop(server).await;
    stop(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pinned_bootnode_with_a_stable_identity_is_rejoined_after_it_restarts() {
    let port = free_port();
    let key = Keypair::generate_ed25519();
    let (first, server) = listener(port, Some(key.clone()));
    let (mut events, client) = follower(&format!("{}/p2p/{first}", bare(port)));
    connected_to(&mut events, first).await;

    stop(server).await;
    disconnected_from(&mut events, first).await;

    let (second, server) = listener(port, Some(key));
    assert_eq!(first, second, "the identity survives the restart");
    connected_to(&mut events, second).await;

    stop(server).await;
    stop(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_pinned_bootnode_with_a_new_identity_is_not_accepted() {
    let port = free_port();
    let (first, server) = listener(port, None);
    let (mut events, client) = follower(&format!("{}/p2p/{first}", bare(port)));
    connected_to(&mut events, first).await;

    stop(server).await;
    disconnected_from(&mut events, first).await;

    let (second, server) = listener(port, None);
    // Several retries fit in this window. None of them may succeed.
    let joined = timeout(Duration::from_secs(5), connected_to(&mut events, second)).await;
    assert!(
        joined.is_err(),
        "connected to an identity that was not pinned"
    );

    stop(server).await;
    stop(client).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bootnode_down_at_the_start_is_reached_when_it_comes_up() {
    let port = free_port();
    let (mut events, client) = follower(&bare(port));
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (peer, server) = listener(port, None);
    connected_to(&mut events, peer).await;

    stop(server).await;
    stop(client).await;
}
