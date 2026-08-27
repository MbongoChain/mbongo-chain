//! External devnet convergence probe.
//!
//! Validates that a set of ALREADY RUNNING nodes are healthy, progressing,
//! and converged. It is the network-addressed counterpart of
//! `devnet_harness`: the harness spawns its own nodes and probes them over
//! loopback, this probe is pointed at endpoints someone else owns (for
//! example three containers on a Docker network).
//!
//! Both share one implementation of readiness and convergence — see
//! [`mbongo_node::convergence`]. This binary contains no convergence rules
//! of its own; it only decides which endpoints to probe and which height
//! floor to require.
//!
//! It never starts, stops, or restarts a node: lifecycle belongs to
//! whoever launched them.
//!
//! # Usage
//!
//! ```sh
//! convergence_probe \
//!     --endpoint producer=http://127.0.0.1:19944 \
//!     --endpoint follower-a=http://127.0.0.1:19945 \
//!     --endpoint follower-b=http://127.0.0.1:19946
//! ```
//!
//! Exit code 0 when every endpoint agrees on the same height and tip hash
//! at or above the required floor; non-zero with diagnostics otherwise.

use std::time::Duration;

use clap::Parser;
use reqwest::Client;

use mbongo_node::convergence::{
    await_convergence, await_endpoints_ready, get_height, NodeEndpoint,
};

/// Minimum number of endpoints: agreement is meaningless below two.
const MIN_ENDPOINTS: usize = 2;

#[derive(Parser, Debug)]
#[command(name = "convergence_probe")]
#[command(about = "Check that already-running Mbongo nodes converge", long_about = None)]
struct Args {
    /// Node endpoint, repeatable: `name=http://host:port` (or just the URL).
    #[arg(long = "endpoint", value_name = "NAME=URL", required = true)]
    endpoints: Vec<String>,

    /// Absolute height floor every node must reach.
    #[arg(long, default_value = "5")]
    min_height: u64,

    /// Bound on how long every endpoint may take to answer ping.
    #[arg(long, default_value = "60")]
    ready_timeout_secs: u64,

    /// Bound on how long the nodes may take to converge once ready.
    #[arg(long, default_value = "60")]
    convergence_timeout_secs: u64,

    /// Delay between polls.
    #[arg(long, default_value = "500")]
    poll_interval_ms: u64,

    /// Only require agreement, not progression. By default the probe also
    /// requires a NEW block after every node is ready, so a stalled chain
    /// whose nodes merely agree on an old tip cannot pass.
    #[arg(long)]
    no_progress_check: bool,
}

/// Parses one `name=url` (or bare `url`) endpoint specification.
fn parse_endpoint(spec: &str) -> Result<NodeEndpoint, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("empty endpoint specification".to_string());
    }
    let (name, url) = match spec.split_once('=') {
        Some((name, url)) => (name.trim(), url.trim()),
        // A bare URL is accepted; it names itself in diagnostics.
        None => (spec, spec),
    };
    if name.is_empty() {
        return Err(format!("endpoint {spec:?} has an empty name"));
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(format!(
            "endpoint {spec:?} must point at an http(s) URL, e.g. name=http://host:9944"
        ));
    }
    Ok(NodeEndpoint::new(name, url))
}

/// Parses and validates the full endpoint list.
fn parse_endpoints(specs: &[String]) -> Result<Vec<NodeEndpoint>, String> {
    let nodes = specs.iter().map(|s| parse_endpoint(s)).collect::<Result<Vec<_>, _>>()?;
    if nodes.len() < MIN_ENDPOINTS {
        return Err(format!(
            "at least {MIN_ENDPOINTS} endpoints are required to check convergence, got {}",
            nodes.len()
        ));
    }
    Ok(nodes)
}

/// The height floor convergence must reach.
///
/// With progression required (the default), the floor is one block above
/// the highest height observed once every node is ready: agreement on a
/// tip that already existed before the probe started cannot satisfy it.
/// The absolute `min_height` is always enforced as well.
fn progress_floor(min_height: u64, baseline: u64, require_progress: bool) -> u64 {
    if require_progress {
        min_height.max(baseline.saturating_add(1))
    } else {
        min_height
    }
}

/// Highest height currently reported by any endpoint.
async fn max_height(client: &Client, nodes: &[NodeEndpoint]) -> Result<u64, String> {
    let mut max = 0u64;
    for node in nodes {
        let h = get_height(client, node).await?;
        max = max.max(h);
    }
    Ok(max)
}

async fn run(args: &Args) -> Result<(), String> {
    let nodes = parse_endpoints(&args.endpoints)?;
    let client = Client::new();
    let poll = Duration::from_millis(args.poll_interval_ms);

    println!("=== Devnet Convergence Probe ===");
    for node in &nodes {
        println!("  endpoint: {} -> {}", node.name(), node.rpc_url());
    }

    println!(
        "Waiting for readiness (up to {}s)...",
        args.ready_timeout_secs
    );
    await_endpoints_ready(
        &client,
        &nodes,
        poll,
        Duration::from_secs(args.ready_timeout_secs),
    )
    .await?;
    println!("  All {} endpoint(s) ready", nodes.len());

    let baseline = max_height(&client, &nodes).await?;
    let floor = progress_floor(args.min_height, baseline, !args.no_progress_check);
    println!("Polling for convergence (baseline {baseline}, floor {floor})...");

    await_convergence(
        &client,
        &nodes,
        floor,
        poll,
        Duration::from_secs(args.convergence_timeout_secs),
    )
    .await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match run(&args).await {
        Ok(()) => {
            println!("\nDEVNET CONVERGENCE: PASS");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("\nDEVNET CONVERGENCE: FAIL");
            eprintln!("  Error: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_for(endpoints: &[&str], ready_secs: u64) -> Args {
        let mut argv = vec!["convergence_probe".to_string()];
        for e in endpoints {
            argv.push("--endpoint".to_string());
            argv.push((*e).to_string());
        }
        argv.push("--ready-timeout-secs".to_string());
        argv.push(ready_secs.to_string());
        argv.push("--poll-interval-ms".to_string());
        argv.push("50".to_string());
        Args::parse_from(argv)
    }

    #[test]
    fn parses_named_and_bare_endpoints() {
        let node = parse_endpoint("producer=http://producer:9944").unwrap();
        assert_eq!(node.name(), "producer");
        assert_eq!(node.rpc_url(), "http://producer:9944/rpc");

        let bare = parse_endpoint("http://127.0.0.1:19944").unwrap();
        assert_eq!(bare.rpc_url(), "http://127.0.0.1:19944/rpc");
    }

    #[test]
    fn rejects_malformed_endpoints() {
        assert!(parse_endpoint("").is_err());
        assert!(parse_endpoint("producer=").is_err());
        assert!(parse_endpoint("=http://x:1").is_err());
        assert!(parse_endpoint("producer=127.0.0.1:9944").is_err());
    }

    #[test]
    fn requires_at_least_two_endpoints() {
        let one = vec!["a=http://127.0.0.1:1".to_string()];
        assert!(parse_endpoints(&one).is_err());
        let three = vec![
            "a=http://127.0.0.1:1".to_string(),
            "b=http://127.0.0.1:2".to_string(),
            "c=http://127.0.0.1:3".to_string(),
        ];
        assert_eq!(parse_endpoints(&three).unwrap().len(), 3);
    }

    #[test]
    fn progress_floor_requires_a_new_block_by_default() {
        // Same invariant the harness uses across restarts: baseline + 1.
        assert_eq!(progress_floor(5, 10, true), 11);
        // The absolute floor still wins when the chain is younger.
        assert_eq!(progress_floor(5, 1, true), 5);
        // Opting out only asserts agreement at the absolute floor.
        assert_eq!(progress_floor(5, 10, false), 5);
    }

    #[tokio::test]
    async fn unreachable_endpoints_fail_with_named_diagnostics() {
        // Nothing is listening on these ports: readiness must time out,
        // and the error must identify BOTH failing endpoints by name.
        let args = args_for(
            &[
                "producer=http://127.0.0.1:45781",
                "follower-a=http://127.0.0.1:45782",
            ],
            1,
        );
        let err = run(&args).await.unwrap_err();
        assert!(err.contains("readiness timeout"), "{err}");
        assert!(
            err.contains("producer"),
            "names the failing endpoint: {err}"
        );
        assert!(
            err.contains("follower-a"),
            "names the failing endpoint: {err}"
        );
        assert!(err.contains("45781"), "names the failing URL: {err}");
    }

    #[tokio::test]
    async fn bad_endpoint_list_fails_before_any_network_call() {
        let args = args_for(&["only=http://127.0.0.1:45783"], 1);
        let err = run(&args).await.unwrap_err();
        assert!(err.contains("at least"), "{err}");
    }
}
