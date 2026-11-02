//! Mbongo Compute Conformance — `compute-conformance-v1`.
//!
//! A named, reusable suite of behavioural tests for any implementation of
//! the compute control plane, private data plane and worker contracts
//! ([E] and [F]) under the privacy architecture ([P]). It derives every
//! case from merged authority and invents no requirement of its own.
//!
//! The suite talks to the implementation under test through one adapter,
//! [`Subject`], in the vocabulary of the contracts — actors, tasks,
//! leases, grants, presentations, outcomes — and never through the
//! reference implementation's types. It does not depend on the
//! reverse-bytes profile, on in-memory storage, on a single process, on a
//! GPU vendor or on confidential hardware. A CPU worker, a GPU worker, an
//! inference worker, a persistent data plane and a distributed control
//! plane are evaluated by implementing [`Subject`] and running
//! [`run_all`]; the reference implementation's adapter is
//! [`reference::ReferenceSubject`].
//!
//! What a green run means: the subject *behaves* as the contracts require
//! in every case below. What it does not mean: that the subject is
//! confidential (an ordinary provider sees plaintext, P8), that its
//! outputs are correct (P16), or that it is exactly-once (E12). The suite
//! fails a subject that claims any of those.
//!
//! [E]: ../../../docs/architecture/compute-control-plane-worker-interface.md
//! [F]: ../../../docs/architecture/compute-private-data-plane-interface.md
//! [P]: ../../../docs/architecture/compute-privacy-data-plane.md

pub mod reference;

use std::fmt::Write as _;

use mbongo_core::{Address, ComputeTask, Receipt, Transaction, TransactionPayload};
use mbongo_verification::verify_receipt_signature;
use parity_scale_codec::Encode;

use crate::chain::ChainClient;

/// The suite version. Implementation-level; not protocol authority.
pub const CONFORMANCE_VERSION: &str = "compute-conformance-v1";

/// The authorities this suite version tests against, by repository path.
pub const AUTHORITIES: &[&str] = &[
    "docs/architecture/compute-privacy-data-plane.md (P1–P18)",
    "docs/architecture/compute-private-data-plane-interface.md (F1–F24)",
    "docs/architecture/compute-control-plane-worker-interface.md (E1–E30)",
    "docs/rfcs/0005-compute-task-commitment-v1.md (§2.1, §2.4, §2.6, §9.2, rule s)",
    "docs/rfcs/0002-receipt-anchoring-v0.3.md (rule g)",
    "docs/specs/RECEIPT_SPEC_v0.1.md",
];

// ── vocabulary ───────────────────────────────────────────────────────────

/// A party in a scenario. The subject holds every key; the suite only
/// names roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    /// The executor the scenario task names.
    Executor,
    /// Another executor: a legitimate worker for other tasks, never for
    /// this one.
    OtherExecutor,
    /// The task's submitter and the owner of its private objects.
    Client,
    /// Nobody: a key no task or object has ever named.
    Stranger,
}

/// A data-plane operation a grant may permit (F §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantOp {
    /// Read the private input.
    FetchInput,
    /// Store the private result.
    PutResult,
    /// Read the private result.
    GetResult,
}

/// How the suite asks a subject to forge a grant: the same grant with one
/// binding changed and nothing re-authorised.
#[derive(Debug, Clone)]
pub enum Retarget {
    /// Point the grant at another task.
    Task([u8; 32]),
    /// Name another presenter.
    Presenter(Actor),
}

/// Where a worker instance is made to fail. A subject that cannot inject
/// a fault returns `false` from [`Subject::inject`] and the case is
/// reported as unsupported, never as passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    /// Crash with a lease and a grant obtained, before presenting.
    CrashBeforeFetch,
    /// Crash after the input is fetched (grant consumed), before verifying.
    CrashAfterFetch,
    /// Crash after execution started, before the result is persisted.
    CrashDuringExecution,
    /// Crash after the result is persisted, before any receipt exists.
    CrashAfterPersist,
    /// Crash after the anchoring transaction was submitted.
    CrashAfterSubmit,
    /// Flip one bit of the fetched input before verification.
    CorruptInput,
}

/// Why an attempt failed, as the worker classifies it (E §20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// Input unavailable or commitment mismatch.
    Input,
    /// Profile unsupported or execution failed.
    Execution,
    /// Result could not be persisted.
    Persistence,
    /// Receipt could not be submitted.
    Receipt,
}

/// What one worker pass did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing was offered to this instance.
    Idle,
    /// An anchoring transaction was submitted for the task.
    Submitted([u8; 32]),
    /// A receipt for the task is observed on-chain.
    Completed([u8; 32]),
    /// The attempt failed at the given stage.
    Failed([u8; 32], FailureKind),
    /// An injected crash.
    Crashed([u8; 32]),
    /// The pass could not proceed: coordination or the data plane refused.
    Aborted(String),
}

/// How an operation was refused. Both are refusals; the suite records
/// which, because "there is no such path" is stronger evidence than
/// "the path exists and said no".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The subject has the operation and denied it.
    Denied(String),
    /// The subject has no operation that could even attempt it.
    NoSuchPath(String),
}

/// What a subject claims about itself. The suite checks the claims it
/// can and fails the ones it cannot evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claims {
    /// Whether the subject claims exactly-once execution. Must be `false`
    /// (E12).
    pub exactly_once: bool,
    /// Whether the subject claims that the provider cannot see plaintext.
    /// `compute-conformance-v1` cannot evaluate this claim and fails a
    /// subject that makes it (P8, P9, F13, E21).
    pub confidential_execution: bool,
}

/// Who is being tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectInfo {
    /// Implementation name, for the report.
    pub name: String,
    /// The `execution_spec` tag of the profile the scenario uses.
    pub profile_tag: String,
    /// Self-description.
    pub claims: Claims,
}

/// The adapter a subject implements. Every method is expressed in the
/// contracts' vocabulary; opaque associated types carry whatever the
/// implementation uses for a grant, a lease, a locator or an instance.
///
/// A fresh subject is one scenario: one client, one private input, one
/// task naming `Actor::Executor`, committed by [`Subject::commit_task`].
/// The suite builds a new subject per case.
#[allow(async_fn_in_trait)]
pub trait Subject {
    /// The chain the subject anchors to. The suite scans it directly.
    type Chain: ChainClient;
    /// A data-plane grant as issued.
    type Grant: Clone + std::fmt::Debug;
    /// An execution lease as issued, with whatever the worker needs to use it.
    type Lease: Clone + std::fmt::Debug;
    /// A private object locator.
    type Locator: Clone + std::fmt::Debug + PartialEq;
    /// One running worker process.
    type Instance;

    /// Self-description.
    fn info(&self) -> SubjectInfo;
    /// The chain.
    fn chain(&self) -> &Self::Chain;
    /// Include everything pending in a new block (or wait for one).
    fn produce_block(&mut self);
    /// Move the subject's clock forward.
    fn advance_time(&mut self, secs: u64);
    /// How long a lease lives without a heartbeat.
    fn lease_ttl_secs(&self) -> u64;
    /// How long a data-plane grant lives once issued.
    fn grant_ttl_secs(&self) -> u64;

    // ── the scenario ──────────────────────────────────────────────────

    /// The private input the client stored. Must never reach the chain.
    fn private_input(&self) -> Vec<u8>;
    /// The output the profile yields for the private input. Used only to
    /// check the owner's retrieval and the absence of those bytes on-chain.
    fn expected_output(&self) -> Vec<u8>;
    /// The scenario task, as committed.
    fn task(&self) -> ComputeTask;
    /// The locator of the private input object.
    fn input_locator(&self) -> Self::Locator;
    /// The public identity of an actor.
    fn address(&self, actor: Actor) -> Address;
    /// Commits the scenario task on-chain and lets coordination observe
    /// it, with whatever confirmation policy the subject applies.
    async fn commit_task(&mut self);
    /// Commits a second task for the same executor whose `execution_spec`
    /// carries the private input bytes and for which **no** private object
    /// is stored. Returns its `task_id`. The subject must not special-case
    /// it. Used to prove that the worker does not source input from a
    /// public payload field (P15).
    async fn commit_task_carrying_input_in_spec(&mut self) -> [u8; 32];

    // ── worker instances ──────────────────────────────────────────────

    /// A new worker process holding `actor`'s key.
    fn new_instance(&mut self, actor: Actor) -> Self::Instance;
    /// Arms a fault on the instance's next pass. `false` if unsupported.
    fn inject(&mut self, instance: &mut Self::Instance, fault: FaultPoint) -> bool;
    /// Makes the data plane's next result store fail. `false` if unsupported.
    fn inject_result_store_failure(&mut self) -> bool;
    /// Makes the chain lose the response to the next submission. `false`
    /// if unsupported.
    fn inject_lost_submission_response(&mut self) -> bool;
    /// One pass: session, offer, attempt.
    async fn run(&mut self, instance: &mut Self::Instance) -> Outcome;

    // ── coordination (E) ──────────────────────────────────────────────

    /// Authenticates the instance and asks for work. `None` if nothing is
    /// offered.
    fn obtain_lease(&mut self, instance: &mut Self::Instance) -> Option<Self::Lease>;
    /// Asks the control plane for a grant under a lease (E §7 step 4/8).
    fn grant_under_lease(
        &mut self,
        instance: &Self::Instance,
        lease: &Self::Lease,
        op: GrantOp,
    ) -> Result<Self::Grant, Refusal>;
    /// Reports that execution started, under the lease.
    fn report_started(
        &mut self,
        instance: &Self::Instance,
        lease: &Self::Lease,
    ) -> Result<(), Refusal>;
    /// Renews the lease.
    fn heartbeat(&mut self, instance: &Self::Instance, lease: &Self::Lease) -> Result<(), Refusal>;
    /// How many attempts coordination has issued for the task.
    fn attempt_count(&self, task_id: [u8; 32]) -> u32;
    /// The executor coordination believes the task names.
    fn executor_of_record(&self, task_id: [u8; 32]) -> Option<Address>;

    // ── the data plane (F) ────────────────────────────────────────────

    /// The owner, or its delegated issuer, asks for a grant naming
    /// `presenter` for `op` on the scenario task, outside any lease.
    fn owner_side_grant(&mut self, presenter: Actor, op: GrantOp) -> Result<Self::Grant, Refusal>;
    /// `actor` presents the grant with a fresh proof of possession and
    /// fetches the input.
    fn present_fetch(&mut self, actor: Actor, grant: &Self::Grant) -> Result<Vec<u8>, Refusal>;
    /// Replays, byte for byte, the last presentation made of this grant.
    fn replay_last_presentation(&mut self, grant: &Self::Grant) -> Result<Vec<u8>, Refusal>;
    /// Presents the grant with no proof of possession at all.
    fn present_fetch_unproven(&mut self, grant: &Self::Grant) -> Result<Vec<u8>, Refusal>;
    /// `actor` presents a put grant and stores `bytes` as the result.
    fn present_put(
        &mut self,
        actor: Actor,
        grant: &Self::Grant,
        bytes: Vec<u8>,
    ) -> Result<(), Refusal>;
    /// `actor` presents a get grant and reads the result.
    fn present_get(&mut self, actor: Actor, grant: &Self::Grant) -> Result<Vec<u8>, Refusal>;
    /// `actor` tries to fetch the input knowing only the task id.
    fn fetch_with_task_id_only(
        &mut self,
        actor: Actor,
        task_id: [u8; 32],
    ) -> Result<Vec<u8>, Refusal>;
    /// `actor` tries to fetch the input knowing only the locator.
    fn fetch_with_locator_only(
        &mut self,
        actor: Actor,
        locator: &Self::Locator,
    ) -> Result<Vec<u8>, Refusal>;
    /// `actor` tries to fetch the input holding a lease and no grant.
    fn fetch_with_lease_only(
        &mut self,
        actor: Actor,
        lease: &Self::Lease,
    ) -> Result<Vec<u8>, Refusal>;
    /// `actor` tries to read the result knowing only the task id.
    fn result_with_task_id_only(
        &mut self,
        actor: Actor,
        task_id: [u8; 32],
    ) -> Result<Vec<u8>, Refusal>;
    /// The issuer revokes the grant.
    fn revoke(&mut self, grant: &Self::Grant) -> Result<(), Refusal>;
    /// The same grant with one binding changed, nothing re-signed.
    fn retarget(&self, grant: &Self::Grant, how: Retarget) -> Self::Grant;
    /// Whether the input object has been consumed by a fetch.
    fn input_consumed(&self, locator: &Self::Locator) -> bool;
    /// Whether a durable result exists for the task.
    fn result_exists(&self, task_id: [u8; 32]) -> bool;
    /// The commitment of the durable result, if one exists.
    fn result_commitment(&self, task_id: [u8; 32]) -> Option<[u8; 32]>;
    /// The owner retrieves the result through the subject's authorised path.
    fn owner_retrieve_result(&mut self, task_id: [u8; 32]) -> Result<Vec<u8>, Refusal>;

    // ── boundaries ────────────────────────────────────────────────────

    /// The receipt a control plane component can build for the task with
    /// the keys it holds. The suite checks that it does not verify as the
    /// executor's.
    fn control_plane_receipt(&mut self, output_commitment: [u8; 32]) -> Receipt;
    /// The anchoring transaction the control plane can build for that
    /// receipt, naming the executor as sender.
    fn control_plane_anchor_transaction(&mut self, receipt: Receipt) -> Transaction;
    /// The legitimate executor anchors a receipt for an arbitrary output
    /// commitment that no execution produced.
    async fn executor_anchor_arbitrary(
        &mut self,
        output_commitment: [u8; 32],
    ) -> Result<(), Refusal>;

    // ── observability ─────────────────────────────────────────────────

    /// Everything ordinary diagnostics would show: captured log output and
    /// the debug rendering of every grant, presentation, key, lease,
    /// plaintext and durable record the scenario touched.
    fn diagnostics(&self) -> String;
    /// Byte strings that must never appear in diagnostics: private keys,
    /// grant signatures, possession proofs, content keys.
    fn secrets(&self) -> Vec<Vec<u8>>;
}

// ── results ──────────────────────────────────────────────────────────────

/// Engineering grouping of the cases. Not a certification tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// Executor binding, control-plane and receipt boundaries, claims.
    Core,
    /// Nothing private on-chain; public fields are not transports.
    Privacy,
    /// Grants, leases, proofs, replay, result access.
    Authorization,
    /// The mandatory order: verify → execute → persist → receipt.
    Lifecycle,
    /// Crash, stale worker, duplicate attempt.
    FailureRecovery,
    /// Diagnostics carry identifiers, never payloads or secrets.
    Observability,
}

impl Group {
    /// Report label.
    pub fn label(self) -> &'static str {
        match self {
            Group::Core => "CORE",
            Group::Privacy => "PRIVACY",
            Group::Authorization => "AUTHORIZATION",
            Group::Lifecycle => "LIFECYCLE",
            Group::FailureRecovery => "FAILURE_RECOVERY",
            Group::Observability => "OBSERVABILITY",
        }
    }

    /// All groups, in report order.
    pub const ALL: [Group; 6] = [
        Group::Core,
        Group::Privacy,
        Group::Authorization,
        Group::Lifecycle,
        Group::FailureRecovery,
        Group::Observability,
    ];
}

/// One case's outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The behaviour was observed.
    Pass,
    /// The behaviour was not observed.
    Fail(String),
    /// The subject cannot be driven through this case. Counts as a
    /// failure of the group: a required invariant that cannot be tested
    /// is not passed.
    Unsupported(String),
}

/// A case in the catalog.
#[derive(Debug, Clone)]
pub struct Case {
    /// Stable identifier, `C01`…
    pub id: &'static str,
    /// Stable semantic name.
    pub name: &'static str,
    /// Group.
    pub group: Group,
    /// The authority invariants it tests.
    pub invariants: &'static [&'static str],
}

/// One case, run.
#[derive(Debug, Clone)]
pub struct CaseResult {
    /// The case.
    pub case: Case,
    /// Outcome.
    pub status: Status,
}

/// The whole run.
#[derive(Debug, Clone)]
pub struct Report {
    /// Suite version.
    pub version: &'static str,
    /// Who was tested.
    pub subject: SubjectInfo,
    /// Every case, in catalog order.
    pub cases: Vec<CaseResult>,
}

impl Report {
    /// Whether a group passed: every case in it passed.
    pub fn group_passed(&self, group: Group) -> bool {
        self.cases
            .iter()
            .filter(|c| c.case.group == group)
            .all(|c| c.status == Status::Pass)
    }

    /// Whether every case passed.
    pub fn passed(&self) -> bool {
        self.cases.iter().all(|c| c.status == Status::Pass)
    }

    /// Passed and total counts.
    pub fn counts(&self) -> (usize, usize) {
        (
            self.cases.iter().filter(|c| c.status == Status::Pass).count(),
            self.cases.len(),
        )
    }

    /// The machine-readable block CI prints.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "MBONGO_COMPUTE_CONFORMANCE");
        let _ = writeln!(out, "VERSION: {}", self.version);
        let _ = writeln!(out, "SUBJECT: {}", self.subject.name);
        let _ = writeln!(out, "PROFILE: {}", self.subject.profile_tag);
        for g in Group::ALL {
            let n = self.cases.iter().filter(|c| c.case.group == g).count();
            let p = self
                .cases
                .iter()
                .filter(|c| c.case.group == g && c.status == Status::Pass)
                .count();
            let _ = writeln!(
                out,
                "{}: {} ({p}/{n})",
                g.label(),
                if self.group_passed(g) { "PASS" } else { "FAIL" }
            );
        }
        for c in &self.cases {
            let (mark, detail) = match &c.status {
                Status::Pass => ("PASS", String::new()),
                Status::Fail(m) => ("FAIL", format!(" — {m}")),
                Status::Unsupported(m) => ("UNSUPPORTED", format!(" — {m}")),
            };
            let _ = writeln!(
                out,
                "  {} {:<55} {} [{}]{}",
                c.case.id,
                c.case.name,
                mark,
                c.case.invariants.join(" "),
                detail
            );
        }
        let (p, n) = self.counts();
        let _ = writeln!(
            out,
            "RESULT: {} ({p}/{n})",
            if self.passed() { "PASS" } else { "FAIL" }
        );
        out
    }
}

// ── the catalog ──────────────────────────────────────────────────────────

macro_rules! case {
    ($id:literal, $name:literal, $group:ident, [$($inv:literal),*]) => {
        Case { id: $id, name: $name, group: Group::$group, invariants: &[$($inv),*] }
    };
}

/// Every case this suite version runs, in order.
#[allow(clippy::too_many_lines)]
pub fn catalog() -> Vec<Case> {
    vec![
        case!(
            "C01",
            "core_correct_executor_completes_flow",
            Core,
            ["E3", "F6", "G1", "G48"]
        ),
        case!(
            "C02",
            "auth_wrong_executor_cannot_execute",
            Authorization,
            ["E1", "E3", "F4", "G1"]
        ),
        case!(
            "C03",
            "core_control_plane_cannot_replace_executor",
            Core,
            ["E1", "E2", "E17", "F20", "G2"]
        ),
        case!(
            "C04",
            "auth_task_id_is_not_a_capability",
            Authorization,
            ["F3", "E24", "G7"]
        ),
        case!(
            "C05",
            "auth_locator_is_not_a_capability",
            Authorization,
            ["F9", "G7"]
        ),
        case!(
            "C06",
            "auth_capability_is_task_scoped",
            Authorization,
            ["F5", "G8"]
        ),
        case!(
            "C07",
            "auth_capability_is_resource_scoped",
            Authorization,
            ["F5", "G9"]
        ),
        case!(
            "C08",
            "auth_capability_is_executor_scoped",
            Authorization,
            ["F4", "G10"]
        ),
        case!(
            "C09",
            "auth_capability_requires_proof_of_possession",
            Authorization,
            ["F4", "E3"]
        ),
        case!(
            "C10",
            "auth_input_capability_is_single_use",
            Authorization,
            ["G11"]
        ),
        case!(
            "C11",
            "auth_consumed_capability_replay_fails",
            Authorization,
            ["E10", "G12", "G14"]
        ),
        case!(
            "C12",
            "auth_expired_capability_fails",
            Authorization,
            ["F10", "G13"]
        ),
        case!(
            "C13",
            "auth_revoked_capability_fails",
            Authorization,
            ["F10", "G13"]
        ),
        case!(
            "C14",
            "auth_cross_task_replay_fails",
            Authorization,
            ["F21", "G14"]
        ),
        case!(
            "C15",
            "auth_cross_executor_replay_fails",
            Authorization,
            ["F22", "G14"]
        ),
        case!(
            "C16",
            "auth_lease_alone_grants_no_data",
            Authorization,
            ["E6", "G6"]
        ),
        case!(
            "C17",
            "auth_capability_alone_grants_no_execution",
            Authorization,
            ["E6", "E7", "E8"]
        ),
        case!(
            "C18",
            "lifecycle_commitment_verified_before_execution",
            Lifecycle,
            ["F6", "F7", "G15"]
        ),
        case!(
            "C19",
            "lifecycle_corrupted_input_blocks_execution",
            Lifecycle,
            ["F7", "G16"]
        ),
        case!(
            "C20",
            "lifecycle_result_persisted_before_receipt",
            Lifecycle,
            ["E13", "F23", "G21"]
        ),
        case!(
            "C21",
            "lifecycle_result_persistence_failure_blocks_receipt",
            Lifecycle,
            ["E13", "F23", "G22"]
        ),
        case!(
            "C22",
            "recovery_crash_after_fetch_requires_fresh_capability",
            FailureRecovery,
            ["E10", "E11", "G33"]
        ),
        case!(
            "C23",
            "recovery_stale_attempt_is_fenced",
            FailureRecovery,
            ["E8", "G34"]
        ),
        case!(
            "C24",
            "recovery_duplicate_execution_does_not_overwrite_result",
            FailureRecovery,
            ["E12", "G32"]
        ),
        case!(
            "C25",
            "core_exactly_once_is_not_claimed",
            Core,
            ["E12", "G32"]
        ),
        case!(
            "C26",
            "auth_task_id_alone_cannot_retrieve_result",
            Authorization,
            ["F15", "G24"]
        ),
        case!(
            "C27",
            "auth_unauthorized_result_retrieval_fails",
            Authorization,
            ["F15", "G23"]
        ),
        case!(
            "C28",
            "privacy_raw_input_absent_from_chain",
            Privacy,
            ["P1", "P2", "P4", "P5", "F1", "G17"]
        ),
        case!(
            "C29",
            "privacy_raw_output_absent_from_chain",
            Privacy,
            ["P1", "P2", "P4", "F2", "G18"]
        ),
        case!(
            "C30",
            "observability_diagnostics_omit_payloads_and_secrets",
            Observability,
            ["F19", "E23", "G37"]
        ),
        case!(
            "C31",
            "core_control_plane_cannot_forge_executor_signature",
            Core,
            ["E17", "G28", "G29", "G30"]
        ),
        case!(
            "C32",
            "core_anchored_receipt_is_not_output_correctness",
            Core,
            ["P16", "F16", "E19", "G47"]
        ),
        case!(
            "C33",
            "privacy_execution_spec_is_not_input_transport",
            Privacy,
            ["P5", "P12"]
        ),
        case!(
            "C34",
            "privacy_receipt_metadata_is_not_output_transport",
            Privacy,
            ["P4", "P12"]
        ),
        case!(
            "C35",
            "privacy_worker_never_sources_input_from_chain_payload",
            Privacy,
            ["P15", "P5"]
        ),
        case!(
            "C36",
            "lifecycle_lease_precedes_capability",
            Lifecycle,
            ["E6", "E7", "G6"]
        ),
        case!(
            "C37",
            "core_no_confidentiality_claim_without_attestation",
            Core,
            ["P8", "P9", "F12", "F13", "E20", "E21", "G19", "G20"]
        ),
        case!(
            "C38",
            "recovery_ambiguous_submission_yields_one_receipt",
            FailureRecovery,
            ["E14", "G31"]
        ),
    ]
}

// ── helpers ──────────────────────────────────────────────────────────────

type CaseOutcome = Result<(), String>;

macro_rules! check {
    ($cond:expr, $($arg:tt)+) => {
        if !($cond) {
            return Err(format!($($arg)+));
        }
    };
}

fn refused<T: std::fmt::Debug>(r: &Result<T, Refusal>) -> bool {
    r.is_err()
}

fn unsupported(what: &str) -> String {
    format!("UNSUPPORTED: {what}")
}

/// Every receipt anchored for `task_id`, with its height.
async fn receipts_for<C: ChainClient>(chain: &C, task_id: [u8; 32]) -> Vec<(u64, Receipt)> {
    let latest = chain.latest_height().await.unwrap_or(0);
    let mut out = Vec::new();
    for h in 0..=latest {
        if let Ok(Some(block)) = chain.block_by_height(h).await {
            for tx in &block.body.transactions {
                if let TransactionPayload::AnchorReceipt(r) = &tx.payload {
                    if r.task_id == task_id {
                        out.push((h, (**r).clone()));
                    }
                }
            }
        }
    }
    out
}

/// The SCALE bytes of every block, concatenated: everything the chain
/// carries, in the form it commits to.
async fn chain_bytes<C: ChainClient>(chain: &C) -> Vec<u8> {
    let latest = chain.latest_height().await.unwrap_or(0);
    let mut out = Vec::new();
    for h in 0..=latest {
        if let Ok(Some(block)) = chain.block_by_height(h).await {
            out.extend(block.encode());
            out.push(0xFF);
        }
    }
    out
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Runs the subject's own executor until it has submitted, then includes
/// a block and observes the receipt. Returns the anchored receipt.
async fn complete_scenario<S: Subject>(s: &mut S) -> Result<Receipt, String> {
    let mut w = s.new_instance(Actor::Executor);
    let outcome = s.run(&mut w).await;
    check!(
        matches!(outcome, Outcome::Submitted(_)),
        "expected the named executor to submit a receipt, got {outcome:?}"
    );
    s.produce_block();
    let task_id = s.task().task_id();
    let receipts = receipts_for(s.chain(), task_id).await;
    check!(
        receipts.len() == 1,
        "expected exactly one anchored receipt, found {}",
        receipts.len()
    );
    Ok(receipts[0].1.clone())
}

// ── cases ────────────────────────────────────────────────────────────────

async fn c01<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let task = s.task();
    let receipt = complete_scenario(s).await?;
    check!(
        receipt.task_id == task.task_id(),
        "receipt names another task"
    );
    check!(
        receipt.input_commitment == task.input_commitment,
        "receipt input_commitment differs from the task"
    );
    check!(
        receipt.executor == task.executor,
        "receipt executor differs from task.executor"
    );
    check!(
        verify_receipt_signature(&receipt),
        "receipt signature does not verify under task.executor"
    );
    check!(
        s.result_exists(task.task_id()),
        "no durable result behind the anchored receipt"
    );
    check!(
        s.result_commitment(task.task_id()) == Some(receipt.output_commitment),
        "durable result commitment differs from the receipt"
    );
    let out = s
        .owner_retrieve_result(task.task_id())
        .map_err(|e| format!("owner could not retrieve: {e:?}"))?;
    check!(
        out == s.expected_output(),
        "owner retrieved bytes that are not the profile's output"
    );
    let mut w = s.new_instance(Actor::Executor);
    let again = s.run(&mut w).await;
    check!(
        matches!(again, Outcome::Completed(_) | Outcome::Idle),
        "after anchoring, a pass must observe completion or idle, got {again:?}"
    );
    Ok(())
}

async fn c02<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let mut other = s.new_instance(Actor::OtherExecutor);
    let outcome = s.run(&mut other).await;
    check!(
        outcome == Outcome::Idle,
        "another executor was offered the task: {outcome:?}"
    );
    check!(
        s.obtain_lease(&mut other).is_none(),
        "another executor obtained a lease"
    );
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    check!(
        refused(&s.present_fetch(Actor::OtherExecutor, &g)),
        "another executor fetched the input with the executor's grant"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a wrong-executor presentation"
    );
    check!(
        !s.result_exists(s.task().task_id()),
        "a result exists although only the wrong executor ran"
    );
    s.produce_block();
    check!(
        receipts_for(s.chain(), s.task().task_id()).await.is_empty(),
        "a receipt was anchored by the wrong executor"
    );
    Ok(())
}

async fn c03<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let task = s.task();
    // The issuer side, which the control plane operates, tries to name B.
    match s.owner_side_grant(Actor::OtherExecutor, GrantOp::FetchInput) {
        Err(_) => {}
        Ok(g) => {
            check!(
                refused(&s.present_fetch(Actor::OtherExecutor, &g)),
                "the issuer produced a grant B could use on A's task"
            );
        }
    }
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed after an issuer attempt to authorise B"
    );
    let mut other = s.new_instance(Actor::OtherExecutor);
    check!(
        s.run(&mut other).await == Outcome::Idle,
        "B was offered A's task"
    );
    check!(
        s.executor_of_record(task.task_id()) == Some(task.executor),
        "coordination's executor of record differs from task.executor"
    );
    // A finishes; the receipt names A.
    let receipt = complete_scenario(s).await?;
    check!(
        receipt.executor == task.executor,
        "anchored receipt names another executor"
    );
    check!(
        s.executor_of_record(task.task_id()) == Some(task.executor),
        "executor of record changed during the flow"
    );
    Ok(())
}

async fn c04<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    for actor in [
        Actor::Executor,
        Actor::OtherExecutor,
        Actor::Client,
        Actor::Stranger,
    ] {
        check!(
            refused(&s.fetch_with_task_id_only(actor, id)),
            "{actor:?} fetched the input with the task id alone"
        );
    }
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a task-id-only request"
    );
    Ok(())
}

async fn c05<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let loc = s.input_locator();
    for actor in [
        Actor::Executor,
        Actor::OtherExecutor,
        Actor::Client,
        Actor::Stranger,
    ] {
        check!(
            refused(&s.fetch_with_locator_only(actor, &loc)),
            "{actor:?} fetched the input with the locator alone"
        );
    }
    check!(
        !s.input_consumed(&loc),
        "input consumed by a locator-only request"
    );
    Ok(())
}

async fn c06<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let forged = s.retarget(&g, Retarget::Task([0x77u8; 32]));
    check!(
        refused(&s.present_fetch(Actor::Executor, &forged)),
        "a grant retargeted to another task was accepted"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a retargeted grant"
    );
    Ok(())
}

async fn c07<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    // A grant for the result object must not serve the input object and
    // vice versa: obtain a put grant and present it as a fetch.
    let put = s
        .owner_side_grant(Actor::Executor, GrantOp::PutResult)
        .map_err(|e| format!("{e:?}"))?;
    check!(
        refused(&s.present_fetch(Actor::Executor, &put)),
        "a put-result grant fetched the input"
    );
    let fetch = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    check!(
        refused(&s.present_put(Actor::Executor, &fetch, vec![1, 2, 3])),
        "a fetch-input grant stored a result"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a resource-mismatched presentation"
    );
    check!(
        !s.result_exists(s.task().task_id()),
        "a result was stored under a fetch grant"
    );
    Ok(())
}

async fn c08<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let forged = s.retarget(&g, Retarget::Presenter(Actor::OtherExecutor));
    check!(
        refused(&s.present_fetch(Actor::OtherExecutor, &forged)),
        "a grant re-pointed at another executor was accepted"
    );
    check!(
        refused(&s.present_fetch(Actor::OtherExecutor, &g)),
        "another executor presented the executor's grant"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a wrong presenter"
    );
    Ok(())
}

async fn c09<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    check!(
        refused(&s.present_fetch_unproven(&g)),
        "the grant was honoured without proof of possession"
    );
    check!(
        refused(&s.present_fetch(Actor::Stranger, &g)),
        "a stranger's proof was accepted for the executor's grant"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed without a valid proof"
    );
    let got = s
        .present_fetch(Actor::Executor, &g)
        .map_err(|e| format!("the executor's own proof was refused: {e:?}"))?;
    check!(
        got == s.private_input(),
        "the executor fetched bytes that are not the private input"
    );
    Ok(())
}

async fn c10<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    s.present_fetch(Actor::Executor, &g)
        .map_err(|e| format!("first use refused: {e:?}"))?;
    check!(
        s.input_consumed(&s.input_locator()),
        "input not marked consumed after a fetch"
    );
    check!(
        refused(&s.present_fetch(Actor::Executor, &g)),
        "a single-use grant served a second fresh presentation"
    );
    Ok(())
}

async fn c11<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    s.present_fetch(Actor::Executor, &g)
        .map_err(|e| format!("first use refused: {e:?}"))?;
    check!(
        refused(&s.replay_last_presentation(&g)),
        "an observed presentation was replayed successfully"
    );
    // A stale presentation must also fail before consumption: replaying a
    // presentation that was refused must not succeed later.
    let g2 = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let _ = s.present_fetch(Actor::Stranger, &g2);
    check!(
        refused(&s.replay_last_presentation(&g2)),
        "a refused presentation was replayed successfully"
    );
    Ok(())
}

async fn c12<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let ttl = s.grant_ttl_secs() + 1;
    s.advance_time(ttl);
    check!(
        refused(&s.present_fetch(Actor::Executor, &g)),
        "an expired grant was honoured"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed under an expired grant"
    );
    Ok(())
}

async fn c13<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    s.revoke(&g).map_err(|e| format!("revocation unavailable: {e:?}"))?;
    check!(
        refused(&s.present_fetch(Actor::Executor, &g)),
        "a revoked grant was honoured"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed under a revoked grant"
    );
    Ok(())
}

async fn c14<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let other_task = s.commit_task_carrying_input_in_spec().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let forged = s.retarget(&g, Retarget::Task(other_task));
    check!(
        refused(&s.present_fetch(Actor::Executor, &forged)),
        "task A's grant served under task B"
    );
    check!(
        refused(&s.fetch_with_task_id_only(Actor::Executor, other_task)),
        "task B's id fetched an input"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a cross-task presentation"
    );
    Ok(())
}

async fn c15<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    // B replays A's grant, with B's own proof, with A's captured proof, and
    // re-pointed at B.
    check!(
        refused(&s.present_fetch(Actor::OtherExecutor, &g)),
        "B presented A's grant successfully"
    );
    s.present_fetch(Actor::Executor, &g)
        .map_err(|e| format!("A's own use refused: {e:?}"))?;
    check!(
        refused(&s.replay_last_presentation(&g)),
        "A's captured presentation replayed"
    );
    let forged = s.retarget(&g, Retarget::Presenter(Actor::OtherExecutor));
    check!(
        refused(&s.present_fetch(Actor::OtherExecutor, &forged)),
        "A's grant re-pointed at B was honoured"
    );
    Ok(())
}

async fn c16<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let mut w = s.new_instance(Actor::Executor);
    let lease = s
        .obtain_lease(&mut w)
        .ok_or_else(|| "the named executor obtained no lease".to_string())?;
    check!(
        refused(&s.fetch_with_lease_only(Actor::Executor, &lease)),
        "a lease alone fetched the input"
    );
    check!(
        refused(&s.fetch_with_lease_only(Actor::OtherExecutor, &lease)),
        "a lease alone fetched the input for another executor"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed by a lease-only request"
    );
    Ok(())
}

async fn c17<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let mut w = s.new_instance(Actor::Executor);
    let lease = s.obtain_lease(&mut w).ok_or_else(|| "no lease".to_string())?;
    let _grant = s
        .grant_under_lease(&w, &lease, GrantOp::FetchInput)
        .map_err(|e| format!("no grant under a live lease: {e:?}"))?;
    // Coordination authority lapses; the grant, whatever its own window,
    // confers none.
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    check!(
        refused(&s.report_started(&w, &lease)),
        "execution was reported started under a lapsed lease"
    );
    check!(
        refused(&s.heartbeat(&w, &lease)),
        "a lapsed lease was renewed"
    );
    check!(
        refused(&s.grant_under_lease(&w, &lease, GrantOp::PutResult)),
        "a put grant was issued under a lapsed lease"
    );
    // A fresh instance is admitted normally.
    let mut w2 = s.new_instance(Actor::Executor);
    let outcome = s.run(&mut w2).await;
    check!(
        matches!(outcome, Outcome::Submitted(_)),
        "a fresh attempt did not proceed after the stale one: {outcome:?}"
    );
    Ok(())
}

async fn c18<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let mut w = s.new_instance(Actor::Executor);
    check!(
        s.inject(&mut w, FaultPoint::CorruptInput),
        "{}",
        unsupported("input corruption fault")
    );
    let outcome = s.run(&mut w).await;
    let id = s.task().task_id();
    check!(
        outcome == Outcome::Failed(id, FailureKind::Input),
        "a corrupted input must fail at the input stage, before execution; got {outcome:?}"
    );
    check!(
        !s.result_exists(id),
        "a result exists although the input failed verification"
    );
    s.produce_block();
    check!(
        receipts_for(s.chain(), id).await.is_empty(),
        "a receipt was anchored for an unverified input"
    );
    Ok(())
}

async fn c19<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    let mut w = s.new_instance(Actor::Executor);
    check!(
        s.inject(&mut w, FaultPoint::CorruptInput),
        "{}",
        unsupported("input corruption fault")
    );
    let outcome = s.run(&mut w).await;
    check!(
        matches!(outcome, Outcome::Failed(_, FailureKind::Input)),
        "got {outcome:?}"
    );
    check!(!s.result_exists(id), "result stored from corrupted input");
    check!(
        s.result_commitment(id).is_none(),
        "a result commitment exists"
    );
    s.produce_block();
    check!(
        receipts_for(s.chain(), id).await.is_empty(),
        "receipt anchored after corrupted input"
    );
    // The private object is intact: an honest attempt then succeeds.
    let receipt = complete_scenario(s).await?;
    check!(
        s.result_commitment(id) == Some(receipt.output_commitment),
        "recovered attempt's receipt does not match its result"
    );
    Ok(())
}

async fn c20<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    let mut w = s.new_instance(Actor::Executor);
    check!(
        s.inject(&mut w, FaultPoint::CrashAfterPersist),
        "{}",
        unsupported("crash-after-persist fault")
    );
    let outcome = s.run(&mut w).await;
    check!(matches!(outcome, Outcome::Crashed(_)), "got {outcome:?}");
    check!(
        s.result_exists(id),
        "no durable result at the crash-after-persist point"
    );
    let persisted = s.result_commitment(id).ok_or("no result commitment")?;
    s.produce_block();
    check!(
        receipts_for(s.chain(), id).await.is_empty(),
        "a receipt existed before the result was durable"
    );
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    let receipt = complete_scenario(s).await?;
    check!(
        receipt.output_commitment == persisted,
        "the receipt commits to something other than the persisted result"
    );
    Ok(())
}

async fn c21<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    check!(
        s.inject_result_store_failure(),
        "{}",
        unsupported("result store failure")
    );
    let mut w = s.new_instance(Actor::Executor);
    let outcome = s.run(&mut w).await;
    check!(
        outcome == Outcome::Failed(id, FailureKind::Persistence),
        "a failed store must fail the attempt at persistence; got {outcome:?}"
    );
    check!(
        !s.result_exists(id),
        "a result exists although the store failed"
    );
    s.produce_block();
    check!(
        receipts_for(s.chain(), id).await.is_empty(),
        "a receipt was anchored although the result store failed"
    );
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    let receipt = complete_scenario(s).await?;
    check!(
        s.result_commitment(id) == Some(receipt.output_commitment),
        "receipt does not match the stored result"
    );
    Ok(())
}

async fn c22<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    let loc = s.input_locator();
    // Attempt 1: lease, grant, fetch (consumed), then the process is gone.
    let mut w1 = s.new_instance(Actor::Executor);
    let lease1 = s.obtain_lease(&mut w1).ok_or("no lease")?;
    let g1 = s
        .grant_under_lease(&w1, &lease1, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    s.present_fetch(Actor::Executor, &g1)
        .map_err(|e| format!("first fetch refused: {e:?}"))?;
    check!(s.input_consumed(&loc), "input not consumed after fetch");
    drop(w1);
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    // Attempt 2: a new instance, a new lease, a fresh grant — and it works.
    let mut w2 = s.new_instance(Actor::Executor);
    let outcome = s.run(&mut w2).await;
    check!(
        matches!(outcome, Outcome::Submitted(_)),
        "the retry did not proceed: {outcome:?}"
    );
    check!(
        s.attempt_count(id) >= 2,
        "the retry did not count as a new attempt"
    );
    // The old grant is dead in every form.
    check!(
        refused(&s.present_fetch(Actor::Executor, &g1)),
        "the consumed grant was reopened for the retry"
    );
    check!(
        refused(&s.replay_last_presentation(&g1)),
        "the consumed grant's presentation replayed"
    );
    Ok(())
}

async fn c23<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    let mut w1 = s.new_instance(Actor::Executor);
    let lease1 = s.obtain_lease(&mut w1).ok_or("no lease")?;
    let g1 = s
        .grant_under_lease(&w1, &lease1, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    s.present_fetch(Actor::Executor, &g1).map_err(|e| format!("{e:?}"))?;
    // Instance 1 goes quiet past its lease; instance 2 takes over and finishes.
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    let receipt = complete_scenario(s).await?;
    // Instance 1 reappears.
    check!(
        refused(&s.report_started(&w1, &lease1)),
        "a stale attempt's report was accepted"
    );
    check!(
        refused(&s.heartbeat(&w1, &lease1)),
        "a stale lease was renewed"
    );
    check!(
        refused(&s.grant_under_lease(&w1, &lease1, GrantOp::PutResult)),
        "a stale attempt obtained a put grant"
    );
    check!(
        refused(&s.present_fetch(Actor::Executor, &g1)),
        "a stale attempt refetched the input"
    );
    check!(
        s.result_commitment(id) == Some(receipt.output_commitment),
        "the result changed after the stale attempt reappeared"
    );
    Ok(())
}

async fn c24<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    // Two attempts: the first is cut off after persisting, the second reuses.
    let mut w1 = s.new_instance(Actor::Executor);
    check!(
        s.inject(&mut w1, FaultPoint::CrashAfterPersist),
        "{}",
        unsupported("crash-after-persist fault")
    );
    check!(
        matches!(s.run(&mut w1).await, Outcome::Crashed(_)),
        "expected a crash"
    );
    let first = s.result_commitment(id).ok_or("no result after first attempt")?;
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    let receipt = complete_scenario(s).await?;
    check!(s.attempt_count(id) >= 2, "only one attempt was counted");
    check!(
        s.result_commitment(id) == Some(first),
        "the second attempt overwrote the first result"
    );
    check!(
        receipt.output_commitment == first,
        "the receipt commits to something other than the one result"
    );
    // An out-of-band put by the executor cannot overwrite it either.
    if let Ok(put) = s.owner_side_grant(Actor::Executor, GrantOp::PutResult) {
        let _ = s.present_put(Actor::Executor, &put, vec![9, 9, 9]);
        check!(
            s.result_commitment(id) == Some(first),
            "a later put overwrote the result"
        );
    }
    let out = s.owner_retrieve_result(id).map_err(|e| format!("{e:?}"))?;
    check!(
        out == s.expected_output(),
        "the owner did not get the one correct result"
    );
    let receipts = receipts_for(s.chain(), id).await;
    check!(
        receipts.len() == 1,
        "more than one receipt for the task on-chain: {}",
        receipts.len()
    );
    Ok(())
}

fn c25<S: Subject>(s: &mut S) -> CaseOutcome {
    check!(
        !s.info().claims.exactly_once,
        "the subject claims exactly-once execution (E12 forbids the claim)"
    );
    Ok(())
}

async fn c26<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    complete_scenario(s).await?;
    for actor in [
        Actor::Client,
        Actor::Executor,
        Actor::OtherExecutor,
        Actor::Stranger,
    ] {
        check!(
            refused(&s.result_with_task_id_only(actor, id)),
            "{actor:?} read the result with the task id alone"
        );
    }
    // The receipt is public; the result is not.
    check!(
        !receipts_for(s.chain(), id).await.is_empty(),
        "no receipt to compare against"
    );
    check!(
        !contains(&chain_bytes(s.chain()).await, &s.expected_output()),
        "the result bytes are on-chain"
    );
    Ok(())
}

async fn c27<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    complete_scenario(s).await?;
    // A stranger cannot be named, or cannot use, a get grant.
    match s.owner_side_grant(Actor::Stranger, GrantOp::GetResult) {
        Err(_) => {}
        Ok(g) => check!(
            refused(&s.present_get(Actor::Stranger, &g)),
            "a stranger read the result under a grant naming it"
        ),
    }
    let g = s
        .owner_side_grant(Actor::Client, GrantOp::GetResult)
        .map_err(|e| format!("{e:?}"))?;
    check!(
        refused(&s.present_get(Actor::Stranger, &g)),
        "a stranger read the result with the owner's grant"
    );
    check!(
        refused(&s.present_get(Actor::OtherExecutor, &g)),
        "another executor read the result with the owner's grant"
    );
    s.revoke(&g).map_err(|e| format!("{e:?}"))?;
    check!(
        refused(&s.present_get(Actor::Client, &g)),
        "a revoked get grant was honoured"
    );
    let g2 = s
        .owner_side_grant(Actor::Client, GrantOp::GetResult)
        .map_err(|e| format!("{e:?}"))?;
    let ttl = s.grant_ttl_secs() + 1;
    s.advance_time(ttl);
    check!(
        refused(&s.present_get(Actor::Client, &g2)),
        "an expired get grant was honoured"
    );
    Ok(())
}

async fn c28<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let receipt = complete_scenario(s).await?;
    let input = s.private_input();
    let bytes = chain_bytes(s.chain()).await;
    check!(
        !contains(&bytes, &input),
        "the private input bytes appear in a block"
    );
    check!(
        !contains(&s.task().execution_spec, &input),
        "the private input bytes are in execution_spec"
    );
    check!(
        !contains(&receipt.metadata, &input),
        "the private input bytes are in receipt metadata"
    );
    Ok(())
}

async fn c29<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let receipt = complete_scenario(s).await?;
    let output = s.expected_output();
    let bytes = chain_bytes(s.chain()).await;
    check!(
        !contains(&bytes, &output),
        "the private output bytes appear in a block"
    );
    check!(
        !contains(&receipt.metadata, &output),
        "the private output bytes are in receipt metadata"
    );
    Ok(())
}

async fn c30<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    // Exercise every path that touches secrets: a refused presentation, a
    // successful flow, an owner retrieval.
    let g = s
        .owner_side_grant(Actor::Executor, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let _ = s.present_fetch(Actor::Stranger, &g);
    complete_scenario(s).await?;
    let id = s.task().task_id();
    let _ = s.owner_retrieve_result(id);
    let diag = s.diagnostics();
    let raw = diag.as_bytes();
    let forms = |b: &[u8]| -> Vec<Vec<u8>> { vec![b.to_vec(), hex::encode(b).into_bytes()] };
    for form in forms(&s.private_input()) {
        check!(!contains(raw, &form), "diagnostics carry the private input");
    }
    for form in forms(&s.expected_output()) {
        check!(
            !contains(raw, &form),
            "diagnostics carry the private output"
        );
    }
    let secrets = s.secrets();
    check!(
        !secrets.is_empty(),
        "the subject declared no secrets to check"
    );
    for (i, secret) in secrets.iter().enumerate() {
        for form in forms(secret) {
            check!(
                !contains(raw, &form),
                "diagnostics carry secret #{i} ({} bytes)",
                secret.len()
            );
        }
    }
    check!(
        diag.contains(&hex::encode(&id[..8])),
        "diagnostics do not even identify the task (identifiers are allowed and expected)"
    );
    Ok(())
}

async fn c31<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let task = s.task();
    let receipt = s.control_plane_receipt([0x42u8; 32]);
    check!(
        receipt.executor == task.executor,
        "the control plane's receipt does not even name the executor; nothing to test"
    );
    check!(
        !verify_receipt_signature(&receipt),
        "a receipt signed without the executor key verifies as the executor's"
    );
    let tx = s.control_plane_anchor_transaction(receipt);
    check!(
        tx.sender == task.executor,
        "the anchoring transaction does not name the executor as sender; nothing to test"
    );
    check!(
        !tx.verify_signature(),
        "an anchoring transaction signed without the executor key verifies as the executor's"
    );
    Ok(())
}

async fn c32<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    let fabricated = [0xFAu8; 32];
    s.executor_anchor_arbitrary(fabricated).await.map_err(|e| format!("{e:?}"))?;
    s.produce_block();
    let receipts = receipts_for(s.chain(), id).await;
    check!(
        receipts.len() == 1 && receipts[0].1.output_commitment == fabricated,
        "the fabricated receipt was not anchored; the boundary could not be shown"
    );
    check!(
        verify_receipt_signature(&receipts[0].1),
        "the fabricated receipt is not even validly signed"
    );
    check!(
        !s.result_exists(id),
        "a result exists although nothing was executed"
    );
    // The suite records the boundary: a validly bound, validly signed,
    // anchored receipt whose commitment no execution produced. Nothing
    // on-chain distinguishes it. Correctness is not what a receipt proves.
    Ok(())
}

async fn c33<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let task = s.task();
    check!(
        !contains(&task.execution_spec, &s.private_input()),
        "the conformant flow put the private input in execution_spec"
    );
    check!(
        task.execution_spec.len() <= 1024,
        "execution_spec exceeds the protocol bound"
    );
    // Protocol permission is not the workflow's licence: the chain accepts
    // a task whose spec carries arbitrary public bytes (here, the input
    // itself), and the conformant workflow still never sources input from it.
    let other = s.commit_task_carrying_input_in_spec().await;
    check!(
        other != task.task_id(),
        "the probe task collides with the scenario task"
    );
    Ok(())
}

async fn c34<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let receipt = complete_scenario(s).await?;
    check!(
        !contains(&receipt.metadata, &s.expected_output()),
        "receipt metadata carries the private output"
    );
    check!(
        !contains(&receipt.metadata, &s.private_input()),
        "receipt metadata carries the private input"
    );
    Ok(())
}

async fn c35<S: Subject>(s: &mut S) -> CaseOutcome {
    // Only the probe task exists: its spec carries the input bytes, and no
    // private object was stored. A worker that sourced input from the
    // payload would execute it.
    let probe = s.commit_task_carrying_input_in_spec().await;
    let mut w = s.new_instance(Actor::Executor);
    let outcome = s.run(&mut w).await;
    check!(
        !matches!(outcome, Outcome::Submitted(_) | Outcome::Completed(_)),
        "the worker executed a task whose only input was the public execution_spec: {outcome:?}"
    );
    check!(
        !s.result_exists(probe),
        "a result exists for a task with no private input"
    );
    s.produce_block();
    check!(
        receipts_for(s.chain(), probe).await.is_empty(),
        "a receipt was anchored for a task with no private input"
    );
    Ok(())
}

async fn c36<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let mut w = s.new_instance(Actor::Executor);
    let lease = s.obtain_lease(&mut w).ok_or("no lease")?;
    let ttl = s.lease_ttl_secs() + 1;
    s.advance_time(ttl);
    check!(
        refused(&s.grant_under_lease(&w, &lease, GrantOp::FetchInput)),
        "a fetch grant was issued under a lapsed lease"
    );
    check!(
        !s.input_consumed(&s.input_locator()),
        "input consumed without a live lease"
    );
    // Under a live lease the order holds: lease, then grant, then fetch.
    let mut w2 = s.new_instance(Actor::Executor);
    let lease2 = s.obtain_lease(&mut w2).ok_or("no second lease")?;
    let g = s
        .grant_under_lease(&w2, &lease2, GrantOp::FetchInput)
        .map_err(|e| format!("{e:?}"))?;
    let got = s.present_fetch(Actor::Executor, &g).map_err(|e| format!("{e:?}"))?;
    check!(got == s.private_input(), "wrong bytes under a live lease");
    Ok(())
}

fn c37<S: Subject>(s: &mut S) -> CaseOutcome {
    let claims = s.info().claims;
    check!(
        !claims.confidential_execution,
        "the subject claims confidential execution; {CONFORMANCE_VERSION} cannot evaluate that claim (P9, F13, E21) and an ordinary worker must not make it (P8)"
    );
    Ok(())
}

async fn c38<S: Subject>(s: &mut S) -> CaseOutcome {
    s.commit_task().await;
    let id = s.task().task_id();
    check!(
        s.inject_lost_submission_response(),
        "{}",
        unsupported("lost submission response")
    );
    let mut w = s.new_instance(Actor::Executor);
    let first = s.run(&mut w).await;
    check!(
        matches!(first, Outcome::Submitted(_) | Outcome::Completed(_)),
        "an ambiguous submission must be resolved by lookup or identical resubmission, got {first:?}"
    );
    s.produce_block();
    let receipts = receipts_for(s.chain(), id).await;
    check!(
        receipts.len() == 1,
        "expected one receipt after an ambiguous submission, found {}",
        receipts.len()
    );
    let again = s.run(&mut w).await;
    check!(
        matches!(again, Outcome::Completed(_) | Outcome::Idle),
        "got {again:?}"
    );
    s.produce_block();
    check!(
        receipts_for(s.chain(), id).await.len() == 1,
        "a second receipt appeared after recovery"
    );
    check!(
        s.result_commitment(id) == Some(receipts[0].1.output_commitment),
        "receipt and result disagree"
    );
    Ok(())
}

// ── the runner ───────────────────────────────────────────────────────────

/// Runs every case against fresh subjects from `make`.
pub async fn run_all<S, F>(mut make: F) -> Report
where
    S: Subject,
    F: FnMut() -> S,
{
    let info = make().info();
    let mut cases = Vec::new();
    for case in catalog() {
        let mut s = make();
        let outcome: CaseOutcome = match case.id {
            "C01" => c01(&mut s).await,
            "C02" => c02(&mut s).await,
            "C03" => c03(&mut s).await,
            "C04" => c04(&mut s).await,
            "C05" => c05(&mut s).await,
            "C06" => c06(&mut s).await,
            "C07" => c07(&mut s).await,
            "C08" => c08(&mut s).await,
            "C09" => c09(&mut s).await,
            "C10" => c10(&mut s).await,
            "C11" => c11(&mut s).await,
            "C12" => c12(&mut s).await,
            "C13" => c13(&mut s).await,
            "C14" => c14(&mut s).await,
            "C15" => c15(&mut s).await,
            "C16" => c16(&mut s).await,
            "C17" => c17(&mut s).await,
            "C18" => c18(&mut s).await,
            "C19" => c19(&mut s).await,
            "C20" => c20(&mut s).await,
            "C21" => c21(&mut s).await,
            "C22" => c22(&mut s).await,
            "C23" => c23(&mut s).await,
            "C24" => c24(&mut s).await,
            "C25" => c25(&mut s),
            "C26" => c26(&mut s).await,
            "C27" => c27(&mut s).await,
            "C28" => c28(&mut s).await,
            "C29" => c29(&mut s).await,
            "C30" => c30(&mut s).await,
            "C31" => c31(&mut s).await,
            "C32" => c32(&mut s).await,
            "C33" => c33(&mut s).await,
            "C34" => c34(&mut s).await,
            "C35" => c35(&mut s).await,
            "C36" => c36(&mut s).await,
            "C37" => c37(&mut s),
            "C38" => c38(&mut s).await,
            other => Err(format!("no runner for {other}")),
        };
        let status = match outcome {
            Ok(()) => Status::Pass,
            Err(m) if m.starts_with("UNSUPPORTED: ") => {
                Status::Unsupported(m["UNSUPPORTED: ".len()..].to_string())
            }
            Err(m) => Status::Fail(m),
        };
        cases.push(CaseResult { case, status });
    }
    Report {
        version: CONFORMANCE_VERSION,
        subject: info,
        cases,
    }
}
