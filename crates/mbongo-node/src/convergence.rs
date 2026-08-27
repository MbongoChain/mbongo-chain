//! Shared devnet convergence primitives.
//!
//! Single source of truth for how a Mbongo devnet is probed over JSON-RPC
//! and for what "converged" means. Used by both:
//!
//! - `devnet_harness` — against the nodes it spawns itself;
//! - `convergence_probe` — against nodes that are already running and
//!   reachable over the network (e.g. a Docker Compose devnet).
//!
//! This module performs **no** process management: it only speaks JSON-RPC
//! to the endpoints it is given. Lifecycle (spawning, killing, restarting)
//! belongs to the caller.

use std::time::Duration;

use reqwest::Client;
use serde_json::{json, Value};
use tokio::time::sleep;

// ── Endpoints ───────────────────────────────────────────────────────────

/// A node JSON-RPC endpoint: a display name plus its base URL.
///
/// The name is only used for diagnostics; the base URL is what is actually
/// dialed. Keeping them together means every error message can point at the
/// endpoint that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEndpoint {
    name: String,
    base_url: String,
}

impl NodeEndpoint {
    /// Builds an endpoint from a base URL (e.g. `http://producer:9944`).
    /// A trailing slash is trimmed so [`NodeEndpoint::rpc_url`] is stable.
    pub fn new(name: impl Into<String>, base_url: impl Into<String>) -> Self {
        let base = base_url.into();
        Self {
            name: name.into(),
            base_url: base.trim_end_matches('/').to_string(),
        }
    }

    /// Builds a loopback endpoint for `port` — the addressing the in-process
    /// harness uses for the nodes it spawns (`http://127.0.0.1:{port}`).
    pub fn localhost_port(name: impl Into<String>, port: u16) -> Self {
        Self::new(name, format!("http://127.0.0.1:{port}"))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The JSON-RPC URL this endpoint is queried on.
    pub fn rpc_url(&self) -> String {
        format!("{}/rpc", self.base_url)
    }
}

// ── RPC helpers ─────────────────────────────────────────────────────────

pub async fn rpc_call(client: &Client, node: &NodeEndpoint, method: &str) -> Result<Value, String> {
    rpc_call_with_params(client, node, method, None).await
}

pub async fn rpc_call_with_params(
    client: &Client,
    node: &NodeEndpoint,
    method: &str,
    params: Option<Value>,
) -> Result<Value, String> {
    let url = node.rpc_url();
    let mut body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "id": 1
    });
    if let Some(p) = params {
        body["params"] = p;
    }

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP request to {url} failed: {e}"))?;

    let json: Value =
        resp.json().await.map_err(|e| format!("failed to parse JSON response: {e}"))?;

    if let Some(err) = json.get("error") {
        return Err(format!("RPC error: {err}"));
    }

    json.get("result")
        .cloned()
        .ok_or_else(|| "missing 'result' in RPC response".to_string())
}

pub async fn get_height(client: &Client, node: &NodeEndpoint) -> Result<u64, String> {
    let result = rpc_call(client, node, "get_block_height").await?;
    result.as_u64().ok_or_else(|| format!("expected u64 height, got: {result}"))
}

pub async fn get_tip_hash(client: &Client, node: &NodeEndpoint) -> Result<String, String> {
    let result = rpc_call(client, node, "get_latest_block_hash").await?;
    result
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("expected string hash, got: {result}"))
}

// ── Readiness ───────────────────────────────────────────────────────────

/// Waits until every endpoint answers `ping`, bounded by `timeout`.
///
/// Process-free: it neither starts nor inspects any child process, so it
/// works against nodes owned by someone else (containers, an already
/// running devnet). On timeout the error names every endpoint that never
/// answered, with its last transport error, so the failing node is
/// identifiable from the exit output alone.
pub async fn await_endpoints_ready(
    client: &Client,
    nodes: &[NodeEndpoint],
    poll_interval: Duration,
    timeout: Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    let mut pending: Vec<(&NodeEndpoint, String)> =
        nodes.iter().map(|n| (n, "not probed yet".to_string())).collect();

    loop {
        let mut still_pending: Vec<(&NodeEndpoint, String)> = Vec::new();
        for (node, _) in pending {
            if let Err(e) = rpc_call(client, node, "ping").await {
                still_pending.push((node, e));
            }
        }
        pending = still_pending;

        if pending.is_empty() {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            let detail = pending
                .iter()
                .map(|(n, e)| format!("    {} ({}): {e}", n.name(), n.rpc_url()))
                .collect::<Vec<_>>()
                .join("\n");
            let pending_count = pending.len();
            let total = nodes.len();
            return Err(format!(
                "readiness timeout after {:.1}s \
                 ({pending_count} of {total} endpoint(s) never answered ping):\n{detail}",
                start.elapsed().as_secs_f64()
            ));
        }
        sleep(poll_interval).await;
    }
}

// ── Convergence validation ──────────────────────────────────────────────

/// One observation of every node height and tip hash.
pub struct ConvergenceResult {
    heights: Vec<(String, u64)>,
    hashes: Vec<(String, String)>,
}

pub async fn check_convergence(
    client: &Client,
    nodes: &[NodeEndpoint],
) -> Result<ConvergenceResult, String> {
    let mut heights = Vec::new();
    let mut hashes = Vec::new();

    for node in nodes {
        let h = get_height(client, node).await?;
        let hash = get_tip_hash(client, node).await?;
        heights.push((node.name().to_string(), h));
        hashes.push((node.name().to_string(), hash));
    }

    Ok(ConvergenceResult { heights, hashes })
}

/// Returns `None` if all nodes agree (identical height >= `min_height`,
/// identical tip hash), otherwise the reason they do not.
pub fn convergence_error(result: &ConvergenceResult, min_height: u64) -> Option<String> {
    // Check minimum height.
    for (name, h) in &result.heights {
        if *h < min_height {
            return Some(format!("{name} height {h} < minimum expected {min_height}"));
        }
    }

    // Check all heights equal.
    let first_height = result.heights[0].1;
    for (name, h) in &result.heights[1..] {
        if *h != first_height {
            return Some(format!(
                "height mismatch: {} has {}, {} has {first_height}",
                name, h, result.heights[0].0
            ));
        }
    }

    // Check all hashes equal.
    let first_hash = &result.hashes[0].1;
    for (name, hash) in &result.hashes[1..] {
        if hash != first_hash {
            return Some(format!(
                "tip hash mismatch: {} has {}, {} has {first_hash}",
                name, hash, result.hashes[0].0
            ));
        }
    }

    None
}

/// Format the observed per-node state for display.
pub fn format_state(result: &ConvergenceResult) -> String {
    result
        .heights
        .iter()
        .zip(&result.hashes)
        .map(|((name, h), (_, hash))| format!("    {name}: height={h}, hash={hash}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Poll all nodes until they report identical height (>= `min_height`) and
/// identical tip hash. Retries every `poll_interval` with an overall
/// `timeout`. Transient RPC errors are retried until the deadline.
///
/// Returns the converged height so callers can derive their own height
/// floor from it.
///
/// On timeout, reports the required floor, the last observed height and
/// hash for every node, the elapsed time, and the number of attempts.
pub async fn await_convergence(
    client: &Client,
    nodes: &[NodeEndpoint],
    min_height: u64,
    poll_interval: Duration,
    timeout: Duration,
) -> Result<u64, String> {
    let start = std::time::Instant::now();
    let mut attempts: u32 = 0;
    let mut last_state: Option<ConvergenceResult> = None;

    loop {
        attempts += 1;
        let failure = match check_convergence(client, nodes).await {
            Ok(conv) => match convergence_error(&conv, min_height) {
                None => {
                    let height = conv.heights[0].1;
                    let hash = conv.hashes[0].1.clone();
                    println!("{}", format_state(&conv));
                    println!(
                        "  Converged: height={height}, hash={hash} \
                         (floor {min_height}, after {attempts} attempts, {:.1}s)",
                        start.elapsed().as_secs_f64()
                    );
                    return Ok(height);
                }
                Some(reason) => {
                    last_state = Some(conv);
                    reason
                }
            },
            Err(e) => e,
        };

        if start.elapsed() >= timeout {
            let state = last_state
                .as_ref()
                .map_or_else(|| "    (no state observed)".to_string(), format_state);
            let deadline_secs = timeout.as_secs();
            return Err(format!(
                "convergence timeout after {:.1}s ({attempts} attempts, \
                 deadline {deadline_secs}s, required floor {min_height})\n\
                   last observed state:\n{state}\n\
                   last failure reason: {failure}",
                start.elapsed().as_secs_f64()
            ));
        }
        sleep(poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `ConvergenceResult` from `(name, height, hash)` tuples.
    fn state(entries: &[(&str, u64, &str)]) -> ConvergenceResult {
        ConvergenceResult {
            heights: entries.iter().map(|(n, h, _)| ((*n).to_string(), *h)).collect(),
            hashes: entries
                .iter()
                .map(|(n, _, hash)| ((*n).to_string(), (*hash).to_string()))
                .collect(),
        }
    }

    #[test]
    fn equal_state_below_floor_fails() {
        let conv = state(&[("a", 5, "0xaa"), ("b", 5, "0xaa"), ("c", 5, "0xaa")]);
        assert!(convergence_error(&conv, 6).is_some());
    }

    #[test]
    fn equal_state_at_old_restart_height_fails() {
        // Restart scenario: all nodes agree on the pre-restart tip (height
        // 10), but the phase floor is baseline + 1 = 11. Stale agreement
        // must not pass.
        let conv = state(&[("a", 10, "0xold"), ("b", 10, "0xold"), ("c", 10, "0xold")]);
        assert!(convergence_error(&conv, 11).is_some());
    }

    #[test]
    fn equal_state_above_floor_passes() {
        let conv = state(&[("a", 12, "0xbb"), ("b", 12, "0xbb"), ("c", 12, "0xbb")]);
        assert!(convergence_error(&conv, 11).is_none());
    }

    #[test]
    fn hash_mismatch_at_equal_height_fails() {
        let conv = state(&[("a", 12, "0xbb"), ("b", 12, "0xbb"), ("c", 12, "0xcc")]);
        assert!(convergence_error(&conv, 5).is_some());
    }

    #[test]
    fn height_mismatch_fails() {
        let conv = state(&[("a", 13, "0xbb"), ("b", 12, "0xbb"), ("c", 13, "0xbb")]);
        assert!(convergence_error(&conv, 5).is_some());
    }

    #[test]
    fn endpoint_rpc_url_is_stable() {
        // A trailing slash must not produce a double slash in the RPC URL.
        assert_eq!(
            NodeEndpoint::new("a", "http://producer:9944/").rpc_url(),
            "http://producer:9944/rpc"
        );
        assert_eq!(
            NodeEndpoint::new("a", "http://producer:9944").rpc_url(),
            "http://producer:9944/rpc"
        );
    }

    #[test]
    fn localhost_endpoint_matches_historical_harness_addressing() {
        // The harness historically dialed http://127.0.0.1:{port}/rpc.
        assert_eq!(
            NodeEndpoint::localhost_port("producer", 19944).rpc_url(),
            "http://127.0.0.1:19944/rpc"
        );
    }
}
