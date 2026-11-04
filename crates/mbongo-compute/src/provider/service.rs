//! The provider service: one per independently operated provider.
//!
//! It holds the existing in-process control plane, private data plane and
//! worker, behind a small signed HTTP interface. There is no central
//! scheduler and no new protocol between those parts. It knows no workload:
//! job specs arrive as data, and runtimes are the provider's own.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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

use crate::chain::ChainClient;
use crate::clock::Clock;
use crate::control_plane::{ControlPlane, ControlPlaneConfig, TaskState};
use crate::data_plane::{
    CapabilityRequest, DataPlaneError, InMemoryDataPlane, LocalKey, Operation, Presentation,
};
use crate::execution::{reference_input_commitment, ExecutionError, ExecutionProfile};
use crate::identity::{ExecutorKey, IdSource, ObjectId};
use crate::job::{
    encode_execution_spec, job_spec_digest, parse_execution_spec, JobError, JobProfile, JobSource,
    JobSpec, MeteringSink, RuntimeAdapter, RuntimeIdentity, UntrustedMetering, JOB_TAG,
};
use crate::worker::{AttemptOutcome, Worker};

use super::auth::{self, AuthError, AuthHeaders, Received, ReplayGuard};
use super::runner::RUNNER_CONTRACT;

/// The runner receives no key, but a runner under the service's account can
/// still read the executor key file. **No key isolation is claimed.**
pub const KEY_ISOLATION_ENFORCED: bool = false;
/// Largest request body.
pub const MAX_BODY_BYTES: usize = 256 * 1024;
/// Most job specs held at once.
const MAX_JOB_SPECS: usize = 256;
/// Most uploads awaiting registration.
const MAX_PENDING_UPLOADS: usize = 64;
/// Most metering records kept.
const MAX_METERING_RECORDS: usize = 1_024;
/// Lifetime of a get-result grant the service issues to itself.
const RESULT_GRANT_SECS: u64 = 30;

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
    /// Data-plane lifetime of an uploaded input.
    pub input_ttl_secs: u64,
    /// Attempts after which a retryable failure parks the task.
    pub max_attempts: u32,
    /// The control plane's policy.
    pub control_plane: ControlPlaneConfig,
    /// What this provider accepts.
    pub limits: ProviderLimits,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            input_ttl_secs: 3_600,
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

    fn add(&self, digest: [u8; 32], bytes: Vec<u8>) -> Result<bool, ()> {
        let mut specs = self.specs.lock().map_err(|_| ())?;
        if specs.contains_key(&digest) {
            return Ok(false);
        }
        if specs.len() >= MAX_JOB_SPECS {
            return Err(());
        }
        specs.insert(digest, bytes);
        Ok(true)
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

/// [`JobProfile`], remembering the code of the last failure so that the task
/// loop can classify it. `Worker` reports only a generic failure.
struct TrackedProfile {
    job: JobProfile,
    last_failure: StdMutex<Option<JobError>>,
}

impl TrackedProfile {
    fn take_failure(&self) -> Option<JobError> {
        self.last_failure.lock().ok()?.take()
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
        self.0.job.run(execution_spec, input_commitment, input).map_err(|e| {
            let code = e.to_string();
            if let Ok(mut last) = self.0.last_failure.lock() {
                *last = Some(e);
            }
            ExecutionError::Failed(code)
        })
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
    /// Always `false` in this version.
    pub key_isolation_enforced: bool,
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

struct Upload {
    object: ObjectId,
    input_commitment: [u8; 32],
}

struct Core {
    ids: IdSource,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    worker: Worker,
    uploads: HashMap<([u8; 32], Address), Upload>,
    registered: HashMap<[u8; 32], ObjectId>,
    parked: HashSet<[u8; 32]>,
    failures: HashMap<[u8; 32], JobError>,
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
    identities: Vec<RuntimeIdentity>,
    profile: Arc<TrackedProfile>,
    config: ProviderConfig,
    replay: StdMutex<ReplayGuard>,
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

fn outcome_task(o: &AttemptOutcome) -> Option<[u8; 32]> {
    match o {
        AttemptOutcome::Idle => None,
        AttemptOutcome::Completed { task_id, .. }
        | AttemptOutcome::Submitted { task_id, .. }
        | AttemptOutcome::Failed { task_id, .. }
        | AttemptOutcome::Crashed { task_id, .. } => Some(*task_id),
    }
}

impl<C: ChainClient + Clone + 'static> ProviderService<C> {
    /// A service over a fresh in-process control plane and data plane.
    ///
    /// `seed` derives the service's owner key, the control plane's issuer key
    /// and every identifier; production passes 32 bytes from the OS RNG.
    pub fn new(
        chain: C,
        clock: Arc<dyn Clock>,
        seed: [u8; 32],
        executor: ExecutorKey,
        parts: ProviderParts,
        config: ProviderConfig,
    ) -> Self {
        let ProviderParts {
            store,
            runtimes,
            metering,
        } = parts;
        let identities = runtimes.iter().map(|r| r.identity()).collect();
        let job = JobProfile::new(Arc::clone(&store) as Arc<dyn JobSource>, runtimes, metering);
        let profile = Arc::new(TrackedProfile {
            job,
            last_failure: StdMutex::new(None),
        });
        let mut ids = IdSource::new(seed);
        let owner = LocalKey::from_seed(&ids.next("provider-owner"));
        let issuer = LocalKey::from_seed(&ids.next("provider-issuer"));
        let cp = ControlPlane::new(
            Arc::clone(&clock),
            IdSource::new(ids.next("control-plane")),
            issuer.clone(),
            config.control_plane.clone(),
        );
        let dp = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(ids.next("data-plane")));
        let worker = Worker::new(
            &mut ids,
            executor.clone(),
            Box::new(SharedProfile(Arc::clone(&profile))),
        );
        Self {
            inner: Arc::new(Inner {
                core: Mutex::new(Core {
                    ids,
                    cp,
                    dp,
                    worker,
                    uploads: HashMap::new(),
                    registered: HashMap::new(),
                    parked: HashSet::new(),
                    failures: HashMap::new(),
                    session_stale: false,
                }),
                chain,
                clock,
                owner,
                issuer,
                executor,
                store,
                identities,
                profile,
                config,
                replay: StdMutex::new(ReplayGuard::default()),
                busy: AtomicBool::new(false),
            }),
        }
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
            key_isolation_enforced: KEY_ISOLATION_ENFORCED,
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
        self.park_failed(core);
        if core.session_stale && Self::quiet(core) {
            core.worker = self.new_worker(&mut core.ids);
            core.session_stale = false;
        }
        let open = core.registered.keys().any(|id| {
            !core.parked.contains(id)
                && core.cp.task(*id).is_some_and(|r| {
                    !matches!(r.state, TaskState::Completed | TaskState::ReceiptSubmitted)
                })
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
            let failure = self.inner.profile.take_failure();
            match result {
                Ok(o) => {
                    if let (Some(id), Some(e)) = (outcome_task(&o), failure) {
                        core.failures.insert(id, e);
                    }
                    outcome = Some(o);
                }
                Err(e) => log::warn!("provider: worker attempt error: {e}"),
            }
            core.cp.observe(&self.inner.chain).await?;
            self.park_failed(core);
        }
        Ok(outcome)
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
        core.registered.keys().all(|id| {
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
        let done: Vec<[u8; 32]> = core
            .registered
            .keys()
            .filter(|id| !core.parked.contains(*id))
            .filter(|id| {
                let terminal = core.failures.get(*id).is_some_and(|e| !e.retryable());
                core.cp.task(**id).is_some_and(|r| {
                    r.state == TaskState::Failed
                        && r.lease.is_none()
                        && (terminal || r.attempt_count >= max)
                })
            })
            .copied()
            .collect();
        if done.is_empty() || !Self::quiet(core) {
            return;
        }
        let mut state = core.cp.snapshot();
        for rec in &mut state.tasks {
            if done.contains(&rec.task_id) {
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
        core.parked.extend(done);
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
        let mut replay = self
            .inner
            .replay
            .lock()
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"))?;
        auth::verify_request(
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
                now: self.inner.clock.now(),
            },
            &mut replay,
        )
        .map_err(|e| {
            let status = if e == AuthError::Busy {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::UNAUTHORIZED
            };
            ApiError(status, e.to_string())
        })
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
        let added = self
            .inner
            .store
            .add(digest, body.to_vec())
            .map_err(|()| ApiError::new(StatusCode::TOO_MANY_REQUESTS, "TOO_MANY_JOB_SPECS"))?;
        if added {
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

    async fn upload(&self, caller: Address, body: &[u8]) -> Result<Response, ApiError> {
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

        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        if core.registered.contains_key(&task_id) {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "TASK_ALREADY_REGISTERED",
            ));
        }
        let key = (task_id, caller);
        if !core.uploads.contains_key(&key) && core.uploads.len() >= MAX_PENDING_UPLOADS {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                "TOO_MANY_PENDING_UPLOADS",
            ));
        }
        let object = core.dp.store_input(
            &self.inner.owner,
            task_id,
            commitment,
            bytes,
            self.inner.config.input_ttl_secs,
        );
        let upload = Upload {
            object,
            input_commitment: commitment,
        };
        if let Some(old) = core.uploads.insert(key, upload) {
            let _ = core.dp.delete_object(&self.inner.owner, &old.object);
        }
        Ok((
            StatusCode::CREATED,
            Json(UploadResponse {
                object_ref: hex::encode(object.0),
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
        if let Some(object) = core.registered.get(&task_id) {
            if object.0 != object_ref {
                return Err(unprocessable("OBJECT_REF_MISMATCH"));
            }
            return Ok(Json(Self::view(core, task_id)).into_response());
        }
        let known = parse_execution_spec(&task.execution_spec)
            .ok()
            .is_some_and(|digest| self.inner.store.job_spec(&digest).is_some());
        if !known {
            return Err(unprocessable("UNKNOWN_JOB_SPEC"));
        }
        let upload = core
            .uploads
            .get(&(task_id, caller))
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "INPUT_NOT_UPLOADED"))?;
        if upload.object.0 != object_ref {
            return Err(unprocessable("OBJECT_REF_MISMATCH"));
        }
        if upload.input_commitment != task.input_commitment {
            return Err(unprocessable("INPUT_COMMITMENT_MISMATCH"));
        }
        let object = upload.object;
        let internal = || ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "REGISTRATION_FAILED");
        core.dp
            .register_task(&self.inner.owner, task_id, task.executor)
            .map_err(|_| internal())?;
        core.dp
            .delegate_issuer(&self.inner.owner, task_id, core.cp.issuer_address())
            .map_err(|_| internal())?;
        core.cp.register_input(task_id, object).map_err(|_| internal())?;
        core.uploads.retain(|(id, _), _| *id != task_id);
        core.registered.insert(task_id, object);
        Ok(Json(Self::view(core, task_id)).into_response())
    }

    fn view(core: &Core, task_id: [u8; 32]) -> Option<JobStatus> {
        let rec = core.cp.task(task_id)?;
        let parked = core.parked.contains(&task_id);
        let state = match (core.registered.contains_key(&task_id), parked) {
            (false, _) => "not_registered",
            (true, true) => "failed",
            (true, false) => state_name(rec.state),
        };
        Some(JobStatus {
            task_id: hex::encode(task_id),
            state: state.to_owned(),
            task_height: rec.observed_height,
            attempts: rec.attempt_count,
            parked,
            failure_code: core.failures.get(&task_id).map(ToString::to_string),
            result_available: core.dp.result_ref(task_id, rec.task.executor).is_some(),
            receipt_height: rec.completed_height,
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
        Ok(Json(Self::view(&core, task_id)).into_response())
    }

    async fn result(&self, caller: Address, task_id: [u8; 32]) -> Result<Response, ApiError> {
        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        let task = core
            .cp
            .task(task_id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "TASK_NOT_FOUND"))?
            .task
            .clone();
        if task.submitter != caller {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        let rr = core
            .dp
            .result_ref(task_id, task.executor)
            .ok_or_else(|| ApiError::new(StatusCode::CONFLICT, "RESULT_NOT_READY"))?;
        let unavailable = |e: DataPlaneError| match e {
            DataPlaneError::Expired => ApiError::new(StatusCode::GONE, "RESULT_EXPIRED"),
            _ => ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "RESULT_UNAVAILABLE"),
        };
        let grant = core
            .dp
            .issue_capability(
                &self.inner.owner,
                &CapabilityRequest {
                    task_id,
                    operation: Operation::GetResult,
                    resource: Some(rr.object),
                    ttl_secs: RESULT_GRANT_SECS,
                    max_uses: 1,
                },
            )
            .map_err(unavailable)?;
        let challenge = core.dp.issue_challenge(self.inner.owner.address());
        let bytes = core
            .dp
            .get_result(&Presentation::sign(grant, challenge, &self.inner.owner))
            .map_err(unavailable)?
            .as_bytes()
            .to_vec();
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
        Ok(caller) => service.upload(caller, &body).await,
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
