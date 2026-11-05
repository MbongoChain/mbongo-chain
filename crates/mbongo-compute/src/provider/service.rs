//! The provider service: one per independently operated provider.
//!
//! It holds the existing in-process control plane, private data plane and
//! worker, behind a small signed HTTP interface. There is no central
//! scheduler and no new protocol between those parts. It knows no workload:
//! job specs arrive as data, and runtimes are the provider's own.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use mbongo_core::Address;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::chain::{find_receipt, ChainClient};
use crate::clock::Clock;
use crate::control_plane::{ControlPlane, ControlPlaneConfig, DurableState, TaskState};
use crate::data_plane::{InMemoryDataPlane, LocalKey};
use crate::execution::{
    reference_input_commitment, reference_output_commitment, ExecutionError, ExecutionProfile,
};
use crate::identity::{ExecutorKey, IdSource};
use crate::job::{
    encode_execution_spec, job_spec_digest, parse_execution_spec, Determinism, JobError,
    JobProfile, JobSource, JobSpec, MeteringSink, RuntimeAdapter, RuntimeIdentity,
    UntrustedMetering, JOB_TAG,
};
use crate::mvp::client::random_bytes;
use crate::worker::{AttemptOutcome, Worker};

use super::auth::{self, AuthHeaders, Received};
use super::containment::ContainmentProof;
use super::durable::{
    AcceptError, DurableStore, HexBytes, RetentionPolicy, StoreError, TaskEntry, Terminal,
    UploadEntry,
};
use super::journal::JournalError;
use super::runner::RUNNER_CONTRACT;

/// Largest request body.
pub const MAX_BODY_BYTES: usize = 256 * 1024;
/// Most job specs held at once.
const MAX_JOB_SPECS: usize = 256;
/// Most metering records kept.
const MAX_METERING_RECORDS: usize = 1_024;
/// A task the control plane observed for this executor is kept across a
/// restart for this many blocks, so that a client can still register it.
const RECENT_TASK_BLOCKS: u64 = 10_000;
/// The job failure after a crash that cannot be safely re-run.
pub const EXECUTION_INTERRUPTED: &str = "PROVIDER_EXECUTION_INTERRUPTED";
/// Attempts ran out on a failure outside the job: input, persistence, receipt.
pub const ATTEMPTS_EXHAUSTED: &str = "PROVIDER_ATTEMPTS_EXHAUSTED";
/// An execution that is not the task the loop admitted. Never expected.
const TASK_NOT_ADMITTED: &str = "PROVIDER_TASK_NOT_ADMITTED";
/// A task on the journal that the chain no longer shows.
pub const TASK_LOST: &str = "PROVIDER_TASK_LOST";

/// What this provider accepts. A job above any of these is refused before
/// it reaches the chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderLimits {
    /// Largest `max_wall_ms`. Must stay below the lease: the worker sends no
    /// heartbeat while a job runs.
    pub max_wall_ms: u64,
    /// Largest `input.max_bytes`.
    pub max_input_bytes: u64,
    /// Largest `output.max_bytes`.
    pub max_output_bytes: u64,
}

/// Deployment policy of the service. None of it is consensus.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// How long state is kept, and how much of it.
    pub retention: RetentionPolicy,
    /// Attempts after which a retryable failure parks the task.
    pub max_attempts: u32,
    /// The control plane's policy.
    pub control_plane: ControlPlaneConfig,
    /// What this provider accepts.
    pub limits: ProviderLimits,
    /// Set once every runtime runs inside an established containment.
    pub contained: Option<ContainmentProof>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            retention: RetentionPolicy::default(),
            max_attempts: 3,
            control_plane: ControlPlaneConfig {
                lease_secs: 900,
                session_secs: 3_600,
                confirmation_depth: 1,
                capability_secs: 3_600,
            },
            limits: ProviderLimits {
                max_wall_ms: 300_000,
                max_input_bytes: 65_536,
                max_output_bytes: 65_536,
            },
            contained: None,
        }
    }
}

/// Job specs in memory, artifacts in a directory the provider manages, named
/// by digest. A client names an artifact; it never supplies one.
pub struct ProviderStore {
    specs: StdMutex<BTreeMap<[u8; 32], Vec<u8>>>,
    artifact_dir: PathBuf,
}

impl ProviderStore {
    /// A store reading artifacts from `artifact_dir`.
    pub fn new(artifact_dir: PathBuf) -> Self {
        Self {
            specs: StdMutex::new(BTreeMap::new()),
            artifact_dir,
        }
    }

    fn holds(&self, digest: &[u8; 32]) -> bool {
        self.specs.lock().is_ok_and(|s| s.contains_key(digest))
    }

    fn is_full(&self) -> bool {
        self.specs.lock().map_or(true, |s| s.len() >= MAX_JOB_SPECS)
    }

    fn insert(&self, digest: [u8; 32], bytes: Vec<u8>) {
        if let Ok(mut specs) = self.specs.lock() {
            specs.insert(digest, bytes);
        }
    }
}

impl JobSource for ProviderStore {
    fn job_spec_digests(&self) -> Vec<[u8; 32]> {
        self.specs.lock().map(|s| s.keys().copied().collect()).unwrap_or_default()
    }

    fn job_spec(&self, digest: &[u8; 32]) -> Option<Vec<u8>> {
        self.specs.lock().ok()?.get(digest).cloned()
    }

    fn artifact(&self, sha256: &str) -> Option<Vec<u8>> {
        // Validated lowercase hex by the job spec: never a path.
        std::fs::read(self.artifact_dir.join(sha256)).ok()
    }
}

/// What a provider plugs in.
pub struct ProviderParts {
    /// Job specs and artifacts.
    pub store: Arc<ProviderStore>,
    /// The runtimes it offers.
    pub runtimes: Vec<Arc<dyn RuntimeAdapter>>,
    /// Where reported metering goes.
    pub metering: Arc<dyn MeteringSink>,
}

/// Reported metering, kept on the private side (D14). Untrusted.
#[derive(Debug, Default)]
pub struct MeteringLog(StdMutex<VecDeque<([u8; 32], UntrustedMetering)>>);

impl MeteringLog {
    /// Records held.
    pub fn len(&self) -> usize {
        self.0.lock().map_or(0, |log| log.len())
    }

    /// Whether nothing is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl MeteringSink for MeteringLog {
    fn record(&self, job_digest: &[u8; 32], metering: &UntrustedMetering) {
        if let Ok(mut log) = self.0.lock() {
            if log.len() == MAX_METERING_RECORDS {
                log.pop_front();
            }
            log.push_back((*job_digest, *metering));
        }
    }
}

/// The one task the worker may run now. Its `task_id` covers all six fields of
/// the task, salt included.
#[derive(Debug, Clone)]
struct Admitted {
    task_id: [u8; 32],
    execution_spec: Vec<u8>,
    input_commitment: [u8; 32],
}

/// [`JobProfile`] with its execution made durable, per task: the start is
/// recorded before the runner is called, and the output before `Worker` can
/// sign a receipt for it. A stored output is reused only by the exact task it
/// was produced for: the receipt must commit to that result (E §9.3).
///
/// `Worker` passes no task identity here, so the task loop admits one task at
/// a time and this profile refuses anything else. If a record cannot be
/// written, the runner is not called or its output is not used: this fails
/// closed.
struct TrackedProfile {
    job: JobProfile,
    store: Arc<DurableStore>,
    admitted: StdMutex<Option<Admitted>>,
}

impl TrackedProfile {
    fn admit(&self, task: Option<Admitted>) {
        if let Ok(mut admitted) = self.admitted.lock() {
            *admitted = task;
        }
    }

    fn run(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ExecutionError> {
        let state_failed = |e: &JournalError| ExecutionError::Failed(e.to_string());
        let admitted = self.admitted.lock().ok().and_then(|a| a.clone());
        let task_id = admitted
            .filter(|a| {
                a.execution_spec == execution_spec && a.input_commitment == *input_commitment
            })
            .map(|a| a.task_id)
            .ok_or_else(|| ExecutionError::Failed(TASK_NOT_ADMITTED.to_owned()))?;
        let stored = self.store.read(|s| s.outputs.get(&task_id).cloned());
        if let Some(output) = stored.map_err(|e| state_failed(&e))? {
            return Ok(output);
        }
        self.store.started(task_id).map_err(|e| state_failed(&e))?;
        match self.job.run(execution_spec, input_commitment, input) {
            Ok(output) => {
                self.store.output(task_id, &output).map_err(|e| state_failed(&e))?;
                Ok(output)
            }
            Err(e) => {
                let code = e.to_string();
                self.store.attempt_failed(task_id, &code).map_err(|e| state_failed(&e))?;
                Err(ExecutionError::Failed(code))
            }
        }
    }
}

/// The worker's profile, shared so the worker can be rebuilt.
struct SharedProfile(Arc<TrackedProfile>);

impl ExecutionProfile for SharedProfile {
    fn spec_tag(&self) -> &'static [u8] {
        JOB_TAG
    }
    fn spec_tags(&self) -> Vec<Vec<u8>> {
        self.0.job.spec_tags()
    }
    fn accepts(&self, execution_spec: &[u8]) -> bool {
        self.0.job.accepts(execution_spec)
    }
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>, ExecutionError> {
        self.0.job.execute(input)
    }
    fn execute_task(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, ExecutionError> {
        self.0.run(execution_spec, input_commitment, input)
    }
}

/// A claim about one runtime. Self-declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeClaim {
    /// Family.
    pub family: String,
    /// Revision.
    pub revision: String,
    /// Accelerator class.
    pub accelerator_class: String,
    /// Build hash, if declared.
    pub binary_sha256: Option<String>,
}

/// The provider's declaration. **Every field is a claim**, not a proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// `capability-advertisement-v1`.
    pub schema: String,
    /// `DECLARED`: nothing here was independently verified.
    pub evidence: String,
    /// The executor a client names, and the signature's recipient.
    pub executor: String,
    /// `mbongo-runner:v1`.
    pub runner_contract: String,
    /// The runtimes offered.
    pub runtimes: Vec<RuntimeClaim>,
    /// `PUBLIC`: the provider sees plaintext.
    pub confidentiality: Vec<String>,
    /// What this provider accepts.
    pub limits: ProviderLimits,
    /// Whether startup proved a runner cannot read this provider's keys.
    pub key_isolation_enforced: bool,
    /// Whether `max_memory_mib` is enforced on the runner's whole tree.
    pub memory_limit_enforced: bool,
    /// Whether startup proved the runner has no loopback network.
    pub runner_network_denied: bool,
}

/// `PUT /v1/job-specs` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpecAccepted {
    /// Hex `execution_spec` the client commits.
    pub execution_spec: String,
}

/// `PUT /v1/inputs` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRequest {
    /// Hex task id.
    pub task_id: String,
    /// Hex input bytes.
    pub input: String,
}

/// `PUT /v1/inputs` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    /// Hex object reference, for registration.
    pub object_ref: String,
    /// Hex input commitment the service computed.
    pub input_commitment: String,
}

/// `POST /v1/tasks/{task_id}/register` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// Hex object reference.
    pub object_ref: String,
}

/// `GET /v1/tasks/{task_id}` response. Codes and counts only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    /// Hex task id.
    pub task_id: String,
    /// Lifecycle state.
    pub state: String,
    /// Height of the block that carries the task.
    pub task_height: u64,
    /// Attempts issued.
    pub attempts: u32,
    /// The service stopped offering the task.
    pub parked: bool,
    /// The job failure code, if the last attempt failed.
    pub failure_code: Option<String>,
    /// A result is held.
    pub result_available: bool,
    /// Height of the anchored receipt.
    pub receipt_height: Option<u64>,
}

/// An error response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    /// The code.
    pub error: String,
}

struct ApiError(StatusCode, String);

impl ApiError {
    fn new(status: StatusCode, code: &str) -> Self {
        Self(status, code.to_owned())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(ErrorBody { error: self.1 })).into_response()
    }
}

struct Core {
    ids: IdSource,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    worker: Worker,
    /// Open registered tasks, for the task loop. The durable store decides
    /// what a task's state is.
    open: BTreeSet<[u8; 32]>,
    /// The one open task handed to the control plane. Tasks run one at a
    /// time, so that an execution always has an exact identity.
    current: Option<[u8; 32]>,
    /// A job spec arrived after the worker's session opened: a session
    /// advertises the `execution_spec` values known when it opened.
    session_stale: bool,
}

struct Inner<C> {
    core: Mutex<Core>,
    chain: C,
    clock: Arc<dyn Clock>,
    owner: LocalKey,
    issuer: LocalKey,
    executor: ExecutorKey,
    store: Arc<ProviderStore>,
    durable: Arc<DurableStore>,
    identities: Vec<RuntimeIdentity>,
    profile: Arc<TrackedProfile>,
    config: ProviderConfig,
    busy: AtomicBool,
}

/// The service. Cheap to clone; every clone shares one state.
pub struct ProviderService<C> {
    inner: Arc<Inner<C>>,
}

impl<C> Clone for ProviderService<C> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

fn parse_hex32(s: &str, code: &str) -> Result<[u8; 32], ApiError> {
    hex::decode(s)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, code))
}

fn state_name(state: TaskState) -> &'static str {
    match state {
        TaskState::Discovered => "queued",
        TaskState::Leased => "leased",
        TaskState::InputAuthorized => "input_authorized",
        TaskState::InputConsumed => "input_consumed",
        TaskState::Executing => "executing",
        TaskState::ResultPersisted => "result_persisted",
        TaskState::ReceiptSubmitted => "receipt_submitted",
        TaskState::Completed => "completed",
        TaskState::Failed => "failed",
    }
}

fn multi_threaded() -> bool {
    tokio::runtime::Handle::try_current().is_ok_and(|h| {
        matches!(
            h.runtime_flavor(),
            tokio::runtime::RuntimeFlavor::MultiThread
        )
    })
}

/// How an interrupted task recovers.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Recovery {
    /// Offer it. A stored output, if any, is reused.
    Resume,
    /// Stop, with this code.
    Park(String),
}

/// Why a service could not open.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// Durable state did not load: the service must not start.
    #[error("{0}")]
    State(#[from] JournalError),
    /// The chain could not be observed, so recovery cannot be decided.
    #[error("chain: {0}")]
    Chain(#[from] crate::chain::ChainError),
}

/// Whether running a job again is safe to do without being asked.
fn rerunnable(spec: &JobSpec) -> bool {
    matches!(
        spec.determinism,
        Determinism::BitExact | Determinism::BitExactPinned
    )
}

impl<C: ChainClient + Clone + 'static> ProviderService<C> {
    /// Opens a service over `durable`, recovering whatever it holds. Fails
    /// closed: without its durable state or the chain, it does not start.
    ///
    /// `seed` derives this run's owner key, the control plane's issuer key
    /// and every identifier; production passes 32 bytes from the OS RNG.
    pub async fn open(
        chain: C,
        clock: Arc<dyn Clock>,
        seed: [u8; 32],
        executor: ExecutorKey,
        parts: ProviderParts,
        durable: Arc<DurableStore>,
        config: ProviderConfig,
    ) -> Result<Self, OpenError> {
        let ProviderParts {
            store,
            runtimes,
            metering,
        } = parts;
        for bytes in durable.read(|s| s.specs.clone())? {
            store.insert(job_spec_digest(&bytes), bytes);
        }
        let identities = runtimes.iter().map(|r| r.identity()).collect();
        let job = JobProfile::new(Arc::clone(&store) as Arc<dyn JobSource>, runtimes, metering);
        let profile = Arc::new(TrackedProfile {
            job,
            store: Arc::clone(&durable),
            admitted: StdMutex::new(None),
        });
        let mut ids = IdSource::new(seed);
        let owner = LocalKey::from_seed(&ids.next("provider-owner"));
        let issuer = LocalKey::from_seed(&ids.next("provider-issuer"));
        let mut restored = durable.read(|s| s.control_plane.clone())?.unwrap_or_default();
        // The data plane is rebuilt: an old object reference means nothing.
        for rec in &mut restored.tasks {
            rec.input_object = None;
        }
        let mut cp = ControlPlane::restore(
            restored,
            Arc::clone(&clock),
            IdSource::new(ids.next("control-plane")),
            issuer.clone(),
            config.control_plane.clone(),
        );
        cp.observe(&chain).await?;
        let dp = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(ids.next("data-plane")));
        let worker = Worker::new(
            &mut ids,
            executor.clone(),
            Box::new(SharedProfile(Arc::clone(&profile))),
        );
        let service = Self {
            inner: Arc::new(Inner {
                core: Mutex::new(Core {
                    ids,
                    cp,
                    dp,
                    worker,
                    open: BTreeSet::new(),
                    current: None,
                    session_stale: false,
                }),
                chain,
                clock,
                owner,
                issuer,
                executor,
                store,
                durable,
                identities,
                profile,
                config,
                busy: AtomicBool::new(false),
            }),
        };
        service.recover().await?;
        Ok(service)
    }

    /// Decides what happens to each open task after a restart.
    ///
    /// | Found | Behaviour |
    /// |---|---|
    /// | Receipt on-chain | Completed; the stored output is bound if it matches the receipt |
    /// | Output stored, no receipt | Offered: the output is reused, the runner is not called |
    /// | Started, no output, deterministic job, attempts left | Offered: the runner runs again |
    /// | Started, no output, otherwise | Parked: `PROVIDER_EXECUTION_INTERRUPTED` |
    /// | Failure that a retry cannot change | Parked with that code |
    /// | Registered, never started | Offered |
    /// | Not on the chain any more | Parked: `PROVIDER_TASK_LOST` |
    async fn recover(&self) -> Result<(), OpenError> {
        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        let now = self.inner.clock.now();
        let open: Vec<[u8; 32]> = self.inner.durable.read(|s| {
            s.tasks
                .iter()
                .filter(|(_, t)| t.terminal.is_none())
                .map(|(id, _)| *id)
                .collect()
        })?;
        for task_id in open {
            let Some(rec) = core.cp.task(task_id) else {
                self.park_entry(task_id, TASK_LOST, 0, now)?;
                continue;
            };
            let attempts = rec.attempt_count;
            if rec.state == TaskState::Completed {
                continue;
            }
            match self.recovery_of(task_id, &rec.task.execution_spec, attempts)? {
                Recovery::Park(reason) => self.park_entry(task_id, &reason, attempts, now)?,
                Recovery::Resume => {
                    core.open.insert(task_id);
                }
            }
        }
        self.bind_completed(core).await?;
        Ok(())
    }

    fn recovery_of(
        &self,
        task_id: [u8; 32],
        execution_spec: &[u8],
        attempts: u32,
    ) -> Result<Recovery, JournalError> {
        let (output, started, failure) = self.inner.durable.read(|s| {
            (
                s.outputs.contains_key(&task_id),
                s.started.get(&task_id).copied().unwrap_or(0),
                s.last_failure.get(&task_id).cloned(),
            )
        })?;
        if output {
            return Ok(Recovery::Resume);
        }
        if started == 0 {
            return Ok(match failure {
                Some(code) if code != JobError::RuntimeFailed.to_string() => Recovery::Park(code),
                _ => Recovery::Resume,
            });
        }
        let spec = parse_execution_spec(execution_spec)
            .ok()
            .and_then(|digest| self.inner.store.job_spec(&digest))
            .and_then(|bytes| JobSpec::from_canonical_bytes(&bytes).ok());
        let safe = spec.as_ref().is_some_and(rerunnable);
        Ok(
            if safe && attempts.max(started) < self.inner.config.max_attempts {
                Recovery::Resume
            } else {
                Recovery::Park(EXECUTION_INTERRUPTED.to_owned())
            },
        )
    }

    fn park_entry(
        &self,
        task_id: [u8; 32],
        code: &str,
        attempts: u32,
        now: u64,
    ) -> Result<(), JournalError> {
        self.inner.durable.terminal(
            task_id,
            Terminal::Parked {
                code: code.to_owned(),
                attempts,
            },
            now,
        )
    }

    /// Hands a task's input to the data plane and the control plane.
    fn admit(
        &self,
        core: &mut Core,
        task_id: [u8; 32],
        entry: &TaskEntry,
    ) -> Result<(), &'static str> {
        let input = entry.input.as_ref().ok_or(TASK_LOST)?;
        let executor = self.executor();
        let object = core.dp.store_input(
            &self.inner.owner,
            task_id,
            entry.input_commitment,
            input.0.clone(),
            self.inner.config.retention.result_retention_secs,
        );
        core.dp
            .register_task(&self.inner.owner, task_id, executor)
            .map_err(|_| TASK_LOST)?;
        core.dp
            .delegate_issuer(&self.inner.owner, task_id, core.cp.issuer_address())
            .map_err(|_| TASK_LOST)?;
        core.cp.register_input(task_id, object).map_err(|_| TASK_LOST)?;
        Ok(())
    }

    /// Keeps the current task until it has an outcome, then admits the oldest
    /// open one. Only an admitted task has an input the control plane can
    /// offer, so an execution is always the task recorded here.
    fn admit_next(&self, core: &mut Core) -> Result<(), JournalError> {
        if core.current.is_some_and(|id| core.open.contains(&id)) {
            return Ok(());
        }
        core.current = None;
        self.inner.profile.admit(None);
        let oldest = |core: &Core| {
            core.open
                .iter()
                .copied()
                .min_by_key(|id| core.cp.task(*id).map_or(u64::MAX, |r| r.observed_height))
        };
        while let Some(task_id) = oldest(core) {
            let entry = self.inner.durable.read(|s| s.tasks.get(&task_id).cloned())?;
            let (execution_spec, input_commitment, attempts) = match core.cp.task(task_id) {
                Some(r) => (
                    r.task.execution_spec.clone(),
                    r.task.input_commitment,
                    r.attempt_count,
                ),
                None => (Vec::new(), [0u8; 32], 0),
            };
            let admitted =
                entry.ok_or(TASK_LOST).and_then(|entry| self.admit(core, task_id, &entry));
            if let Err(reason) = admitted {
                self.park_entry(task_id, reason, attempts, self.inner.clock.now())?;
                core.open.remove(&task_id);
                continue;
            }
            self.inner.profile.admit(Some(Admitted {
                task_id,
                execution_spec,
                input_commitment,
            }));
            core.current = Some(task_id);
            break;
        }
        Ok(())
    }

    /// Makes a receipt observed on-chain a durable outcome. The stored output
    /// is bound only if it is what the receipt commits to; otherwise the task
    /// is completed with no result, and nothing is run again. If the receipt
    /// cannot be read now, the next tick tries again.
    async fn bind_completed(&self, core: &mut Core) -> Result<(), JournalError> {
        let now = self.inner.clock.now();
        let pending: Vec<[u8; 32]> = self.inner.durable.read(|s| {
            s.tasks
                .iter()
                .filter(|(_, t)| t.terminal.is_none())
                .map(|(id, _)| *id)
                .collect()
        })?;
        for task_id in pending {
            let height = core.cp.task(task_id).and_then(|r| {
                (r.state == TaskState::Completed).then_some(r.completed_height).flatten()
            });
            let Some(receipt_height) = height else {
                continue;
            };
            let found = find_receipt(&self.inner.chain, task_id, receipt_height, receipt_height);
            let Ok(Some((_, receipt))) = found.await else {
                continue;
            };
            let output = self
                .inner
                .durable
                .read(|s| s.outputs.get(&task_id).cloned())?
                .filter(|o| reference_output_commitment(o) == receipt.output_commitment);
            self.inner.durable.terminal(
                task_id,
                Terminal::Completed {
                    receipt_height,
                    output: output.map(HexBytes),
                },
                now,
            )?;
            core.open.remove(&task_id);
        }
        Ok(())
    }

    /// The durable coordination state, kept small: tasks this executor can
    /// still be asked about. Other executors' tasks are never offered here.
    fn persist_control_plane(&self, core: &Core) -> Result<(), JournalError> {
        let mut state: DurableState = core.cp.snapshot();
        let executor = self.executor();
        let scanned = state.last_scanned;
        let known = self.inner.durable.read(|s| {
            s.tasks
                .keys()
                .copied()
                .chain(s.uploads.keys().map(|(id, _)| *id))
                .collect::<std::collections::HashSet<_>>()
        })?;
        state.tasks.retain(|r| {
            r.task.executor == executor
                && (known.contains(&r.task_id)
                    || r.observed_height.saturating_add(RECENT_TASK_BLOCKS) >= scanned)
        });
        self.inner.durable.control_plane(state)
    }

    /// The executor this provider runs: its identity for clients.
    pub fn executor(&self) -> Address {
        self.inner.executor.address()
    }

    /// The router.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/v1/capabilities", get(capabilities_route::<C>))
            .route("/v1/job-specs", put(job_spec_route::<C>))
            .route("/v1/inputs", put(upload_route::<C>))
            .route("/v1/tasks/:task_id/register", post(register_route::<C>))
            .route("/v1/tasks/:task_id", get(status_route::<C>))
            .route("/v1/tasks/:task_id/result", get(result_route::<C>))
            .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
            .with_state(self.clone())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            schema: "capability-advertisement-v1".to_owned(),
            evidence: "DECLARED".to_owned(),
            executor: hex::encode(self.executor().0),
            runner_contract: RUNNER_CONTRACT.to_owned(),
            runtimes: self
                .inner
                .identities
                .iter()
                .map(|i| RuntimeClaim {
                    family: i.family.clone(),
                    revision: i.revision.clone(),
                    accelerator_class: i.accelerator_class.clone(),
                    binary_sha256: i.binary_sha256.clone(),
                })
                .collect(),
            confidentiality: vec!["PUBLIC".to_owned()],
            limits: self.inner.config.limits,
            key_isolation_enforced: self.inner.config.contained.is_some(),
            memory_limit_enforced: self.inner.config.contained.is_some(),
            runner_network_denied: self
                .inner
                .config
                .contained
                .is_some_and(ContainmentProof::network_denied),
        }
    }

    // ── the task loop ────────────────────────────────────────────────

    /// One step: observe the chain, park finished failures, run one attempt
    /// if a registered task is open, observe again.
    pub async fn tick(&self) -> Result<Option<AttemptOutcome>, crate::chain::ChainError> {
        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        core.cp.observe(&self.inner.chain).await?;
        core.cp.sweep(&mut core.dp);
        self.settle(core).await;
        if core.session_stale && Self::quiet(core) {
            core.worker = self.new_worker(&mut core.ids);
            core.session_stale = false;
        }
        if let Err(e) = self.admit_next(core) {
            log::error!("provider: outcome not persisted: {e}");
        }
        let open = core.current.and_then(|id| core.cp.task(id)).is_some_and(|r| {
            !matches!(r.state, TaskState::Completed | TaskState::ReceiptSubmitted)
        });
        let mut outcome = None;
        if open {
            self.inner.busy.store(true, Ordering::SeqCst);
            let attempt = core.worker.run_once(&mut core.cp, &mut core.dp, &self.inner.chain);
            let result = if multi_threaded() {
                tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(attempt))
            } else {
                attempt.await
            };
            self.inner.busy.store(false, Ordering::SeqCst);
            match result {
                Ok(o) => outcome = Some(o),
                Err(e) => log::warn!("provider: worker attempt error: {e}"),
            }
            core.cp.observe(&self.inner.chain).await?;
            self.settle(core).await;
            if let Err(e) = self.persist_control_plane(core) {
                log::error!("provider: control-plane state not persisted: {e}");
            }
        }
        if let Err(e) = self.inner.durable.maintain(self.inner.clock.now()) {
            log::error!("provider: state maintenance failed: {e}");
        }
        Ok(outcome)
    }

    /// Records outcomes: receipts seen on-chain, and failed tasks to park.
    async fn settle(&self, core: &mut Core) {
        if let Err(e) = self.bind_completed(core).await {
            log::error!("provider: outcome not persisted: {e}");
        }
        self.park_failed(core);
    }

    fn new_worker(&self, ids: &mut IdSource) -> Worker {
        Worker::new(
            ids,
            self.inner.executor.clone(),
            Box::new(SharedProfile(Arc::clone(&self.inner.profile))),
        )
    }

    /// No task depends on a live lease.
    fn quiet(core: &Core) -> bool {
        core.open.iter().all(|id| {
            core.cp.task(*id).map_or(true, |r| {
                matches!(
                    r.state,
                    TaskState::Discovered | TaskState::Failed | TaskState::Completed
                ) && r.lease.is_none()
            })
        })
    }

    /// Stops offering a failed task: at once when another attempt cannot
    /// change the outcome, otherwise after `max_attempts`. The control plane
    /// has no terminal failure, so this is the service's job. As in the MVP
    /// front, parking clears the input reference through the control plane's
    /// own durable-state restore, only when no task depends on a live lease.
    fn park_failed(&self, core: &mut Core) {
        let max = self.inner.config.max_attempts;
        let retryable = JobError::RuntimeFailed.to_string();
        let known = self.inner.durable.read(|s| s.last_failure.clone()).unwrap_or_default();
        let done: Vec<([u8; 32], String, u32)> = core
            .open
            .iter()
            .filter_map(|id| {
                let failure = known.get(id);
                let terminal = failure.is_some_and(|f| *f != retryable);
                core.cp.task(*id).and_then(|r| {
                    (r.state == TaskState::Failed
                        && r.lease.is_none()
                        && (terminal || r.attempt_count >= max))
                        .then(|| {
                            let reason =
                                failure.cloned().unwrap_or_else(|| ATTEMPTS_EXHAUSTED.to_owned());
                            (*id, reason, r.attempt_count)
                        })
                })
            })
            .collect();
        if done.is_empty() || !Self::quiet(core) {
            return;
        }
        let now = self.inner.clock.now();
        for (id, code, attempts) in &done {
            if let Err(e) = self.park_entry(*id, code, *attempts, now) {
                log::error!("provider: outcome not persisted: {e}");
                return;
            }
        }
        let mut state = core.cp.snapshot();
        for rec in &mut state.tasks {
            if done.iter().any(|(id, ..)| *id == rec.task_id) {
                rec.input_object = None;
            }
        }
        core.cp = ControlPlane::restore(
            state,
            Arc::clone(&self.inner.clock),
            IdSource::new(core.ids.next("control-plane-restore")),
            self.inner.issuer.clone(),
            self.inner.config.control_plane.clone(),
        );
        core.worker = self.new_worker(&mut core.ids);
        for (id, ..) in done {
            core.open.remove(&id);
        }
    }

    // ── route bodies ─────────────────────────────────────────────────

    fn authenticate(
        &self,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<Address, ApiError> {
        let h = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
        let now = self.inner.clock.now();
        let verified = auth::verify_request(
            AuthHeaders {
                address: h(auth::HEADER_ADDRESS),
                recipient: h(auth::HEADER_RECIPIENT),
                issued_at: h(auth::HEADER_ISSUED_AT),
                expires_at: h(auth::HEADER_EXPIRES_AT),
                request_id: h(auth::HEADER_REQUEST_ID),
                signature: h(auth::HEADER_SIGNATURE),
            },
            Received {
                provider: self.executor(),
                method: method.as_str(),
                path: uri.path(),
                body,
                now,
            },
        )
        .map_err(|e| ApiError(StatusCode::UNAUTHORIZED, e.to_string()))?;
        // Durable before acknowledged. If it cannot be recorded, the request
        // is refused: replay protection fails closed.
        self.inner
            .durable
            .accept_request(
                verified.signer,
                verified.request_id,
                verified.expires_at,
                now,
            )
            .map_err(|e| match e {
                AcceptError::Replayed => ApiError(StatusCode::UNAUTHORIZED, e.to_string()),
                AcceptError::Full | AcceptError::State(_) => ApiError(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "AUTH_REPLAY_STORE_UNAVAILABLE".to_owned(),
                ),
            })?;
        Ok(verified.signer)
    }

    /// Refuses, before anything reaches the chain, a job this provider
    /// cannot run. The worker checks all of it again on every task.
    fn preflight(&self, spec: &JobSpec) -> Result<(), JobError> {
        let family: Vec<&RuntimeIdentity> = self
            .inner
            .identities
            .iter()
            .filter(|i| i.family == spec.runtime.family)
            .collect();
        if family.is_empty() {
            return Err(JobError::RuntimeUnknown);
        }
        let need = &spec.runtime;
        let matched = family.iter().any(|i| {
            i.revision == need.revision
                && i.accelerator_class == need.accelerator_class
                && need
                    .binary_sha256
                    .as_ref()
                    .map_or(true, |pinned| i.binary_sha256.as_ref() == Some(pinned))
        });
        if !matched {
            return Err(JobError::RuntimeMismatch);
        }
        for a in &spec.artifacts {
            let bytes = self.inner.store.artifact(&a.sha256).ok_or(JobError::ArtifactNotFound)?;
            if u64::try_from(bytes.len()).ok() != Some(a.size_bytes) {
                return Err(JobError::ArtifactSizeMismatch);
            }
            if hex::encode(Sha256::digest(&bytes)) != a.sha256 {
                return Err(JobError::ArtifactDigestMismatch);
            }
        }
        Ok(())
    }

    async fn put_job_spec(&self, body: &[u8]) -> Result<Response, ApiError> {
        let refused = |e: &JobError| ApiError(StatusCode::UNPROCESSABLE_ENTITY, e.to_string());
        let spec = JobSpec::from_canonical_bytes(body).map_err(|e| refused(&e))?;
        let limits = self.inner.config.limits;
        if spec.max_wall_ms > limits.max_wall_ms
            || spec.input.max_bytes > limits.max_input_bytes
            || spec.output.max_bytes > limits.max_output_bytes
        {
            return Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "PROVIDER_LIMIT_EXCEEDED",
            ));
        }
        self.preflight(&spec).map_err(|e| refused(&e))?;
        let digest = job_spec_digest(body);
        if !self.inner.store.holds(&digest) {
            if self.inner.store.is_full() {
                return Err(ApiError::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    "TOO_MANY_JOB_SPECS",
                ));
            }
            self.inner.durable.add_spec(body).map_err(|e| state_error(&e))?;
            self.inner.store.insert(digest, body.to_vec());
            self.inner.core.lock().await.session_stale = true;
        }
        Ok((
            StatusCode::CREATED,
            Json(JobSpecAccepted {
                execution_spec: hex::encode(encode_execution_spec(&digest)),
            }),
        )
            .into_response())
    }

    fn upload(&self, caller: Address, body: &[u8]) -> Result<Response, ApiError> {
        let invalid = || ApiError::new(StatusCode::BAD_REQUEST, "INVALID_REQUEST");
        let req: UploadRequest = serde_json::from_slice(body).map_err(|_| invalid())?;
        let task_id = parse_hex32(&req.task_id, "INVALID_TASK_ID")?;
        let bytes = hex::decode(&req.input).map_err(|_| invalid())?;
        if u64::try_from(bytes.len()).map_or(true, |n| n > self.inner.config.limits.max_input_bytes)
        {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "INPUT_TOO_LARGE",
            ));
        }
        let commitment = reference_input_commitment(&bytes);
        let terminal = self
            .inner
            .durable
            .read(|s| s.tasks.contains_key(&task_id))
            .map_err(|e| state_error(&e))?;
        if terminal {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "TASK_ALREADY_REGISTERED",
            ));
        }
        let object_ref = random_bytes::<32>();
        let upload = UploadEntry {
            caller,
            object_ref,
            input_commitment: commitment,
            input: HexBytes(bytes),
            uploaded_at: self.inner.clock.now(),
        };
        self.inner.durable.add_upload(task_id, upload).map_err(|e| match e {
            StoreError::Full => ApiError::new(StatusCode::TOO_MANY_REQUESTS, "STORE_FULL"),
            StoreError::State(e) => state_error(&e),
        })?;
        Ok((
            StatusCode::CREATED,
            Json(UploadResponse {
                object_ref: hex::encode(object_ref),
                input_commitment: hex::encode(commitment),
            }),
        )
            .into_response())
    }

    async fn register(
        &self,
        caller: Address,
        task_id: [u8; 32],
        body: &[u8],
    ) -> Result<Response, ApiError> {
        let req: RegisterRequest = serde_json::from_slice(body)
            .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "INVALID_REQUEST"))?;
        let object_ref = parse_hex32(&req.object_ref, "INVALID_OBJECT_REF")?;
        let unprocessable = |code: &str| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, code);

        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        core.cp
            .observe(&self.inner.chain)
            .await
            .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "CHAIN_UNAVAILABLE"))?;
        let task = core
            .cp
            .task(task_id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "TASK_NOT_ON_CHAIN"))?
            .task
            .clone();
        if task.submitter != caller {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        if task.executor != self.executor() {
            return Err(unprocessable("EXECUTOR_NOT_SERVED"));
        }
        let known_entry = self
            .inner
            .durable
            .read(|s| s.tasks.get(&task_id).cloned())
            .map_err(|e| state_error(&e))?;
        if let Some(entry) = known_entry {
            if entry.object_ref != object_ref {
                return Err(unprocessable("OBJECT_REF_MISMATCH"));
            }
            return self.view_response(core, task_id);
        }
        let known = parse_execution_spec(&task.execution_spec)
            .ok()
            .is_some_and(|digest| self.inner.store.job_spec(&digest).is_some());
        if !known {
            return Err(unprocessable("UNKNOWN_JOB_SPEC"));
        }
        let upload = self
            .inner
            .durable
            .read(|s| s.uploads.get(&(task_id, caller)).cloned())
            .map_err(|e| state_error(&e))?
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "INPUT_NOT_UPLOADED"))?;
        if upload.object_ref != object_ref {
            return Err(unprocessable("OBJECT_REF_MISMATCH"));
        }
        if upload.input_commitment != task.input_commitment {
            return Err(unprocessable("INPUT_COMMITMENT_MISMATCH"));
        }
        let entry = TaskEntry {
            input: Some(upload.input),
            input_commitment: upload.input_commitment,
            object_ref,
            terminal: None,
            terminal_at: None,
        };
        self.inner.durable.register(task_id, entry).map_err(|e| state_error(&e))?;
        core.open.insert(task_id);
        self.persist_control_plane(core).map_err(|e| state_error(&e))?;
        self.view_response(core, task_id)
    }

    fn view_response(&self, core: &Core, task_id: [u8; 32]) -> Result<Response, ApiError> {
        let view = self.view(core, task_id).map_err(|e| state_error(&e))?;
        Ok(Json(view).into_response())
    }

    fn view(&self, core: &Core, task_id: [u8; 32]) -> Result<Option<JobStatus>, JournalError> {
        let Some(rec) = core.cp.task(task_id) else {
            return Ok(None);
        };
        self.inner.durable.read(|d| {
            let entry = d.tasks.get(&task_id);
            let terminal = entry.and_then(|t| t.terminal.as_ref());
            let failure = d.last_failure.get(&task_id).cloned();
            let (state, parked, failure_code, attempts) = match (entry, terminal) {
                (None, _) => ("not_registered".to_owned(), false, None, rec.attempt_count),
                (_, Some(Terminal::Parked { code, attempts })) => {
                    ("failed".to_owned(), true, Some(code.clone()), *attempts)
                }
                (_, Some(Terminal::Completed { .. })) => {
                    ("completed".to_owned(), false, None, rec.attempt_count)
                }
                (Some(_), None) => (
                    state_name(rec.state).to_owned(),
                    false,
                    failure,
                    rec.attempt_count,
                ),
            };
            let (result_available, receipt_height) = match terminal {
                Some(Terminal::Completed {
                    receipt_height,
                    output,
                }) => (output.is_some(), Some(*receipt_height)),
                _ => (false, rec.completed_height),
            };
            Some(JobStatus {
                task_id: hex::encode(task_id),
                state,
                task_height: rec.observed_height,
                attempts,
                parked,
                failure_code,
                result_available,
                receipt_height,
            })
        })
    }

    async fn status(&self, caller: Address, task_id: [u8; 32]) -> Result<Response, ApiError> {
        let core = self.inner.core.lock().await;
        let rec = core
            .cp
            .task(task_id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "TASK_NOT_FOUND"))?;
        if rec.task.submitter != caller {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        self.view_response(&core, task_id)
    }

    async fn result(&self, caller: Address, task_id: [u8; 32]) -> Result<Response, ApiError> {
        let submitter = {
            let core = self.inner.core.lock().await;
            core.cp
                .task(task_id)
                .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "TASK_NOT_FOUND"))?
                .task
                .submitter
        };
        if submitter != caller {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        let terminal = self
            .inner
            .durable
            .read(|d| d.tasks.get(&task_id).and_then(|t| t.terminal.clone()))
            .map_err(|e| state_error(&e))?;
        let bytes = match terminal {
            Some(Terminal::Completed {
                output: Some(output),
                ..
            }) => output.0,
            Some(Terminal::Completed { output: None, .. }) => {
                return Err(ApiError::new(StatusCode::GONE, "RESULT_LOST"));
            }
            _ => return Err(ApiError::new(StatusCode::CONFLICT, "RESULT_NOT_READY")),
        };
        let mut response = Bytes::from(bytes).into_response();
        let h = response.headers_mut();
        h.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        Ok(response)
    }
}

fn state_error(e: &JournalError) -> ApiError {
    log::error!("provider: state: {e}");
    ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "STATE_UNAVAILABLE")
}

// ── handlers ──────────────────────────────────────────────────────────────

async fn capabilities_route<C: ChainClient + Clone + 'static>(
    State(service): State<ProviderService<C>>,
) -> Response {
    Json(service.capabilities()).into_response()
}

async fn job_spec_route<C: ChainClient + Clone + 'static>(
    State(service): State<ProviderService<C>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let r = match service.authenticate(&method, &uri, &headers, &body) {
        Ok(_) => service.put_job_spec(&body).await,
        Err(e) => Err(e),
    };
    r.unwrap_or_else(IntoResponse::into_response)
}

async fn upload_route<C: ChainClient + Clone + 'static>(
    State(service): State<ProviderService<C>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let r = match service.authenticate(&method, &uri, &headers, &body) {
        Ok(caller) => service.upload(caller, &body),
        Err(e) => Err(e),
    };
    r.unwrap_or_else(IntoResponse::into_response)
}

async fn register_route<C: ChainClient + Clone + 'static>(
    State(service): State<ProviderService<C>>,
    Path(task_id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = service.authenticate(&method, &uri, &headers, &body);
    let r = match (auth, parse_hex32(&task_id, "INVALID_TASK_ID")) {
        (Ok(caller), Ok(id)) => service.register(caller, id, &body).await,
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    r.unwrap_or_else(IntoResponse::into_response)
}

async fn status_route<C: ChainClient + Clone + 'static>(
    State(service): State<ProviderService<C>>,
    Path(task_id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = service.authenticate(&method, &uri, &headers, &body);
    let r = match (auth, parse_hex32(&task_id, "INVALID_TASK_ID")) {
        (Ok(caller), Ok(id)) => service.status(caller, id).await,
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    r.unwrap_or_else(IntoResponse::into_response)
}

async fn result_route<C: ChainClient + Clone + 'static>(
    State(service): State<ProviderService<C>>,
    Path(task_id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = service.authenticate(&method, &uri, &headers, &body);
    let r = match (auth, parse_hex32(&task_id, "INVALID_TASK_ID")) {
        (Ok(caller), Ok(id)) => service.result(caller, id).await,
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    r.unwrap_or_else(IntoResponse::into_response)
}

#[cfg(test)]
#[path = "service_conformance.rs"]
mod conformance;
