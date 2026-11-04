//! A provider service: one per independently operated provider. Loopback only.
//!
//! ```text
//! provider_service --config <file>
//! ```
//!
//! The configuration names the node, the executor key file, the artifact
//! directory, the runtimes and the isolation mode, which has no default.
//! `required` is Linux only and refuses to start unless containment is proven
//! (`docs/runbooks/PROVIDER_CONTAINMENT_LINUX.md`). `none` isolates nothing,
//! and is for controlled local testing only.
//! Remote exposure and TLS are the provider's responsibility.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use mbongo_compute::chain::RpcChainClient;
use mbongo_compute::clock::SystemClock;
use mbongo_compute::job::{JobSource, MeteringSink, RuntimeAdapter, RuntimeIdentity};
use mbongo_compute::mvp::client::{load_key_file, parse_hex32, random_bytes};
use mbongo_compute::provider::containment::{Containment, ContainmentConfig};
use mbongo_compute::provider::durable::DurableStore;
use mbongo_compute::provider::runner::{RunnerConfig, SubprocessRuntime, RUNNER_CONTRACT};
use mbongo_compute::provider::service::{
    MeteringLog, ProviderConfig, ProviderParts, ProviderService, ProviderStore,
};
use serde::Deserialize;

#[derive(Parser)]
#[command(
    name = "provider_service",
    about = "Mbongo provider service (loopback)"
)]
struct Cli {
    /// Configuration file (JSON).
    #[arg(long)]
    config: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeEntry {
    family: String,
    revision: String,
    accelerator_class: String,
    binary_sha256: Option<String>,
    command: Vec<String>,
    #[serde(default)]
    pass_env: Vec<String>,
}

/// Stated explicitly: a configuration that omits it does not load.
#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum Isolation {
    /// Nothing is isolated, and nothing is claimed. For controlled local
    /// testing only.
    None,
    /// Proven at startup, or the service does not start.
    Required {
        runner_uid: u32,
        runner_gid: u32,
        cgroup_root: PathBuf,
        /// Ceiling on a job's memory, whatever its spec declares.
        max_memory_mib: u64,
        max_pids: u64,
        /// Further paths a runner must not read.
        #[serde(default)]
        denied_paths: Vec<PathBuf>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    bind: SocketAddr,
    rpc_url: String,
    rest_url: String,
    executor_key_file: PathBuf,
    /// Durable state. Fails closed: damaged state stops the service.
    state_dir: PathBuf,
    /// File holding the 32-byte state encryption key (64 hex characters).
    state_key_file: PathBuf,
    artifact_dir: PathBuf,
    work_dir: PathBuf,
    isolation: Isolation,
    runtimes: Vec<RuntimeEntry>,
}

/// Runs after the state is open: the lock is held, so the jobs a crash left
/// behind are this service's own.
fn contain(config: &Config) -> Result<Option<Arc<Containment>>, String> {
    let Isolation::Required {
        runner_uid,
        runner_gid,
        cgroup_root,
        max_memory_mib,
        max_pids,
        denied_paths,
    } = &config.isolation
    else {
        return Ok(None);
    };
    let mut denied = denied_paths.clone();
    let state = std::fs::read_dir(&config.state_dir).map_err(|e| format!("state: {}", e.kind()))?;
    denied.extend(state.filter_map(Result::ok).map(|e| e.path()));
    let containment = Containment::establish(ContainmentConfig {
        runner_uid: *runner_uid,
        runner_gid: *runner_gid,
        cgroup_root: cgroup_root.clone(),
        max_memory_mib: *max_memory_mib,
        max_pids: *max_pids,
        secret_files: vec![
            config.executor_key_file.clone(),
            config.state_key_file.clone(),
        ],
        private_dirs: vec![config.state_dir.clone(), config.artifact_dir.clone()],
        work_dir: config.work_dir.clone(),
        denied_paths: denied,
    })
    .map_err(|e| e.to_string())?;
    println!("ISOLATION_PROBE_PATHS={}", containment.probed_paths());
    println!("CONTAINMENT_SWEPT={}", containment.swept());
    Ok(Some(Arc::new(containment)))
}

/// Binds through `std`, whose sockets are never inherited: a runner must not
/// get a handle on this socket. A tokio-created listener was inherited on
/// Windows, so a runner orphaned by a crash kept the port. The wait covers a
/// port the system has not released yet.
async fn bind(address: SocketAddr) -> Result<tokio::net::TcpListener, String> {
    let started = std::time::Instant::now();
    loop {
        match std::net::TcpListener::bind(address) {
            Ok(listener) => {
                println!("BIND_WAIT_MS={}", started.elapsed().as_millis());
                listener.set_nonblocking(true).map_err(|e| format!("bind: {}", e.kind()))?;
                return tokio::net::TcpListener::from_std(listener)
                    .map_err(|e| format!("bind: {}", e.kind()));
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::AddrInUse
                    && started.elapsed() < Duration::from_secs(30) =>
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(format!("bind {address}: {}", e.kind())),
        }
    }
}

/// Ctrl-C, or `SIGTERM` where there is one.
async fn stop_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

async fn serve(config: Config) -> Result<(), String> {
    if !config.bind.ip().is_loopback() {
        return Err(format!(
            "refusing to bind {}: this service binds loopback only",
            config.bind
        ));
    }
    let executor = load_key_file(&config.executor_key_file)?;
    let state_key = std::fs::read_to_string(&config.state_key_file)
        .ok()
        .and_then(|k| parse_hex32(k.trim()))
        .ok_or("state key: expected 64 hex characters")?;
    let mut provider_config = ProviderConfig::default();
    let durable = DurableStore::open(&config.state_dir, &state_key, provider_config.retention)
        .map_err(|e| e.to_string())?;
    let containment = contain(&config)?;
    provider_config.contained = containment.as_ref().map(|c| c.proof());
    let store = Arc::new(ProviderStore::new(config.artifact_dir));
    let mut runtimes: Vec<Arc<dyn RuntimeAdapter>> = Vec::new();
    for r in config.runtimes {
        let runtime = SubprocessRuntime::new(
            RunnerConfig {
                identity: RuntimeIdentity {
                    family: r.family,
                    revision: r.revision,
                    binary_sha256: r.binary_sha256,
                    accelerator_class: r.accelerator_class,
                },
                command: r.command,
                pass_env: r.pass_env,
                work_dir: config.work_dir.clone(),
            },
            Arc::clone(&store) as Arc<dyn JobSource>,
        );
        let runtime = match &containment {
            Some(c) => runtime.contained(Arc::clone(c)),
            None => runtime,
        };
        runtime.verify_binary()?;
        runtimes.push(Arc::new(runtime));
    }
    let service = ProviderService::open(
        RpcChainClient::new(config.rpc_url, config.rest_url),
        Arc::new(SystemClock),
        random_bytes::<32>(),
        executor,
        ProviderParts {
            store,
            runtimes,
            metering: Arc::new(MeteringLog::default()) as Arc<dyn MeteringSink>,
        },
        Arc::new(durable),
        provider_config,
    )
    .await
    .map_err(|e| e.to_string())?;
    let listener = bind(config.bind).await?;
    println!(
        "PROVIDER_BIND={}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    println!("EXECUTOR={}", hex::encode(service.executor().0));
    println!("RUNNER_CONTRACT={RUNNER_CONTRACT}");
    let enforced = if containment.is_some() { "YES" } else { "NO" };
    if containment.is_none() {
        log::warn!("provider: isolation is off: for controlled local testing only");
    }
    println!("KEY_ISOLATION_ENFORCED={enforced}");
    println!("MEMORY_LIMIT_ENFORCED={enforced}");
    println!("STATE=DURABLE");
    println!("EXECUTION=AT_LEAST_ONCE (one receipt per task; a runner may run twice)");
    let ticker = {
        let service = service.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = service.tick().await {
                    log::warn!("provider: tick: {e}");
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
    };
    let stop_ticking = ticker.abort_handle();
    let result = axum::serve(listener, service.router())
        .with_graceful_shutdown(async move {
            stop_signal().await;
            stop_ticking.abort();
            // A job must not outlive the service that bounds it. It stays
            // unfinished in the journal, and the next start runs it again.
            if let Some(containment) = containment {
                println!("SHUTDOWN_JOBS_KILLED={}", containment.stop());
            }
        })
        .await
        .map_err(|e| format!("serve: {}", e.kind()));
    ticker.abort();
    result
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let config = std::fs::read(&cli.config)
        .map_err(|e| format!("config: {}", e.kind()))
        .and_then(|b| serde_json::from_slice(&b).map_err(|e| format!("config: {e}")));
    let result = match config {
        Ok(c) => serve(c).await,
        Err(e) => Err(e),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
