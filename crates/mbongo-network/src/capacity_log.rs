//! Turns libp2p's per-stream capacity warning into a counter (S3 A2.5).
//!
//! When a connection already holds as many request/response streams as its
//! cap allows, `libp2p-request-response` 0.26.3 drops the next inbound stream
//! and logs one `WARN`, "Dropping inbound stream because we are at capacity",
//! from target `libp2p_request_response::handler` (`handler.rs`, line 170).
//! It reports the drop in no other way: no event and no counter. Under
//! saturation that is one log line per refused stream, hundreds of thousands
//! a minute (measured in A2.0).
//!
//! [`CapacityLog`] wraps the process logger. It counts that one record, by
//! exact level, target and text, and passes every other record, at every
//! level, unchanged to the wrapped logger. The count is shown in the P2P
//! summary line, at most once a minute.
//!
//! **What the count is.** It is the number of times libp2p reached that
//! warning, which in 0.26.3 is once per dropped inbound stream. libp2p gives
//! no API for the drop itself, so this is a count of the warning's
//! occurrences, not a transport statistic. If an upgrade changes the text or
//! the target, the warning simply passes through again and the count stops
//! rising; nothing is hidden.
//!
//! It counts for the whole process. Only `WARN` from that target and with that
//! text is withheld; every other record from the same target passes through.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use log::{Level, LevelFilter, Log, Metadata, Record};

/// The target libp2p logs the capacity warning from.
pub const CAPACITY_TARGET: &str = "libp2p_request_response::handler";

/// The capacity warning's text in `libp2p-request-response` 0.26.3.
pub const CAPACITY_MESSAGE: &str = "Dropping inbound stream because we are at capacity";

/// The target yamux 0.13 logs its stream-limit error from.
pub const YAMUX_TARGET: &str = "yamux::connection";

/// How yamux 0.13 ends that error, after a connection id: "{id}: maximum
/// number of streams reached". Logged when a peer opens a stream past
/// `max_num_streams` (yamux then terminates the connection), and when this
/// side tries to.
pub const YAMUX_LIMIT_SUFFIX: &str = ": maximum number of streams reached";

/// yamux stream-limit errors seen by the installed [`CapacityLog`] (S3 A2.3).
static YAMUX_LIMIT: AtomicU64 = AtomicU64::new(0);

/// yamux stream-limit errors counted in this process: `None` when no
/// [`CapacityLog`] is installed. Unlike the capacity warning, those errors are
/// still written; this only counts them.
pub fn yamux_stream_limit_reached() -> Option<u64> {
    INSTALLED.get().map(|_| YAMUX_LIMIT.load(Ordering::Relaxed))
}

/// The counter of the installed [`CapacityLog`], if one is installed.
static INSTALLED: OnceLock<Arc<AtomicU64>> = OnceLock::new();

/// Capacity warnings counted in this process by the installed [`CapacityLog`]:
/// `None` when none is installed, so that a missing count is never shown as 0.
pub fn dropped_at_capacity() -> Option<u64> {
    INSTALLED.get().map(|count| count.load(Ordering::Relaxed))
}

/// A logger that counts libp2p's capacity warning instead of writing it.
#[derive(Debug)]
pub struct CapacityLog<L> {
    inner: L,
    count: Arc<AtomicU64>,
}

impl<L: Log> CapacityLog<L> {
    /// Wraps `inner`, counting into `count`.
    pub fn new(inner: L, count: Arc<AtomicU64>) -> Self {
        Self { inner, count }
    }

    fn is_yamux_limit_error(record: &Record<'_>) -> bool {
        record.level() == Level::Error
            && record.target() == YAMUX_TARGET
            && match record.args().as_str() {
                Some(text) => text.ends_with(YAMUX_LIMIT_SUFFIX),
                None => record.args().to_string().ends_with(YAMUX_LIMIT_SUFFIX),
            }
    }

    fn is_capacity_warning(record: &Record<'_>) -> bool {
        record.level() == Level::Warn
            && record.target() == CAPACITY_TARGET
            && match record.args().as_str() {
                Some(text) => text == CAPACITY_MESSAGE,
                None => record.args().to_string() == CAPACITY_MESSAGE,
            }
    }
}

impl<L: Log> Log for CapacityLog<L> {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        // The capacity warning is always wanted, to be counted, whatever the
        // filter of the wrapped logger says.
        (metadata.level() == Level::Warn && metadata.target() == CAPACITY_TARGET)
            || (metadata.level() == Level::Error && metadata.target() == YAMUX_TARGET)
            || self.inner.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if Self::is_capacity_warning(record) {
            // Saturates instead of wrapping, so the count never goes back.
            let _ = self.count.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(1))
            });
            return;
        }
        if Self::is_yamux_limit_error(record) {
            // Counted, and still written below.
            let _ = YAMUX_LIMIT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                Some(n.saturating_add(1))
            });
        }
        if self.inner.enabled(record.metadata()) {
            self.inner.log(record);
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Installs `inner`, wrapped in a [`CapacityLog`], as the process logger.
///
/// `inner_max` is the most verbose level `inner` writes. The global maximum is
/// raised to at least `WARN`, so that the capacity warning reaches the
/// counter even when the wrapped logger writes only errors; the wrapped
/// logger's own filter still decides what is written.
///
/// # Errors
///
/// Returns an error if a logger is already installed.
pub fn install<L: Log + 'static>(
    inner: L,
    inner_max: LevelFilter,
) -> Result<(), log::SetLoggerError> {
    let count = Arc::new(AtomicU64::new(0));
    log::set_boxed_logger(Box::new(CapacityLog::new(inner, Arc::clone(&count))))?;
    log::set_max_level(inner_max.max(LevelFilter::Warn));
    let _ = INSTALLED.set(count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Records every record it is given, and writes only what its filter allows.
    struct Recorder {
        max: LevelFilter,
        seen: Mutex<Vec<(Level, String, String)>>,
    }

    impl Recorder {
        fn new(max: LevelFilter) -> Self {
            Self {
                max,
                seen: Mutex::new(Vec::new()),
            }
        }
        fn seen(&self) -> Vec<(Level, String, String)> {
            self.seen.lock().unwrap().clone()
        }
    }

    impl Log for &Recorder {
        fn enabled(&self, metadata: &Metadata<'_>) -> bool {
            metadata.level() <= self.max
        }
        fn log(&self, record: &Record<'_>) {
            self.seen.lock().unwrap().push((
                record.level(),
                record.target().to_string(),
                record.args().to_string(),
            ));
        }
        fn flush(&self) {}
    }

    fn emit(logger: &dyn Log, level: Level, target: &str, text: &str) {
        logger.log(
            &Record::builder()
                .level(level)
                .target(target)
                .args(format_args!("{text}"))
                .build(),
        );
    }

    #[test]
    fn a_hundred_thousand_capacity_warnings_write_nothing_and_count_exactly() {
        let recorder = Recorder::new(LevelFilter::Info);
        let count = Arc::new(AtomicU64::new(0));
        let logger = CapacityLog::new(&recorder, Arc::clone(&count));
        for _ in 0..100_000 {
            emit(&logger, Level::Warn, CAPACITY_TARGET, CAPACITY_MESSAGE);
        }
        assert_eq!(count.load(Ordering::Relaxed), 100_000);
        assert!(recorder.seen().is_empty());
    }

    #[test]
    fn every_other_record_passes_unchanged() {
        let recorder = Recorder::new(LevelFilter::Trace);
        let count = Arc::new(AtomicU64::new(0));
        let logger = CapacityLog::new(&recorder, Arc::clone(&count));
        let others = [
            // Same target, other text.
            (Level::Warn, CAPACITY_TARGET, "some other warning"),
            // Same text, other level.
            (Level::Error, CAPACITY_TARGET, CAPACITY_MESSAGE),
            (Level::Info, CAPACITY_TARGET, CAPACITY_MESSAGE),
            // Same text and level, other target.
            (Level::Warn, "mbongo_network::p2p", CAPACITY_MESSAGE),
            (Level::Warn, "libp2p_request_response", CAPACITY_MESSAGE),
            // Text that only starts like it.
            (
                Level::Warn,
                CAPACITY_TARGET,
                "Dropping inbound stream because we are at capacity!",
            ),
            (Level::Error, "mbongo_node", "storage error"),
        ];
        for (level, target, text) in others {
            emit(&logger, level, target, text);
        }
        let seen = recorder.seen();
        assert_eq!(seen.len(), others.len(), "every record passed: {seen:?}");
        for ((level, target, text), (l, t, m)) in others.iter().zip(&seen) {
            assert_eq!((level, *target, *text), (l, t.as_str(), m.as_str()));
        }
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn the_wrapped_filter_still_decides_what_is_written() {
        let recorder = Recorder::new(LevelFilter::Error);
        let logger = CapacityLog::new(&recorder, Arc::new(AtomicU64::new(0)));
        emit(
            &logger,
            Level::Warn,
            "mbongo_node",
            "a warning the filter excludes",
        );
        emit(&logger, Level::Error, "mbongo_node", "an error it keeps");
        assert_eq!(recorder.seen().len(), 1);
        // The capacity warning is enabled for counting even below the filter.
        let metadata = Metadata::builder().level(Level::Warn).target(CAPACITY_TARGET).build();
        assert!(logger.enabled(&metadata));
    }

    #[test]
    fn concurrent_warnings_are_all_counted() {
        let recorder = Recorder::new(LevelFilter::Info);
        let count = Arc::new(AtomicU64::new(0));
        let logger = CapacityLog::new(&recorder, Arc::clone(&count));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..12_500 {
                        emit(&logger, Level::Warn, CAPACITY_TARGET, CAPACITY_MESSAGE);
                    }
                });
            }
        });
        assert_eq!(count.load(Ordering::Relaxed), 100_000);
        assert!(recorder.seen().is_empty());
    }

    #[test]
    fn the_count_saturates_instead_of_wrapping() {
        let recorder = Recorder::new(LevelFilter::Info);
        let count = Arc::new(AtomicU64::new(u64::MAX - 1));
        let logger = CapacityLog::new(&recorder, Arc::clone(&count));
        for _ in 0..3 {
            emit(&logger, Level::Warn, CAPACITY_TARGET, CAPACITY_MESSAGE);
        }
        assert_eq!(count.load(Ordering::Relaxed), u64::MAX);
    }

    /// The yamux stream-limit error is counted and still written; nothing
    /// else from yamux, and no other level or target, is counted.
    #[test]
    fn yamux_stream_limit_errors_are_counted_and_still_written() {
        let recorder = Recorder::new(LevelFilter::Warn);
        let logger = CapacityLog::new(&recorder, Arc::new(AtomicU64::new(0)));
        let before = YAMUX_LIMIT.load(Ordering::Relaxed);
        emit(
            &logger,
            Level::Error,
            YAMUX_TARGET,
            "7f3a: maximum number of streams reached",
        );
        emit(
            &logger,
            Level::Error,
            YAMUX_TARGET,
            "1b2c: maximum number of streams reached",
        );
        emit(
            &logger,
            Level::Error,
            YAMUX_TARGET,
            "1b2c: stream already exists",
        );
        emit(
            &logger,
            Level::Warn,
            YAMUX_TARGET,
            "1b2c: maximum number of streams reached",
        );
        emit(
            &logger,
            Level::Error,
            "other",
            "x: maximum number of streams reached",
        );
        // No other test in this binary logs that error, so the count is exact.
        assert_eq!(YAMUX_LIMIT.load(Ordering::Relaxed), before + 2);
        let seen = recorder.seen();
        assert_eq!(seen.len(), 5, "every record still written: {seen:?}");
    }

    /// Wanted even when the wrapped logger filters yamux out, so it is counted.
    #[test]
    fn the_yamux_limit_error_is_wanted_whatever_the_inner_filter() {
        let recorder = Recorder::new(LevelFilter::Off);
        let logger = CapacityLog::new(&recorder, Arc::new(AtomicU64::new(0)));
        let yamux = Metadata::builder().level(Level::Error).target(YAMUX_TARGET).build();
        assert!(logger.enabled(&yamux));
        let other = Metadata::builder().level(Level::Error).target("other").build();
        assert!(!logger.enabled(&other));
    }
}
