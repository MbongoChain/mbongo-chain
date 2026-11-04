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
use mbongo_compute::mvp::client::{load_key_file, random_bytes};
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
    artifact_dir: PathBuf,
    work_dir: PathBuf,
    runtimes: Vec<RuntimeEntry>,
}

async fn serve(config: Config) -> Result<(), String> {
    if !config.bind.ip().is_loopback() {
        return Err(format!(
            "refusing to bind {}: this service binds loopback only",
            config.bind
        ));
    }
    let executor = load_key_file(&config.executor_key_file)?;
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
    let service = ProviderService::new(
        RpcChainClient::new(config.rpc_url, config.rest_url),
        Arc::new(SystemClock),
        random_bytes::<32>(),
        executor,
        ProviderParts {
            store,
            runtimes,
            metering: Arc::new(MeteringLog::default()) as Arc<dyn MeteringSink>,
        },
        ProviderConfig::default(),
    );
    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {}: {}", config.bind, e.kind()))?;
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
    println!("STATE=IN_MEMORY (lost when this process stops)");
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
