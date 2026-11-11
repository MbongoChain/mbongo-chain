//! S3 A2.3: the yamux stream cap between a real node and a raw client, over
//! loopback.
//!
//! The client speaks TCP, Noise and yamux directly, with no protocol
//! negotiation of its own. It opens `n` streams, writes one byte on each (yamux
//! 0.13 announces a stream with its first frame) and keeps them open: streams
//! the node holds without being able to negotiate them.
//!
//! - Well under `MAX_YAMUX_STREAMS_PER_CONNECTION`, the connection stays up.
//! - Past it, the node's yamux terminates the whole connection, before any
//!   protocol negotiation, and the installed `CapacityLog` counts the error.
//!
//! A client that sends ordinary requests, even 150 at once, does not hold
//! that many streams: the node drops refused streams as fast as they arrive.
//! The cap bounds streams held at once, not the rate of opening them.

use std::net::Ipv4Addr;
use std::time::Duration;

use futures::future::poll_fn;
use futures::AsyncWriteExt;
use libp2p::core::muxing::{StreamMuxerBox, StreamMuxerExt};
use libp2p::core::transport::Transport;
use libp2p::core::upgrade::Version;
use libp2p::{identity, noise, tcp, yamux, Multiaddr};
use mbongo_network::capacity_log;
use mbongo_network::limits::MAX_YAMUX_STREAMS_PER_CONNECTION;
use mbongo_network::P2PNode;
use tokio::time::{timeout, Instant};

const WAIT: Duration = Duration::from_secs(30);

// Fixed, below the ephemeral range; `scripts/check-harness-ports.mjs` covers it.
const CAP_P2P: u16 = 27123;

/// A logger that writes nothing; installed so that `CapacityLog` counts.
struct Quiet;

impl log::Log for Quiet {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        false
    }
    fn log(&self, _: &log::Record<'_>) {}
    fn flush(&self) {}
}

/// Connects, opens `n` streams holding one byte each, and reports whether
/// the node closed the connection within `window`.
async fn hold_streams(addr: &Multiaddr, n: usize, window: Duration) -> bool {
    let key = identity::Keypair::generate_ed25519();
    let mut transport = tcp::tokio::Transport::new(tcp::Config::default())
        .upgrade(Version::V1)
        .authenticate(noise::Config::new(&key).unwrap())
        .multiplex(yamux::Config::default())
        .map(|(peer, muxer), _| (peer, StreamMuxerBox::new(muxer)))
        .boxed();
    let (_peer, mut muxer) = timeout(WAIT, transport.dial(addr.clone()).unwrap())
        .await
        .expect("dial timed out")
        .expect("dial failed");

    let mut streams = Vec::new();
    for _ in 0..n {
        let mut stream = poll_fn(|cx| muxer.poll_outbound_unpin(cx))
            .await
            .expect("could not open a stream");
        // Buffered by yamux; sent once the connection is polled below.
        stream.write_all(&[0x13]).await.expect("write failed");
        streams.push(stream);
    }

    // Drive the connection: an error is the node closing it.
    let deadline = Instant::now() + window;
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => return false,
            event = poll_fn(|cx| muxer.poll_unpin(cx)) => {
                if event.is_err() {
                    return true;
                }
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_connection_holding_too_many_streams_is_terminated() {
    capacity_log::install(Quiet, log::LevelFilter::Off).unwrap();
    assert_eq!(MAX_YAMUX_STREAMS_PER_CONNECTION, 64);

    let mut victim = P2PNode::with_mdns(false).unwrap();
    victim.listen_on_host(Ipv4Addr::LOCALHOST, CAP_P2P).unwrap();
    tokio::spawn(victim.run());
    let addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{CAP_P2P}").parse().unwrap();

    // Well under the cap: the connection stays up.
    assert!(
        !hold_streams(&addr, 30, Duration::from_secs(3)).await,
        "30 held streams closed the connection"
    );
    assert_eq!(capacity_log::yamux_stream_limit_reached(), Some(0));

    // Past the cap: yamux terminates the connection.
    assert!(
        hold_streams(&addr, 100, Duration::from_secs(10)).await,
        "100 held streams did not close the connection"
    );
    let reached = capacity_log::yamux_stream_limit_reached().unwrap();
    println!("yamux stream-limit errors counted: {reached}");
    assert!(reached >= 1);
}
