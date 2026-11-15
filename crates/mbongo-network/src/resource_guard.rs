//! The P2P resource guard, Phase 1: it observes inbound streams and decides
//! nothing.
//!
//! libp2p 0.53 gives a behaviour no say between yamux accepting an inbound
//! stream and the swarm negotiating a protocol on it. The one place a node can
//! stand there is the muxer: [`P2pResourceGuard::observe`] wraps the muxer of
//! each authenticated connection, so every inbound stream passes through
//! [`GuardedMuxer::poll_inbound`] before any negotiation byte is read.
//!
//! The module keeps two things apart.
//!
//! - **Observation.** Each inbound stream is counted when yamux hands it over
//!   and again when its handle is dropped: in all, per connection, with a
//!   monotonic timestamp and its lifetime. The counts are atomics; nothing on
//!   the stream path takes a lock or allocates beyond one `Arc` clone.
//! - **Policy.** There is none. [`GuardMode`] has one value,
//!   [`GuardMode::ObserveOnly`], and the guard takes no configuration, reads no
//!   environment variable and has no setter. A stream that yamux yields is
//!   returned to the swarm, always; a failed count is itself counted
//!   ([`GuardSnapshot::observation_errors_total`]) and changes nothing else.
//!   Refusing a stream needs new code here, not a setting.
//!
//! **What it does not change.** TCP, Noise and yamux are configured by the
//! caller exactly as before; reads, writes, flushes and closes are delegated
//! to the wrapped stream untouched; errors are passed on as they are.
//!
//! **What it cannot see.** The protocol a stream ends up speaking, and whether
//! negotiation succeeded: both happen in the swarm, after the muxer has given
//! the stream away. What is counted is a negotiation *started*: a first read
//! or write was polled on the stream, regardless of whether the operation
//! returned `Pending`, succeeded, or failed. This does not prove that bytes
//! were exchanged or that protocol negotiation succeeded.

use std::collections::HashMap;
use std::fmt;
use std::io::{self, IoSlice, IoSliceMut};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::{AsyncRead, AsyncWrite};
use libp2p::core::muxing::{StreamMuxer, StreamMuxerEvent, StreamMuxerExt};
use libp2p::core::ConnectedPoint;
use log::{debug, trace};

/// Counting needs no ordering between counters: each is exact on its own.
const ORD: Ordering = Ordering::Relaxed;

/// What the guard is allowed to do with a stream.
///
/// One value. Adding another is a deliberate source change that every
/// exhaustive `match` on this type, here and in the tests, has to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuardMode {
    /// Count, time and report; pass every stream on unchanged.
    #[default]
    ObserveOnly,
}

impl GuardMode {
    /// The mode's name, as reported.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObserveOnly => "observe_only",
        }
    }
}

/// The mode this build runs in. A constant: nothing at run time selects it.
pub const GUARD_MODE: GuardMode = GuardMode::ObserveOnly;

/// Upper bounds of the stream-lifetime histogram; one more bucket takes the
/// rest.
pub const LIFETIME_BOUNDS: [Duration; 7] = [
    Duration::from_millis(1),
    Duration::from_millis(10),
    Duration::from_millis(100),
    Duration::from_secs(1),
    Duration::from_secs(10),
    Duration::from_secs(60),
    Duration::from_secs(600),
];

/// The bounds above, as Prometheus `le` values.
const LIFETIME_LABELS: [&str; 8] = ["0.001", "0.01", "0.1", "1", "10", "60", "600", "+Inf"];

/// Adds one, stopping at the maximum instead of wrapping.
fn bump(counter: &AtomicU64) -> u64 {
    match counter.fetch_update(ORD, ORD, |v| v.checked_add(1)) {
        Ok(previous) => previous + 1,
        Err(full) => full,
    }
}

/// Takes one from a gauge. `false` when the gauge was already zero, which
/// means an opening was never counted.
fn release(gauge: &AtomicU64) -> bool {
    gauge.fetch_update(ORD, ORD, |v| v.checked_sub(1)).is_ok()
}

/// What the guard has seen on one connection. Shared by the connection's
/// muxer and by each of its inbound streams, so it lives as long as either.
struct ConnectionObservation {
    guard: Arc<P2pResourceGuard>,
    /// The guard's own number for the connection. libp2p's `ConnectionId` is
    /// assigned by the swarm and is not known where the muxer is wrapped.
    id: u64,
    /// Whether the remote dialed this node.
    inbound: bool,
    opened: Instant,
    streams_total: AtomicU64,
    streams_active: AtomicU64,
    streams_peak: AtomicU64,
    /// Set by the first muxer error, so one failing connection counts once.
    failed: AtomicBool,
}

/// The single owner of what the node observes about inbound streams.
///
/// One per node. Cloning the `Arc` gives a read handle; there is nothing to
/// write to.
pub struct P2pResourceGuard {
    started: Instant,
    next_connection: AtomicU64,
    /// The open connections, for the per-connection maximum. Touched when a
    /// connection is wrapped, when its muxer is dropped and when a snapshot is
    /// taken; never per stream.
    connections: Mutex<HashMap<u64, Weak<ConnectionObservation>>>,
    connections_total: AtomicU64,
    inbound_total: AtomicU64,
    inbound_active: AtomicU64,
    inbound_active_peak: AtomicU64,
    per_connection_peak: AtomicU64,
    passed_on: AtomicU64,
    negotiations_started: AtomicU64,
    outbound_total: AtomicU64,
    stream_errors: AtomicU64,
    muxer_errors: AtomicU64,
    observation_errors: AtomicU64,
    lifetime_buckets: [AtomicU64; 8],
    lifetime_sum_micros: AtomicU64,
    closed_total: AtomicU64,
}

impl fmt::Debug for P2pResourceGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("P2pResourceGuard").field("snapshot", &self.snapshot()).finish()
    }
}

impl Default for P2pResourceGuard {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            next_connection: AtomicU64::new(0),
            connections: Mutex::new(HashMap::new()),
            connections_total: AtomicU64::new(0),
            inbound_total: AtomicU64::new(0),
            inbound_active: AtomicU64::new(0),
            inbound_active_peak: AtomicU64::new(0),
            per_connection_peak: AtomicU64::new(0),
            passed_on: AtomicU64::new(0),
            negotiations_started: AtomicU64::new(0),
            outbound_total: AtomicU64::new(0),
            stream_errors: AtomicU64::new(0),
            muxer_errors: AtomicU64::new(0),
            observation_errors: AtomicU64::new(0),
            lifetime_buckets: Default::default(),
            lifetime_sum_micros: AtomicU64::new(0),
            closed_total: AtomicU64::new(0),
        }
    }
}

impl P2pResourceGuard {
    /// A guard that has seen nothing yet.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The mode this guard runs in: always [`GuardMode::ObserveOnly`]. A
    /// method, so that a caller asks the guard it holds; the answer is the
    /// constant [`GUARD_MODE`].
    #[must_use]
    #[allow(clippy::unused_self)]
    pub const fn mode(&self) -> GuardMode {
        GUARD_MODE
    }

    /// The connection table. A poisoned lock is an observation error; the
    /// table is still usable, since every update to it is a single insert or
    /// remove.
    fn table(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Weak<ConnectionObservation>>> {
        self.connections.lock().unwrap_or_else(|poisoned| {
            bump(&self.observation_errors);
            poisoned.into_inner()
        })
    }

    /// Wraps the muxer of a connection that has just been authenticated. The
    /// muxer's behaviour is unchanged; its inbound streams are counted.
    #[must_use]
    pub fn observe<M>(self: &Arc<Self>, muxer: M, endpoint: &ConnectedPoint) -> GuardedMuxer<M> {
        let id = bump(&self.next_connection);
        let connection = Arc::new(ConnectionObservation {
            guard: Arc::clone(self),
            id,
            inbound: endpoint.is_listener(),
            opened: Instant::now(),
            streams_total: AtomicU64::new(0),
            streams_active: AtomicU64::new(0),
            streams_peak: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        bump(&self.connections_total);
        self.table().insert(id, Arc::downgrade(&connection));
        debug!(
            "P2P guard: connection {id} observed ({})",
            if connection.inbound {
                "inbound"
            } else {
                "outbound"
            }
        );
        GuardedMuxer {
            inner: muxer,
            connection,
        }
    }

    /// Microseconds since this guard was created, on the monotonic clock.
    fn monotonic_micros(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_micros()).unwrap_or(u64::MAX)
    }

    /// The counts as they are now. Each is exact; taken together they are
    /// consistent only when no stream is opening or closing meanwhile.
    #[must_use]
    pub fn snapshot(&self) -> GuardSnapshot {
        let (connections_active, streams_per_connection_max) = {
            let table = self.table();
            let live: Vec<_> = table.values().filter_map(Weak::upgrade).collect();
            let max = live.iter().map(|c| c.streams_active.load(ORD)).max().unwrap_or(0);
            (u64::try_from(live.len()).unwrap_or(u64::MAX), max)
        };
        let mut lifetime_buckets = [0u64; 8];
        for (out, bucket) in lifetime_buckets.iter_mut().zip(&self.lifetime_buckets) {
            *out = bucket.load(ORD);
        }
        GuardSnapshot {
            mode: self.mode(),
            inbound_streams_total: self.inbound_total.load(ORD),
            inbound_streams_active: self.inbound_active.load(ORD),
            inbound_streams_active_peak: self.inbound_active_peak.load(ORD),
            inbound_streams_closed_total: self.closed_total.load(ORD),
            streams_per_connection_max,
            streams_per_connection_peak: self.per_connection_peak.load(ORD),
            connections_active,
            connections_total: self.connections_total.load(ORD),
            stream_negotiations_started_total: self.negotiations_started.load(ORD),
            streams_observation_only_total: self.passed_on.load(ORD),
            observation_errors_total: self.observation_errors.load(ORD),
            stream_errors_total: self.stream_errors.load(ORD),
            muxer_errors_total: self.muxer_errors.load(ORD),
            outbound_streams_total: self.outbound_total.load(ORD),
            lifetime_buckets,
            lifetime_sum_micros: self.lifetime_sum_micros.load(ORD),
        }
    }
}

/// The guard's counts at one moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GuardSnapshot {
    /// `p2p_guard_mode`.
    pub mode: GuardMode,
    /// `p2p_inbound_streams_total`: inbound streams yamux handed over.
    pub inbound_streams_total: u64,
    /// `p2p_inbound_streams_active`: of those, the ones whose handle is still
    /// held.
    pub inbound_streams_active: u64,
    /// The highest `inbound_streams_active` since start.
    pub inbound_streams_active_peak: u64,
    /// Inbound streams whose handle was dropped; the count of
    /// `p2p_stream_lifetime_seconds`.
    pub inbound_streams_closed_total: u64,
    /// `p2p_streams_per_connection`, now: the most active inbound streams on
    /// any open connection.
    pub streams_per_connection_max: u64,
    /// `p2p_streams_per_connection`, since start: the most any connection held
    /// at once.
    pub streams_per_connection_peak: u64,
    /// Connections whose muxer is alive.
    pub connections_active: u64,
    /// Connections wrapped since start.
    pub connections_total: u64,
    /// `p2p_stream_negotiations_started_total`: number of inbound streams on
    /// which a first read or write was polled, regardless of whether the
    /// operation returned `Pending`, succeeded, or failed. This does not
    /// prove that bytes were exchanged or protocol negotiation succeeded.
    /// The swarm polls a stream as soon as it takes it up, so in practice
    /// this follows `inbound_streams_total` closely.
    pub stream_negotiations_started_total: u64,
    /// `p2p_streams_observation_only_total`: inbound streams returned to the
    /// swarm untouched. Equal to `inbound_streams_total` in this mode.
    pub streams_observation_only_total: u64,
    /// `p2p_stream_observation_errors_total`: counts that could not be kept
    /// (a gauge released below zero, a poisoned table). The stream concerned
    /// was passed on regardless.
    pub observation_errors_total: u64,
    /// Inbound streams on which a read, write, flush or close failed.
    pub stream_errors_total: u64,
    /// Connections whose muxer reported an error.
    pub muxer_errors_total: u64,
    /// Outbound streams this node opened.
    pub outbound_streams_total: u64,
    /// `p2p_stream_lifetime_seconds`: streams per bucket of
    /// [`LIFETIME_BOUNDS`], then the rest. Not cumulative.
    pub lifetime_buckets: [u64; 8],
    /// The sum of all recorded lifetimes, in microseconds.
    pub lifetime_sum_micros: u64,
}

impl GuardSnapshot {
    /// The lifetime histogram as cumulative counts, the way Prometheus reads
    /// `le` buckets.
    #[must_use]
    pub fn lifetime_cumulative(&self) -> [u64; 8] {
        let mut cumulative = [0u64; 8];
        let mut running = 0u64;
        for (out, bucket) in cumulative.iter_mut().zip(&self.lifetime_buckets) {
            running = running.saturating_add(*bucket);
            *out = running;
        }
        cumulative
    }

    /// The sum of lifetimes in seconds, as text with microsecond precision.
    fn lifetime_sum_seconds(&self) -> String {
        format!(
            "{}.{:06}",
            self.lifetime_sum_micros / 1_000_000,
            self.lifetime_sum_micros % 1_000_000
        )
    }

    /// The counts in the Prometheus text exposition format. No label carries a
    /// peer or connection identifier. Nothing in this crate serves it: it is
    /// here so that an existing scraper or log pipeline can be pointed at it
    /// without another dependency.
    #[must_use]
    pub fn prometheus_text(&self) -> String {
        use fmt::Write as _;
        let mut out = String::new();
        let mut metric = |name: &str, kind: &str, help: &str, lines: &[(String, u64)]| {
            let _ = writeln!(out, "# HELP {name} {help}");
            let _ = writeln!(out, "# TYPE {name} {kind}");
            for (labels, value) in lines {
                let _ = writeln!(out, "{name}{labels} {value}");
            }
        };
        let plain = |value: u64| vec![(String::new(), value)];
        metric(
            "p2p_inbound_streams_total",
            "counter",
            "Inbound streams observed before protocol negotiation.",
            &plain(self.inbound_streams_total),
        );
        metric(
            "p2p_inbound_streams_active",
            "gauge",
            "Inbound streams whose handle is still held.",
            &plain(self.inbound_streams_active),
        );
        metric(
            "p2p_streams_per_connection",
            "gauge",
            "Active inbound streams on the busiest connection: now, and the highest since start.",
            &[
                (
                    "{stat=\"max\"}".to_string(),
                    self.streams_per_connection_max,
                ),
                (
                    "{stat=\"peak\"}".to_string(),
                    self.streams_per_connection_peak,
                ),
            ],
        );
        metric(
            "p2p_stream_observation_errors_total",
            "counter",
            "Counts the guard could not keep; the stream was passed on regardless.",
            &plain(self.observation_errors_total),
        );
        metric(
            "p2p_stream_negotiations_started_total",
            "counter",
            "Inbound streams on which a first read or write was polled, whether it returned \
             Pending, succeeded or failed. Not proof of bytes exchanged or of a successful \
             negotiation.",
            &plain(self.stream_negotiations_started_total),
        );
        metric(
            "p2p_streams_observation_only_total",
            "counter",
            "Inbound streams returned to the swarm untouched.",
            &plain(self.streams_observation_only_total),
        );
        metric(
            "p2p_guard_mode",
            "gauge",
            "The guard's mode; 1 for the active mode.",
            &[(format!("{{mode=\"{}\"}}", self.mode.as_str()), 1)],
        );
        let _ = writeln!(
            out,
            "# HELP p2p_stream_lifetime_seconds Time from an inbound stream's observation to the drop of its handle."
        );
        let _ = writeln!(out, "# TYPE p2p_stream_lifetime_seconds histogram");
        for (label, count) in LIFETIME_LABELS.iter().zip(self.lifetime_cumulative()) {
            let _ = writeln!(
                out,
                "p2p_stream_lifetime_seconds_bucket{{le=\"{label}\"}} {count}"
            );
        }
        let _ = writeln!(
            out,
            "p2p_stream_lifetime_seconds_sum {}",
            self.lifetime_sum_seconds()
        );
        let _ = writeln!(
            out,
            "p2p_stream_lifetime_seconds_count {}",
            self.inbound_streams_closed_total
        );
        out
    }
}

/// One line of `name=value` pairs, for the node's once-a-minute summary.
impl fmt::Display for GuardSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "p2p_guard_mode={} p2p_inbound_streams_total={} p2p_inbound_streams_active={} \
             p2p_inbound_streams_active_peak={} p2p_streams_per_connection_max={} \
             p2p_streams_per_connection_peak={} p2p_stream_negotiations_started_total={} \
             p2p_streams_observation_only_total={} p2p_stream_observation_errors_total={} \
             p2p_stream_errors_total={} p2p_muxer_errors_total={} p2p_guard_connections={} \
             p2p_guard_connections_total={} p2p_outbound_streams_total={} \
             p2p_stream_lifetime_seconds_count={} p2p_stream_lifetime_seconds_sum={} \
             p2p_stream_lifetime_seconds_le=",
            self.mode.as_str(),
            self.inbound_streams_total,
            self.inbound_streams_active,
            self.inbound_streams_active_peak,
            self.streams_per_connection_max,
            self.streams_per_connection_peak,
            self.stream_negotiations_started_total,
            self.streams_observation_only_total,
            self.observation_errors_total,
            self.stream_errors_total,
            self.muxer_errors_total,
            self.connections_active,
            self.connections_total,
            self.outbound_streams_total,
            self.inbound_streams_closed_total,
            self.lifetime_sum_seconds(),
        )?;
        for (i, (label, count)) in
            LIFETIME_LABELS.iter().zip(self.lifetime_cumulative()).enumerate()
        {
            write!(f, "{}{label}:{count}", if i == 0 { "" } else { "," })?;
        }
        Ok(())
    }
}

/// A connection's muxer, with its inbound streams counted on the way out.
///
/// Every method delegates to the wrapped muxer and returns what it returned.
pub struct GuardedMuxer<M> {
    inner: M,
    connection: Arc<ConnectionObservation>,
}

impl<M> GuardedMuxer<M> {
    /// Counts the first error this connection's muxer reports.
    fn muxer_failed(&self) {
        if !self.connection.failed.swap(true, ORD) {
            bump(&self.connection.guard.muxer_errors);
        }
    }
}

impl<M> StreamMuxer for GuardedMuxer<M>
where
    M: StreamMuxer + Unpin,
    M::Substream: Unpin,
{
    type Substream = GuardedStream<M::Substream>;
    type Error = M::Error;

    fn poll_inbound(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Substream, Self::Error>> {
        let this = self.get_mut();
        let stream = match this.inner.poll_inbound_unpin(cx) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(Err(error)) => {
                this.muxer_failed();
                return Poll::Ready(Err(error));
            }
            Poll::Ready(Ok(stream)) => stream,
        };
        let observation = StreamObservation::open(&this.connection);
        // The whole policy. Whatever was or was not counted above, the stream
        // goes to the swarm.
        match this.connection.guard.mode() {
            GuardMode::ObserveOnly => {
                bump(&this.connection.guard.passed_on);
                Poll::Ready(Ok(GuardedStream {
                    inner: stream,
                    observation: Some(observation),
                }))
            }
        }
    }

    fn poll_outbound(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Substream, Self::Error>> {
        let this = self.get_mut();
        match this.inner.poll_outbound_unpin(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => {
                this.muxer_failed();
                Poll::Ready(Err(error))
            }
            Poll::Ready(Ok(stream)) => {
                bump(&this.connection.guard.outbound_total);
                Poll::Ready(Ok(GuardedStream {
                    inner: stream,
                    observation: None,
                }))
            }
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        let result = this.inner.poll_close_unpin(cx);
        if matches!(result, Poll::Ready(Err(_))) {
            this.muxer_failed();
        }
        result
    }

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<StreamMuxerEvent, Self::Error>> {
        let this = self.get_mut();
        let result = this.inner.poll_unpin(cx);
        if matches!(result, Poll::Ready(Err(_))) {
            this.muxer_failed();
        }
        result
    }
}

impl<M> Drop for GuardedMuxer<M> {
    fn drop(&mut self) {
        let connection = &self.connection;
        connection.guard.table().remove(&connection.id);
        debug!(
            "P2P guard: connection {} closed after {} ms: inbound_streams={} peak_active={} \
             still_active={} muxer_error={}",
            connection.id,
            connection.opened.elapsed().as_millis(),
            connection.streams_total.load(ORD),
            connection.streams_peak.load(ORD),
            connection.streams_active.load(ORD),
            connection.failed.load(ORD),
        );
    }
}

/// What the guard holds for one inbound stream, from the moment yamux yields
/// it until its handle is dropped.
struct StreamObservation {
    connection: Arc<ConnectionObservation>,
    /// The stream's number on its connection, from 1.
    number: u64,
    opened: Instant,
    /// Set when a first read or write is polled, whatever it returns.
    io_started: bool,
    /// Set by the first failed read, write, flush or close.
    errored: bool,
}

impl StreamObservation {
    fn open(connection: &Arc<ConnectionObservation>) -> Self {
        let guard = &connection.guard;
        let opened = Instant::now();
        bump(&guard.inbound_total);
        let number = bump(&connection.streams_total);
        let on_connection = bump(&connection.streams_active);
        let in_all = bump(&guard.inbound_active);
        connection.streams_peak.fetch_max(on_connection, ORD);
        guard.per_connection_peak.fetch_max(on_connection, ORD);
        guard.inbound_active_peak.fetch_max(in_all, ORD);
        trace!(
            "stream_opened conn={} stream={number} t_us={} conn_active={on_connection} \
             active={in_all}",
            connection.id,
            guard.monotonic_micros(),
        );
        Self {
            connection: Arc::clone(connection),
            number,
            opened,
            io_started: false,
            errored: false,
        }
    }

    /// Called when the swarm polls a first read or write on the stream,
    /// before the wrapped stream is polled: a negotiation has been started.
    /// The poll may then return `Pending`, succeed or fail; none of that is
    /// known here, and none of it undoes the count.
    fn io(&mut self) {
        if !self.io_started {
            self.io_started = true;
            bump(&self.connection.guard.negotiations_started);
        }
    }

    fn error(&mut self) {
        if !self.errored {
            self.errored = true;
            bump(&self.connection.guard.stream_errors);
        }
    }
}

impl Drop for StreamObservation {
    fn drop(&mut self) {
        let connection = &self.connection;
        let guard = &connection.guard;
        let lifetime = self.opened.elapsed();
        if !release(&connection.streams_active) {
            bump(&guard.observation_errors);
        }
        if !release(&guard.inbound_active) {
            bump(&guard.observation_errors);
        }
        let bucket = LIFETIME_BOUNDS
            .iter()
            .position(|bound| lifetime <= *bound)
            .unwrap_or(LIFETIME_BOUNDS.len());
        bump(&guard.lifetime_buckets[bucket]);
        let micros = u64::try_from(lifetime.as_micros()).unwrap_or(u64::MAX);
        let _ = guard
            .lifetime_sum_micros
            .fetch_update(ORD, ORD, |sum| Some(sum.saturating_add(micros)));
        bump(&guard.closed_total);
        trace!(
            "stream_closed conn={} stream={} t_us={} lifetime_us={micros} negotiation_started={} \
             error={} conn_active={} active={}",
            connection.id,
            self.number,
            guard.monotonic_micros(),
            self.io_started,
            self.errored,
            connection.streams_active.load(ORD),
            guard.inbound_active.load(ORD),
        );
    }
}

/// A stream from a [`GuardedMuxer`]. Reads and writes go to the wrapped
/// stream unchanged; an inbound stream also carries its observation, which
/// ends when this handle is dropped.
pub struct GuardedStream<S> {
    inner: S,
    /// `Some` for inbound streams.
    observation: Option<StreamObservation>,
}

impl<S> GuardedStream<S> {
    fn io(&mut self) {
        if let Some(observation) = &mut self.observation {
            observation.io();
        }
    }

    fn outcome<T>(&mut self, result: Poll<io::Result<T>>) -> Poll<io::Result<T>> {
        if let (Poll::Ready(Err(_)), Some(observation)) = (&result, &mut self.observation) {
            observation.error();
        }
        result
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for GuardedStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.io();
        let result = Pin::new(&mut this.inner).poll_read(cx, buf);
        this.outcome(result)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.io();
        let result = Pin::new(&mut this.inner).poll_read_vectored(cx, bufs);
        this.outcome(result)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for GuardedStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.io();
        let result = Pin::new(&mut this.inner).poll_write(cx, buf);
        this.outcome(result)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.io();
        let result = Pin::new(&mut this.inner).poll_write_vectored(cx, bufs);
        this.outcome(result)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.inner).poll_flush(cx);
        this.outcome(result)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.inner).poll_close(cx);
        this.outcome(result)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::task::Waker;

    use futures::task::noop_waker;
    use libp2p::Multiaddr;

    use super::*;

    /// What the scripted muxer answers to one `poll_inbound`.
    enum Step {
        Stream(u32),
        Pending,
        Error,
    }

    /// A stream that remembers what was written and serves scripted bytes.
    #[derive(Default)]
    struct MockStream {
        id: u32,
        to_read: VecDeque<u8>,
        written: Vec<u8>,
        fail: bool,
    }

    impl AsyncRead for MockStream {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let this = self.get_mut();
            if this.fail {
                return Poll::Ready(Err(io::Error::from(io::ErrorKind::ConnectionReset)));
            }
            let n = buf.len().min(this.to_read.len());
            for slot in &mut buf[..n] {
                *slot = this.to_read.pop_front().unwrap();
            }
            Poll::Ready(Ok(n))
        }
    }

    impl AsyncWrite for MockStream {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let this = self.get_mut();
            if this.fail {
                return Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe)));
            }
            this.written.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[derive(Default)]
    struct MockMuxer {
        script: VecDeque<Step>,
    }

    impl MockMuxer {
        fn yielding(streams: u32) -> Self {
            Self {
                script: (0..streams).map(Step::Stream).collect(),
            }
        }
    }

    impl StreamMuxer for MockMuxer {
        type Substream = MockStream;
        type Error = io::Error;

        fn poll_inbound(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Result<MockStream, io::Error>> {
            match self.get_mut().script.pop_front() {
                None | Some(Step::Pending) => Poll::Pending,
                Some(Step::Error) => Poll::Ready(Err(io::Error::other("muxer failed"))),
                Some(Step::Stream(id)) => Poll::Ready(Ok(MockStream {
                    id,
                    ..MockStream::default()
                })),
            }
        }
        fn poll_outbound(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Result<MockStream, io::Error>> {
            Poll::Ready(Ok(MockStream::default()))
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
            Poll::Ready(Ok(()))
        }
        fn poll(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Result<StreamMuxerEvent, io::Error>> {
            Poll::Pending
        }
    }

    fn listener() -> ConnectedPoint {
        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/1".parse().unwrap();
        ConnectedPoint::Listener {
            local_addr: addr.clone(),
            send_back_addr: addr,
        }
    }

    fn waker() -> Waker {
        noop_waker()
    }

    fn take(
        muxer: &mut GuardedMuxer<MockMuxer>,
    ) -> Poll<Result<GuardedStream<MockStream>, io::Error>> {
        let waker = waker();
        Pin::new(muxer).poll_inbound(&mut Context::from_waker(&waker))
    }

    fn stream(muxer: &mut GuardedMuxer<MockMuxer>) -> GuardedStream<MockStream> {
        match take(muxer) {
            Poll::Ready(Ok(stream)) => stream,
            Poll::Ready(Err(e)) => panic!("muxer error: {e}"),
            Poll::Pending => panic!("no stream"),
        }
    }

    /// Every count that must be zero when nothing is open and nothing failed.
    fn assert_quiescent(snapshot: &GuardSnapshot) {
        assert_eq!(snapshot.inbound_streams_active, 0, "{snapshot:?}");
        assert_eq!(snapshot.streams_per_connection_max, 0, "{snapshot:?}");
        assert_eq!(snapshot.observation_errors_total, 0, "{snapshot:?}");
        assert_eq!(
            snapshot.inbound_streams_closed_total, snapshot.inbound_streams_total,
            "{snapshot:?}"
        );
        assert_eq!(
            snapshot.lifetime_buckets.iter().sum::<u64>(),
            snapshot.inbound_streams_total,
            "{snapshot:?}"
        );
    }

    #[test]
    fn every_inbound_stream_is_passed_on_in_order() {
        const N: u32 = 1000;
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(N), &listener());
        let streams: Vec<_> = (0..N).map(|_| stream(&mut muxer)).collect();
        assert!(matches!(take(&mut muxer), Poll::Pending));

        let ids: Vec<u32> = streams.iter().map(|s| s.inner.id).collect();
        assert_eq!(
            ids,
            (0..N).collect::<Vec<_>>(),
            "a stream was lost or reordered"
        );
        let seen = guard.snapshot();
        assert_eq!(seen.inbound_streams_total, u64::from(N));
        assert_eq!(seen.streams_observation_only_total, u64::from(N));
        assert_eq!(seen.inbound_streams_active, u64::from(N));
        assert_eq!(seen.streams_per_connection_max, u64::from(N));
        assert_eq!(seen.inbound_streams_active_peak, u64::from(N));

        drop(streams);
        let seen = guard.snapshot();
        assert_quiescent(&seen);
        assert_eq!(seen.streams_per_connection_peak, u64::from(N));
        assert_eq!(
            seen.stream_negotiations_started_total, 0,
            "no stream was read or written"
        );
    }

    #[test]
    fn pending_and_errors_pass_through_unchanged() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(
            MockMuxer {
                script: VecDeque::from([
                    Step::Pending,
                    Step::Stream(7),
                    Step::Error,
                    Step::Error,
                    Step::Stream(8),
                ]),
            },
            &listener(),
        );
        assert!(matches!(take(&mut muxer), Poll::Pending));
        assert_eq!(stream(&mut muxer).inner.id, 7);
        assert!(matches!(take(&mut muxer), Poll::Ready(Err(_))));
        assert!(matches!(take(&mut muxer), Poll::Ready(Err(_))));
        // The wrapper decides nothing after an error either.
        assert_eq!(stream(&mut muxer).inner.id, 8);

        let seen = guard.snapshot();
        assert_eq!(seen.inbound_streams_total, 2);
        assert_eq!(seen.streams_observation_only_total, 2);
        assert_eq!(
            seen.muxer_errors_total, 1,
            "one failing connection counts once"
        );
        assert_eq!(seen.observation_errors_total, 0);
    }

    #[test]
    fn bytes_are_untouched_and_the_first_io_marks_negotiation() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(1), &listener());
        let mut stream = stream(&mut muxer);
        stream.inner.to_read = VecDeque::from(b"\x13/multistream/1.0.0\n".to_vec());
        let waker = waker();
        let mut cx = Context::from_waker(&waker);

        assert_eq!(guard.snapshot().stream_negotiations_started_total, 0);
        let mut buf = [0u8; 64];
        let Poll::Ready(Ok(n)) = Pin::new(&mut stream).poll_read(&mut cx, &mut buf) else {
            panic!("read did not complete");
        };
        assert_eq!(&buf[..n], b"\x13/multistream/1.0.0\n");
        assert_eq!(guard.snapshot().stream_negotiations_started_total, 1);

        let Poll::Ready(Ok(4)) = Pin::new(&mut stream).poll_write(&mut cx, b"na\n\0") else {
            panic!("write did not complete");
        };
        let slices = [IoSlice::new(b"ab"), IoSlice::new(b"cd")];
        assert!(matches!(
            Pin::new(&mut stream).poll_write_vectored(&mut cx, &slices),
            Poll::Ready(Ok(n)) if n > 0
        ));
        assert!(matches!(
            Pin::new(&mut stream).poll_flush(&mut cx),
            Poll::Ready(Ok(()))
        ));
        assert!(matches!(
            Pin::new(&mut stream).poll_close(&mut cx),
            Poll::Ready(Ok(()))
        ));
        assert!(stream.inner.written.starts_with(b"na\n\0ab"));
        assert_eq!(
            guard.snapshot().stream_negotiations_started_total,
            1,
            "counted once per stream"
        );
        assert_eq!(guard.snapshot().stream_errors_total, 0);

        drop(stream);
        assert_quiescent(&guard.snapshot());
    }

    /// The counter is of negotiations started, not of negotiations that
    /// succeeded: it moves when a first read or write is polled, and what
    /// that poll returns, or what happens to the stream afterwards, does not
    /// move it back.
    #[test]
    fn a_negotiation_is_counted_when_it_starts_whatever_becomes_of_it() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(3), &listener());
        let waker = waker();
        let mut cx = Context::from_waker(&waker);
        let started = |guard: &P2pResourceGuard| guard.snapshot().stream_negotiations_started_total;

        // Observed, never read or written: no negotiation started.
        let untouched = stream(&mut muxer);
        assert_eq!(guard.snapshot().inbound_streams_total, 1);
        assert_eq!(started(&guard), 0);
        drop(untouched);
        assert_eq!(started(&guard), 0);

        // The first read starts one; the peer's proposal is then refused, as
        // multistream-select does for a protocol it does not have.
        let mut refused = stream(&mut muxer);
        refused.inner.to_read =
            VecDeque::from(b"\x13/multistream/1.0.0\n\x0b/unknown/1\n".to_vec());
        let mut buf = [0u8; 64];
        assert!(matches!(
            Pin::new(&mut refused).poll_read(&mut cx, &mut buf),
            Poll::Ready(Ok(n)) if n > 0
        ));
        assert_eq!(started(&guard), 1, "counted at the first read");
        assert!(matches!(
            Pin::new(&mut refused).poll_write(&mut cx, b"\x03na\n"),
            Poll::Ready(Ok(4))
        ));
        drop(refused);
        assert_eq!(started(&guard), 1, "a refused negotiation stays counted");

        // The first read fails outright: still a negotiation started.
        let mut failing = stream(&mut muxer);
        failing.inner.fail = true;
        assert!(matches!(
            Pin::new(&mut failing).poll_read(&mut cx, &mut buf),
            Poll::Ready(Err(_))
        ));
        drop(failing);
        let seen = guard.snapshot();
        assert_eq!(seen.stream_negotiations_started_total, 2);
        assert_eq!(seen.inbound_streams_total, 3, "three observed, two started");
        assert_eq!(seen.stream_errors_total, 1);
        assert_quiescent(&seen);
    }

    #[test]
    fn a_stream_error_is_counted_and_passed_on() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(1), &listener());
        let mut stream = stream(&mut muxer);
        stream.inner.fail = true;
        let waker = waker();
        let mut cx = Context::from_waker(&waker);
        let mut buf = [0u8; 8];
        let read = Pin::new(&mut stream).poll_read(&mut cx, &mut buf);
        assert!(
            matches!(&read, Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::ConnectionReset),
            "{read:?}"
        );
        let write = Pin::new(&mut stream).poll_write(&mut cx, b"x");
        assert!(
            matches!(&write, Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::BrokenPipe),
            "{write:?}"
        );
        drop(stream);
        let seen = guard.snapshot();
        assert_eq!(
            seen.stream_errors_total, 1,
            "one failing stream counts once"
        );
        assert_quiescent(&seen);
    }

    #[test]
    fn outbound_streams_are_counted_apart() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::default(), &listener());
        let waker = waker();
        let outbound = Pin::new(&mut muxer).poll_outbound(&mut Context::from_waker(&waker));
        assert!(matches!(outbound, Poll::Ready(Ok(_))));
        let seen = guard.snapshot();
        assert_eq!(seen.outbound_streams_total, 1);
        assert_eq!(seen.inbound_streams_total, 0);
        assert_eq!(seen.inbound_streams_active, 0);
    }

    #[test]
    fn the_connection_table_follows_the_muxers() {
        let guard = P2pResourceGuard::new();
        let mut first = guard.observe(MockMuxer::yielding(3), &listener());
        let second = guard.observe(MockMuxer::default(), &listener());
        let held: Vec<_> = (0..3).map(|_| stream(&mut first)).collect();
        let seen = guard.snapshot();
        assert_eq!((seen.connections_active, seen.connections_total), (2, 2));
        assert_eq!(seen.streams_per_connection_max, 3);

        // A stream may outlive its muxer; it is still counted, and released.
        drop(first);
        drop(second);
        let seen = guard.snapshot();
        assert_eq!((seen.connections_active, seen.connections_total), (0, 2));
        assert_eq!(seen.inbound_streams_active, 3);
        drop(held);
        assert_quiescent(&guard.snapshot());
    }

    #[test]
    fn lifetimes_fall_in_their_buckets() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(2), &listener());
        let quick = stream(&mut muxer);
        let slow = stream(&mut muxer);
        drop(quick);
        std::thread::sleep(Duration::from_millis(30));
        drop(slow);
        let seen = guard.snapshot();
        assert_eq!(seen.inbound_streams_closed_total, 2);
        // One at or under 10 ms, one between 10 ms and 1 s.
        assert_eq!(
            seen.lifetime_buckets[0] + seen.lifetime_buckets[1],
            1,
            "{seen:?}"
        );
        assert_eq!(
            seen.lifetime_buckets[2] + seen.lifetime_buckets[3],
            1,
            "{seen:?}"
        );
        assert!(seen.lifetime_sum_micros >= 30_000, "{seen:?}");
        assert_eq!(seen.lifetime_cumulative()[7], 2);
    }

    /// T8. Counters forced into impossible states never stop a stream: the
    /// failure is counted and the stream is passed on.
    #[test]
    fn corrupted_counters_never_stop_a_stream() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(6), &listener());

        // Every total and gauge at its maximum.
        for counter in [
            &guard.inbound_total,
            &guard.inbound_active,
            &guard.inbound_active_peak,
            &guard.per_connection_peak,
            &guard.passed_on,
            &guard.closed_total,
            &guard.lifetime_sum_micros,
            &muxer.connection.streams_total,
            &muxer.connection.streams_active,
        ] {
            counter.store(u64::MAX, ORD);
        }
        let saturated: Vec<_> = (0..3).map(|_| stream(&mut muxer).inner.id).collect();
        assert_eq!(saturated, [0, 1, 2]);
        assert_eq!(
            guard.inbound_total.load(ORD),
            u64::MAX,
            "saturates, never wraps to zero"
        );

        // Gauges at zero while streams are open: each release fails.
        let held: Vec<_> = (0..3).map(|_| stream(&mut muxer)).collect();
        guard.inbound_active.store(0, ORD);
        muxer.connection.streams_active.store(0, ORD);
        let errors_before = guard.snapshot().observation_errors_total;
        drop(held);
        let seen = guard.snapshot();
        assert_eq!(seen.observation_errors_total, errors_before + 6, "{seen:?}");
        assert_eq!(
            seen.inbound_streams_active, 0,
            "a gauge never goes below zero"
        );

        // A poisoned connection table is counted and still usable.
        let poisoner = Arc::clone(&guard);
        let _ = std::thread::spawn(move || {
            let _held = poisoner.connections.lock().unwrap();
            panic!("poison the table");
        })
        .join();
        assert!(guard.connections.is_poisoned());
        let mut later = guard.observe(MockMuxer::yielding(1), &listener());
        assert_eq!(stream(&mut later).inner.id, 0);
        assert!(guard.snapshot().observation_errors_total > seen.observation_errors_total);
        assert_eq!(guard.mode(), GuardMode::ObserveOnly);
    }

    /// T8. The mode is a constant with one value. If a variant is added, this
    /// `match` stops compiling: the change cannot happen unnoticed.
    #[test]
    fn the_only_mode_is_observe_only_whatever_the_environment_says() {
        for (name, value) in [
            ("MBONGO_P2P_GUARD_MODE", "enforce"),
            ("MBONGO_GUARD_MODE", "enforce"),
            ("P2P_GUARD_MODE", "refuse"),
            ("P2P_RESOURCE_GUARD", "on"),
            ("MBONGO_P2P_GUARD_ENFORCE", "1"),
            ("MBONGO_P2P_STREAM_LIMIT", "1"),
            ("OBSERVE_ONLY", "0"),
        ] {
            // Nothing in this crate reads these names, which is the point.
            std::env::set_var(name, value);
        }
        let guard = P2pResourceGuard::new();
        match guard.mode() {
            GuardMode::ObserveOnly => {}
        }
        match GUARD_MODE {
            GuardMode::ObserveOnly => {}
        }
        assert_eq!(GuardMode::default(), GuardMode::ObserveOnly);
        assert_eq!(guard.mode().as_str(), "observe_only");

        let mut muxer = guard.observe(MockMuxer::yielding(100), &listener());
        let passed = (0..100).filter(|_| matches!(take(&mut muxer), Poll::Ready(Ok(_)))).count();
        assert_eq!(passed, 100);
        let seen = guard.snapshot();
        assert_eq!(
            seen.streams_observation_only_total,
            seen.inbound_streams_total
        );
        assert_eq!(seen.inbound_streams_total, 100);
    }

    /// T8. This module has no way to be told anything: no environment read,
    /// no configuration type, no setter on the mode.
    #[test]
    fn the_module_reads_no_environment_and_has_no_mode_setter() {
        let source = include_str!("resource_guard.rs");
        let production = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "std::env",
            "env::var",
            "env!(",
            "option_env!",
            "set_mode",
            "fn enforce",
        ] {
            assert!(
                !production.contains(forbidden),
                "production code mentions {forbidden}"
            );
        }
        // The only construction of a refusal would be dropping the stream
        // inside poll_inbound; the stream is moved into the returned value.
        assert_eq!(production.matches("GuardMode::ObserveOnly =>").count(), 1);
        assert_eq!(production.matches("drop(stream)").count(), 0);
    }

    /// T4. Sixteen threads open and close streams through one guard; every
    /// count is exact and every gauge returns to zero.
    #[test]
    fn counts_are_exact_under_thread_contention() {
        const THREADS: u32 = 16;
        const PER_THREAD: u32 = 20_000;
        let guard = P2pResourceGuard::new();
        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let guard = Arc::clone(&guard);
                std::thread::spawn(move || {
                    let mut muxer = guard.observe(MockMuxer::yielding(PER_THREAD), &listener());
                    let mut held = Vec::new();
                    for i in 0..PER_THREAD {
                        held.push(stream(&mut muxer));
                        // Keep a few open, so openings and closings interleave.
                        if i % 4 == 3 {
                            held.clear();
                        }
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let seen = guard.snapshot();
        let total = u64::from(THREADS) * u64::from(PER_THREAD);
        assert_eq!(seen.inbound_streams_total, total);
        assert_eq!(seen.streams_observation_only_total, total);
        assert_eq!(seen.connections_total, u64::from(THREADS));
        assert_eq!(seen.connections_active, 0);
        assert!(seen.streams_per_connection_peak <= 4, "{seen:?}");
        assert!(
            seen.inbound_streams_active_peak <= u64::from(THREADS) * 4,
            "{seen:?}"
        );
        assert_quiescent(&seen);
    }

    #[test]
    fn the_report_names_every_required_metric() {
        let guard = P2pResourceGuard::new();
        let mut muxer = guard.observe(MockMuxer::yielding(2), &listener());
        drop(stream(&mut muxer));
        let _held = stream(&mut muxer);
        let seen = guard.snapshot();
        let required = [
            "p2p_inbound_streams_total",
            "p2p_inbound_streams_active",
            "p2p_streams_per_connection",
            "p2p_stream_lifetime_seconds",
            "p2p_stream_observation_errors_total",
            "p2p_stream_negotiations_started_total",
            "p2p_streams_observation_only_total",
            "p2p_guard_mode",
        ];
        let line = seen.to_string();
        let text = seen.prometheus_text();
        assert_eq!(required.len(), 8);
        for name in required {
            assert!(line.contains(name), "summary line lacks {name}: {line}");
            assert!(
                text.contains(&format!("# TYPE {name} ")),
                "exposition lacks {name}"
            );
        }
        assert!(line.starts_with("p2p_guard_mode=observe_only "), "{line}");
        assert!(text.contains("p2p_inbound_streams_total 2\n"), "{text}");
        assert!(text.contains("p2p_inbound_streams_active 1\n"), "{text}");
        assert!(
            text.contains("p2p_guard_mode{mode=\"observe_only\"} 1\n"),
            "{text}"
        );
        assert!(
            text.contains("p2p_stream_lifetime_seconds_bucket{le=\"+Inf\"} 1\n"),
            "{text}"
        );
        assert!(
            text.contains("p2p_stream_lifetime_seconds_count 1\n"),
            "{text}"
        );
        // No identifier of a peer or a connection in any label.
        assert!(!text.contains("peer") && !text.contains("conn="), "{text}");
    }
}
