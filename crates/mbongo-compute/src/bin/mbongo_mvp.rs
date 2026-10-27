//! `mbongo-mvp` — the MVP demo client (MVP-C, `docs/product/MVP_HTTP_CLI.md`).
//!
//! ```text
//! mbongo-mvp submit <cv.txt | -> [--wait] [--timeout <s>] [--show-result] [--json]
//! mbongo-mvp status <task_id> [--json]
//! mbongo-mvp result <task_id> [--out <file>] [--show-result]
//! mbongo-mvp verify <task_id> [--from-height <h>] [--input <cv.txt>] [--json] [--show-result]
//! ```
//!
//! Configuration comes from the environment (or the matching flag):
//! `MBONGO_MVP_FRONT`, `MBONGO_MVP_RPC_URL`, `MBONGO_MVP_REST_URL`,
//! `MBONGO_MVP_EXECUTOR`, `MBONGO_MVP_KEY_FILE` (a file holding the client
//! key seed — the key itself is never an argument), and optionally
//! `MBONGO_MVP_REFERENCE_USD_PER_HOUR` for the displayed reference cost.
//! There is no model, runtime, executor-routing or `execution_spec` option:
//! the task is always the active `mbongo-infer:v1 / kaayu-cv-extract:v3`.

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use mbongo_compute::chain::RpcChainClient;
use mbongo_compute::mvp::cli::{self, Context, SubmitOptions};
use mbongo_compute::mvp::client::{load_key_file, parse_hex32, FrontClient};
use mbongo_compute::mvp::exit;
use mbongo_core::Address;

#[derive(Parser)]
#[command(
    name = "mbongo-mvp",
    about = "Mbongo MVP client: Kaayu CV extraction (mbongo-infer:v1 / kaayu-cv-extract:v3) through the MVP HTTP front and the chain",
    after_help = "The Mbongo-operated worker processes the CV in plaintext (PUBLIC profile); the input and the result stay off-chain.\nBusiness accuracy of an extraction is not assessed by Mbongo."
)]
struct Cli {
    /// MVP HTTP front.
    #[arg(
        long,
        env = "MBONGO_MVP_FRONT",
        default_value = "http://127.0.0.1:8740"
    )]
    front: String,
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
    /// File holding the client key seed (64 hex characters).
    #[arg(long, env = "MBONGO_MVP_KEY_FILE", hide_env_values = true)]
    key_file: Option<PathBuf>,
    /// The configured MVP executor address (hex).
    #[arg(long, env = "MBONGO_MVP_EXECUTOR")]
    executor: Option<String>,
    /// Hourly infrastructure rate (USD) for the displayed reference cost.
    #[arg(long, env = "MBONGO_MVP_REFERENCE_USD_PER_HOUR")]
    reference_usd_per_hour: Option<f64>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Submit a CV text file as a kaayu-cv-extract:v3 task: upload the
    /// private input, commit the ComputeTask on-chain, register it.
    Submit {
        /// CV text file, or `-` for standard input. Plain text only.
        input: String,
        /// Wait for completion, then retrieve the result and verify it.
        #[arg(long)]
        wait: bool,
        /// Bound on the whole wait, in seconds.
        #[arg(long, default_value_t = 900)]
        timeout: u64,
        /// After the verification report, print the extracted profile.
        #[arg(long)]
        show_result: bool,
        /// Print the report as one JSON object (one line per job).
        #[arg(long)]
        json: bool,
    },
    /// Show a task's lifecycle state.
    Status {
        /// Task id (64 hex characters).
        task_id: String,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Retrieve the result's exact canonical bytes (integrity is checked by `verify`).
    Result {
        /// Task id (64 hex characters).
        task_id: String,
        /// Write the exact canonical bytes to this file.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Print the exact canonical bytes.
        #[arg(long)]
        show_result: bool,
    },
    /// Verify receipt and result integrity; validate output schema.
    #[command(
        long_about = "Verify receipt and result integrity: the anchored receipt's \
executor signature and its task, executor and input-commitment bindings, then the retrieved \
result bytes against the receipt's output commitment.\n\nValidate output schema: the retrieved \
bytes against kaayu-cv-profile:v2, reported separately.\n\nBusiness extraction accuracy is not \
assessed; NOT ASSESSED is not an error.\n\nInjection screen (with --input): whether the result \
echoes a value an instruction inside the CV asked for, or system-prompt text; reported on its own \
line, never an accuracy check."
    )]
    Verify {
        /// Task id (64 hex characters).
        task_id: String,
        /// First block to search for the task.
        #[arg(long, default_value_t = 1)]
        from_height: u64,
        /// The CV text file the task was submitted with: enables the
        /// injection screen (never uploaded, never printed).
        #[arg(long)]
        input: Option<String>,
        /// Print the report as one JSON object.
        #[arg(long)]
        json: bool,
        /// After the report, print the extracted profile.
        #[arg(long)]
        show_result: bool,
    },
}

fn read_input(path: &str) -> Result<String, String> {
    if path == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("cannot read standard input: {}", e.kind()))?;
        return Ok(s);
    }
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {}", e.kind()))
}

fn code(c: i32) -> ExitCode {
    ExitCode::from(u8::try_from(c).unwrap_or(1))
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut out = std::io::stdout();
    let mut err = std::io::stderr();
    let Some(key_file) = cli.key_file.as_deref() else {
        eprintln!("error: no client key file (MBONGO_MVP_KEY_FILE or --key-file)");
        return code(exit::CONFIG);
    };
    let key = match load_key_file(key_file) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: {e}");
            return code(exit::CONFIG);
        }
    };
    let executor = match cli.executor.as_deref() {
        None => None,
        Some(s) => match parse_hex32(s.trim_start_matches("0x")) {
            Some(a) => Some(Address(a)),
            None => {
                eprintln!("error: executor must be a 32-byte hex address");
                return code(exit::CONFIG);
            }
        },
    };
    let front = match FrontClient::new(&cli.front, key.clone()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {e}");
            return code(exit::CONFIG);
        }
    };
    let chain = RpcChainClient::new(cli.rpc_url, cli.rest_url);
    let ctx = Context {
        chain: &chain,
        front: &front,
        key: &key,
        executor,
        reference_usd_per_hour: cli.reference_usd_per_hour,
        poll: Duration::from_secs(1),
    };
    let c = match cli.command {
        Command::Submit {
            input,
            wait,
            timeout,
            show_result,
            json,
        } => match read_input(&input) {
            Ok(text) => {
                cli::submit(
                    &ctx,
                    text,
                    SubmitOptions {
                        wait,
                        timeout: Duration::from_secs(timeout),
                        show_result,
                        json,
                    },
                    &mut out,
                    &mut err,
                )
                .await
            }
            Err(e) => {
                eprintln!("error: {e}");
                exit::CONFIG
            }
        },
        Command::Status { task_id, json } => {
            cli::status(&ctx, &task_id, json, &mut out, &mut err).await
        }
        Command::Result {
            task_id,
            out: file,
            show_result,
        } => {
            cli::result(
                &ctx,
                &task_id,
                file.as_deref(),
                show_result,
                &mut out,
                &mut err,
            )
            .await
        }
        Command::Verify {
            task_id,
            from_height,
            input,
            json,
            show_result,
        } => {
            let cv = match input.as_deref().map(read_input).transpose() {
                Ok(cv) => cv,
                Err(e) => {
                    eprintln!("error: {e}");
                    return code(exit::CONFIG);
                }
            };
            cli::verify(
                &ctx,
                &task_id,
                from_height,
                cv.as_deref(),
                json,
                show_result,
                &mut out,
                &mut err,
            )
            .await
        }
    };
    code(c)
}
