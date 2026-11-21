//! Rate limits on inbound application requests (S3 A2.3-b-1).
//!
//! Decided when a decoded `GetHeight`, `GetBlocks` or block announcement
//! reaches the node, before the A1 queue and therefore before any storage
//! read. Three levels must all have room:
//!
//! - **peer**: one bucket per peer identity and per message kind;
//! - **source**: one bucket per source (IPv4 address or IPv6 /64, as in
//!   A2.3-a) and per kind, charged for every source the peer is connected
//!   from, so a new identity from the same address gets no fresh quota;
//! - **global**: one budget of work units for every request together.
//!
//! **Costs.** `GetHeight` costs 1. `GetBlocks` costs 1 plus the number of
//! blocks requested, capped at [`MAX_RANGE`], charged before the node reads
//! anything: the requested range, not what is later served, so a request past
//! the local tip pays for what it asked. An announcement costs 1 in its
//! kind's bucket and `1 + size / 64 KiB` in the global budget.
//!
//! **What this does not cover.** Everything before the request is decoded:
//! TCP, Noise (A2.3-a), yamux stream creation, protocol negotiation, frame
//! reading (A2.2b-lite bounds its memory) and decoding. Profiling found that
//! work, not request handling, dominates the CPU of a sync flood.
//!
//! **State.** Tables are bounded. When one is full, only entries whose
//! buckets have all refilled are dropped (absent and full are the same
//! state); otherwise newcomers share one overflow entry. Entries outlive a
//! disconnection, so reconnecting does not refill a bucket.
//!
//! The defaults are experimental values for a small private devnet, not
//! values that are safe everywhere.

use std::collections::HashMap;
use std::hash::Hash;
use std::net::IpAddr;
use std::time::Instant;

use libp2p::swarm::ConnectionId;
use libp2p::PeerId;

use crate::p2p_protocol::{SyncRequest, MAX_RANGE};
use crate::rate_limit::{source_key, Bucket, SourceKey};

/// The kinds of request limited separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `GetHeight`.
    Height = 0,
    /// `GetBlocks`.
    Blocks = 1,
    /// A block announcement.
    Announce = 2,
}

const KINDS: usize = 3;

/// A refill rate and a capacity, in units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    /// Units refilled per second.
    pub per_second: u32,
    /// The bucket's capacity.
    pub burst: u32,
}

const fn rate(per_second: u32, burst: u32) -> Rate {
    Rate { per_second, burst }
}

/// What one request costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cost {
    /// Which kind's buckets it is charged to.
    pub kind: Kind,
    /// Units charged to the peer's and the sources' bucket for that kind.
    pub kind_units: u32,
    /// Units charged to the global work budget.
    pub work_units: u32,
}

/// The cost of a sync request, from the request alone.
#[must_use]
pub fn sync_request_cost(request: &SyncRequest) -> Cost {
    match request {
        SyncRequest::GetHeight => Cost {
            kind: Kind::Height,
            kind_units: 1,
            work_units: 1,
        },
        SyncRequest::GetBlocks {
            start_height,
            end_height,
        } => {
            let blocks = end_height.saturating_sub(*start_height).min(MAX_RANGE);
            let units = 1 + u32::try_from(blocks).unwrap_or(u32::MAX);
            Cost {
                kind: Kind::Blocks,
                kind_units: units,
                work_units: units,
            }
        }
    }
}

/// The cost of an announcement whose block encodes to `encoded_len` bytes.
#[must_use]
pub fn announcement_cost(encoded_len: usize) -> Cost {
    let chunks = u32::try_from(encoded_len / (64 * 1024)).unwrap_or(u32::MAX);
    Cost {
        kind: Kind::Announce,
        kind_units: 1,
        work_units: chunks.saturating_add(1),
    }
}

/// Rates and table bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestRateConfig {
    /// Per peer, indexed by [`Kind`].
    pub peer: [Rate; KINDS],
    /// Per source, indexed by [`Kind`].
    pub source: [Rate; KINDS],
    /// The global work budget.
    pub global: Rate,
    /// The IPv6 prefix length that defines one source.
    pub ipv6_prefix: u8,
    /// The most peers tracked at once.
    pub max_peers: usize,
    /// The most sources tracked at once.
    pub max_sources: usize,
}

impl Default for RequestRateConfig {
    /// Per peer: `GetHeight` 2/s, bursts of 8; `GetBlocks` 512 units/s (two
    /// full ranges), bursts of 1024; announcements 2/s, bursts of 8. Per
    /// source: four times that. Global: 8192 units/s, bursts of 16384.
    /// An honest peer polls heights every 15 s, asks one range at a time and
    /// announces one block per block time.
    fn default() -> Self {
        Self {
            peer: [rate(2, 8), rate(512, 1024), rate(2, 8)],
            source: [rate(8, 32), rate(2048, 4096), rate(8, 32)],
            global: rate(8192, 16_384),
            ipv6_prefix: 64,
            max_peers: 4096,
            max_sources: 4096,
        }
    }
}

/// Which level refused a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestRefusal {
    /// The peer's bucket for that kind.
    Peer,
    /// One of the peer's sources' bucket for that kind.
    Source,
    /// The global work budget.
    Global,
}

/// Counts since start, and table sizes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequestRateStats {
    /// Requests admitted, indexed by [`Kind`].
    pub admitted: [u64; KINDS],
    /// Work units admitted.
    pub work_admitted: u64,
    /// Refused by a peer bucket, indexed by [`Kind`].
    pub refused_peer: [u64; KINDS],
    /// Refused by a source bucket, indexed by [`Kind`].
    pub refused_source: [u64; KINDS],
    /// Refused by the global budget, indexed by [`Kind`].
    pub refused_global: [u64; KINDS],
    /// Requests charged to the peer table's overflow entry.
    pub overflow_peer: u64,
    /// Requests charged to the source table's overflow entry.
    pub overflow_source: u64,
    /// Peers tracked now.
    pub peers: usize,
    /// Sources tracked now.
    pub sources: usize,
}

impl std::fmt::Display for RequestRateStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [h, b, a] = ["height", "blocks", "announce"];
        write!(
            f,
            "request_rate_admitted_{h}={} request_rate_admitted_{b}={} \
             request_rate_admitted_{a}={} request_rate_work_admitted={} \
             request_rate_refused_peer={}/{}/{} request_rate_refused_source={}/{}/{} \
             request_rate_refused_global={}/{}/{} request_rate_overflow_peer={} \
             request_rate_overflow_source={} request_rate_peers={} request_rate_sources={}",
            self.admitted[0],
            self.admitted[1],
            self.admitted[2],
            self.work_admitted,
            self.refused_peer[0],
            self.refused_peer[1],
            self.refused_peer[2],
            self.refused_source[0],
            self.refused_source[1],
            self.refused_source[2],
            self.refused_global[0],
            self.refused_global[1],
            self.refused_global[2],
            self.overflow_peer,
            self.overflow_source,
            self.peers,
            self.sources
        )
    }
}

type Buckets = [Bucket; KINDS];

fn full(rates: &[Rate; KINDS], now: Instant) -> Buckets {
    [
        Bucket::full(rates[0].burst, now),
        Bucket::full(rates[1].burst, now),
        Bucket::full(rates[2].burst, now),
    ]
}

/// Where a key's buckets live: its own entry, or the shared overflow entry.
#[derive(Debug, Clone, Copy)]
enum Slot<K> {
    Own(K),
    Overflow,
}

/// A bounded map from keys to per-kind buckets.
#[derive(Debug)]
struct Table<K> {
    entries: HashMap<K, Buckets>,
    overflow: Buckets,
    max: usize,
}

impl<K: Hash + Eq + Copy> Table<K> {
    fn new(max: usize, rates: &[Rate; KINDS], now: Instant) -> Self {
        Self {
            entries: HashMap::new(),
            overflow: full(rates, now),
            max,
        }
    }

    /// Where `key` lives, creating its entry if there is room. A full table
    /// first drops entries whose buckets have all refilled, except the keys in
    /// `keep` (the ones this request is being checked against).
    fn slot(&mut self, key: K, keep: &[K], rates: &[Rate; KINDS], now: Instant) -> Slot<K> {
        if self.entries.contains_key(&key) {
            return Slot::Own(key);
        }
        if self.entries.len() >= self.max {
            self.entries.retain(|k, buckets| {
                if keep.contains(k) {
                    return true;
                }
                let mut all_full = true;
                for (bucket, r) in buckets.iter_mut().zip(rates) {
                    bucket.refill(r.per_second, r.burst, now);
                    all_full &= bucket.is_full(r.burst);
                }
                !all_full
            });
        }
        if self.entries.len() < self.max {
            self.entries.insert(key, full(rates, now));
            Slot::Own(key)
        } else {
            Slot::Overflow
        }
    }

    fn bucket(&mut self, slot: Slot<K>, kind: Kind) -> &mut Bucket {
        let buckets = match slot {
            Slot::Own(key) => self.entries.get_mut(&key).unwrap_or(&mut self.overflow),
            Slot::Overflow => &mut self.overflow,
        };
        &mut buckets[kind as usize]
    }
}

/// The most connections remembered for source lookup; A1 allows 80.
const MAX_CONNECTIONS: usize = 1024;

/// The limiter: pure bookkeeping, with the clock passed in.
#[derive(Debug)]
pub struct RequestLimiter {
    config: RequestRateConfig,
    peers: Table<PeerId>,
    sources: Table<SourceKey>,
    global: Bucket,
    connections: HashMap<ConnectionId, (PeerId, SourceKey)>,
    stats: RequestRateStats,
}

impl RequestLimiter {
    /// A limiter with full buckets at `now`.
    #[must_use]
    pub fn new(config: RequestRateConfig, now: Instant) -> Self {
        Self {
            config,
            peers: Table::new(config.max_peers, &config.peer, now),
            sources: Table::new(config.max_sources, &config.source, now),
            global: Bucket::full(config.global.burst, now),
            connections: HashMap::new(),
            stats: RequestRateStats::default(),
        }
    }

    /// Records the source of an established connection.
    pub fn connection_established(&mut self, id: ConnectionId, peer: PeerId, ip: Option<IpAddr>) {
        if let Some(ip) = ip {
            if self.connections.len() < MAX_CONNECTIONS {
                self.connections.insert(id, (peer, source_key(ip, self.config.ipv6_prefix)));
            }
        }
    }

    /// Forgets a closed connection's source. The peer's and the source's
    /// buckets are kept: reconnecting does not refill them.
    pub fn connection_closed(&mut self, id: ConnectionId) {
        self.connections.remove(&id);
    }

    /// The distinct sources `peer` is connected from now.
    fn sources_of(&self, peer: PeerId) -> Vec<SourceKey> {
        let mut keys: Vec<SourceKey> = Vec::new();
        for (p, key) in self.connections.values() {
            if *p == peer && !keys.contains(key) {
                keys.push(*key);
            }
        }
        keys
    }

    /// Counts so far.
    #[must_use]
    pub fn stats(&self) -> RequestRateStats {
        RequestRateStats {
            peers: self.peers.entries.len(),
            sources: self.sources.entries.len(),
            ..self.stats
        }
    }

    /// Admits one request from `peer` at `now`, or says which level refused
    /// it. Tokens are taken from every bucket only when all of them have
    /// room; a refusal takes nothing.
    ///
    /// # Errors
    ///
    /// Returns the first level, most specific first, without room.
    pub fn try_admit(
        &mut self,
        peer: PeerId,
        cost: Cost,
        now: Instant,
    ) -> Result<(), RequestRefusal> {
        let c = self.config;
        let k = cost.kind as usize;
        let refuse = |stats: &mut RequestRateStats, level: RequestRefusal| {
            let counter = match level {
                RequestRefusal::Peer => &mut stats.refused_peer[k],
                RequestRefusal::Source => &mut stats.refused_source[k],
                RequestRefusal::Global => &mut stats.refused_global[k],
            };
            *counter = counter.saturating_add(1);
            Err(level)
        };

        let peer_slot = self.peers.slot(peer, &[peer], &c.peer, now);
        if matches!(peer_slot, Slot::Overflow) {
            self.stats.overflow_peer = self.stats.overflow_peer.saturating_add(1);
        }
        let bucket = self.peers.bucket(peer_slot, cost.kind);
        bucket.refill(c.peer[k].per_second, c.peer[k].burst, now);
        if !bucket.has(cost.kind_units) {
            return refuse(&mut self.stats, RequestRefusal::Peer);
        }

        let keys = self.sources_of(peer);
        let mut source_slots = Vec::new();
        for key in keys.iter().copied() {
            let slot = self.sources.slot(key, &keys, &c.source, now);
            if matches!(slot, Slot::Overflow) {
                self.stats.overflow_source = self.stats.overflow_source.saturating_add(1);
            }
            let bucket = self.sources.bucket(slot, cost.kind);
            bucket.refill(c.source[k].per_second, c.source[k].burst, now);
            if !bucket.has(cost.kind_units) {
                return refuse(&mut self.stats, RequestRefusal::Source);
            }
            source_slots.push(slot);
        }
        // Two connections from one overflowed source share one bucket: charge
        // the overflow entry once.
        let mut charged_overflow = false;
        source_slots.retain(|slot| match slot {
            Slot::Overflow if charged_overflow => false,
            Slot::Overflow => {
                charged_overflow = true;
                true
            }
            Slot::Own(_) => true,
        });

        self.global.refill(c.global.per_second, c.global.burst, now);
        if !self.global.has(cost.work_units) {
            return refuse(&mut self.stats, RequestRefusal::Global);
        }

        self.peers.bucket(peer_slot, cost.kind).take(cost.kind_units);
        for slot in source_slots {
            self.sources.bucket(slot, cost.kind).take(cost.kind_units);
        }
        self.global.take(cost.work_units);
        self.stats.admitted[k] = self.stats.admitted[k].saturating_add(1);
        self.stats.work_admitted =
            self.stats.work_admitted.saturating_add(u64::from(cost.work_units));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
    }

    fn height() -> Cost {
        sync_request_cost(&SyncRequest::GetHeight)
    }

    fn blocks(n: u64) -> Cost {
        sync_request_cost(&SyncRequest::GetBlocks {
            start_height: 10,
            end_height: 10 + n,
        })
    }

    fn connect(l: &mut RequestLimiter, conn: usize, peer: PeerId, last: u8) {
        l.connection_established(ConnectionId::new_unchecked(conn), peer, Some(ip(last)));
    }

    #[test]
    fn costs_follow_the_request_and_never_exceed_the_bursts() {
        assert_eq!(height().work_units, 1);
        assert_eq!(blocks(1).kind_units, 2);
        assert_eq!(blocks(256).kind_units, 257);
        assert_eq!(blocks(10_000).kind_units, 257, "capped at MAX_RANGE");
        let backwards = sync_request_cost(&SyncRequest::GetBlocks {
            start_height: 50,
            end_height: 10,
        });
        assert_eq!(backwards.kind_units, 1, "an empty range costs the request");
        assert_eq!(announcement_cost(0).work_units, 1);
        assert_eq!(announcement_cost(16 * 1024 * 1024).work_units, 257);
        assert_eq!(announcement_cost(16 * 1024 * 1024).kind_units, 1);
        // The largest cost fits every default burst, so nothing is refused forever.
        let c = RequestRateConfig::default();
        for level in [&c.peer, &c.source] {
            assert!(level[Kind::Blocks as usize].burst >= 257);
            assert!(level[Kind::Height as usize].burst >= 1);
            assert!(level[Kind::Announce as usize].burst >= 1);
        }
        assert!(c.global.burst >= 257);
    }

    #[test]
    fn a_peer_gets_its_burst_then_its_rate_per_kind() {
        let t0 = Instant::now();
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        for _ in 0..8 {
            assert_eq!(l.try_admit(peer, height(), t0), Ok(()));
        }
        assert_eq!(l.try_admit(peer, height(), t0), Err(RequestRefusal::Peer));
        // Other kinds have their own buckets.
        assert_eq!(l.try_admit(peer, blocks(256), t0), Ok(()));
        // Half a second refills one GetHeight at 2/s.
        assert_eq!(
            l.try_admit(peer, height(), t0 + Duration::from_millis(500)),
            Ok(())
        );
        assert_eq!(
            l.try_admit(peer, height(), t0 + Duration::from_millis(500)),
            Err(RequestRefusal::Peer)
        );
    }

    #[test]
    fn get_blocks_is_charged_by_the_range_requested() {
        let t0 = Instant::now();
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        // Burst 1024: three full ranges (3 × 257 = 771) fit, a fourth does not.
        for _ in 0..3 {
            assert_eq!(l.try_admit(peer, blocks(256), t0), Ok(()));
        }
        assert_eq!(
            l.try_admit(peer, blocks(256), t0),
            Err(RequestRefusal::Peer)
        );
        // A small range still fits in what is left (1024 - 771 = 253).
        assert_eq!(l.try_admit(peer, blocks(100), t0), Ok(()));
        assert_eq!(l.stats().work_admitted, 3 * 257 + 101);
    }

    #[test]
    fn new_identities_from_one_address_share_its_source_bucket() {
        let t0 = Instant::now();
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let mut admitted = 0;
        for i in 0..20 {
            let peer = PeerId::random();
            connect(&mut l, i, peer, 7);
            for _ in 0..8 {
                if l.try_admit(peer, height(), t0).is_ok() {
                    admitted += 1;
                }
            }
        }
        assert_eq!(admitted, 32, "the source's burst, however many identities");
        assert_eq!(l.stats().refused_source[Kind::Height as usize], 160 - 32);
        assert_eq!(l.stats().refused_peer[Kind::Height as usize], 0);
    }

    #[test]
    fn a_peer_on_two_addresses_is_charged_to_both() {
        let t0 = Instant::now();
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        connect(&mut l, 2, peer, 2);
        let other = PeerId::random();
        connect(&mut l, 3, other, 2);
        // `peer` spends 8 of address 2's 32 GetHeight tokens.
        for _ in 0..8 {
            assert!(l.try_admit(peer, height(), t0).is_ok());
        }
        let mut others = 0;
        while l.try_admit(other, height(), t0).is_ok() {
            others += 1;
        }
        assert_eq!(others, 8, "its own peer burst; address 2 still had 24");
        // A closed connection stops charging that source.
        l.connection_closed(ConnectionId::new_unchecked(2));
        assert_eq!(l.sources_of(peer).len(), 1);
    }

    #[test]
    fn rotating_sources_and_identities_cannot_exceed_the_global_budget() {
        let t0 = Instant::now();
        let config = RequestRateConfig {
            global: rate(100, 1000),
            ..RequestRateConfig::default()
        };
        let mut l = RequestLimiter::new(config, t0);
        let mut work = 0u64;
        for i in 0..200u8 {
            let peer = PeerId::random();
            connect(&mut l, usize::from(i), peer, i);
            if l.try_admit(peer, blocks(256), t0).is_ok() {
                work += 257;
            }
        }
        assert!(work <= 1000, "{work}");
        assert_eq!(work, 3 * 257);
        assert!(l.stats().refused_global[Kind::Blocks as usize] > 0);
    }

    #[test]
    fn a_refusal_takes_nothing_from_any_bucket() {
        let t0 = Instant::now();
        let config = RequestRateConfig {
            global: rate(1, 300),
            ..RequestRateConfig::default()
        };
        let mut l = RequestLimiter::new(config, t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        assert!(l.try_admit(peer, blocks(256), t0).is_ok());
        // Refused by the global budget: the peer's own bucket is untouched.
        assert_eq!(
            l.try_admit(peer, blocks(256), t0),
            Err(RequestRefusal::Global)
        );
        assert_eq!(
            l.try_admit(peer, blocks(256), t0),
            Err(RequestRefusal::Global)
        );
        // Give the global budget room again: the peer still has 1024 - 257 =
        // 767 units, two full ranges and no third, so the refusals took nothing.
        l.config.global = rate(1, 10_000);
        l.global = Bucket::full(10_000, t0);
        assert!(l.try_admit(peer, blocks(256), t0).is_ok());
        assert!(l.try_admit(peer, blocks(256), t0).is_ok());
        assert_eq!(
            l.try_admit(peer, blocks(256), t0),
            Err(RequestRefusal::Peer)
        );
    }

    #[test]
    fn reconnecting_does_not_refill_a_bucket() {
        let t0 = Instant::now();
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        while l.try_admit(peer, height(), t0).is_ok() {}
        l.connection_closed(ConnectionId::new_unchecked(1));
        connect(&mut l, 2, peer, 1);
        assert_eq!(l.try_admit(peer, height(), t0), Err(RequestRefusal::Peer));
    }

    #[test]
    fn full_tables_never_grow_and_never_reset_a_used_quota() {
        let t0 = Instant::now();
        let config = RequestRateConfig {
            max_peers: 2,
            max_sources: 2,
            ..RequestRateConfig::default()
        };
        let mut l = RequestLimiter::new(config, t0);
        let used: Vec<PeerId> = (0..2).map(|_| PeerId::random()).collect();
        for (i, peer) in used.iter().enumerate() {
            connect(&mut l, i, *peer, u8::try_from(i).unwrap());
            while l.try_admit(*peer, height(), t0).is_ok() {}
        }
        // Newcomers, on new addresses: they share the overflow entries.
        let mut newcomers = 0;
        for i in 10..20u8 {
            let peer = PeerId::random();
            connect(&mut l, usize::from(i), peer, i);
            while l.try_admit(peer, height(), t0).is_ok() {
                newcomers += 1;
            }
        }
        assert_eq!(newcomers, 8, "one overflow peer burst, shared");
        let s = l.stats();
        assert_eq!((s.peers, s.sources), (2, 2));
        assert!(s.overflow_peer > 0 && s.overflow_source > 0);
        for peer in &used {
            assert_eq!(l.try_admit(*peer, height(), t0), Err(RequestRefusal::Peer));
        }
        // Once refilled, old entries make room again.
        let later = t0 + Duration::from_secs(60);
        let peer = PeerId::random();
        connect(&mut l, 99, peer, 99);
        assert!(l.try_admit(peer, height(), later).is_ok());
        assert!(l.stats().peers <= 2);
    }

    #[test]
    fn clock_edges_neither_panic_nor_mint_tokens() {
        let t0 = Instant::now() + Duration::from_secs(10);
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        while l.try_admit(peer, height(), t0).is_ok() {}
        let earlier = t0.checked_sub(Duration::from_secs(5)).unwrap();
        assert!(l.try_admit(peer, height(), earlier).is_err());
        let far = t0 + Duration::from_secs(10 * 365 * 24 * 3600);
        let mut n = 0;
        while l.try_admit(peer, height(), far).is_ok() {
            n += 1;
        }
        assert_eq!(n, 8, "capped at the burst");
    }

    #[test]
    fn counters_add_up_to_the_attempts() {
        let t0 = Instant::now();
        let mut l = RequestLimiter::new(RequestRateConfig::default(), t0);
        let peer = PeerId::random();
        connect(&mut l, 1, peer, 1);
        for _ in 0..20 {
            let _ = l.try_admit(peer, height(), t0);
            let _ = l.try_admit(peer, blocks(256), t0);
            let _ = l.try_admit(peer, announcement_cost(100), t0);
        }
        let s = l.stats();
        for k in 0..KINDS {
            assert_eq!(
                s.admitted[k] + s.refused_peer[k] + s.refused_source[k] + s.refused_global[k],
                20
            );
        }
        assert_eq!(s.admitted, [8, 3, 8]);
    }

    #[test]
    fn concurrent_requests_get_exactly_the_burst() {
        let t0 = Instant::now();
        let peer = PeerId::random();
        let mut inner = RequestLimiter::new(RequestRateConfig::default(), t0);
        connect(&mut inner, 1, peer, 1);
        let limiter = std::sync::Mutex::new(inner);
        let admitted = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..50 {
                        if limiter.lock().unwrap().try_admit(peer, height(), t0).is_ok() {
                            admitted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        assert_eq!(admitted.into_inner(), 8);
    }
}
