//! Keeps configured bootnodes connected.
//!
//! Pure bookkeeping: it dials nothing and reads no clock, so every rule can be
//! tested directly. All of it is node-local, and nothing here reaches the wire.

use std::time::Duration;

use libp2p::multiaddr::Protocol;
use libp2p::swarm::ConnectionId;
use libp2p::{Multiaddr, PeerId};
use tokio::time::Instant;

/// Delay after the first failure.
const BACKOFF_BASE: Duration = Duration::from_secs(1);

/// Longest delay, jitter included.
const BACKOFF_CAP: Duration = Duration::from_secs(60);

/// A connection up for this long clears the failures before it.
const STABLE_AFTER: Duration = Duration::from_secs(30);

/// Delay before the next dial, after `failures` failures in a row.
///
/// `jitter` runs from 0 to 1000, and maps to a spread of 20 percent either
/// side, so that nodes restarted together do not dial in step.
#[must_use]
pub fn retry_delay(failures: u32, jitter: u16) -> Duration {
    let doublings = failures.saturating_sub(1).min(6);
    let base = BACKOFF_BASE.saturating_mul(1 << doublings).min(BACKOFF_CAP);
    let millis = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
    let spread = millis / 5;
    let offset = spread * 2 * u64::from(jitter.min(1000)) / 1000;
    Duration::from_millis(millis - spread + offset).min(BACKOFF_CAP)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Not connected. The next dial is allowed from this moment.
    Waiting { retry_at: Instant },
    /// A dial is in progress.
    Dialing(ConnectionId),
    /// Connected to this peer since this moment.
    Connected { peer: PeerId, since: Instant },
}

#[derive(Debug)]
struct Bootnode {
    addr: Multiaddr,
    /// From a `/p2p/` part, or learned at connection.
    peer: Option<PeerId>,
    state: State,
    failures: u32,
}

impl Bootnode {
    fn fail(&mut self, now: Instant, jitter: u16) {
        self.failures = self.failures.saturating_add(1);
        self.state = State::Waiting {
            retry_at: now + retry_delay(self.failures, jitter),
        };
    }
}

/// The configured bootnodes, and what to do about each.
#[derive(Debug, Default)]
pub struct BootnodeSet {
    nodes: Vec<Bootnode>,
}

impl BootnodeSet {
    /// Adds a bootnode, to be dialed at once.
    pub fn add(&mut self, addr: Multiaddr, now: Instant) {
        let peer = addr.iter().find_map(|protocol| match protocol {
            Protocol::P2p(peer) => Some(peer),
            _ => None,
        });
        self.nodes.push(Bootnode {
            addr,
            peer,
            state: State::Waiting { retry_at: now },
            failures: 0,
        });
    }

    /// Bootnodes to dial now, by index.
    #[must_use]
    pub fn dials_due(&self, now: Instant) -> Vec<(usize, Multiaddr)> {
        self.nodes
            .iter()
            .enumerate()
            .filter(
                |(_, node)| matches!(node.state, State::Waiting { retry_at } if retry_at <= now),
            )
            .map(|(index, node)| (index, node.addr.clone()))
            .collect()
    }

    /// A dial started.
    pub fn dial_started(&mut self, index: usize, connection: ConnectionId) {
        if let Some(node) = self.nodes.get_mut(index) {
            node.state = State::Dialing(connection);
        }
    }

    /// The dial was refused at once: no event will follow it.
    pub fn dial_refused(&mut self, index: usize, now: Instant, jitter: u16) {
        if let Some(node) = self.nodes.get_mut(index) {
            node.fail(now, jitter);
        }
    }

    /// A dial failed. Dials that are not a bootnode's are ignored.
    pub fn dial_failed(&mut self, connection: ConnectionId, now: Instant, jitter: u16) {
        for node in &mut self.nodes {
            if node.state == State::Dialing(connection) {
                node.fail(now, jitter);
            }
        }
    }

    /// A connection was established. It belongs to a bootnode if that dial
    /// produced it, or if it reached the same peer some other way. A dial
    /// learns the peer behind a bare address, and a later dial replaces it.
    pub fn connected(&mut self, connection: ConnectionId, peer: PeerId, now: Instant) {
        for node in &mut self.nodes {
            if node.state == State::Dialing(connection) {
                node.peer = Some(peer);
                node.state = State::Connected { peer, since: now };
            } else if node.peer == Some(peer) && !matches!(node.state, State::Connected { .. }) {
                node.state = State::Connected { peer, since: now };
            }
        }
    }

    /// The last connection to a peer closed. A connection that lasted clears
    /// the failures before it, and one that did not adds to them, so a peer
    /// that keeps dropping is not redialed at full speed.
    pub fn disconnected(&mut self, peer: PeerId, now: Instant, jitter: u16) {
        for node in &mut self.nodes {
            if let State::Connected { peer: held, since } = node.state {
                if held == peer {
                    if now.saturating_duration_since(since) >= STABLE_AFTER {
                        node.failures = 0;
                    }
                    node.fail(now, jitter);
                }
            }
        }
    }

    /// The earliest moment after `now` that a dial is due, so that a retry
    /// needs no other event to happen.
    #[must_use]
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        self.nodes
            .iter()
            .filter_map(|node| match node.state {
                State::Waiting { retry_at } if retry_at > now => Some(retry_at),
                _ => None,
            })
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIDDLE: u16 = 500;

    fn addr() -> Multiaddr {
        "/ip4/127.0.0.1/tcp/30333".parse().unwrap()
    }

    fn one(now: Instant) -> BootnodeSet {
        let mut set = BootnodeSet::default();
        set.add(addr(), now);
        set
    }

    fn id(n: usize) -> ConnectionId {
        ConnectionId::new_unchecked(n)
    }

    #[test]
    fn the_delay_doubles_up_to_its_cap() {
        let seconds: Vec<u64> = (1..=9).map(|f| retry_delay(f, MIDDLE).as_secs()).collect();
        assert_eq!(seconds, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(retry_delay(u32::MAX, MIDDLE), BACKOFF_CAP);
    }

    #[test]
    fn jitter_stays_within_a_fifth_and_under_the_cap() {
        assert_eq!(retry_delay(1, 0), Duration::from_millis(800));
        assert_eq!(retry_delay(1, 1000), Duration::from_millis(1200));
        assert_eq!(retry_delay(1, u16::MAX), Duration::from_millis(1200));
        assert_eq!(retry_delay(9, 1000), BACKOFF_CAP);
    }

    #[test]
    fn a_new_bootnode_is_dialed_at_once_and_only_once() {
        let now = Instant::now();
        let mut set = one(now);
        assert_eq!(set.dials_due(now).len(), 1);
        set.dial_started(0, id(1));
        assert!(set.dials_due(now).is_empty());
        assert_eq!(set.next_wake(now), None);
    }

    #[test]
    fn a_failed_dial_is_retried_after_the_delay() {
        let now = Instant::now();
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.dial_failed(id(1), now, MIDDLE);
        assert!(set.dials_due(now).is_empty());
        let retry = now + Duration::from_secs(1);
        assert_eq!(set.next_wake(now), Some(retry));
        assert_eq!(set.dials_due(retry).len(), 1);
    }

    #[test]
    fn a_dial_refused_at_once_is_retried_too() {
        let now = Instant::now();
        let mut set = one(now);
        set.dial_refused(0, now, MIDDLE);
        assert_eq!(set.next_wake(now), Some(now + Duration::from_secs(1)));
    }

    #[test]
    fn a_failure_of_some_other_dial_changes_nothing() {
        let now = Instant::now();
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.dial_failed(id(2), now, MIDDLE);
        assert_eq!(set.next_wake(now), None);
        assert!(set.dials_due(now + BACKOFF_CAP).is_empty());
    }

    #[test]
    fn a_connected_bootnode_is_not_dialed() {
        let now = Instant::now();
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.connected(id(1), PeerId::random(), now);
        assert!(set.dials_due(now + BACKOFF_CAP).is_empty());
        assert_eq!(set.next_wake(now), None);
    }

    #[test]
    fn a_closed_connection_is_redialed() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.connected(id(1), peer, now);
        set.disconnected(peer, now, MIDDLE);
        assert_eq!(set.dials_due(now + Duration::from_secs(1)).len(), 1);
    }

    #[test]
    fn a_close_of_some_other_peer_changes_nothing() {
        let now = Instant::now();
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.connected(id(1), PeerId::random(), now);
        set.disconnected(PeerId::random(), now, MIDDLE);
        assert!(set.dials_due(now + BACKOFF_CAP).is_empty());
    }

    #[test]
    fn a_peer_that_keeps_dropping_is_not_redialed_at_full_speed() {
        let mut now = Instant::now();
        let peer = PeerId::random();
        let mut set = one(now);
        let mut delays = Vec::new();
        for n in 0..4 {
            set.dial_started(0, id(n));
            set.connected(id(n), peer, now);
            set.disconnected(peer, now, MIDDLE);
            let wake = set.next_wake(now).unwrap();
            delays.push((wake - now).as_secs());
            now = wake;
        }
        assert_eq!(delays, vec![1, 2, 4, 8]);
    }

    #[test]
    fn a_connection_that_lasted_clears_the_failures() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.dial_failed(id(1), now, MIDDLE);
        set.dial_started(0, id(2));
        set.dial_failed(id(2), now, MIDDLE);
        set.dial_started(0, id(3));
        set.connected(id(3), peer, now);
        let later = now + STABLE_AFTER;
        set.disconnected(peer, later, MIDDLE);
        assert_eq!(set.next_wake(later), Some(later + Duration::from_secs(1)));
    }

    #[test]
    fn a_peer_reached_another_way_counts_as_connected() {
        let now = Instant::now();
        let peer = PeerId::random();
        let pinned: Multiaddr = format!("/ip4/127.0.0.1/tcp/30333/p2p/{peer}").parse().unwrap();
        let mut set = BootnodeSet::default();
        set.add(pinned, now);
        set.connected(id(9), peer, now);
        assert!(set.dials_due(now).is_empty());
    }

    #[test]
    fn a_later_dial_replaces_the_peer_learned_at_a_bare_address() {
        let now = Instant::now();
        let (old, new) = (PeerId::random(), PeerId::random());
        let mut set = one(now);
        set.dial_started(0, id(1));
        set.connected(id(1), old, now);
        set.disconnected(old, now, MIDDLE);
        set.dial_started(0, id(2));
        set.connected(id(2), new, now);
        set.disconnected(old, now, MIDDLE);
        assert!(
            set.dials_due(now + BACKOFF_CAP).is_empty(),
            "the old peer is gone"
        );
        set.disconnected(new, now, MIDDLE);
        assert!(!set.dials_due(now + BACKOFF_CAP).is_empty());
    }

    /// Ten minutes of a bootnode that never answers: the dials slow to the cap,
    /// and stop neither early nor run away.
    #[test]
    fn dials_stay_bounded_and_never_stop() {
        let start = Instant::now();
        let mut now = start;
        let mut set = one(now);
        let mut dials = 0usize;
        let mut last_dial = start;
        for n in 0..1000 {
            if now - start >= Duration::from_secs(600) {
                break;
            }
            let due = set.dials_due(now);
            assert_eq!(due.len(), 1);
            dials += 1;
            last_dial = now;
            set.dial_started(0, id(n));
            set.dial_failed(id(n), now, MIDDLE);
            now = set.next_wake(now).expect("a retry is always scheduled");
        }
        // At 0, 1, 3, 7, 15, 31, 63, then every 60 seconds.
        assert_eq!(dials, 15);
        assert!(start + Duration::from_secs(540) < last_dial);
    }

    #[test]
    fn a_wake_never_repeats_for_a_retry_already_due() {
        let now = Instant::now();
        let mut set = one(now);
        set.dial_refused(0, now, MIDDLE);
        let due = now + Duration::from_secs(1);
        assert_eq!(set.next_wake(due), None, "due now: dialed, not waited for");
    }
}
