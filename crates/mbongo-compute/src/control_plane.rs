//! Reference control plane (E): coordination, and only coordination.
//!
//! The control plane observes tasks and receipts on the chain, admits
//! workers under an executor identity they prove, issues short execution
//! leases with attempt identities, obtains data-plane capabilities under
//! those leases as the client's delegated issuer, and records what each
//! attempt reported. It holds **no executor key**: it can relay a
//! signature and can never produce one (E §11.1). It cannot change
//! `task.executor` — it does not even have a field for it, and the data
//! plane checks the chain-committed executor independently (F §5.1).
//!
//! Its database is never protocol truth (E5). Everything chain-derived in
//! it is a cache re-derivable by block scan; leases and attempts are
//! durable coordination state whose loss costs a restart round-trip, not
//! correctness (E §15–16).

use std::collections::BTreeMap;
use std::sync::Arc;

use mbongo_core::{Address, ComputeTask, Hash};
use serde::{Deserialize, Serialize};

use crate::chain::{scan_receipts, scan_tasks, ChainClient, ChainError};
use crate::clock::Clock;
use crate::confidential::{ReleaseAuthority, ReleaseError, ReleaseGrant, ReleaseId};
use crate::data_plane::{
    Capability, CapabilityRequest, DataPlaneError, InMemoryDataPlane, LocalKey, Operation,
};
use crate::identity::{
    verify_possession, AttemptId, CapabilityId, Challenge, IdSource, LeaseId, ObjectId, SessionId,
    WorkerInstanceId,
};

/// Domain tag for session possession proofs.
const DOMAIN_SESSION: &str = "mbongo:ref-session:v1";
/// Domain tag for lease issuer signatures.
const DOMAIN_LEASE: &str = "mbongo:ref-lease:v1";

/// Implementation policy of this control plane. None of it is consensus.
#[derive(Debug, Clone)]
pub struct ControlPlaneConfig {
    /// Lease lifetime; renewed by heartbeat.
    pub lease_secs: u64,
    /// Session lifetime; renewed by any authenticated call.
    pub session_secs: u64,
    /// Blocks that must exist above a task's block before it is offered.
    /// A conservative policy for the current single-producer devnet, **not**
    /// protocol finality (E §22).
    pub confirmation_depth: u64,
    /// Capability lifetime obtained under a lease.
    pub capability_secs: u64,
}

impl Default for ControlPlaneConfig {
    fn default() -> Self {
        Self {
            lease_secs: 60,
            session_secs: 600,
            confirmation_depth: 1,
            capability_secs: 60,
        }
    }
}

/// An authenticated worker session (E §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// The session.
    pub session_id: SessionId,
    /// The process.
    pub worker_instance: WorkerInstanceId,
    /// The executor identity it proved.
    pub executor: Address,
    /// The `execution_spec` tags it claims to run (worker claims, E §5.1).
    pub spec_tags: Vec<Vec<u8>>,
    /// Last authenticated activity.
    pub last_seen: u64,
    /// Expiry.
    pub expires_at: u64,
}

/// An execution lease (E §3, §7 step 3). Not persisted: a lease the control
/// plane cannot recover after a restart is expired (E §15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    /// The lease.
    pub lease_id: LeaseId,
    /// The task.
    pub task_id: [u8; 32],
    /// The executor — copied from the chain, never chosen here.
    pub executor: Address,
    /// The instance coordinating this attempt.
    pub worker_instance: WorkerInstanceId,
    /// This attempt.
    pub attempt_id: AttemptId,
    /// Expiry; renewed by heartbeat.
    pub not_after: u64,
    /// The control plane's own service identity.
    pub issuer: Address,
    /// Issuer signature over the fields above.
    pub issuer_signature: [u8; 64],
}

impl Lease {
    fn signing_message(&self) -> Vec<u8> {
        let mut m = Vec::with_capacity(200);
        m.extend_from_slice(DOMAIN_LEASE.as_bytes());
        m.extend_from_slice(&self.lease_id.0);
        m.extend_from_slice(&self.task_id);
        m.extend_from_slice(&self.executor.0);
        m.extend_from_slice(&self.worker_instance.0);
        m.extend_from_slice(&self.attempt_id.0);
        m.extend_from_slice(&self.not_after.to_le_bytes());
        m.extend_from_slice(&self.issuer.0);
        m
    }
}

/// Coordination record of a task (E §9.1). Not a protocol state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    /// Seen in a block.
    Discovered,
    /// A live lease exists.
    Leased,
    /// A fetch capability was issued under the lease.
    InputAuthorized,
    /// The worker reported the input consumed and verified.
    InputConsumed,
    /// The worker reported execution started.
    Executing,
    /// The worker reported a durable result.
    ResultPersisted,
    /// The worker reported a receipt submission.
    ReceiptSubmitted,
    /// A receipt is observed on-chain — chain-derived.
    Completed,
    /// The last attempt failed; a new attempt may be offered.
    Failed,
}

impl TaskState {
    fn is_terminal(self) -> bool {
        matches!(self, TaskState::Completed)
    }
}

/// Failure classes a worker reports (E §20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureClass {
    /// Input could not be fetched, or did not match its commitment.
    Input,
    /// The profile failed or is unsupported.
    Execution,
    /// The result could not be persisted.
    Persistence,
    /// The receipt could not be submitted.
    Receipt,
}

/// What a worker reports under a lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptEvent {
    /// Input fetched and its commitment verified.
    InputConsumed,
    /// Execution started.
    Started,
    /// The data plane confirmed the result durable.
    ResultReady {
        /// The commitment the receipt will carry.
        output_commitment: [u8; 32],
    },
    /// The anchoring transaction was submitted.
    ReceiptSubmitted {
        /// The hash the node reported.
        tx_hash: Hash,
    },
    /// The attempt failed; the lease is released.
    Failed(FailureClass),
}

/// Durable coordination record for one task (E §16).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    /// The envelope as observed — chain-derived cache.
    pub task: ComputeTask,
    /// Its identity.
    pub task_id: [u8; 32],
    /// The block that carries it.
    pub observed_height: u64,
    /// Coordination state.
    pub state: TaskState,
    /// The current lease, if any.
    pub lease: Option<LeaseId>,
    /// Attempts issued so far.
    pub attempt_count: u32,
    /// Capabilities issued, by attempt — ids only, never grants (E §16).
    pub capabilities: Vec<(CapabilityId, AttemptId)>,
    /// The input object the client registered for this task.
    pub input_object: Option<ObjectId>,
    /// Whether that object is confidential (K): its fetch capability is
    /// issued only against a redeemed release authorization, never by the
    /// ordinary path.
    #[serde(default)]
    pub confidential_input: bool,
    /// Last reported failure.
    pub last_failure: Option<FailureClass>,
    /// Submitted anchoring transaction, for §11.2 lookup.
    pub submitted_tx: Option<Hash>,
    /// Height at which a receipt was observed.
    pub completed_height: Option<u64>,
}

/// Everything the control plane persists across a restart (E §16).
/// Sessions and leases are deliberately absent: leases it cannot recover
/// are expired at restart (E §15).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DurableState {
    /// Task records.
    pub tasks: Vec<TaskRecord>,
    /// Last block height scanned.
    pub last_scanned: u64,
    /// Lease and attempt identities issued so far, so a restart never
    /// re-issues an old attempt id.
    pub issued_attempts: u64,
}

#[derive(Debug, Clone)]
struct LeaseRecord {
    lease: Lease,
    last_seen: u64,
    released: bool,
}

/// Why the control plane refused.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ControlPlaneError {
    /// No such session.
    #[error("unknown session")]
    UnknownSession,
    /// The session expired; re-authenticate.
    #[error("session expired")]
    SessionExpired,
    /// The possession proof did not verify for the claimed executor.
    #[error("possession proof invalid")]
    BadPossessionProof,
    /// The challenge is unknown or spent.
    #[error("bad challenge")]
    BadChallenge,
    /// No such lease, or not this session's lease.
    #[error("unknown lease")]
    UnknownLease,
    /// The lease expired, was released, or was superseded: the worker is stale.
    #[error("stale lease")]
    StaleLease,
    /// No such task.
    #[error("unknown task")]
    UnknownTask,
    /// The task is completed on-chain; nothing to coordinate.
    #[error("task completed")]
    TaskCompleted,
    /// The client registered no input object for the task.
    #[error("no input reference registered")]
    NoInputReference,
    /// The input is confidential: the ordinary fetch path never serves it
    /// (K21). Obtain a release authorization first.
    #[error("confidential input: release authorization required")]
    ConfidentialReleaseRequired,
    /// The release authority refused.
    #[error("release: {0}")]
    Release(#[from] ReleaseError),
    /// The data plane refused.
    #[error("data plane: {0}")]
    DataPlane(#[from] DataPlaneError),
    /// The chain could not be read.
    #[error("chain: {0}")]
    Chain(#[from] ChainError),
}

/// The reference control plane.
pub struct ControlPlane {
    clock: Arc<dyn Clock>,
    ids: IdSource,
    /// The control plane's own service key: signs leases and, as the
    /// client's delegate, capabilities. **Not** an executor key.
    issuer: LocalKey,
    config: ControlPlaneConfig,
    sessions: BTreeMap<SessionId, Session>,
    session_challenges: BTreeMap<Challenge, (Address, u64)>,
    leases: BTreeMap<LeaseId, LeaseRecord>,
    tasks: BTreeMap<[u8; 32], TaskRecord>,
    last_scanned: u64,
    latest_height: u64,
    issued_attempts: u64,
}

impl ControlPlane {
    /// A fresh control plane.
    pub fn new(
        clock: Arc<dyn Clock>,
        ids: IdSource,
        issuer: LocalKey,
        config: ControlPlaneConfig,
    ) -> Self {
        Self {
            clock,
            ids,
            issuer,
            config,
            sessions: BTreeMap::new(),
            session_challenges: BTreeMap::new(),
            leases: BTreeMap::new(),
            tasks: BTreeMap::new(),
            last_scanned: 0,
            latest_height: 0,
            issued_attempts: 0,
        }
    }

    /// The identity that signs leases and delegated capabilities.
    pub fn issuer_address(&self) -> Address {
        self.issuer.address()
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    // ── durability ────────────────────────────────────────────────────

    /// The durable coordination state (E §16).
    pub fn snapshot(&self) -> DurableState {
        DurableState {
            tasks: self.tasks.values().cloned().collect(),
            last_scanned: self.last_scanned,
            issued_attempts: self.issued_attempts,
        }
    }

    /// A control plane restored after a restart: tasks and attempt counters
    /// come back; sessions and leases do not, and any lease a worker still
    /// cites is stale (E §15). Task states that depended on a live lease
    /// fall back to `Discovered` so a new attempt can be offered.
    pub fn restore(
        state: DurableState,
        clock: Arc<dyn Clock>,
        ids: IdSource,
        issuer: LocalKey,
        config: ControlPlaneConfig,
    ) -> Self {
        let mut cp = Self::new(clock, ids, issuer, config);
        cp.last_scanned = state.last_scanned;
        cp.latest_height = state.last_scanned;
        cp.issued_attempts = state.issued_attempts;
        for mut rec in state.tasks {
            if !rec.state.is_terminal() {
                rec.lease = None;
                rec.state = TaskState::Discovered;
            }
            cp.tasks.insert(rec.task_id, rec);
        }
        cp
    }

    // ── chain observation ─────────────────────────────────────────────

    /// Scans new blocks for tasks and receipts (E §4). Tasks become
    /// `Discovered`; a receipt for a known task marks it `Completed`, which
    /// is the only chain-derived terminal fact.
    pub async fn observe<C: ChainClient>(&mut self, chain: &C) -> Result<(), ChainError> {
        let latest = chain.latest_height().await?;
        self.latest_height = latest;
        if latest <= self.last_scanned && self.last_scanned != 0 {
            return Ok(());
        }
        let from = if self.last_scanned == 0 {
            1
        } else {
            self.last_scanned + 1
        };
        if from > latest {
            return Ok(());
        }
        for observed in scan_tasks(chain, from, latest).await? {
            self.tasks.entry(observed.task_id).or_insert_with(|| {
                log::info!(
                    "control-plane: task {} observed at height {} for executor {}",
                    hex::encode(&observed.task_id[..8]),
                    observed.height,
                    observed.task.executor
                );
                TaskRecord {
                    task: observed.task.clone(),
                    task_id: observed.task_id,
                    observed_height: observed.height,
                    state: TaskState::Discovered,
                    lease: None,
                    attempt_count: 0,
                    capabilities: Vec::new(),
                    input_object: None,
                    confidential_input: false,
                    last_failure: None,
                    submitted_tx: None,
                    completed_height: None,
                }
            });
        }
        for (height, receipt) in scan_receipts(chain, from, latest).await? {
            if let Some(rec) = self.tasks.get_mut(&receipt.task_id) {
                if rec.state != TaskState::Completed {
                    log::info!(
                        "control-plane: task {} completed at height {height}",
                        hex::encode(&receipt.task_id[..8])
                    );
                    rec.state = TaskState::Completed;
                    rec.completed_height = Some(height);
                    rec.lease = None;
                }
            }
        }
        self.last_scanned = latest;
        Ok(())
    }

    /// The client tells the control plane which private object serves a
    /// task (the input reference, F §4.2). Confidential; never on-chain.
    pub fn register_input(
        &mut self,
        task_id: [u8; 32],
        object: ObjectId,
    ) -> Result<(), ControlPlaneError> {
        let rec = self.tasks.get_mut(&task_id).ok_or(ControlPlaneError::UnknownTask)?;
        rec.input_object = Some(object);
        Ok(())
    }

    /// As [`Self::register_input`], for a **confidential** object: the
    /// ordinary fetch path refuses it, and its capability is issued only
    /// through [`Self::authorize_fetch_confidential`] against a redeemed
    /// release authorization (K18, K21, K25).
    pub fn register_confidential_input(
        &mut self,
        task_id: [u8; 32],
        object: ObjectId,
    ) -> Result<(), ControlPlaneError> {
        let rec = self.tasks.get_mut(&task_id).ok_or(ControlPlaneError::UnknownTask)?;
        rec.input_object = Some(object);
        rec.confidential_input = true;
        Ok(())
    }

    /// The coordination record for a task.
    pub fn task(&self, task_id: [u8; 32]) -> Option<&TaskRecord> {
        self.tasks.get(&task_id)
    }

    /// Records a chain-derived completion the worker observed itself.
    pub fn mark_completed(&mut self, task_id: [u8; 32], height: u64) {
        if let Some(rec) = self.tasks.get_mut(&task_id) {
            rec.state = TaskState::Completed;
            rec.completed_height = Some(height);
            rec.lease = None;
        }
    }

    // ── sessions ──────────────────────────────────────────────────────

    /// A fresh challenge for a worker claiming `executor`.
    pub fn session_challenge(&mut self, executor: Address) -> Challenge {
        let ch = Challenge(self.ids.next("session-challenge"));
        self.session_challenges.insert(ch, (executor, self.now() + 120));
        ch
    }

    /// Opens a session once the worker proves possession of the executor
    /// key over the challenge (E §5). The session is bound to this instance
    /// and this executor.
    pub fn open_session(
        &mut self,
        worker_instance: WorkerInstanceId,
        executor: Address,
        spec_tags: Vec<Vec<u8>>,
        challenge: Challenge,
        proof: &[u8; 64],
    ) -> Result<Session, ControlPlaneError> {
        let (for_whom, expires) = self
            .session_challenges
            .remove(&challenge)
            .ok_or(ControlPlaneError::BadChallenge)?;
        if for_whom != executor || self.now() > expires {
            return Err(ControlPlaneError::BadChallenge);
        }
        if !verify_possession(
            &executor,
            DOMAIN_SESSION,
            &worker_instance.0,
            &challenge,
            proof,
        ) {
            log::warn!("control-plane: session refused — possession proof invalid for {executor}");
            return Err(ControlPlaneError::BadPossessionProof);
        }
        let now = self.now();
        let session = Session {
            session_id: SessionId(self.ids.next("session")),
            worker_instance,
            executor,
            spec_tags,
            last_seen: now,
            expires_at: now + self.config.session_secs,
        };
        self.sessions.insert(session.session_id, session.clone());
        log::info!(
            "control-plane: session {} opened for instance {} as {executor}",
            session.session_id,
            worker_instance
        );
        Ok(session)
    }

    fn live_session(&mut self, session_id: SessionId) -> Result<Session, ControlPlaneError> {
        let now = self.now();
        let s = self.sessions.get_mut(&session_id).ok_or(ControlPlaneError::UnknownSession)?;
        if now > s.expires_at {
            return Err(ControlPlaneError::SessionExpired);
        }
        s.last_seen = now;
        s.expires_at = now + self.config.session_secs;
        Ok(s.clone())
    }

    // ── leases ────────────────────────────────────────────────────────

    /// Expires leases past `not_after` (E §9.2), revoking any capability
    /// still unconsumed under them so a stale holder is fenced at the data
    /// plane too (E §9.3).
    pub fn sweep(&mut self, dp: &mut InMemoryDataPlane) {
        let now = self.now();
        let expired: Vec<LeaseId> = self
            .leases
            .iter()
            .filter(|(_, r)| !r.released && now > r.lease.not_after)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.release_lease(id, dp, "expired");
        }
    }

    fn release_lease(&mut self, lease_id: LeaseId, dp: &mut InMemoryDataPlane, why: &str) {
        let Some(rec) = self.leases.get_mut(&lease_id) else {
            return;
        };
        rec.released = true;
        let lease = rec.lease.clone();
        log::info!("control-plane: lease {lease_id} released ({why})");
        if let Some(task) = self.tasks.get_mut(&lease.task_id) {
            if task.lease == Some(lease_id) {
                task.lease = None;
                if !task.state.is_terminal() {
                    task.state = if task.last_failure.is_some() {
                        TaskState::Failed
                    } else {
                        TaskState::Discovered
                    };
                }
            }
            for (cap_id, attempt) in task.capabilities.clone() {
                if attempt == lease.attempt_id {
                    // Best effort; an already-consumed grant is unaffected.
                    let _ = dp.revoke(&self.issuer, lease.task_id, cap_id);
                }
            }
        }
    }

    /// Offers the session an admissible task under a fresh lease, or
    /// `None`. Every chain-derived check is taken from the observed block;
    /// the executor is never chosen here (E §6).
    pub fn offer(
        &mut self,
        session_id: SessionId,
        dp: &mut InMemoryDataPlane,
    ) -> Result<Option<Lease>, ControlPlaneError> {
        let session = self.live_session(session_id)?;
        self.sweep(dp);
        let now = self.now();
        // An instance that already holds a live lease on an open task
        // continues it (E §9): the same attempt, not a new one.
        if let Some(existing) = self
            .leases
            .values()
            .filter(|r| !r.released && now <= r.lease.not_after)
            .filter(|r| {
                r.lease.worker_instance == session.worker_instance
                    && r.lease.executor == session.executor
            })
            .find(|r| {
                self.tasks
                    .get(&r.lease.task_id)
                    .is_some_and(|t| t.lease == Some(r.lease.lease_id) && !t.state.is_terminal())
            })
            .map(|r| r.lease.clone())
        {
            return Ok(Some(existing));
        }
        let depth = self.config.confirmation_depth;
        let latest = self.latest_height;
        let candidate = self
            .tasks
            .values()
            .filter(|t| !t.state.is_terminal())
            .filter(|t| t.task.executor == session.executor)
            .filter(|t| t.observed_height + depth <= latest)
            .filter(|t| session.spec_tags.contains(&t.task.execution_spec))
            .filter(|t| t.lease.is_none())
            .filter(|t| t.input_object.is_some())
            .min_by_key(|t| t.observed_height)
            .map(|t| t.task_id);
        let Some(task_id) = candidate else {
            return Ok(None);
        };
        self.issued_attempts += 1;
        let attempt_id = AttemptId(self.ids.next("attempt"));
        let mut lease = Lease {
            lease_id: LeaseId(self.ids.next("lease")),
            task_id,
            executor: session.executor,
            worker_instance: session.worker_instance,
            attempt_id,
            not_after: now + self.config.lease_secs,
            issuer: self.issuer.address(),
            issuer_signature: [0u8; 64],
        };
        lease.issuer_signature = self.issuer.sign(&lease.signing_message());
        self.leases.insert(
            lease.lease_id,
            LeaseRecord {
                lease: lease.clone(),
                last_seen: now,
                released: false,
            },
        );
        let rec = self.tasks.get_mut(&task_id).expect("candidate exists");
        rec.lease = Some(lease.lease_id);
        rec.attempt_count += 1;
        rec.state = TaskState::Leased;
        log::info!(
            "control-plane: lease {} (attempt {}) issued to instance {} for task {}",
            lease.lease_id,
            attempt_id,
            session.worker_instance,
            hex::encode(&task_id[..8])
        );
        Ok(Some(lease))
    }

    /// The live lease behind `lease_id` for this session, or `StaleLease`.
    fn live_lease(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
    ) -> Result<Lease, ControlPlaneError> {
        let session = self.live_session(session_id)?;
        let now = self.now();
        let rec = self.leases.get_mut(&lease_id).ok_or(ControlPlaneError::UnknownLease)?;
        if rec.lease.worker_instance != session.worker_instance
            || rec.lease.executor != session.executor
        {
            return Err(ControlPlaneError::UnknownLease);
        }
        if rec.released || now > rec.lease.not_after {
            return Err(ControlPlaneError::StaleLease);
        }
        let current = self.tasks.get(&rec.lease.task_id).and_then(|t| t.lease);
        if current != Some(lease_id) {
            return Err(ControlPlaneError::StaleLease);
        }
        rec.last_seen = now;
        Ok(rec.lease.clone())
    }

    /// Renews the lease (E §9.2). A stale lease is refused, not revived.
    pub fn heartbeat(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
    ) -> Result<Lease, ControlPlaneError> {
        self.live_lease(session_id, lease_id)?;
        let now = self.now();
        let rec = self.leases.get_mut(&lease_id).expect("checked");
        rec.lease.not_after = now + self.config.lease_secs;
        rec.lease.issuer_signature = self.issuer.sign(&rec.lease.signing_message());
        Ok(rec.lease.clone())
    }

    /// Obtains a fetch-input capability under the lease, as the client's
    /// delegated issuer (E §7 step 4). Lease first, then capability.
    pub fn authorize_fetch(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
        dp: &mut InMemoryDataPlane,
    ) -> Result<Capability, ControlPlaneError> {
        let lease = self.live_lease(session_id, lease_id)?;
        let rec = self.tasks.get(&lease.task_id).ok_or(ControlPlaneError::UnknownTask)?;
        let object = rec.input_object.ok_or(ControlPlaneError::NoInputReference)?;
        if rec.confidential_input {
            log::warn!(
                "control-plane: ordinary fetch refused for confidential task {} — release authorization required",
                hex::encode(&lease.task_id[..8])
            );
            return Err(ControlPlaneError::ConfidentialReleaseRequired);
        }
        let cap = dp.issue_capability(
            &self.issuer,
            &CapabilityRequest {
                task_id: lease.task_id,
                operation: Operation::FetchInput,
                resource: Some(object),
                ttl_secs: self.config.capability_secs,
                max_uses: 1,
            },
        )?;
        let rec = self.tasks.get_mut(&lease.task_id).expect("checked");
        rec.capabilities.push((cap.capability_id, lease.attempt_id));
        rec.state = TaskState::InputAuthorized;
        Ok(cap)
    }

    /// The confidential counterpart of [`Self::authorize_fetch`] (K §14):
    /// redeems `release_id` with the release authority — once, inside this
    /// session, for this lease's task — and only then issues the fetch
    /// capability. The grant it returns is what the key service wraps the
    /// content key against. Lease first, then release, then capability.
    pub fn authorize_fetch_confidential(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
        release_id: ReleaseId,
        authority: &mut ReleaseAuthority,
        dp: &mut InMemoryDataPlane,
    ) -> Result<(Capability, ReleaseGrant), ControlPlaneError> {
        let lease = self.live_lease(session_id, lease_id)?;
        let session = self
            .sessions
            .get(&session_id)
            .cloned()
            .ok_or(ControlPlaneError::UnknownSession)?;
        let rec = self.tasks.get(&lease.task_id).ok_or(ControlPlaneError::UnknownTask)?;
        let object = rec.input_object.ok_or(ControlPlaneError::NoInputReference)?;
        if !rec.confidential_input {
            return Err(ControlPlaneError::NoInputReference);
        }
        let grant = authority.redeem(&session, release_id, lease.task_id)?;
        debug_assert_eq!(grant.executor, lease.executor);
        let cap = dp.issue_capability(
            &self.issuer,
            &CapabilityRequest {
                task_id: lease.task_id,
                operation: Operation::FetchInput,
                resource: Some(object),
                ttl_secs: self.config.capability_secs,
                max_uses: 1,
            },
        )?;
        let rec = self.tasks.get_mut(&lease.task_id).expect("checked");
        rec.capabilities.push((cap.capability_id, lease.attempt_id));
        rec.state = TaskState::InputAuthorized;
        log::info!(
            "control-plane: confidential fetch authorized for task {} under release {}",
            hex::encode(&lease.task_id[..8]),
            release_id
        );
        Ok((cap, grant))
    }

    /// Obtains a put-result capability under the lease (E §7 step 8).
    pub fn authorize_put(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
        dp: &mut InMemoryDataPlane,
    ) -> Result<Capability, ControlPlaneError> {
        let lease = self.live_lease(session_id, lease_id)?;
        let cap = dp.issue_capability(
            &self.issuer,
            &CapabilityRequest {
                task_id: lease.task_id,
                operation: Operation::PutResult,
                resource: None,
                ttl_secs: self.config.capability_secs,
                max_uses: 1,
            },
        )?;
        let rec = self.tasks.get_mut(&lease.task_id).expect("checked");
        rec.capabilities.push((cap.capability_id, lease.attempt_id));
        Ok(cap)
    }

    /// Records what the worker reports under a live lease (E §20). A stale
    /// lease's reports are refused.
    pub fn report(
        &mut self,
        session_id: SessionId,
        lease_id: LeaseId,
        event: AttemptEvent,
        dp: &mut InMemoryDataPlane,
    ) -> Result<(), ControlPlaneError> {
        let lease = self.live_lease(session_id, lease_id)?;
        let rec = self.tasks.get_mut(&lease.task_id).ok_or(ControlPlaneError::UnknownTask)?;
        log::info!(
            "control-plane: attempt {} on task {} reports {event:?}",
            lease.attempt_id,
            hex::encode(&lease.task_id[..8])
        );
        match event {
            AttemptEvent::InputConsumed => rec.state = TaskState::InputConsumed,
            AttemptEvent::Started => rec.state = TaskState::Executing,
            AttemptEvent::ResultReady { .. } => rec.state = TaskState::ResultPersisted,
            AttemptEvent::ReceiptSubmitted { tx_hash } => {
                rec.state = TaskState::ReceiptSubmitted;
                rec.submitted_tx = Some(tx_hash);
            }
            AttemptEvent::Failed(class) => {
                rec.last_failure = Some(class);
                self.release_lease(lease_id, dp, "failed");
            }
        }
        Ok(())
    }

    /// Whether `lease_id` is the current live lease of its task.
    pub fn lease_is_live(&self, lease_id: LeaseId) -> bool {
        self.leases
            .get(&lease_id)
            .is_some_and(|r| !r.released && self.now() <= r.lease.not_after)
    }

    /// Verifies a lease's issuer signature.
    pub fn lease_signature_valid(lease: &Lease) -> bool {
        crate::identity::verify_signature(
            &lease.issuer,
            &lease.signing_message(),
            &lease.issuer_signature,
        )
    }
}
