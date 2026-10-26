//! Worker-side configuration of the CPU inference runtime (MVP-B §4).
//!
//! Everything here is **operator configuration**, never task input: the
//! runtime executable, the model file, the port and the thread count come
//! from the worker's own environment or defaults, so a task cannot point the
//! worker at another binary, another model or another file (MVP-B §65–§67).
//! None of it is consensus, and none of it enters the job spec: changing the
//! thread count does not change what a task means.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// The upstream build tag whose commit equals the pinned release `v0.7.0`
/// (`620d2225cc436ab0b001e3200e590e3b286ef9ab`): the `v0.7.0` GitHub release
/// carries only a pointer file naming it.
pub const LLAMA_CPP_BUILD_TAG: &str = "b11669";
/// Build number `llama-server --version` reports for that tag.
pub const LLAMA_CPP_BUILD_NUMBER: u64 = 11_669;

/// A pinned prebuilt runtime for one platform: the upstream archive and its
/// published SHA-256, and the SHA-256 of the `llama-server` executable inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinnedRuntime {
    /// Release asset name.
    pub archive: &'static str,
    /// SHA-256 GitHub publishes for the asset (and the setup script checks).
    pub archive_sha256: &'static str,
    /// Path of the server executable relative to the extraction directory.
    pub server_relpath: &'static str,
    /// SHA-256 of that executable.
    pub server_sha256: &'static str,
}

/// Windows x86-64, CPU-only build.
pub const PINNED_WINDOWS_X64: PinnedRuntime = PinnedRuntime {
    archive: "llama-b11669-bin-win-cpu-x64.zip",
    archive_sha256: "37b3efcf962847ca869745fd4ccf7f5e7e8dacfeff9fa3523b4ca8cdc66d632b",
    server_relpath: "llama-server.exe",
    server_sha256: "6a4635aeb35656153d4bfeeddf761d24ed84aada9c9108d580737cbed465141b",
};

/// Linux x86-64, CPU-only build.
pub const PINNED_LINUX_X64: PinnedRuntime = PinnedRuntime {
    archive: "llama-b11669-bin-ubuntu-x64.tar.gz",
    archive_sha256: "cf07c1c112d709bd67e522017471de57ed0ec8bea031d732c5422351c07f65ac",
    server_relpath: "llama-b11669/llama-server",
    server_sha256: "3f2b53013c2eb596c8aca87e4740240a41f95bcebf72e61084d6d5ba0db761cc",
};

/// The pinned runtime for the platform this worker was built for, if any.
pub fn pinned_runtime_for_this_platform() -> Option<PinnedRuntime> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some(PINNED_WINDOWS_X64)
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(PINNED_LINUX_X64)
    } else {
        None
    }
}

/// The local loopback address. Never configurable to a wildcard: the raw
/// llama-server must not be reachable from outside the worker host
/// (MVP-B §62–§63).
pub const LOOPBACK: &str = "127.0.0.1";

/// Worker runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfig {
    /// Absolute path of the `llama-server` executable.
    pub runtime_path: PathBuf,
    /// Expected SHA-256 of that executable (lowercase hex).
    pub runtime_sha256: String,
    /// Absolute path of the model file.
    pub model_path: PathBuf,
    /// Directory for the server's own log file.
    pub log_dir: PathBuf,
    /// Loopback port the server listens on.
    pub port: u16,
    /// CPU threads for generation and prompt processing.
    pub threads: u32,
    /// Context size in tokens (prompt + generation).
    pub ctx_size: u32,
    /// Time allowed for the server to load the model and report healthy.
    pub startup_timeout: Duration,
    /// Time allowed for one inference request. Operational, not the product
    /// latency target.
    pub inference_timeout: Duration,
    /// Upper bound on any HTTP response body read from the server.
    pub max_response_bytes: usize,
}

/// Environment variables the worker reads (MVP-B §69). Only these are read;
/// the environment is never logged.
pub mod env {
    /// Cache root (default `~/.cache/mbongo/inference`).
    pub const HOME: &str = "MBONGO_INFER_HOME";
    /// Overrides the runtime executable path.
    pub const RUNTIME_PATH: &str = "MBONGO_INFER_RUNTIME";
    /// Overrides the model file path.
    pub const MODEL_PATH: &str = "MBONGO_INFER_MODEL";
    /// Loopback port.
    pub const PORT: &str = "MBONGO_INFER_PORT";
    /// CPU threads.
    pub const THREADS: &str = "MBONGO_INFER_THREADS";
    /// Inference timeout in seconds.
    pub const INFERENCE_TIMEOUT_SECS: &str = "MBONGO_INFER_TIMEOUT_SECS";
}

/// Default cache root: `$MBONGO_INFER_HOME`, else `~/.cache/mbongo/inference`.
pub fn default_home() -> PathBuf {
    if let Some(h) = std::env::var_os(env::HOME) {
        return PathBuf::from(h);
    }
    let base = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    base.join(".cache").join("mbongo").join("inference")
}

/// Directory the setup script extracts the pinned runtime into.
pub fn runtime_dir(home: &Path, pinned: &PinnedRuntime) -> PathBuf {
    let platform = if pinned.archive.contains("win") {
        "win-cpu-x64"
    } else {
        "ubuntu-x64"
    };
    home.join("runtime").join(format!("{LLAMA_CPP_BUILD_TAG}-{platform}"))
}

impl RuntimeConfig {
    /// Defaults for the cache layout of `scripts/setup-mvp-inference-runtime.sh`,
    /// with environment overrides. `model_file` is the job spec's
    /// `artifact_name`.
    pub fn from_env(model_file: &str) -> Result<Self, String> {
        let home = default_home();
        let pinned = pinned_runtime_for_this_platform()
            .ok_or("no pinned llama.cpp build for this platform")?;
        let runtime_path = std::env::var_os(env::RUNTIME_PATH).map_or_else(
            || runtime_dir(&home, &pinned).join(pinned.server_relpath),
            PathBuf::from,
        );
        let model_path = std::env::var_os(env::MODEL_PATH)
            .map_or_else(|| home.join("models").join(model_file), PathBuf::from);
        let port = parse_env(env::PORT, 9931u16)?;
        let threads = parse_env(env::THREADS, default_threads())?;
        let timeout = parse_env(env::INFERENCE_TIMEOUT_SECS, 180u64)?;
        Ok(Self {
            runtime_path,
            runtime_sha256: pinned.server_sha256.to_owned(),
            model_path,
            log_dir: home.join("logs"),
            port,
            threads,
            ctx_size: 8192,
            startup_timeout: Duration::from_secs(120),
            inference_timeout: Duration::from_secs(timeout),
            max_response_bytes: 256 * 1024,
        })
    }
}

/// Physical-core-sized default: half the logical processors, at least one.
pub fn default_threads() -> u32 {
    let logical = std::thread::available_parallelism().map_or(2, std::num::NonZeroUsize::get);
    u32::try_from((logical / 2).max(1)).unwrap_or(1)
}

fn parse_env<T: std::str::FromStr>(name: &str, default: T) -> Result<T, String> {
    match std::env::var(name) {
        Ok(v) => v.parse().map_err(|_| format!("{name} is not a valid value")),
        Err(_) => Ok(default),
    }
}
