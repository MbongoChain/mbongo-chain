//! `p2p_stress`: resource-exhaustion traffic against one local `mbongo-node`
//! (S3 A2.0). Test tooling only; it is not part of any node.
//!
//! It speaks the frozen wire format directly (`u32` LE length, then bytes) over
//! the node's own negotiation strings, through a raw request/response codec,
//! so the traffic is the same whatever node version receives it. It never
//! answers anything it is asked.
//!
//! **Local targets only.** The target must be a loopback or private address
//! (IPv4 loopback or RFC 1918, IPv6 loopback or unique-local); any other
//! address, and any DNS name, is refused before a socket is opened.

use std::io;
use std::iter;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clap::{Parser, ValueEnum};
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, StreamExt};
use libp2p::multiaddr::Protocol;
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm};
use mbongo_core::{
    Address, Block, BlockBody, BlockHeader, Hash, Transaction, TransactionPayload, TransactionType,
};
use mbongo_network::{SyncNotification, SyncRequest, BLOCK_NOTIFY_PROTOCOL, SYNC_PROTOCOL};
use parity_scale_codec::Encode;
use tokio::time::Instant;

/// The frozen maximum frame (Lock v0.4 section 9).
const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Scenario {
    /// Frames that announce 16 MiB and send 64 bytes, then hold the stream.
    Partial,
    /// Complete maximum-size valid frames: 16 MiB blocks and padded requests.
    Full,
    /// Small valid block announcements, as fast as they are accepted.
    Announce,
    /// Full-range block requests, each answer read and discarded.
    Sync,
    /// Connection attempts on a fixed schedule, `--attempts-per-second` per
    /// identity, whatever happened to the previous ones; each connection that
    /// comes up is closed at once. The offered rate does not depend on the
    /// target's answers, so two targets can be compared at the same rate.
    ChurnRate,
    /// Connect, disconnect at once, and again.
    Churn,
    /// Streams opened and negotiated, then held with nothing sent.
    Streams,
}

#[derive(Parser, Debug)]
#[command(about = "Local resource-exhaustion traffic for one mbongo-node (A2.0)")]
struct Args {
    /// The node's P2P address, e.g. /ip4/172.31.250.20/tcp/30333. Loopback or
    /// private addresses only.
    #[arg(long)]
    target: Multiaddr,
    #[arg(long, value_enum)]
    scenario: Scenario,
    /// Peer identities, each with its own connection.
    #[arg(long, default_value_t = 4)]
    identities: usize,
    /// Requests kept in flight per identity.
    #[arg(long, default_value_t = 64)]
    concurrency: usize,
    /// How long to send traffic.
    #[arg(long, default_value_t = 60)]
    duration_secs: u64,
    /// How long a held stream is kept before this side gives up.
    #[arg(long, default_value_t = 120)]
    hold_secs: u64,
    /// Attempts per second per identity (churn-rate only).
    #[arg(long, default_value_t = 10)]
    attempts_per_second: u32,
}

/// Refuses anything that is not a loopback or private address.
fn local_only(target: &Multiaddr) -> Result<(), String> {
    let private_v6 = |ip: &Ipv6Addr| ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00;
    let private_v4 = |ip: &Ipv4Addr| ip.is_loopback() || ip.is_private();
    match target.iter().next() {
        Some(Protocol::Ip4(ip)) if private_v4(&ip) => Ok(()),
        Some(Protocol::Ip6(ip)) if private_v6(&ip) => Ok(()),
        _ => Err(format!(
            "refusing {target}: only loopback or private IP addresses may be targeted"
        )),
    }
}

/// Bytes written verbatim on a stream, then the stream is closed or held.
#[derive(Clone)]
struct Raw {
    bytes: Arc<Vec<u8>>,
    hold: bool,
}

impl std::fmt::Debug for Raw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Raw({} bytes, hold {})", self.bytes.len(), self.hold)
    }
}

#[derive(Clone, Default)]
struct RawCodec;

#[async_trait]
impl request_response::Codec for RawCodec {
    type Protocol = &'static str;
    type Request = Raw;
    type Response = u64;

    async fn read_request<T>(&mut self, _: &Self::Protocol, _: &mut T) -> io::Result<Raw>
    where
        T: AsyncRead + Unpin + Send,
    {
        Err(io::Error::other("this peer answers nothing"))
    }

    /// Reads an answer to its end, keeps only its length.
    async fn read_response<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<u64>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut buf = vec![0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let n = io.read(&mut buf).await?;
            if n == 0 {
                return Ok(total);
            }
            total += n as u64;
        }
    }

    async fn write_request<T>(&mut self, _: &Self::Protocol, io: &mut T, req: Raw) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        io.write_all(&req.bytes).await?;
        io.flush().await?;
        if req.hold {
            // Until this side's request timeout drops the stream.
            futures::future::pending::<()>().await;
        }
        io.close().await
    }

    async fn write_response<T>(&mut self, _: &Self::Protocol, _: &mut T, _: u64) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
struct Flood {
    sync: request_response::Behaviour<RawCodec>,
    notify: request_response::Behaviour<RawCodec>,
}

#[derive(Default)]
struct Stats {
    sent: AtomicU64,
    /// Answers that carried bytes: an acknowledgement or a reply.
    answered: AtomicU64,
    /// Streams the node closed without a byte of reply: a refusal.
    closed_unanswered: AtomicU64,
    failed: AtomicU64,
    connections: AtomicU64,
    closed: AtomicU64,
    dial_failed: AtomicU64,
    bytes_queued: AtomicU64,
    bytes_in: AtomicU64,
}

impl Stats {
    fn line(&self, elapsed: Duration, scenario: Scenario) -> String {
        let r = |c: &AtomicU64| c.load(Ordering::Relaxed);
        serde_json::json!({
            "t": elapsed.as_secs_f64(),
            "scenario": format!("{scenario:?}"),
            "sent": r(&self.sent),
            "answered": r(&self.answered),
            "closed_unanswered": r(&self.closed_unanswered),
            "failed": r(&self.failed),
            "connections": r(&self.connections),
            "closed": r(&self.closed),
            "dial_failed": r(&self.dial_failed),
            "bytes_queued": r(&self.bytes_queued),
            "bytes_in": r(&self.bytes_in),
            "own_cpu_ms": own_cpu_ms(),
        })
        .to_string()
    }
}

fn add(counter: &AtomicU64, n: u64) {
    counter.fetch_add(n, Ordering::Relaxed);
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let len = u32::try_from(payload.len()).expect("payload fits a frame");
    let mut bytes = len.to_le_bytes().to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

fn junk_block(height: u64, salt: u64, transactions: Vec<Transaction>) -> Block {
    Block {
        header: BlockHeader {
            parent_hash: Hash([u8::try_from(salt % 251).unwrap_or(0); 32]),
            state_root: Hash::zero(),
            transactions_root: Hash::zero(),
            timestamp: salt,
            height,
        },
        body: BlockBody { transactions },
    }
}

fn transfer(nonce: u64) -> Transaction {
    Transaction {
        tx_type: TransactionType::Transfer,
        sender: Address([1u8; 32]),
        receiver: Address([2u8; 32]),
        amount: 1,
        nonce,
        payload: TransactionPayload::None,
        signature: [0u8; 64],
    }
}

/// What each scenario sends, built once and shared by every identity.
struct Payloads {
    sync: Vec<Raw>,
    notify: Vec<Raw>,
}

fn payloads(scenario: Scenario) -> Payloads {
    let raw = |bytes: Vec<u8>, hold: bool| Raw {
        bytes: Arc::new(bytes),
        hold,
    };
    match scenario {
        Scenario::Partial => {
            let claimed = u32::try_from(MAX_FRAME).unwrap().to_le_bytes();
            let mut bytes = claimed.to_vec();
            bytes.extend_from_slice(&[0xAB; 64]);
            Payloads {
                sync: vec![raw(bytes.clone(), true)],
                notify: vec![raw(bytes, true)],
            }
        }
        Scenario::Full => {
            let one = SyncNotification::NewBlock {
                block: junk_block(1, 1, vec![transfer(0)]),
            }
            .encode()
            .len();
            let per_tx = transfer(0).encode().len();
            let count = (MAX_FRAME - one - 64) / per_tx;
            let block = SyncNotification::NewBlock {
                block: junk_block(u64::MAX - 1, 7, (0..count as u64).map(transfer).collect()),
            }
            .encode();
            assert!(block.len() <= MAX_FRAME, "the block fits the maximum frame");
            let mut padded = SyncRequest::GetHeight.encode();
            padded.resize(MAX_FRAME, 0);
            Payloads {
                sync: vec![raw(frame(&padded), false)],
                notify: vec![raw(frame(&block), false)],
            }
        }
        Scenario::Announce => Payloads {
            sync: Vec::new(),
            notify: (0..64u64)
                .map(|i| {
                    let height = if i % 2 == 0 { u64::MAX - i } else { 1 + i };
                    let payload = SyncNotification::NewBlock {
                        block: junk_block(height, i, Vec::new()),
                    }
                    .encode();
                    raw(frame(&payload), false)
                })
                .collect(),
        },
        Scenario::Sync => Payloads {
            sync: vec![raw(
                frame(
                    &SyncRequest::GetBlocks {
                        start_height: 0,
                        end_height: 256,
                    }
                    .encode(),
                ),
                false,
            )],
            notify: Vec::new(),
        },
        Scenario::Streams => Payloads {
            sync: vec![raw(Vec::new(), true)],
            notify: vec![raw(Vec::new(), true)],
        },
        Scenario::Churn | Scenario::ChurnRate => Payloads {
            sync: Vec::new(),
            notify: Vec::new(),
        },
    }
}

/// This process's CPU time in milliseconds, user plus system, all threads
/// (Linux `/proc/self/stat`, in 1/100 s ticks); `None` elsewhere.
fn own_cpu_ms() -> Option<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the parenthesised command name; utime and stime are the
    // 12th and 13th of them (14th and 15th of the line).
    let rest = &stat[stat.rfind(')')? + 2..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    let ticks = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    Some(ticks * 10)
}

/// One identity, attempting connections on a fixed schedule until the
/// deadline. `sent` counts attempts offered, `connections` those that came
/// up (and were closed at once), `dial_failed` those that did not.
async fn run_identity_at_rate(
    args: Arc<Args>,
    stats: Arc<Stats>,
    deadline: Instant,
) -> Result<(), String> {
    let mut swarm = build_swarm(&args).map_err(|e| e.to_string())?;
    let period = Duration::from_secs(1) / args.attempts_per_second.max(1);
    let mut tick = tokio::time::interval(period);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Burst);
    loop {
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => return Ok(()),
            _ = tick.tick() => {
                add(&stats.sent, 1);
                if swarm.dial(args.target.clone()).is_err() {
                    add(&stats.dial_failed, 1);
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    add(&stats.connections, 1);
                    let _ = swarm.disconnect_peer_id(peer_id);
                }
                SwarmEvent::ConnectionClosed { .. } => add(&stats.closed, 1),
                SwarmEvent::OutgoingConnectionError { .. } => add(&stats.dial_failed, 1),
                _ => {}
            },
        }
    }
}

fn build_swarm(args: &Args) -> Result<Swarm<Flood>, Box<dyn std::error::Error>> {
    let streams = args.concurrency.max(1) * 2;
    let config = request_response::Config::default()
        .with_request_timeout(Duration::from_secs(args.hold_secs))
        .with_max_concurrent_streams(streams);
    Ok(libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|_| Flood {
            sync: request_response::Behaviour::new(
                iter::once((SYNC_PROTOCOL, ProtocolSupport::Outbound)),
                config.clone(),
            ),
            notify: request_response::Behaviour::new(
                iter::once((BLOCK_NOTIFY_PROTOCOL, ProtocolSupport::Outbound)),
                config.clone(),
            ),
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(600)))
        .build())
}

/// One identity: one connection, kept full of requests until the deadline.
async fn run_identity(
    args: Arc<Args>,
    payloads: Arc<Payloads>,
    stats: Arc<Stats>,
    deadline: Instant,
) -> Result<(), String> {
    let mut swarm = build_swarm(&args).map_err(|e| e.to_string())?;
    let mut peer: Option<PeerId> = None;
    let mut in_flight = 0usize;
    let mut next = 0usize;
    let mut redial_at: Option<Instant> = Some(Instant::now());
    loop {
        if let Some(at) = redial_at {
            if Instant::now() >= at {
                redial_at = None;
                if swarm.dial(args.target.clone()).is_err() {
                    add(&stats.dial_failed, 1);
                    redial_at = Some(Instant::now() + Duration::from_millis(200));
                }
            }
        }
        if let Some(target) = peer {
            let both = payloads.sync.len() + payloads.notify.len();
            while both > 0 && in_flight < args.concurrency && Instant::now() < deadline {
                let index = next % both;
                next += 1;
                let (behaviour_is_sync, raw) = if index < payloads.sync.len() {
                    (true, payloads.sync[index].clone())
                } else {
                    (false, payloads.notify[index - payloads.sync.len()].clone())
                };
                add(&stats.bytes_queued, raw.bytes.len() as u64);
                if behaviour_is_sync {
                    swarm.behaviour_mut().sync.send_request(&target, raw);
                } else {
                    swarm.behaviour_mut().notify.send_request(&target, raw);
                }
                in_flight += 1;
                add(&stats.sent, 1);
            }
        }
        let wake = redial_at.unwrap_or(deadline).min(deadline);
        tokio::select! {
            () = tokio::time::sleep_until(wake) => {
                if Instant::now() >= deadline {
                    return Ok(());
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    add(&stats.connections, 1);
                    peer = Some(peer_id);
                    if args.scenario == Scenario::Churn {
                        let _ = swarm.disconnect_peer_id(peer_id);
                    }
                }
                SwarmEvent::ConnectionClosed { num_established: 0, .. } => {
                    add(&stats.closed, 1);
                    peer = None;
                    in_flight = 0;
                    redial_at = Some(Instant::now() + if args.scenario == Scenario::Churn {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(100)
                    });
                }
                SwarmEvent::OutgoingConnectionError { .. } => {
                    add(&stats.dial_failed, 1);
                    redial_at = Some(Instant::now() + Duration::from_millis(200));
                }
                SwarmEvent::Behaviour(FloodEvent::Sync(e) | FloodEvent::Notify(e)) => match e {
                    request_response::Event::Message {
                        message: request_response::Message::Response { response, .. },
                        ..
                    } => {
                        in_flight = in_flight.saturating_sub(1);
                        // A refused request is closed with nothing written: no
                        // acknowledgement. Only an answer with bytes counts as one.
                        if response == 0 {
                            add(&stats.closed_unanswered, 1);
                        } else {
                            add(&stats.answered, 1);
                        }
                        add(&stats.bytes_in, response);
                    }
                    request_response::Event::OutboundFailure { .. } => {
                        in_flight = in_flight.saturating_sub(1);
                        add(&stats.failed, 1);
                    }
                    _ => {}
                },
                _ => {}
            },
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arc::new(Args::parse());
    local_only(&args.target)?;
    let payloads = Arc::new(payloads(args.scenario));
    let stats = Arc::new(Stats::default());
    let started = Instant::now();
    let deadline = started + Duration::from_secs(args.duration_secs);

    let mut tasks = Vec::new();
    for _ in 0..args.identities.max(1) {
        if args.scenario == Scenario::ChurnRate {
            tasks.push(tokio::spawn(run_identity_at_rate(
                Arc::clone(&args),
                Arc::clone(&stats),
                deadline,
            )));
            continue;
        }
        tasks.push(tokio::spawn(run_identity(
            Arc::clone(&args),
            Arc::clone(&payloads),
            Arc::clone(&stats),
            deadline,
        )));
    }
    let reporter = {
        let stats = Arc::clone(&stats);
        let scenario = args.scenario;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            loop {
                tick.tick().await;
                println!("{}", stats.line(started.elapsed(), scenario));
            }
        })
    };
    for task in tasks {
        if let Err(e) = task.await? {
            eprintln!("identity failed: {e}");
        }
    }
    reporter.abort();
    println!("{}", stats.line(started.elapsed(), args.scenario));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    #[test]
    fn only_loopback_and_private_targets_are_accepted() {
        for ok in [
            "/ip4/127.0.0.1/tcp/1",
            "/ip4/10.1.2.3/tcp/1",
            "/ip4/172.31.250.20/tcp/1",
            "/ip4/192.168.0.9/tcp/1",
            "/ip6/::1/tcp/1",
            "/ip6/fd00::5/tcp/1",
        ] {
            assert!(local_only(&addr(ok)).is_ok(), "{ok}");
        }
        for refused in [
            "/ip4/8.8.8.8/tcp/1",
            "/ip4/172.32.0.1/tcp/1",
            "/ip4/0.0.0.0/tcp/1",
            "/ip6/2001:db8::1/tcp/1",
            "/dns4/localhost/tcp/1",
        ] {
            assert!(local_only(&addr(refused)).is_err(), "{refused}");
        }
    }

    #[test]
    fn every_payload_is_a_well_formed_frame() {
        let mut checked = 0;
        for scenario in [Scenario::Full, Scenario::Announce, Scenario::Sync] {
            let p = payloads(scenario);
            for raw in p.sync.iter().chain(&p.notify) {
                let len = u32::from_le_bytes(raw.bytes[..4].try_into().unwrap()) as usize;
                assert_eq!(len, raw.bytes.len() - 4);
                assert!(len <= MAX_FRAME);
                checked += 1;
            }
        }
        assert_eq!(checked, 2 + 64 + 1);
        let partial = payloads(Scenario::Partial);
        let claimed = u32::from_le_bytes(partial.notify[0].bytes[..4].try_into().unwrap());
        assert_eq!(claimed as usize, MAX_FRAME);
        assert_eq!(partial.notify[0].bytes.len(), 4 + 64);
    }
}
