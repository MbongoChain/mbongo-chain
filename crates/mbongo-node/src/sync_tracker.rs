//! Per-peer state of the sync orchestrator.
//!
//! Pure bookkeeping: it sends nothing and reads no clock, so every recovery
//! rule can be tested directly. All of it is node-local, and nothing here
//! reaches the wire.

use std::collections::BTreeMap;
use std::time::Duration;

use libp2p::PeerId;
use mbongo_network::SyncRequestKind;
use tokio::time::Instant;

/// Delay after a peer's first failure.
const BACKOFF_BASE: Duration = Duration::from_secs(1);

/// Longest delay after repeated failures.
const BACKOFF_CAP: Duration = Duration::from_secs(60);

/// Delay before a peer is used again, after `failures` failures in a row.
fn backoff(failures: u32) -> Duration {
    let doublings = failures.saturating_sub(1).min(6);
    BACKOFF_BASE.saturating_mul(1 << doublings).min(BACKOFF_CAP)
}

/// What a block answer amounted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchOutcome {
    /// At least one block was applied, and none was refused.
    Progress,
    /// Every block was at or below the local height: a pushed block arrived
    /// first. Nothing is wrong with the peer.
    AlreadyHeld,
    /// The peer sent no block.
    Empty,
    /// A block was refused.
    Refused,
}

#[derive(Debug, Default)]
struct PeerSync {
    connected: bool,
    /// Last height this peer answered. Never set from an announcement: an
    /// announced height is unverified, and only prompts a height request.
    reported_height: u64,
    wants_height: bool,
    height_in_flight: bool,
    failures: u32,
    retry_at: Option<Instant>,
}

impl PeerSync {
    fn ready(&self, now: Instant) -> bool {
        self.connected && self.retry_at.map_or(true, |at| at <= now)
    }
}

/// What the orchestrator knows about each peer, and the one block request it
/// may have in flight.
#[derive(Debug, Default)]
pub struct SyncTracker {
    peers: BTreeMap<PeerId, PeerSync>,
    blocks_in_flight: Option<PeerId>,
}

impl SyncTracker {
    /// A peer connected. Any backoff it earned before stays in force.
    pub fn peer_connected(&mut self, peer: PeerId) {
        let entry = self.peers.entry(peer).or_default();
        entry.connected = true;
        entry.wants_height = true;
    }

    /// The last connection to a peer closed. Its entry stays, so that a peer
    /// reconnecting at once does not escape its backoff.
    pub fn peer_disconnected(&mut self, peer: PeerId, abnormal: bool, now: Instant) {
        if self.blocks_in_flight == Some(peer) {
            self.blocks_in_flight = None;
        }
        if let Some(entry) = self.peers.get_mut(&peer) {
            entry.connected = false;
            entry.wants_height = false;
            entry.height_in_flight = false;
        }
        if abnormal {
            self.peer_failed(peer, now);
        }
    }

    /// A peer announced a block above the next expected one. It may be ahead,
    /// so it is asked: the announced height itself is not believed.
    pub fn ask_height(&mut self, peer: PeerId) {
        let entry = self.peers.entry(peer).or_default();
        entry.connected = true;
        entry.wants_height = true;
    }

    /// Peers to send a height request to now.
    pub fn heights_due(&self, now: Instant) -> Vec<PeerId> {
        self.peers
            .iter()
            .filter(|(_, p)| p.ready(now) && p.wants_height && !p.height_in_flight)
            .map(|(id, _)| *id)
            .collect()
    }

    /// A height request was handed to the P2P layer.
    pub fn height_requested(&mut self, peer: PeerId) {
        if let Some(entry) = self.peers.get_mut(&peer) {
            entry.height_in_flight = true;
            entry.wants_height = false;
        }
    }

    /// A peer answered its height. This touches that peer only.
    pub fn height_answered(&mut self, peer: PeerId, height: u64) {
        let entry = self.peers.entry(peer).or_default();
        entry.connected = true;
        entry.reported_height = height;
        entry.height_in_flight = false;
        entry.wants_height = false;
    }

    /// The peer to ask for blocks now, and the height it reported: the highest
    /// above `local_height` among connected peers out of backoff. Ties go to
    /// the lower `PeerId`, so the choice is deterministic.
    pub fn block_source(&self, local_height: u64, now: Instant) -> Option<(PeerId, u64)> {
        if self.blocks_in_flight.is_some() {
            return None;
        }
        self.peers
            .iter()
            .filter(|(_, p)| p.ready(now) && p.reported_height > local_height)
            .fold(None, |best: Option<(PeerId, u64)>, (id, p)| match best {
                Some((_, height)) if height >= p.reported_height => best,
                _ => Some((*id, p.reported_height)),
            })
    }

    /// A block request was handed to the P2P layer.
    pub fn blocks_requested(&mut self, peer: PeerId) {
        self.blocks_in_flight = Some(peer);
    }

    /// Whether blocks from this peer answer the request in flight.
    pub fn expects_blocks(&self, peer: &PeerId) -> bool {
        self.blocks_in_flight.as_ref() == Some(peer)
    }

    /// The answer to the request in flight has been handled.
    pub fn blocks_answered(&mut self, peer: PeerId, outcome: BatchOutcome, now: Instant) {
        self.blocks_in_flight = None;
        match outcome {
            BatchOutcome::Progress => self.peer_progressed(peer),
            BatchOutcome::AlreadyHeld => {}
            BatchOutcome::Empty | BatchOutcome::Refused => self.peer_failed(peer, now),
        }
    }

    /// A request failed before any answer. A failed height request is asked
    /// again once the backoff ends.
    pub fn request_failed(&mut self, peer: PeerId, kind: SyncRequestKind, now: Instant) {
        match kind {
            SyncRequestKind::Height => {
                let entry = self.peers.entry(peer).or_default();
                entry.height_in_flight = false;
                entry.wants_height = true;
            }
            SyncRequestKind::Blocks => {
                if self.blocks_in_flight == Some(peer) {
                    self.blocks_in_flight = None;
                }
            }
        }
        self.peer_failed(peer, now);
    }

    /// A peer answered with an error. The answer does not say to which
    /// request, so whatever was in flight to that peer is released.
    pub fn error_answered(&mut self, peer: PeerId, now: Instant) {
        if self.blocks_in_flight == Some(peer) {
            self.blocks_in_flight = None;
        }
        let entry = self.peers.entry(peer).or_default();
        if entry.height_in_flight {
            entry.height_in_flight = false;
            entry.wants_height = true;
        }
        self.peer_failed(peer, now);
    }

    /// A peer supplied a block that was applied.
    pub fn peer_progressed(&mut self, peer: PeerId) {
        if let Some(entry) = self.peers.get_mut(&peer) {
            entry.failures = 0;
            entry.retry_at = None;
        }
    }

    /// Counts a failure against a peer, and keeps it out of use for a while.
    pub fn peer_failed(&mut self, peer: PeerId, now: Instant) {
        let entry = self.peers.entry(peer).or_default();
        entry.failures = entry.failures.saturating_add(1);
        entry.retry_at = Some(now + backoff(entry.failures));
    }

    /// The earliest moment a backoff ends, so that a retry needs no other
    /// event to happen.
    pub fn next_wake(&self) -> Option<Instant> {
        self.peers.values().filter(|p| p.connected).filter_map(|p| p.retry_at).min()
    }

    /// Clears the backoffs that have ended, so that `next_wake` moves on.
    pub fn release_expired(&mut self, now: Instant) {
        for entry in self.peers.values_mut() {
            if entry.retry_at.is_some_and(|at| at <= now) {
                entry.retry_at = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker_with(peers: &[PeerId]) -> SyncTracker {
        let mut tracker = SyncTracker::default();
        for peer in peers {
            tracker.peer_connected(*peer);
        }
        tracker
    }

    fn two_peers() -> (PeerId, PeerId) {
        let (a, b) = (PeerId::random(), PeerId::random());
        if a < b {
            (a, b)
        } else {
            (b, a)
        }
    }

    #[test]
    fn backoff_doubles_up_to_its_cap() {
        let seconds: Vec<u64> = (1..=9).map(|f| backoff(f).as_secs()).collect();
        assert_eq!(seconds, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(backoff(u32::MAX), BACKOFF_CAP);
    }

    #[test]
    fn a_connected_peer_is_asked_its_height_once() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        assert_eq!(tracker.heights_due(now), vec![peer]);
        tracker.height_requested(peer);
        assert!(tracker.heights_due(now).is_empty());
    }

    #[test]
    fn a_failed_height_request_is_asked_again_after_the_backoff() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_requested(peer);
        tracker.request_failed(peer, SyncRequestKind::Height, now);
        assert!(tracker.heights_due(now).is_empty(), "still backing off");
        assert_eq!(tracker.next_wake(), Some(now + Duration::from_secs(1)));
        assert_eq!(
            tracker.heights_due(now + Duration::from_secs(1)),
            vec![peer]
        );
    }

    #[test]
    fn a_failed_block_request_releases_the_request_and_rotates() {
        let now = Instant::now();
        let (low, high) = two_peers();
        let mut tracker = tracker_with(&[low, high]);
        tracker.height_answered(low, 10);
        tracker.height_answered(high, 10);
        assert_eq!(tracker.block_source(0, now), Some((low, 10)));
        tracker.blocks_requested(low);
        assert_eq!(tracker.block_source(0, now), None, "one request at a time");

        tracker.request_failed(low, SyncRequestKind::Blocks, now);
        assert!(!tracker.expects_blocks(&low));
        assert_eq!(tracker.block_source(0, now), Some((high, 10)));
    }

    #[test]
    fn the_highest_reported_peer_is_chosen_and_ties_are_deterministic() {
        let now = Instant::now();
        let (low, high) = two_peers();
        let mut tracker = tracker_with(&[low, high]);
        tracker.height_answered(low, 5);
        tracker.height_answered(high, 9);
        assert_eq!(tracker.block_source(0, now), Some((high, 9)));
        tracker.height_answered(low, 9);
        assert_eq!(tracker.block_source(0, now), Some((low, 9)));
    }

    #[test]
    fn a_low_height_from_one_peer_does_not_stop_a_sync_another_serves() {
        let now = Instant::now();
        let (low, high) = two_peers();
        let mut tracker = tracker_with(&[low, high]);
        tracker.height_answered(high, 100);
        tracker.height_answered(low, 0);
        assert_eq!(tracker.block_source(10, now), Some((high, 100)));
    }

    #[test]
    fn only_the_peer_asked_can_answer_a_block_request() {
        let (asked, other) = two_peers();
        let mut tracker = tracker_with(&[asked, other]);
        tracker.blocks_requested(asked);
        assert!(tracker.expects_blocks(&asked));
        assert!(!tracker.expects_blocks(&other));
    }

    #[test]
    fn a_disconnect_releases_the_request_and_the_peer_is_not_chosen() {
        let now = Instant::now();
        let (gone, stays) = two_peers();
        let mut tracker = tracker_with(&[gone, stays]);
        tracker.height_answered(gone, 10);
        tracker.height_answered(stays, 8);
        tracker.blocks_requested(gone);

        tracker.peer_disconnected(gone, false, now);
        assert_eq!(tracker.block_source(0, now), Some((stays, 8)));
        assert_eq!(tracker.next_wake(), None, "an ordinary close is no failure");
    }

    #[test]
    fn a_peer_that_reconnects_keeps_its_backoff() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.peer_disconnected(peer, true, now);
        tracker.peer_connected(peer);
        assert!(tracker.heights_due(now).is_empty());
        assert_eq!(
            tracker.heights_due(now + Duration::from_secs(1)),
            vec![peer]
        );
    }

    #[test]
    fn a_refused_or_empty_answer_backs_off_and_failures_add_up() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_answered(peer, 10);
        tracker.blocks_requested(peer);
        tracker.blocks_answered(peer, BatchOutcome::Refused, now);
        assert_eq!(tracker.block_source(0, now), None);
        let later = now + Duration::from_secs(1);
        assert_eq!(tracker.block_source(0, later), Some((peer, 10)));

        tracker.blocks_requested(peer);
        tracker.blocks_answered(peer, BatchOutcome::Empty, later);
        assert_eq!(tracker.next_wake(), Some(later + Duration::from_secs(2)));
    }

    #[test]
    fn a_batch_already_held_is_no_failure() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_answered(peer, 10);
        tracker.blocks_requested(peer);
        tracker.blocks_answered(peer, BatchOutcome::AlreadyHeld, now);
        assert_eq!(tracker.next_wake(), None);
        assert_eq!(tracker.block_source(0, now), Some((peer, 10)));
    }

    #[test]
    fn progress_clears_the_failures() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_answered(peer, 10);
        tracker.peer_failed(peer, now);
        tracker.peer_failed(peer, now);
        tracker.blocks_requested(peer);
        tracker.blocks_answered(peer, BatchOutcome::Progress, now);
        assert_eq!(tracker.next_wake(), None);
        tracker.peer_failed(peer, now);
        assert_eq!(tracker.next_wake(), Some(now + Duration::from_secs(1)));
    }

    #[test]
    fn an_error_answer_releases_whatever_was_in_flight() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_requested(peer);
        tracker.blocks_requested(peer);
        tracker.error_answered(peer, now);
        assert!(!tracker.expects_blocks(&peer));
        assert_eq!(
            tracker.heights_due(now + Duration::from_secs(1)),
            vec![peer]
        );
    }

    #[test]
    fn an_announcement_asks_but_is_not_believed() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_requested(peer);
        tracker.height_answered(peer, 3);
        tracker.ask_height(peer);
        assert_eq!(tracker.heights_due(now), vec![peer]);
        assert_eq!(
            tracker.block_source(3, now),
            None,
            "nothing above the answered height"
        );
    }

    #[test]
    fn expired_backoffs_are_cleared_so_a_wake_does_not_repeat() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.peer_failed(peer, now);
        tracker.release_expired(now);
        assert!(tracker.next_wake().is_some(), "not over yet");
        tracker.release_expired(now + Duration::from_secs(1));
        assert_eq!(tracker.next_wake(), None);
    }

    #[test]
    fn the_largest_possible_height_is_handled() {
        let now = Instant::now();
        let peer = PeerId::random();
        let mut tracker = tracker_with(&[peer]);
        tracker.height_answered(peer, u64::MAX);
        assert_eq!(tracker.block_source(0, now), Some((peer, u64::MAX)));
        assert_eq!(tracker.block_source(u64::MAX, now), None);
    }
}
