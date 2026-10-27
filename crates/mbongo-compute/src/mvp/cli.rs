//! The `mbongo-mvp` commands, as library functions over any
//! [`ChainClient`] so the tests drive the exact code the binary runs.
//!
//! Every command writes its report to `out` and its progress and errors to
//! `err`, and returns an exit code ([`super::exit`]). Neither stream ever
//! carries CV text or the prompt; result content appears only when the user
//! asks for it (`--show-result`, `result --out`), after the integrity
//! report, and labelled as a display rendering.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mbongo_core::{Address, ComputeTask, Receipt};
use serde_json::json;

use crate::chain::{ChainClient, ChainError};
use crate::data_plane::LocalKey;
use crate::execution::reference_output_commitment;
use crate::inference::kaayu::CvExtractionInputV1;

use super::client::{
    build_task, find_anchored_receipt, find_task, is_not_yet, random_bytes, submit_task,
    ClientError, FrontClient,
};
use super::dto::StatusView;
use super::screen::{screen, ScreenReport, ScreenStatus};
use super::verify::{evaluate, Check, VerificationReport};
use super::{exit, RUNTIME_PROFILE};

/// The note shown with every verification report (MVP-C §25).
pub const BUSINESS_ACCURACY_NOTE: &str = "Business accuracy has not been evaluated by Mbongo: \
the receipt and the output commitment prove which executor answered and that these are the exact \
bytes it committed to, not that the extracted facts are correct.";

/// The human-review notice (MVP-E): extraction output is advisory.
pub const HUMAN_REVIEW_NOTE: &str = "Human review: this profile is AI-generated structured \
extraction. Review it before any recruiting action; never reject, rank or hire a candidate on it alone.";

/// What the CLI says when the private result has expired.
pub const RESULT_EXPIRED_NOTE: &str = "The private result has expired: the data plane keeps a \
result for 3,600 s after it is persisted and it cannot be recovered. The anchored receipt is \
unaffected and remains on-chain; its integrity is reported above.";

/// The provider-visibility disclosure (PUBLIC profile).
pub const PROVIDER_VISIBILITY_NOTE: &str = "Provider visibility: the Mbongo-operated worker \
processes the CV in plaintext (PUBLIC profile); the input and the result stay off-chain.";

/// What every command needs.
pub struct Context<'a, C> {
    /// The chain (`rpc_v0.3`).
    pub chain: &'a C,
    /// The MVP front.
    pub front: &'a FrontClient,
    /// The client's chain key: signs the `ComputeTask` and every request.
    pub key: &'a LocalKey,
    /// The configured MVP executor.
    pub executor: Option<Address>,
    /// Hourly infrastructure rate for the displayed reference cost.
    pub reference_usd_per_hour: Option<f64>,
    /// Poll interval while waiting.
    pub poll: Duration,
}

/// `submit` options.
#[derive(Debug, Clone, Copy)]
pub struct SubmitOptions {
    /// Wait for completion, then retrieve and verify.
    pub wait: bool,
    /// Bound on the whole wait.
    pub timeout: Duration,
    /// Print the extracted profile after the report.
    pub show_result: bool,
    /// Print the report as one JSON object.
    pub json: bool,
}

/// Unix milliseconds, this machine's clock.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn client_exit(e: &ClientError) -> i32 {
    match e {
        ClientError::Unavailable(_) | ClientError::Protocol(_) => exit::UNAVAILABLE,
        ClientError::Rejected { .. } => exit::REJECTED,
    }
}

fn chain_exit(e: &ChainError) -> i32 {
    match e {
        ChainError::Rejected(_) => exit::REJECTED,
        ChainError::Transport(_) => exit::UNAVAILABLE,
    }
}

fn parse_task_id(err: &mut dyn Write, s: &str) -> Result<[u8; 32], i32> {
    super::client::parse_hex32(s).ok_or_else(|| {
        let _ = writeln!(err, "error: task_id must be 64 hex characters");
        exit::USAGE
    })
}

/// The timings MVP-D records per job. Observations, not metering.
#[derive(Debug, Clone, Default)]
pub struct JobTimings {
    /// CLI clock, before the upload.
    pub submitted_at_ms: Option<u64>,
    /// CLI clock, when the task was seen in a block.
    pub task_included_at_ms: Option<u64>,
    /// CLI clock, when the front accepted the registration.
    pub registered_at_ms: Option<u64>,
    /// CLI clock, when the front reported `completed`.
    pub completed_at_ms: Option<u64>,
    /// CLI clock, after verification.
    pub verified_at_ms: Option<u64>,
}

struct Verified {
    report: VerificationReport,
    task: Option<ComputeTask>,
    task_height: Option<u64>,
    receipt: Option<(u64, Receipt)>,
    result: Option<Vec<u8>>,
    /// The front answered 410: the result outlived its data-plane lifetime.
    expired: bool,
    screen: ScreenReport,
}

impl Verified {
    /// The one-word outcome and the exit code. Integrity and schema decide
    /// `outcome` exactly as [`VerificationReport::outcome`]; an expired
    /// result and a flagged screen are reported on top, never instead.
    fn outcome(&self) -> (&'static str, i32) {
        let base = self.report.outcome();
        if self.expired && self.report.receipt_integrity.is_verified() {
            return ("RESULT_EXPIRED", exit::RESULT_EXPIRED);
        }
        if base == "VERIFIED" && self.screen.status == ScreenStatus::Flagged {
            return (base, exit::CONTENT_FLAGGED);
        }
        (base, self.report.exit_code())
    }
}

/// The active profile's system prompt, for the injection screen.
fn active_prompt() -> &'static str {
    crate::profiles::assets(&crate::profiles::active_job_spec()).map_or("", |(p, _)| p)
}

/// Client-side verification in the frozen order: the committed task from
/// the chain, its `task_id`, the anchored receipt from the chain, the result
/// bytes from the front; then [`evaluate`].
async fn run_verification<C: ChainClient>(
    ctx: &Context<'_, C>,
    task_id: [u8; 32],
    from_height: u64,
    cv_text: Option<&str>,
) -> Result<Verified, (i32, String)> {
    let observed = find_task(ctx.chain, task_id, from_height)
        .await
        .map_err(|e| (chain_exit(&e), format!("chain: {e}")))?;
    let Some(observed) = observed else {
        return Err((
            exit::NOT_READY,
            "committed task not found on chain".to_owned(),
        ));
    };
    let receipt = find_anchored_receipt(ctx.chain, task_id, observed.height)
        .await
        .map_err(|e| (chain_exit(&e), format!("chain: {e}")))?;
    let mut expired = false;
    let result = match ctx.front.result(task_id).await {
        Ok(bytes) => Some(bytes),
        Err(e) if is_not_yet(&e) => None,
        Err(e) if e.code() == Some("RESULT_EXPIRED") => {
            expired = true;
            None
        }
        Err(e) => return Err((client_exit(&e), e.to_string())),
    };
    let mut report = evaluate(
        task_id,
        &observed.task,
        receipt.as_ref().map(|(_, r)| r),
        result.as_deref(),
        ctx.executor,
    );
    if expired {
        report.result_integrity =
            Check::NotFound("private result expired; the anchored receipt is unaffected".into());
        report.schema_conformity = Check::NotFound("private result expired".into());
    }
    let screen = match (cv_text, result.as_deref()) {
        (Some(cv), Some(bytes)) => screen(cv, bytes, active_prompt()),
        _ => ScreenReport::not_run(),
    };
    Ok(Verified {
        report,
        task_height: Some(observed.height),
        task: Some(observed.task),
        receipt,
        result,
        expired,
        screen,
    })
}

fn secs(ms: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let s = ms as f64 / 1000.0;
    format!("{s:.1} s")
}

fn reference_cost(rate: Option<f64>, inference_ms: Option<u64>) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)]
    Some(rate? * inference_ms? as f64 / 3_600_000.0)
}

#[allow(clippy::too_many_lines)]
fn print_report<C>(
    ctx: &Context<'_, C>,
    out: &mut dyn Write,
    v: &Verified,
    status: Option<&StatusView>,
    timings: &JobTimings,
    json_mode: bool,
    show_result: bool,
) {
    let r = &v.report;
    let execution_status = status.map_or("unknown", |s| s.state.as_str());
    let inference_ms = status.and_then(|s| s.inference_ms);
    let cost = reference_cost(ctx.reference_usd_per_hour, inference_ms);
    let total_ms = match (timings.submitted_at_ms, timings.verified_at_ms) {
        (Some(a), Some(b)) => Some(b.saturating_sub(a)),
        _ => None,
    };
    let executor = v.task.as_ref().map(|t| t.executor.to_string());
    if json_mode {
        let check = |c: &super::verify::Check| json!({"status": c.status(), "reason": c.reason()});
        let mut doc = json!({
            "task_id": r.task_id,
            "execution_status": execution_status,
            "task_profile": check(&r.task_profile),
            "receipt_integrity": check(&r.receipt_integrity),
            "result_integrity": check(&r.result_integrity),
            "schema_conformity": check(&r.schema_conformity),
            "business_accuracy": check(&r.business_accuracy),
            "outcome": v.outcome().0,
            "injection_screen": {
                "status": v.screen.status,
                "findings": v.screen.findings,
                "instruction_like_lines": v.screen.instruction_like_lines,
            },
            "result_expired": v.expired,
            "human_review_required": true,
            "executor": executor,
            "runtime_profile": RUNTIME_PROFILE,
            "task_height": v.task_height,
            "receipt_height": v.receipt.as_ref().map(|(h, _)| *h),
            "result_bytes": v.result.as_ref().map(Vec::len),
            "attempts": status.map(|s| s.attempts),
            "failure_code": status.and_then(|s| s.failure_code.clone()),
            "timings": {
                "submitted_at_ms": timings.submitted_at_ms,
                "task_included_at_ms": timings.task_included_at_ms,
                "registered_at_ms": timings.registered_at_ms,
                "inference_started_at_ms": status.and_then(|s| s.started_at_ms),
                "inference_completed_at_ms": status.and_then(|s| s.completed_at_ms),
                "inference_ms": inference_ms,
                "completed_at_ms": timings.completed_at_ms,
                "verified_at_ms": timings.verified_at_ms,
                "total_ms": total_ms,
            },
            "input_tokens": status.and_then(|s| s.input_tokens),
            "output_tokens": status.and_then(|s| s.output_tokens),
            "reference_cost_usd": cost,
            "notes": [BUSINESS_ACCURACY_NOTE, HUMAN_REVIEW_NOTE, PROVIDER_VISIBILITY_NOTE],
        });
        if show_result {
            if let Some(bytes) = &v.result {
                doc["result_canonical"] = json!(String::from_utf8_lossy(bytes));
            }
        }
        let _ = writeln!(out, "{doc}");
        return;
    }
    let _ = writeln!(out, "Task: {}", r.task_id);
    let _ = writeln!(out, "Status: {execution_status}");
    let _ = writeln!(out, "Receipt integrity: {}", r.receipt_integrity.label());
    let _ = writeln!(out, "Result commitment: {}", r.result_integrity.label());
    let _ = writeln!(out, "Schema conformity: {}", r.schema_conformity.label());
    let _ = writeln!(
        out,
        "Business extraction accuracy: {}",
        r.business_accuracy.label()
    );
    let _ = writeln!(
        out,
        "Injection screen: {}",
        match v.screen.status {
            ScreenStatus::Clear =>
                "CLEAR (no instruction-following artifact detected; this is not an accuracy check)"
                    .to_owned(),
            ScreenStatus::Flagged => format!(
                "FLAGGED ({}): the result shows signs of following instructions embedded in the CV; review it before any use",
                v.screen.findings.join(", ")
            ),
            ScreenStatus::NotRun => "NOT RUN (the CV text was not provided: pass --input)".to_owned(),
        }
    );
    if !r.task_profile.is_verified() {
        let _ = writeln!(out, "Task profile: {}", r.task_profile.label());
    }
    if v.expired {
        let _ = writeln!(out, "{RESULT_EXPIRED_NOTE}");
    }
    if let Some(e) = &executor {
        let _ = writeln!(out, "Executor: {e}");
    }
    let _ = writeln!(out, "Runtime profile: {RUNTIME_PROFILE}");
    if let Some((h, _)) = &v.receipt {
        let _ = writeln!(out, "Receipt anchored at height: {h}");
    }
    let mut duration = Vec::new();
    if let Some(ms) = inference_ms {
        duration.push(format!("inference {}", secs(ms)));
    }
    if let Some(ms) = total_ms {
        duration.push(format!("submission to verified {}", secs(ms)));
    }
    if !duration.is_empty() {
        let _ = writeln!(out, "Duration: {}", duration.join("; "));
    }
    match cost {
        Some(c) => {
            let _ = writeln!(out, "Reference cost: US${c:.5} (display only, not charged)");
        }
        None => {
            let _ = writeln!(out, "Reference cost: not configured (display only)");
        }
    }
    let _ = writeln!(out, "{BUSINESS_ACCURACY_NOTE}");
    let _ = writeln!(out, "{PROVIDER_VISIBILITY_NOTE}");
    let _ = writeln!(out, "{HUMAN_REVIEW_NOTE}");
    let _ = writeln!(out, "OUTCOME={}", v.outcome().0);
    if show_result {
        if let Some(bytes) = &v.result {
            let pretty = serde_json::from_slice::<serde_json::Value>(bytes)
                .ok()
                .and_then(|j| serde_json::to_string_pretty(&j).ok());
            let _ = writeln!(
                out,
                "\nExtracted profile — display rendering, pretty-printed from the retrieved canonical \
bytes (result commitment: {}). The commitment covers the canonical bytes, not this rendering; \
its facts are not verified:",
                r.result_integrity.status()
            );
            let _ = writeln!(
                out,
                "{}",
                pretty.unwrap_or_else(|| "(result is not JSON)".into())
            );
        }
    }
}

/// `submit`: validate the CV text, upload the private input, commit the
/// frozen task on-chain, register it; with `wait`, poll, retrieve, verify.
#[allow(clippy::too_many_lines)]
pub async fn submit<C: ChainClient>(
    ctx: &Context<'_, C>,
    cv_text: String,
    opts: SubmitOptions,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let started = Instant::now();
    let mut timings = JobTimings {
        submitted_at_ms: Some(now_ms()),
        ..JobTimings::default()
    };
    let Some(executor) = ctx.executor else {
        let _ = writeln!(
            err,
            "error: no executor configured (MBONGO_MVP_EXECUTOR or --executor)"
        );
        return exit::CONFIG;
    };
    let input = match CvExtractionInputV1::new(cv_text) {
        Ok(i) => i,
        Err(e) => {
            let _ = writeln!(err, "error: input refused: {}", e.code());
            return exit::CONFIG;
        }
    };
    let commitment = input.input_commitment();
    let task = build_task(
        ctx.key.address(),
        executor,
        random_bytes::<32>(),
        commitment,
    );
    let task_id = task.task_id();
    let _ = writeln!(err, "task_id {}", hex::encode(task_id));

    // 1. Private input → front (off-chain).
    let upload = match ctx.front.upload(task_id, &input.cv_text).await {
        Ok(u) => u,
        Err(e) => {
            let _ = writeln!(err, "error: upload: {e}");
            return client_exit(&e);
        }
    };
    let cv_text = input.cv_text.clone();
    drop(input);
    if upload.input_commitment != hex::encode(commitment) {
        let _ = writeln!(
            err,
            "error: the front computed a different input commitment; task not submitted"
        );
        return exit::VERIFICATION_FAILED;
    }
    let _ = writeln!(
        err,
        "input uploaded: {} canonical bytes, input_commitment {}",
        upload.input_bytes, upload.input_commitment
    );

    // 2. The committed task → chain.
    let start_height = match ctx.chain.latest_height().await {
        Ok(h) => h,
        Err(e) => {
            let _ = writeln!(err, "error: chain: {e}");
            return chain_exit(&e);
        }
    };
    match submit_task(ctx.chain, ctx.key, &task).await {
        Ok(hash) => {
            let _ = writeln!(err, "ComputeTask submitted: tx {hash}");
        }
        Err(e) => {
            let _ = writeln!(err, "error: ComputeTask: {e}");
            return chain_exit(&e);
        }
    }
    let task_height = loop {
        match find_task(ctx.chain, task_id, start_height).await {
            Ok(Some(t)) => break t.height,
            Ok(None) => {}
            Err(e) => {
                let _ = writeln!(err, "error: chain: {e}");
                return chain_exit(&e);
            }
        }
        if started.elapsed() > opts.timeout.min(Duration::from_secs(180)) {
            let _ = writeln!(err, "error: task not included in a block in time");
            return exit::NOT_READY;
        }
        tokio::time::sleep(ctx.poll).await;
    };
    timings.task_included_at_ms = Some(now_ms());
    let _ = writeln!(err, "task committed at height {task_height}");

    // 3. Registration with the control plane, through the front.
    let status = loop {
        match ctx.front.register(task_id, &upload.object_ref, executor).await {
            Ok(s) => break s,
            Err(e) if is_not_yet(&e) && started.elapsed() < opts.timeout => {
                tokio::time::sleep(ctx.poll).await;
            }
            Err(e) => {
                let _ = writeln!(err, "error: register: {e}");
                return client_exit(&e);
            }
        }
    };
    timings.registered_at_ms = Some(now_ms());
    let _ = writeln!(err, "registered: state {}", status.state);
    if !opts.wait {
        if opts.json {
            let _ = writeln!(
                out,
                "{}",
                json!({"task_id": hex::encode(task_id), "task_height": task_height, "state": status.state})
            );
        } else {
            let _ = writeln!(out, "Task: {}", hex::encode(task_id));
            let _ = writeln!(out, "Status: {}", status.state);
        }
        return exit::SUCCESS;
    }

    // 4. Bounded wait.
    let mut last = String::new();
    let status = loop {
        match ctx.front.status(task_id).await {
            Ok(s) => {
                let line = format!("state {} attempts {}", s.state, s.attempts);
                if line != last {
                    let _ = writeln!(err, "[{:>6.1}s] {line}", started.elapsed().as_secs_f64());
                    last = line;
                }
                if s.state == "completed" {
                    break s;
                }
                if s.attempts_exhausted {
                    let _ = writeln!(
                        err,
                        "error: task failed after {} attempts ({})",
                        s.attempts,
                        s.failure_code
                            .as_deref()
                            .or(s.last_failure.as_deref())
                            .unwrap_or("unknown")
                    );
                    return exit::TASK_FAILED;
                }
            }
            Err(e) => {
                let _ = writeln!(err, "status: {e}");
            }
        }
        if started.elapsed() > opts.timeout {
            let _ = writeln!(err, "error: timed out waiting for completion");
            return exit::NOT_READY;
        }
        tokio::time::sleep(ctx.poll).await;
    };
    timings.completed_at_ms = Some(now_ms());

    // 5. Client-side verification.
    match run_verification(ctx, task_id, task_height, Some(&cv_text)).await {
        Ok(v) => {
            timings.verified_at_ms = Some(now_ms());
            print_report(
                ctx,
                out,
                &v,
                Some(&status),
                &timings,
                opts.json,
                opts.show_result,
            );
            v.outcome().1
        }
        Err((code, msg)) => {
            let _ = writeln!(err, "error: {msg}");
            code
        }
    }
}

/// `status`: the front's lifecycle view.
pub async fn status<C>(
    ctx: &Context<'_, C>,
    task_id: &str,
    json_mode: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let task_id = match parse_task_id(err, task_id) {
        Ok(t) => t,
        Err(code) => return code,
    };
    match ctx.front.status(task_id).await {
        Ok(s) => {
            if json_mode {
                let _ = writeln!(out, "{}", serde_json::to_string(&s).unwrap_or_default());
            } else {
                let _ = writeln!(out, "Task: {}", s.task_id);
                let _ = writeln!(
                    out,
                    "Status: {}{}",
                    s.state,
                    if s.worker_busy { " (worker busy)" } else { "" }
                );
                let _ = writeln!(out, "Attempts: {}", s.attempts);
                if let Some(f) = s.failure_code.as_ref().or(s.last_failure.as_ref()) {
                    let _ = writeln!(out, "Last failure: {f}");
                }
                if let Some(ms) = s.inference_ms {
                    let _ = writeln!(
                        out,
                        "Inference: {} (finish {}, tokens in {} / out {})",
                        secs(ms),
                        s.finish_reason.as_deref().unwrap_or("-"),
                        s.input_tokens.map_or("-".into(), |t| t.to_string()),
                        s.output_tokens.map_or("-".into(), |t| t.to_string())
                    );
                }
                let _ = writeln!(
                    out,
                    "Result available: {}",
                    if s.result_available { "yes" } else { "no" }
                );
                let _ = writeln!(
                    out,
                    "Receipt: {}",
                    match (s.receipt_height, s.receipt_submitted) {
                        (Some(h), _) =>
                            format!("observed at height {h} (not verified here: run verify)"),
                        (None, true) => "submitted, not yet observed in a block".into(),
                        (None, false) => "none yet".into(),
                    }
                );
            }
            exit::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(err, "error: status: {e}");
            if e.code() == Some("TASK_NOT_FOUND") {
                exit::NOT_READY
            } else {
                client_exit(&e)
            }
        }
    }
}

/// `result`: the exact canonical bytes. Integrity is not checked here —
/// `verify` checks it — so the command says so.
pub async fn result<C>(
    ctx: &Context<'_, C>,
    task_id: &str,
    out_file: Option<&Path>,
    show: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let task_id = match parse_task_id(err, task_id) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let bytes = match ctx.front.result(task_id).await {
        Ok(b) => b,
        Err(e) => {
            if e.code() == Some("RESULT_EXPIRED") {
                let _ = writeln!(
                    err,
                    "{RESULT_EXPIRED_NOTE} Run verify to check the receipt."
                );
                return exit::RESULT_EXPIRED;
            }
            let _ = writeln!(err, "error: result: {e}");
            return if is_not_yet(&e) {
                exit::NOT_READY
            } else {
                client_exit(&e)
            };
        }
    };
    let _ = writeln!(
        err,
        "result: {} bytes, recomputed output commitment {} (not compared with the receipt: run verify)",
        bytes.len(),
        hex::encode(reference_output_commitment(&bytes))
    );
    if let Some(path) = out_file {
        if let Err(e) = std::fs::write(path, &bytes) {
            let _ = writeln!(err, "error: cannot write {}: {}", path.display(), e.kind());
            return exit::CONFIG;
        }
        let _ = writeln!(err, "exact canonical bytes written to {}", path.display());
    }
    if show {
        let _ = out.write_all(&bytes);
        let _ = writeln!(out);
    }
    exit::SUCCESS
}

/// `verify`: receipt and result integrity, and schema conformity,
/// reported separately.
#[allow(clippy::too_many_arguments)]
pub async fn verify<C: ChainClient>(
    ctx: &Context<'_, C>,
    task_id: &str,
    from_height: u64,
    cv_text: Option<&str>,
    json_mode: bool,
    show_result: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let task_id = match parse_task_id(err, task_id) {
        Ok(t) => t,
        Err(code) => return code,
    };
    let status = ctx.front.status(task_id).await.ok();
    match run_verification(ctx, task_id, from_height, cv_text).await {
        Ok(v) => {
            let timings = JobTimings {
                verified_at_ms: Some(now_ms()),
                ..JobTimings::default()
            };
            print_report(
                ctx,
                out,
                &v,
                status.as_ref(),
                &timings,
                json_mode,
                show_result,
            );
            v.outcome().1
        }
        Err((code, msg)) => {
            let _ = writeln!(err, "error: {msg}");
            code
        }
    }
}
