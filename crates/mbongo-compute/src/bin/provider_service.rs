//! A provider service: one per independently operated provider. Loopback only.
//!
//! ```text
//! provider_service --config <file>
//! ```
//!
//! The configuration names the node, the executor key file, the artifact
//! directory and the runtimes. Remote exposure, TLS and OS isolation of the
//! runner are the provider's responsibility and are not provided here.

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
use mbongo_compute::provider::durable::DurableStore;
use mbongo_compute::provider::runner::{RunnerConfig, SubprocessRuntime, RUNNER_CONTRACT};
use mbongo_compute::provider::service::{
    MeteringLog, ProviderConfig, ProviderParts, ProviderService, ProviderStore,
    KEY_ISOLATION_ENFORCED,
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
    runtimes: Vec<RuntimeEntry>,
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
    let provider_config = ProviderConfig::default();
    let durable = DurableStore::open(&config.state_dir, &state_key, provider_config.retention)
        .map_err(|e| e.to_string())?;
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
    println!(
        "KEY_ISOLATION_ENFORCED={}",
        if KEY_ISOLATION_ENFORCED { "YES" } else { "NO" }
    );
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
    let result = axum::serve(listener, service.router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
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
