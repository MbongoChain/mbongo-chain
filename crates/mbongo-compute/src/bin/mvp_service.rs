//! The MVP service (MVP-C): the HTTP front, the in-process control plane
//! and private data plane, and the MVP-B CPU inference worker, in one
//! loopback process (`docs/product/MVP_HTTP_CLI.md`).
//!
//! ```text
//! mvp_service serve            [--bind 127.0.0.1:8740] [--rpc-url …] [--rest-url …]
//! mvp_service executor-address
//! mvp_service fund-executor    --amount <n>            (operator, devnet only)
//! ```
//!
//! Keys are read from files named by environment variables, never from the
//! command line: `MBONGO_MVP_EXECUTOR_KEY_FILE` (the worker's executor key)
//! and, for `fund-executor` only, `MBONGO_MVP_FUNDER_KEY_FILE`. The model
//! and runtime are the pinned MVP-B ones, resolved by
//! `RuntimeConfig::from_env`; there is no option to choose another.
//!
//! The front binds loopback only. A deployment that needs more is a later
//! gate (TLS, a reverse proxy, client allow-lists), not a flag here.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use mbongo_compute::chain::{ChainClient, RpcChainClient};
use mbongo_compute::clock::SystemClock;
use mbongo_compute::cpu_inference::config::RuntimeConfig;
use mbongo_compute::cpu_inference::{InferenceBackend, InferenceProfile, LlamaServer};
use mbongo_compute::data_plane::LocalKey;
use mbongo_compute::mvp::client::{load_key_file, random_bytes};
use mbongo_compute::mvp::front::{Front, FrontConfig};
use mbongo_compute::mvp::{DEFAULT_BIND, RUNTIME_PROFILE};
use mbongo_compute::profiles;
use mbongo_core::{Address, Transaction, TransactionPayload, TransactionType};

#[derive(Parser)]
#[command(
    name = "mvp_service",
    about = "Mbongo MVP service: loopback HTTP front + control plane + private data plane + CPU inference worker"
)]
struct Cli {
    /// Node JSON-RPC endpoint.
    #[arg(
        long,
        env = "MBONGO_MVP_RPC_URL",
        default_value = "http://127.0.0.1:9944/rpc"
    )]
    rpc_url: String,
    /// Node REST endpoint (account nonces).
    #[arg(
        long,
        env = "MBONGO_MVP_REST_URL",
        default_value = "http://127.0.0.1:8080"
    )]
    rest_url: String,
    /// File holding the executor key seed (64 hex characters).
    #[arg(long, env = "MBONGO_MVP_EXECUTOR_KEY_FILE", hide_env_values = true)]
    executor_key_file: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve the HTTP front and run the worker.
    Serve {
        /// Loopback address to bind.
        #[arg(long, env = "MBONGO_MVP_BIND", default_value = DEFAULT_BIND)]
        bind: SocketAddr,
    },
    /// Print the executor address clients must name.
    ExecutorAddress,
    /// Devnet operator step: transfer funds to the executor so its account
    /// exists before it anchors receipts.
    FundExecutor {
        /// Amount to transfer.
        #[arg(long, default_value_t = 1_000)]
        amount: u128,
        /// File holding the funder key seed (64 hex characters).
        #[arg(long, env = "MBONGO_MVP_FUNDER_KEY_FILE", hide_env_values = true)]
        funder_key_file: PathBuf,
    },
}

fn load(path: &std::path::Path) -> Result<LocalKey, String> {
    load_key_file(path)
}

async fn fund(
    chain: &RpcChainClient,
    funder: &LocalKey,
    to: Address,
    amount: u128,
) -> Result<(), String> {
    let nonce = chain
        .account_nonce(&funder.address())
        .await
        .map_err(|e| format!("chain: {e}"))?;
    let mut tx = Transaction {
        tx_type: TransactionType::Transfer,
        sender: funder.address(),
        receiver: to,
        amount,
        nonce,
        payload: TransactionPayload::None,
        signature: [0u8; 64],
    };
    tx.signature = funder.sign(&tx.signing_payload());
    let hash = chain
        .submit_transaction(&tx)
        .await
        .map_err(|e| format!("transfer refused: {e}"))?;
    println!("FUND_TX={hash}");
    for _ in 0..300 {
        if chain
            .account_nonce(&funder.address())
            .await
            .map_err(|e| format!("chain: {e}"))?
            > nonce
        {
            println!("EXECUTOR_FUNDED={to} amount={amount}");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Err("timed out waiting for the transfer to be included".into())
}

async fn serve(chain: RpcChainClient, executor: LocalKey, bind: SocketAddr) -> Result<(), String> {
    mbongo_compute::mvp::check_loopback_bind(bind)?;
    let spec = profiles::active_job_spec();
    let cfg = RuntimeConfig::from_env(&spec.model.artifact_name)?;
    let (server, report) =
        LlamaServer::start(cfg, &spec).map_err(|e| format!("worker start-up refused: {e}"))?;
    println!(
        "WORKER_RUNTIME={} build={} commit={}",
        report.runtime_version, report.runtime_build, report.runtime_commit_prefix
    );
    println!("WORKER_MODEL_SHA256={}", report.model_sha256);
    println!("WORKER_COLD_START_MS={}", report.cold_start_ms);
    let server = Arc::new(server);
    let profile = InferenceProfile::new(Arc::clone(&server) as Arc<dyn InferenceBackend>)
        .map_err(|e| format!("profile refused: {e}"))?;
    let observations = profile.observations();
    let front = Front::new(
        chain,
        Arc::new(SystemClock),
        random_bytes::<32>(),
        executor,
        Arc::new(profile),
        Some(observations),
        FrontConfig::default(),
    );
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| format!("bind {bind}: {}", e.kind()))?;
    println!(
        "FRONT_BIND={}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    println!("EXECUTOR={}", front.executor());
    println!("RUNTIME_PROFILE={RUNTIME_PROFILE}");
    println!("STATE=IN_MEMORY (lost when this process stops):");
    for (item, consequence) in mbongo_compute::mvp::IN_MEMORY_STATE {
        println!("  {item}: {consequence}");
    }

    let ticker = {
        let front = front.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = front.tick().await {
                    log::warn!("mvp-front: tick: {e}");
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        })
    };
    let result = axum::serve(listener, front.router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| format!("serve: {}", e.kind()));
    ticker.abort();
    drop(server);
    result
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let executor = match load(&cli.executor_key_file) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(3);
        }
    };
    let chain = RpcChainClient::new(cli.rpc_url, cli.rest_url);
    let result = match cli.command {
        Command::ExecutorAddress => {
            println!("{}", executor.address());
            Ok(())
        }
        Command::FundExecutor {
            amount,
            funder_key_file,
        } => match load(&funder_key_file) {
            Ok(funder) => fund(&chain, &funder, executor.address(), amount).await,
            Err(e) => Err(e),
        },
        Command::Serve { bind } => serve(chain, executor, bind).await,
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(4)
        }
    }
}
