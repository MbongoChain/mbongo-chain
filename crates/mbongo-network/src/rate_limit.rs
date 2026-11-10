//! Rate limits on inbound connection attempts (S3 A2.3-a).
//!
//! libp2p offers every inbound TCP connection to the behaviours through
//! `handle_pending_inbound_connection` *before* it runs the Noise handshake
//! and multiplexer upgrade; a refusal there drops the socket with no
//! cryptographic work done. [`InboundRateLimit`] decides at that point, from
//! the remote address alone.
//!
//! Two token buckets must both have a token:
//!
//! - one **per source**: an IPv4 address, or an IPv6 prefix (`/64` by
//!   default, see [`InboundRateConfig::ipv6_prefix`]). An IPv4 address mapped
//!   into IPv6 counts as that IPv4 address;
//! - one **global**, for every attempt together, so rotating source addresses
//!   cannot exceed the global rate.
//!
//! The source table is bounded ([`InboundRateConfig::max_sources`]). When it
//! is full, only entries whose bucket has refilled completely are dropped (an
//! absent entry and a full bucket are the same state, so dropping one gives no
//! source an advantage). If none can be dropped, new sources share one
//! overflow bucket instead of getting their own: the table never grows and no
//! quota is reset in a source's favour.
//!
//! Outbound dials are not limited here.
//!
//! The defaults are experimental values for a small private devnet, not
//! values that are safe everywhere.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use libp2p::core::{multiaddr::Protocol, Endpoint};
use libp2p::swarm::{
    dummy, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};

/// Token arithmetic is in thousandths of a token, in integers.
const MILLI: u64 = 1000;

/// Rates and table bounds. Experimental defaults; see [`Default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboundRateConfig {
    /// Inbound attempts per second refilled to the global bucket.
    pub global_per_second: u32,
    /// The global bucket's capacity.
    pub global_burst: u32,
    /// Inbound attempts per second refilled to each source's bucket.
    pub source_per_second: u32,
    /// Each source bucket's capacity.
    pub source_burst: u32,
    /// The IPv6 prefix length that defines one source (0 to 128).
    pub ipv6_prefix: u8,
    /// The most sources tracked at once.
    pub max_sources: usize,
}

impl Default for InboundRateConfig {
    /// Global: 20 attempts per second, bursts of 100. Per source: 1 per
    /// second, bursts of 10, so a handful of legitimate peers behind one NAT
    /// address, or one peer reconnecting after an outage, are not refused.
    /// IPv6 sources are /64 prefixes. At most 4096 sources.
    fn default() -> Self {
        Self {
            global_per_second: 20,
            global_burst: 100,
            source_per_second: 1,
            source_burst: 10,
            ipv6_prefix: 64,
            max_sources: 4096,
        }
    }
}

/// One token bucket, in thousandths of a token. Also used by the request
/// limits (S3 A2.3-b-1).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bucket {
    tokens: u64,
    updated: Instant,
}

impl Bucket {
    /// Whether `units` whole tokens are available.
    pub(crate) fn has(&self, units: u32) -> bool {
        self.tokens >= u64::from(units) * MILLI
    }

    /// Takes `units` whole tokens; call only after [`Bucket::has`].
    pub(crate) fn take(&mut self, units: u32) {
        self.tokens -= u64::from(units) * MILLI;
    }

    pub(crate) fn full(burst: u32, now: Instant) -> Self {
        Self {
            tokens: u64::from(burst) * MILLI,
            updated: now,
        }
    }

    /// Adds what has accrued since the last update, up to the capacity. A
    /// clock reading earlier than the last one adds nothing.
    pub(crate) fn refill(&mut self, per_second: u32, burst: u32, now: Instant) {
        let elapsed = now.saturating_duration_since(self.updated);
        let millis = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let earned = millis.saturating_mul(u64::from(per_second));
        let capacity = u64::from(burst) * MILLI;
        self.tokens = self.tokens.saturating_add(earned).min(capacity);
        if now > self.updated {
            self.updated = now;
        }
    }

    fn has_one(&self) -> bool {
        self.tokens >= MILLI
    }

    fn take_one(&mut self) {
        self.tokens -= MILLI;
    }

    pub(crate) fn is_full(&self, burst: u32) -> bool {
        self.tokens >= u64::from(burst) * MILLI
    }
}

/// What one source is, for rate purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKey {
    /// An IPv4 address (including one mapped into IPv6).
    V4([u8; 4]),
    /// An IPv6 address masked to the configured prefix.
    V6Prefix(u128),
}

/// The source an address belongs to, under `ipv6_prefix`.
#[must_use]
pub fn source_key(ip: IpAddr, ipv6_prefix: u8) -> SourceKey {
    match ip {
        IpAddr::V4(v4) => SourceKey::V4(v4.octets()),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => SourceKey::V4(v4.octets()),
            None => SourceKey::V6Prefix(mask(v6, ipv6_prefix)),
        },
    }
}

fn mask(v6: Ipv6Addr, prefix: u8) -> u128 {
    let bits = u128::from(v6);
    match prefix.min(128) {
        0 => 0,
        p => bits & (u128::MAX << (128 - u32::from(p))),
    }
}

/// Why an attempt was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateRefusal {
    /// The source's bucket was empty.
    Source,
    /// The global bucket was empty.
    Global,
}

impl std::fmt::Display for RateRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source => f.write_str("inbound connection rate exceeded for this source"),
            Self::Global => f.write_str("inbound connection rate exceeded"),
        }
    }
}

impl std::error::Error for RateRefusal {}

/// Counts since start, and the table's size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RateStats {
    /// Attempts admitted.
    pub admitted: u64,
    /// Attempts refused: the source's bucket was empty.
    pub refused_source: u64,
    /// Attempts refused: the global bucket was empty.
    pub refused_global: u64,
    /// Attempts charged to the overflow bucket because the table was full.
    pub overflow_charged: u64,
    /// Sources tracked now.
    pub sources: usize,
}

impl std::fmt::Display for RateStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inbound_rate_admitted={} inbound_rate_refused_source={} \
             inbound_rate_refused_global={} inbound_rate_overflow_charged={} \
             inbound_rate_sources={}",
            self.admitted,
            self.refused_source,
            self.refused_global,
            self.overflow_charged,
            self.sources
        )
    }
}

/// The limiter itself: pure bookkeeping, with the clock passed in.
#[derive(Debug)]
pub struct RateLimiter {
    config: InboundRateConfig,
    global: Bucket,
    overflow: Bucket,
    sources: HashMap<SourceKey, Bucket>,
    stats: RateStats,
}

impl RateLimiter {
    /// A limiter with full buckets at `now`.
    #[must_use]
    pub fn new(config: InboundRateConfig, now: Instant) -> Self {
        Self {
            config,
            global: Bucket::full(config.global_burst, now),
            overflow: Bucket::full(config.source_burst, now),
            sources: HashMap::new(),
            stats: RateStats::default(),
        }
    }

    /// Counts so far.
    #[must_use]
    pub fn stats(&self) -> RateStats {
        RateStats {
            sources: self.sources.len(),
            ..self.stats
        }
    }

    /// Drops every entry whose bucket has refilled completely: absent and
    /// full are the same state.
    fn prune_refilled(&mut self, now: Instant) {
        let (per_second, burst) = (self.config.source_per_second, self.config.source_burst);
        self.sources.retain(|_, bucket| {
            bucket.refill(per_second, burst, now);
            !bucket.is_full(burst)
        });
    }

    /// Admits one attempt from `ip` at `now`, or says why not. A token is
    /// taken from both buckets only when both have one.
    ///
    /// # Errors
    ///
    /// Returns which bucket was empty.
    pub fn try_admit(&mut self, ip: IpAddr, now: Instant) -> Result<(), RateRefusal> {
        let c = self.config;
        self.global.refill(c.global_per_second, c.global_burst, now);
        if !self.global.has_one() {
            self.stats.refused_global = self.stats.refused_global.saturating_add(1);
            return Err(RateRefusal::Global);
        }
        let key = source_key(ip, c.ipv6_prefix);
        if !self.sources.contains_key(&key) && self.sources.len() >= c.max_sources {
            self.prune_refilled(now);
        }
        let in_table = self.sources.contains_key(&key) || self.sources.len() < c.max_sources;
        let bucket = if in_table {
            self.sources.entry(key).or_insert_with(|| Bucket::full(c.source_burst, now))
        } else {
            self.stats.overflow_charged = self.stats.overflow_charged.saturating_add(1);
            &mut self.overflow
        };
        bucket.refill(c.source_per_second, c.source_burst, now);
        if !bucket.has_one() {
            self.stats.refused_source = self.stats.refused_source.saturating_add(1);
            return Err(RateRefusal::Source);
        }
        bucket.take_one();
        self.global.take_one();
        self.stats.admitted = self.stats.admitted.saturating_add(1);
        Ok(())
    }
}

/// The IP address a multiaddr starts with, if any.
pub(crate) fn ip_of(addr: &Multiaddr) -> Option<IpAddr> {
    match addr.iter().next()? {
        Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
        _ => None,
    }
}

/// The behaviour that applies [`RateLimiter`] to inbound connections before
/// their handshake. Outbound connections pass untouched.
#[derive(Debug)]
pub struct InboundRateLimit {
    limiter: RateLimiter,
}

impl InboundRateLimit {
    /// A limiter with `config`, starting now.
    #[must_use]
    pub fn new(config: InboundRateConfig) -> Self {
        Self {
            limiter: RateLimiter::new(config, Instant::now()),
        }
    }

    /// Counts so far.
    #[must_use]
    pub fn stats(&self) -> RateStats {
        self.limiter.stats()
    }
}

impl Default for InboundRateLimit {
    fn default() -> Self {
        Self::new(InboundRateConfig::default())
    }
}

impl NetworkBehaviour for InboundRateLimit {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = std::convert::Infallible;

    fn handle_pending_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        // A remote address with no IP (not expected over TCP) is charged to
        // the global bucket only, through the unspecified address's source.
        let ip = ip_of(remote_addr).unwrap_or(IpAddr::V6(Ipv6Addr::UNSPECIFIED));
        self.limiter.try_admit(ip, Instant::now()).map_err(ConnectionDenied::new)
    }

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
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

    fn on_swarm_event(&mut self, _event: FromSwarm) {}

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

/// A duration, for tests and callers that reason in seconds.
#[must_use]
pub fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn v6(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn config(global: (u32, u32), source: (u32, u32), max_sources: usize) -> InboundRateConfig {
        InboundRateConfig {
            global_per_second: global.0,
            global_burst: global.1,
            source_per_second: source.0,
            source_burst: source.1,
            ipv6_prefix: 64,
            max_sources,
        }
    }

    #[test]
    fn a_source_gets_its_burst_then_one_per_refill_period() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(config((1000, 1000), (1, 3), 16), t0);
        let ip = v4(10, 0, 0, 1);
        for _ in 0..3 {
            assert_eq!(l.try_admit(ip, t0), Ok(()));
        }
        assert_eq!(l.try_admit(ip, t0), Err(RateRefusal::Source));
        let almost = t0 + Duration::from_millis(999);
        assert_eq!(l.try_admit(ip, almost), Err(RateRefusal::Source));
        assert_eq!(l.try_admit(ip, t0 + secs(1)), Ok(()));
        assert_eq!(l.try_admit(ip, t0 + secs(1)), Err(RateRefusal::Source));
        // Long idle refills to the burst, not beyond.
        let later = t0 + secs(3600);
        for _ in 0..3 {
            assert_eq!(l.try_admit(ip, later), Ok(()));
        }
        assert_eq!(l.try_admit(ip, later), Err(RateRefusal::Source));
    }

    #[test]
    fn rotating_sources_cannot_exceed_the_global_rate() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(config((5, 20), (1, 10), 10_000), t0);
        let mut admitted = 0;
        for i in 0..1000u32 {
            let ip = IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + i));
            if l.try_admit(ip, t0).is_ok() {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 20, "the global burst, whatever the sources");
        let s = l.stats();
        assert_eq!((s.admitted, s.refused_global), (20, 980));
        // One second later: five more, across any sources.
        let mut later = 0;
        for i in 1000..1100u32 {
            if l.try_admit(IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + i)), t0 + secs(1)).is_ok() {
                later += 1;
            }
        }
        assert_eq!(later, 5);
    }

    #[test]
    fn a_refused_source_does_not_spend_global_tokens() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(config((1, 5), (1, 1), 16), t0);
        let noisy = v4(10, 0, 0, 9);
        assert!(l.try_admit(noisy, t0).is_ok());
        for _ in 0..100 {
            assert_eq!(l.try_admit(noisy, t0), Err(RateRefusal::Source));
        }
        for i in 1..=4 {
            assert!(
                l.try_admit(v4(10, 0, 1, i), t0).is_ok(),
                "others still get the global budget"
            );
        }
    }

    #[test]
    fn ipv4_and_ipv4_mapped_addresses_are_the_same_source() {
        let mapped = v6("::ffff:192.0.2.7");
        assert_eq!(source_key(mapped, 64), source_key(v4(192, 0, 2, 7), 64));
        assert_ne!(
            source_key(v4(192, 0, 2, 7), 64),
            source_key(v4(192, 0, 2, 8), 64)
        );
    }

    #[test]
    fn ipv6_addresses_in_one_prefix_share_a_source_and_others_do_not() {
        let a = v6("fd00:1:2:3::1");
        let b = v6("fd00:1:2:3:ffff:ffff:ffff:ffff");
        let c = v6("fd00:1:2:4::1");
        assert_eq!(source_key(a, 64), source_key(b, 64));
        assert_ne!(source_key(a, 64), source_key(c, 64));
        // Configurable prefix.
        assert_eq!(source_key(a, 48), source_key(c, 48));
        assert_ne!(source_key(a, 128), source_key(b, 128));
        assert_eq!(source_key(a, 0), source_key(c, 0));
        assert_eq!(source_key(a, 200), source_key(a, 128), "clamped to 128");

        let t0 = Instant::now();
        let mut l = RateLimiter::new(config((1000, 1000), (1, 2), 16), t0);
        assert!(l.try_admit(a, t0).is_ok());
        assert!(l.try_admit(b, t0).is_ok());
        assert_eq!(
            l.try_admit(a, t0),
            Err(RateRefusal::Source),
            "one /64, one bucket"
        );
        assert!(l.try_admit(c, t0).is_ok(), "another /64, another bucket");
    }

    #[test]
    fn a_full_table_never_grows_and_never_resets_a_used_quota() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(config((1000, 1000), (1, 2), 3), t0);
        let used: Vec<IpAddr> = (1..=3).map(|i| v4(10, 0, 0, i)).collect();
        for ip in &used {
            assert!(l.try_admit(*ip, t0).is_ok());
            assert!(l.try_admit(*ip, t0).is_ok());
            assert_eq!(l.try_admit(*ip, t0), Err(RateRefusal::Source));
        }
        assert_eq!(l.stats().sources, 3);
        // A new source while every entry is still in use: charged to the
        // shared overflow bucket; the table does not grow.
        let newcomers: Vec<IpAddr> = (10..=13).map(|i| v4(10, 0, 0, i)).collect();
        let mut overflow_ok = 0;
        for ip in &newcomers {
            if l.try_admit(*ip, t0).is_ok() {
                overflow_ok += 1;
            }
        }
        assert_eq!(overflow_ok, 2, "the overflow bucket's burst, shared");
        assert_eq!(l.stats().sources, 3);
        assert_eq!(l.stats().overflow_charged, 4);
        // The used sources are still exhausted: nothing was reset.
        for ip in &used {
            assert_eq!(l.try_admit(*ip, t0), Err(RateRefusal::Source));
        }
        // Once their buckets have refilled, they may be dropped for newcomers.
        let later = t0 + secs(10);
        assert!(l.try_admit(v4(10, 0, 0, 20), later).is_ok());
        assert!(l.stats().sources <= 3);
    }

    #[test]
    fn clock_edges_neither_panic_nor_mint_tokens() {
        let t0 = Instant::now() + secs(10);
        let mut l = RateLimiter::new(config((1, 1), (1, 1), 4), t0);
        let ip = v4(10, 0, 0, 1);
        assert!(l.try_admit(ip, t0).is_ok());
        // An earlier reading adds nothing and does not move the clock back.
        assert_eq!(
            l.try_admit(ip, t0.checked_sub(secs(5)).unwrap()),
            Err(RateRefusal::Global)
        );
        // The same instant again adds nothing.
        assert_eq!(l.try_admit(ip, t0), Err(RateRefusal::Global));
        // A long gap at the highest rate saturates instead of overflowing,
        // and the bucket is still capped at its burst.
        let mut fast = RateLimiter::new(config((u32::MAX, 2), (u32::MAX, 2), 4), t0);
        let far = t0 + Duration::from_secs(10 * 365 * 24 * 3600);
        assert!(fast.try_admit(ip, far).is_ok());
        assert!(fast.try_admit(ip, far).is_ok());
        assert!(fast.try_admit(ip, far).is_err(), "capped at the burst");
    }

    /// The swarm calls the behaviour through `&mut`, so attempts are already
    /// serialised; behind a lock, many threads still get exactly the burst.
    #[test]
    fn concurrent_attempts_on_one_source_get_exactly_the_burst() {
        let t0 = Instant::now();
        let limiter =
            std::sync::Mutex::new(RateLimiter::new(config((10_000, 10_000), (1, 10), 16), t0));
        let admitted = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..100 {
                        if limiter.lock().unwrap().try_admit(v4(10, 0, 0, 1), t0).is_ok() {
                            admitted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        assert_eq!(admitted.into_inner(), 10);
        assert_eq!(limiter.into_inner().unwrap().stats().refused_source, 790);
    }

    #[test]
    fn legitimate_peers_behind_one_address_get_the_source_burst() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(InboundRateConfig::default(), t0);
        let nat = v4(198, 51, 100, 1);
        let ok = (0..10).filter(|_| l.try_admit(nat, t0).is_ok()).count();
        assert_eq!(ok, 10, "ten peers behind one NAT connect at once");
        assert!(l.try_admit(nat, t0).is_err());
        // A peer reconnecting after an outage gets a token back within a second.
        assert!(l.try_admit(nat, t0 + secs(1)).is_ok());
    }

    #[test]
    fn counters_add_up_to_the_attempts() {
        let t0 = Instant::now();
        let mut l = RateLimiter::new(config((2, 3), (1, 2), 2), t0);
        let attempts = [
            v4(1, 1, 1, 1),
            v4(1, 1, 1, 1),
            v4(1, 1, 1, 1),
            v4(2, 2, 2, 2),
            v4(3, 3, 3, 3),
            v4(4, 4, 4, 4),
        ];
        for ip in attempts {
            let _ = l.try_admit(ip, t0);
        }
        let s = l.stats();
        assert_eq!(s.admitted + s.refused_source + s.refused_global, 6);
        assert_eq!((s.admitted, s.refused_source, s.refused_global), (3, 1, 2));
        assert!(s.sources <= 2);
    }

    #[test]
    fn outbound_connections_are_never_limited() {
        let mut behaviour = InboundRateLimit::new(config((0, 0), (0, 0), 1));
        let addr: Multiaddr = "/ip4/10.0.0.1/tcp/1".parse().unwrap();
        for i in 0..100 {
            assert!(behaviour
                .handle_established_outbound_connection(
                    ConnectionId::new_unchecked(i),
                    PeerId::random(),
                    &addr,
                    Endpoint::Dialer,
                )
                .is_ok());
        }
        assert!(
            behaviour
                .handle_pending_inbound_connection(ConnectionId::new_unchecked(1), &addr, &addr)
                .is_err(),
            "while inbound, with no budget, is refused"
        );
    }
}
