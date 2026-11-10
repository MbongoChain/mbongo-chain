//! Admission limits for inbound P2P work (S3 A1).
//!
//! Everything here is node-local: no byte on the wire changes. A limit decides
//! whether this node accepts a connection, a stream or a message; a refused
//! message gets no acknowledgement, so the sender sees a failure, never a
//! silent success.
//!
//! What is bounded, and by what:
//!
//! | Object | Bound |
//! |---|---|
//! | Connections | [`MAX_ESTABLISHED_INCOMING`], [`MAX_ESTABLISHED_OUTGOING`], the pending limits, [`MAX_ESTABLISHED_PER_PEER`], and [`MAX_INCOMING_PER_IP`] |
//! | Streams per connection | [`MAX_NEGOTIATING_INBOUND_STREAMS`], [`SYNC_STREAMS_PER_CONNECTION`], [`NOTIFY_STREAMS_PER_CONNECTION`] |
//! | Inbound sync requests not yet answered | [`SYNC_QUEUE_CAPACITY`] in all, [`SYNC_QUOTA_PER_PEER`] per peer |
//! | Inbound block announcements not yet processed | [`BLOCK_QUEUE_CAPACITY`] in all, [`BLOCK_QUOTA_PER_PEER`] per peer |
//! | Local blocks waiting to be announced | [`BROADCAST_QUEUE_CAPACITY`] |
//!
//! Not bounded here: the bytes in flight across all streams together, and the
//! rate of anything. Both belong to A2.
//!
//! Not prevented here: a party with many identities and addresses can hold
//! every inbound slot, so that an honest peer cannot connect *to* this node.
//! A reserved slot for a pinned peer identity would not close that gap: the
//! identity is known only after the handshake, by which time the pending
//! slots it must pass through can already be held. Reservation, eviction or
//! any other admission rule is a design decision outside A1. What A1 does
//! guarantee is that one peer cannot do it, and that an established honest
//! connection keeps the node synchronising (periodic height requests, per-peer
//! quotas and backoff). An operator who must keep one path open does so
//! outside the node, by firewalling the P2P port to known hosts.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

use libp2p::core::{multiaddr::Protocol, Endpoint};
use libp2p::swarm::{
    dummy, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};

// ── Connections ────────────────────────────────────────────────────────

/// Most established inbound connections.
pub const MAX_ESTABLISHED_INCOMING: u32 = 64;
/// Most established outbound connections.
pub const MAX_ESTABLISHED_OUTGOING: u32 = 16;
/// Most inbound connections still in their handshake.
pub const MAX_PENDING_INCOMING: u32 = 32;
/// Most outbound connections still being dialed.
pub const MAX_PENDING_OUTGOING: u32 = 16;
/// Most connections to one peer: two lets a simultaneous dial succeed.
pub const MAX_ESTABLISHED_PER_PEER: u32 = 2;
/// Most inbound connections, pending or established, from one IP address.
/// No address is exempt, loopback included: behind a tunnel or a local proxy,
/// remote peers arrive from loopback, and an address alone earns no privilege.
/// A single-host devnet with more than this many peers of one node needs a
/// larger value.
pub const MAX_INCOMING_PER_IP: usize = 8;
/// Outbound connections, established or pending, past which an mDNS
/// discovery is not dialed. It keeps outbound room for bootnodes.
pub const MDNS_DIAL_BUDGET: u32 = 8;

// ── Streams ────────────────────────────────────────────────────────────

/// Most inbound streams negotiating a protocol on one connection.
pub const MAX_NEGOTIATING_INBOUND_STREAMS: usize = 16;
/// Most sync streams on one connection, inbound and outbound together. This
/// node keeps at most two outbound to a peer (a height and a block request).
pub const SYNC_STREAMS_PER_CONNECTION: usize = 4;
/// Most block-announcement streams on one connection, both directions.
pub const NOTIFY_STREAMS_PER_CONNECTION: usize = 2;

// ── Queues ─────────────────────────────────────────────────────────────

/// Inbound sync requests admitted and not yet answered, from all peers.
pub const SYNC_QUEUE_CAPACITY: usize = 32;
/// Inbound sync requests admitted and not yet answered, from one peer.
/// An honest peer has at most two in flight.
pub const SYNC_QUOTA_PER_PEER: usize = 2;
/// Inbound block announcements admitted and not yet processed, from all peers.
pub const BLOCK_QUEUE_CAPACITY: usize = 8;
/// Inbound block announcements admitted and not yet processed, from one peer.
pub const BLOCK_QUOTA_PER_PEER: usize = 2;
/// Locally produced blocks waiting to be announced.
pub const BROADCAST_QUEUE_CAPACITY: usize = 16;

// ── Admission ──────────────────────────────────────────────────────────

/// Why a message was not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The sending peer already holds its quota.
    PeerQuota,
    /// The queue holds its capacity.
    Full,
}

#[derive(Debug)]
struct AdmissionState {
    per_peer: HashMap<PeerId, usize>,
    total: usize,
    /// The most messages ever held at once; for diagnostics only (A2.2a).
    peak: usize,
}

/// Counts admitted messages, per peer and in all, against two limits. Each
/// admitted message carries a [`Permit`]; dropping it releases the slot,
/// whatever became of the message.
#[derive(Debug, Clone)]
pub(crate) struct Admission {
    state: Arc<Mutex<AdmissionState>>,
    per_peer_limit: usize,
    total_limit: usize,
}

impl Admission {
    pub(crate) fn new(per_peer_limit: usize, total_limit: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(AdmissionState {
                per_peer: HashMap::new(),
                total: 0,
                peak: 0,
            })),
            per_peer_limit,
            total_limit,
        }
    }

    /// Admits one message from `peer`, or says why not. The per-peer quota is
    /// checked first, so a peer over its own quota is told so even when the
    /// queue is also full.
    pub(crate) fn try_admit(&self, peer: PeerId) -> Result<Permit, Refusal> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let held = state.per_peer.get(&peer).copied().unwrap_or(0);
        if held >= self.per_peer_limit {
            return Err(Refusal::PeerQuota);
        }
        if state.total >= self.total_limit {
            return Err(Refusal::Full);
        }
        state.per_peer.insert(peer, held + 1);
        state.total += 1;
        state.peak = state.peak.max(state.total);
        Ok(Permit {
            peer,
            state: Arc::clone(&self.state),
        })
    }

    /// Messages held now, and the most ever held at once. Read-only.
    pub(crate) fn occupancy(&self) -> (usize, usize) {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        (state.total, state.peak)
    }

    /// Messages admitted and not yet released: in all, and for `peer`.
    #[cfg(test)]
    pub(crate) fn held(&self, peer: &PeerId) -> (usize, usize) {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        (state.total, state.per_peer.get(peer).copied().unwrap_or(0))
    }
}

/// One admitted message's slot. Dropping it, whether the message was
/// processed, refused later or abandoned, gives the slot back.
#[derive(Debug)]
pub struct Permit {
    peer: PeerId,
    state: Arc<Mutex<AdmissionState>>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.total = state.total.saturating_sub(1);
        if let Some(held) = state.per_peer.get_mut(&self.peer) {
            *held = held.saturating_sub(1);
            if *held == 0 {
                state.per_peer.remove(&self.peer);
            }
        }
    }
}

// ── Inbound connections per IP address ─────────────────────────────────

/// The IP address of a multiaddr, if it starts with one.
fn ip_of(addr: &Multiaddr) -> Option<IpAddr> {
    match addr.iter().next()? {
        Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
        _ => None,
    }
}

/// The address a per-IP limit applies to: every IP address, with no exemption.
fn limited_ip(addr: &Multiaddr) -> Option<IpAddr> {
    ip_of(addr)
}

/// The inbound connection was refused: its IP address holds its limit.
#[derive(Debug, Clone, Copy)]
pub struct IpLimitExceeded {
    /// The number of inbound connections that address already holds.
    pub held: usize,
}

impl std::fmt::Display for IpLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inbound connections from this IP address: {} held",
            self.held
        )
    }
}

impl std::error::Error for IpLimitExceeded {}

/// Refuses an inbound connection when its IP address already holds
/// [`MAX_INCOMING_PER_IP`] inbound connections, pending or established. A
/// connection is counted from its first moment and released when it closes or
/// is refused, by this behaviour or any other.
#[derive(Debug)]
pub struct IpLimits {
    limit: usize,
    connections: HashMap<ConnectionId, IpAddr>,
    per_ip: HashMap<IpAddr, usize>,
}

impl Default for IpLimits {
    fn default() -> Self {
        Self::new(MAX_INCOMING_PER_IP)
    }
}

impl IpLimits {
    /// A limit of `limit` inbound connections per IP address.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            connections: HashMap::new(),
            per_ip: HashMap::new(),
        }
    }

    fn admit(
        &mut self,
        connection: ConnectionId,
        remote: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        let Some(ip) = limited_ip(remote) else {
            return Ok(());
        };
        if self.connections.contains_key(&connection) {
            return Ok(());
        }
        let held = self.per_ip.get(&ip).copied().unwrap_or(0);
        if held >= self.limit {
            return Err(ConnectionDenied::new(IpLimitExceeded { held }));
        }
        self.per_ip.insert(ip, held + 1);
        self.connections.insert(connection, ip);
        Ok(())
    }

    fn release(&mut self, connection: ConnectionId) {
        if let Some(ip) = self.connections.remove(&connection) {
            if let Some(held) = self.per_ip.get_mut(&ip) {
                *held = held.saturating_sub(1);
                if *held == 0 {
                    self.per_ip.remove(&ip);
                }
            }
        }
    }

    /// Inbound connections counted against `ip`.
    #[cfg(test)]
    fn held(&self, ip: IpAddr) -> usize {
        self.per_ip.get(&ip).copied().unwrap_or(0)
    }
}

impl NetworkBehaviour for IpLimits {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = std::convert::Infallible;

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        self.admit(connection_id, remote_addr)
    }

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Already counted when pending; this only covers a connection that
        // skipped that step.
        self.admit(connection_id, remote_addr)?;
        Ok(dummy::ConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionClosed(closed) => self.release(closed.connection_id),
            FromSwarm::ListenFailure(failure) => self.release(failure.connection_id),
            _ => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {}
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        Poll::Pending
    }
}

// ── Counters ───────────────────────────────────────────────────────────

macro_rules! counters {
    ($($(#[$doc:meta])* $name:ident,)*) => {
        /// What the P2P layer admitted, refused and dropped since it started.
        /// Each refusal is counted here rather than logged one by one; a
        /// summary is logged at most once a minute, and only when it changed.
        #[derive(Debug, Default)]
        pub struct P2PCounters {
            $($(#[$doc])* pub $name: AtomicU64,)*
        }

        /// A reading of [`P2PCounters`].
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        pub struct P2PCountersSnapshot {
            $($(#[$doc])* pub $name: u64,)*
        }

        impl P2PCounters {
            /// Reads every counter.
            pub fn snapshot(&self) -> P2PCountersSnapshot {
                P2PCountersSnapshot {
                    $($name: self.$name.load(Ordering::Relaxed),)*
                }
            }
        }

        impl std::fmt::Display for P2PCountersSnapshot {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut first = true;
                $(
                    if self.$name != 0 {
                        if !first {
                            f.write_str(" ")?;
                        }
                        first = false;
                        write!(f, "{}={}", stringify!($name), self.$name)?;
                    }
                )*
                if first {
                    f.write_str("nothing")?;
                }
                Ok(())
            }
        }
    };
}

counters! {
    /// Block announcements admitted and acknowledged.
    announcements_admitted,
    /// Block announcements refused, unacknowledged: the peer held its quota.
    announcements_refused_peer_quota,
    /// Block announcements refused, unacknowledged: the queue was full.
    announcements_refused_full,
    /// Sync requests admitted.
    sync_requests_admitted,
    /// Sync requests refused, unanswered: the peer held its quota.
    sync_requests_refused_peer_quota,
    /// Sync requests refused, unanswered: the queue was full.
    sync_requests_refused_full,
    /// Admitted sync requests found canceled when their turn came, and dropped
    /// unanswered.
    sync_requests_canceled,
    /// Inbound request failures reported by libp2p (timeouts, closed
    /// connections, omitted responses, undecodable frames).
    inbound_failures,
    /// Outbound request failures reported by libp2p.
    outbound_failures,
    /// Block announcements this node sent and the peer acknowledged.
    announcements_acknowledged,
    /// Local blocks not announced: the broadcast queue was full.
    broadcasts_dropped,
    /// Inbound connections refused by a connection limit.
    connections_denied,
    /// Of those, refused before the handshake because their source exceeded
    /// its connection-attempt rate (S3 A2.3-a).
    inbound_rate_refused_source,
    /// Of those, refused before the handshake because all sources together
    /// exceeded the global connection-attempt rate (S3 A2.3-a).
    inbound_rate_refused_global,
    /// Sync requests refused by the request rate limits (S3 A2.3-b-1).
    sync_requests_refused_rate,
    /// Announcements refused by the request rate limits (S3 A2.3-b-1).
    announcements_refused_rate,
    /// mDNS discoveries not dialed: the outbound budget was spent.
    mdns_dials_skipped,
}

impl P2PCounters {
    pub(crate) fn add(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn occupancy_reports_the_current_and_the_highest_count() {
        let admission = Admission::new(4, 8);
        let peer = PeerId::random();
        assert_eq!(admission.occupancy(), (0, 0));
        let a = admission.try_admit(peer).unwrap();
        let b = admission.try_admit(peer).unwrap();
        let c = admission.try_admit(PeerId::random()).unwrap();
        assert_eq!(admission.occupancy(), (3, 3));
        drop((a, b));
        assert_eq!(admission.occupancy(), (1, 3), "the peak stays");
        let d = admission.try_admit(peer).unwrap();
        assert_eq!(admission.occupancy(), (2, 3));
        drop((c, d));
        assert_eq!(admission.occupancy(), (0, 3));
        // A refusal changes neither.
        let full = Admission::new(1, 1);
        let held = full.try_admit(peer).unwrap();
        assert!(full.try_admit(peer).is_err());
        assert_eq!(full.occupancy(), (1, 1));
        drop(held);
    }

    #[test]
    fn a_peer_cannot_take_more_than_its_quota() {
        let admission = Admission::new(2, 8);
        let peer = PeerId::random();
        let a = admission.try_admit(peer).unwrap();
        let b = admission.try_admit(peer).unwrap();
        assert_eq!(admission.try_admit(peer).unwrap_err(), Refusal::PeerQuota);
        assert_eq!(admission.held(&peer), (2, 2));
        drop((a, b));
        assert_eq!(admission.held(&peer), (0, 0));
    }

    #[test]
    fn the_queue_refuses_when_full_and_admits_again_once_released() {
        let admission = Admission::new(2, 3);
        let peers: Vec<PeerId> = (0..4).map(|_| PeerId::random()).collect();
        let mut permits: Vec<Permit> =
            peers[..3].iter().map(|p| admission.try_admit(*p).unwrap()).collect();
        assert_eq!(admission.try_admit(peers[3]).unwrap_err(), Refusal::Full);
        permits.pop();
        assert!(admission.try_admit(peers[3]).is_ok());
    }

    #[test]
    fn a_peer_over_quota_is_told_so_before_full() {
        let admission = Admission::new(1, 1);
        let peer = PeerId::random();
        let _held = admission.try_admit(peer).unwrap();
        assert_eq!(admission.try_admit(peer).unwrap_err(), Refusal::PeerQuota);
        assert_eq!(
            admission.try_admit(PeerId::random()).unwrap_err(),
            Refusal::Full
        );
    }

    #[test]
    fn a_permit_moved_and_dropped_elsewhere_still_releases() {
        let admission = Admission::new(1, 1);
        let peer = PeerId::random();
        let permit = admission.try_admit(peer).unwrap();
        std::thread::spawn(move || drop(permit)).join().unwrap();
        assert_eq!(admission.held(&peer), (0, 0));
        assert!(admission.try_admit(peer).is_ok());
    }

    fn addr(s: &str) -> Multiaddr {
        s.parse().unwrap()
    }

    #[test]
    fn an_ip_address_cannot_hold_more_than_its_limit() {
        let mut limits = IpLimits::new(2);
        let remote = addr("/ip4/203.0.113.7/tcp/4000");
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        let (a, b, c) = (
            ConnectionId::new_unchecked(1),
            ConnectionId::new_unchecked(2),
            ConnectionId::new_unchecked(3),
        );
        limits.admit(a, &remote).unwrap();
        limits.admit(b, &remote).unwrap();
        assert!(limits.admit(c, &remote).is_err());
        assert!(limits.admit(c, &addr("/ip4/203.0.113.8/tcp/4000")).is_ok());
        limits.release(a);
        assert_eq!(limits.held(ip), 1);
        assert!(limits.admit(ConnectionId::new_unchecked(4), &remote).is_ok());
    }

    #[test]
    fn a_connection_counted_when_pending_is_not_counted_again() {
        let mut limits = IpLimits::new(1);
        let remote = addr("/ip6/2001:db8::1/tcp/4000");
        let id = ConnectionId::new_unchecked(1);
        limits.admit(id, &remote).unwrap();
        limits.admit(id, &remote).unwrap();
        limits.release(id);
        limits.release(id);
        assert_eq!(limits.held("2001:db8::1".parse().unwrap()), 0);
    }

    #[test]
    fn loopback_is_limited_like_any_address() {
        let mut limits = IpLimits::new(1);
        let local = addr("/ip4/127.0.0.1/tcp/4000");
        limits.admit(ConnectionId::new_unchecked(1), &local).unwrap();
        assert!(limits.admit(ConnectionId::new_unchecked(2), &local).is_err());
        assert!(limits.admit(ConnectionId::new_unchecked(3), &addr("/ip6/::1/tcp/4000")).is_ok());
        assert_eq!(limits.held("127.0.0.1".parse().unwrap()), 1);
    }

    #[test]
    fn counters_print_only_what_happened() {
        let counters = P2PCounters::default();
        assert_eq!(counters.snapshot().to_string(), "nothing");
        P2PCounters::add(&counters.announcements_refused_full);
        P2PCounters::add(&counters.announcements_refused_full);
        P2PCounters::add(&counters.connections_denied);
        assert_eq!(
            counters.snapshot().to_string(),
            "announcements_refused_full=2 connections_denied=1"
        );
    }
}
