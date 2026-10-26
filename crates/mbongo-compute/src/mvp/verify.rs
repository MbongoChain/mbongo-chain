//! Client-side verification report (MVP-C §3, §20, §61).
//!
//! Three properties are checked and reported **independently**; none implies
//! another, and they are never collapsed into a single "verified":
//!
//! | Field | Means | Does not mean |
//! |---|---|---|
//! | `receipt_integrity` | an anchored receipt exists, its executor signature verifies, and it is bound to this task, its named executor and its input commitment | anything about the result bytes |
//! | `result_integrity` | the retrieved bytes recompute to the receipt's `output_commitment` (exact bytes, never a re-serialization) | that the bytes are well-formed |
//! | `schema_conformity` | the bytes are canonical JSON conforming to `kaayu-cv-profile:v2` | that the extracted facts are correct |
//! | `business_accuracy` | always `NOT_ASSESSED` in the MVP | — |
//!
//! `task_profile` records whether the committed task is the one asked for
//! (its recomputed `task_id`) and the frozen `mbongo-infer:v1` /
//! `kaayu-cv-extract:v2` task at all.
//!
//! Receipt-signature verification is `mbongo_verification::verify_receipt_signature`,
//! the same function the node uses; this module only orders the checks and
//! keeps their results apart.

use mbongo_core::{Address, ComputeTask, Receipt};
use mbongo_verification::verify_receipt_signature;
use serde::Serialize;

use crate::execution::reference_output_commitment;
use crate::inference::canonical;
use crate::inference::kaayu::MAX_OUTPUT_BYTES;
use crate::profiles::kaayu_v2;

use super::{exit, frozen_execution_spec};

/// The outcome of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "status",
    content = "reason",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum Check {
    /// The check ran and passed.
    Verified,
    /// The check ran and failed; the reason names what failed, never content.
    Failed(String),
    /// The material to check does not exist (yet).
    NotFound(String),
    /// Not assessed by Mbongo.
    NotAssessed,
}

impl Check {
    /// Whether this check passed.
    pub fn is_verified(&self) -> bool {
        matches!(self, Check::Verified)
    }

    /// `VERIFIED`, `FAILED`, `NOT_FOUND` or `NOT_ASSESSED`.
    pub fn status(&self) -> &'static str {
        match self {
            Check::Verified => "VERIFIED",
            Check::Failed(_) => "FAILED",
            Check::NotFound(_) => "NOT_FOUND",
            Check::NotAssessed => "NOT_ASSESSED",
        }
    }

    /// What failed or is missing, if anything.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Check::Failed(r) | Check::NotFound(r) => Some(r),
            _ => None,
        }
    }

    /// `VERIFIED`, `FAILED (…)`, `NOT FOUND (…)`, `NOT ASSESSED`.
    pub fn label(&self) -> String {
        match self {
            Check::Verified => "VERIFIED".into(),
            Check::Failed(r) => format!("FAILED ({r})"),
            Check::NotFound(r) => format!("NOT FOUND ({r})"),
            Check::NotAssessed => "NOT ASSESSED".into(),
        }
    }
}

/// The four independent properties, plus the task-profile precondition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationReport {
    /// Hex task id.
    pub task_id: String,
    /// The committed task is the frozen MVP inference task.
    pub task_profile: Check,
    /// Receipt signature and bindings.
    pub receipt_integrity: Check,
    /// Retrieved bytes ↔ receipt output commitment.
    pub result_integrity: Check,
    /// `kaayu-cv-profile:v2` conformity of the retrieved bytes.
    pub schema_conformity: Check,
    /// Never assessed in the MVP.
    pub business_accuracy: Check,
}

impl VerificationReport {
    /// Receipt and result both verified: the bytes are the ones the named
    /// executor committed to for this task.
    pub fn integrity_verified(&self) -> bool {
        self.task_profile.is_verified()
            && self.receipt_integrity.is_verified()
            && self.result_integrity.is_verified()
    }

    /// Integrity could not be decided yet: nothing failed, but the receipt
    /// or the result does not exist.
    pub fn pending(&self) -> bool {
        let checks = [
            &self.task_profile,
            &self.receipt_integrity,
            &self.result_integrity,
        ];
        !checks.iter().any(|c| matches!(c, Check::Failed(_)))
            && checks.iter().any(|c| matches!(c, Check::NotFound(_)))
    }

    /// The one-word outcome: `VERIFIED`, `NOT_READY`,
    /// `VERIFICATION_FAILED` or `SCHEMA_FAILED`.
    pub fn outcome(&self) -> &'static str {
        if self.integrity_verified() {
            if self.schema_conformity.is_verified() {
                "VERIFIED"
            } else {
                "SCHEMA_FAILED"
            }
        } else if self.pending() {
            "NOT_READY"
        } else {
            "VERIFICATION_FAILED"
        }
    }

    /// CLI exit code ([`exit`]). `business_accuracy` never affects it.
    pub fn exit_code(&self) -> i32 {
        match self.outcome() {
            "VERIFIED" => exit::SUCCESS,
            "SCHEMA_FAILED" => exit::SCHEMA_FAILED,
            "NOT_READY" => exit::NOT_READY,
            _ => exit::VERIFICATION_FAILED,
        }
    }
}

/// Evaluates the committed task, its anchored receipt (if found) and the
/// retrieved result bytes (if any), in the frozen order of
/// `docs/product/MVP_HTTP_CLI.md` §9: task, `task_id`, receipt signature,
/// task / executor / input bindings, result bytes, output commitment,
/// parse, schema.
pub fn evaluate(
    requested_task_id: [u8; 32],
    task: &ComputeTask,
    receipt: Option<&Receipt>,
    result: Option<&[u8]>,
    expected_executor: Option<Address>,
) -> VerificationReport {
    let task_id = task.task_id();
    let task_profile = if task_id != requested_task_id {
        Check::Failed("committed task does not hash to the requested task_id".into())
    } else if task.execution_spec != frozen_execution_spec() {
        Check::Failed("execution_spec is not mbongo-infer:v1 / kaayu-cv-extract:v2".into())
    } else {
        Check::Verified
    };

    let receipt_integrity = match receipt {
        None => Check::NotFound("no anchored receipt for this task".into()),
        Some(r) if !verify_receipt_signature(r) => Check::Failed("receipt signature".into()),
        Some(r) if r.task_id != requested_task_id || r.task_id != task_id => {
            Check::Failed("receipt task binding".into())
        }
        Some(r) if r.executor != task.executor => Check::Failed("receipt executor binding".into()),
        Some(_) if expected_executor.is_some_and(|e| e != task.executor) => {
            Check::Failed("task executor is not the configured MVP executor".into())
        }
        Some(r) if r.input_commitment != task.input_commitment => {
            Check::Failed("receipt input binding".into())
        }
        Some(_) => Check::Verified,
    };

    let result_integrity = match (receipt, result) {
        (None, _) => Check::NotFound("no anchored receipt to compare against".into()),
        (Some(_), None) => Check::NotFound("result not retrieved".into()),
        (Some(_), Some(_)) if !receipt_integrity.is_verified() => {
            Check::Failed("receipt not verified; its output commitment cannot be trusted".into())
        }
        (Some(r), Some(bytes)) if reference_output_commitment(bytes) == r.output_commitment => {
            Check::Verified
        }
        (Some(_), Some(_)) => Check::Failed("output commitment mismatch".into()),
    };

    let schema_conformity = match result {
        None => Check::NotFound("result not retrieved".into()),
        Some(bytes) => match std::str::from_utf8(bytes) {
            Err(_) => Check::Failed("result is not UTF-8".into()),
            Ok(text) => match kaayu_v2::parse_output(text) {
                Err(e) => Check::Failed(e.code().into()),
                Ok(_) if !canonical::is_canonical(bytes, MAX_OUTPUT_BYTES) => {
                    Check::Failed("result bytes are not canonical".into())
                }
                Ok(_) => Check::Verified,
            },
        },
    };

    VerificationReport {
        task_id: hex::encode(requested_task_id),
        task_profile,
        receipt_integrity,
        result_integrity,
        schema_conformity,
        business_accuracy: Check::NotAssessed,
    }
}
