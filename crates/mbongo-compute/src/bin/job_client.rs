//! A standalone generic job client: submits one job to one provider and
//! verifies the result against the chain, up to T1.
//!
//! ```text
//! job_client --provider <url> --executor <hex> --job-spec <file> --input <file>
//!            --key-file <file> [--rpc-url …] [--rest-url …] [--timeout-secs n]
//! ```
//!
//! Prints a JSON report and exits 0 on a verified result; exits 3 when the
//! job failed or could not be verified, with the code on stderr. The key is
//! read from a file, never the command line.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use mbongo_compute::chain::RpcChainClient;
use mbongo_compute::mvp::client::{load_key_file, parse_hex32};
use mbongo_compute::provider::client::{run_job, ProviderClient, WaitPolicy};
use mbongo_core::Address;

#[derive(Parser)]
#[command(name = "job_client", about = "Submit a generic job to one provider")]
struct Cli {
    /// The provider's base URL.
    #[arg(long)]
    provider: String,
    /// The provider's executor address, known out of band: the signature's
    /// recipient and the task's executor.
    #[arg(long)]
    executor: String,
    /// The canonical job spec.
    #[arg(long)]
    job_spec: PathBuf,
    /// The input bytes.
    #[arg(long)]
    input: PathBuf,
    /// File holding the client's key seed (64 hex characters).
    #[arg(long)]
    key_file: PathBuf,
    /// Node JSON-RPC endpoint.
    #[arg(long, default_value = "http://127.0.0.1:9944/rpc")]
    rpc_url: String,
    /// Node REST endpoint.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    rest_url: String,
    /// Give up after this long.
    #[arg(long, default_value_t = 180)]
    timeout_secs: u64,
}

async fn run(cli: Cli) -> Result<String, String> {
    let key = load_key_file(&cli.key_file)?;
    let executor = parse_hex32(&cli.executor).ok_or("executor is not 32 bytes of hex")?;
    let read = |p: &PathBuf| std::fs::read(p).map_err(|e| format!("read: {}", e.kind()));
    let (job_spec, input) = (read(&cli.job_spec)?, read(&cli.input)?);
    let chain = RpcChainClient::new(cli.rpc_url, cli.rest_url);
    let client = ProviderClient::new(&cli.provider, key.clone(), Address(executor))
        .map_err(|e| e.to_string())?;
    let wait = WaitPolicy {
        timeout: Duration::from_secs(cli.timeout_secs),
        poll: Duration::from_millis(300),
    };
    let report = run_job(&client, &chain, &key, &job_spec, &input, wait)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_string(&report).map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(3)
        }
    }
}
