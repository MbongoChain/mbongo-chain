//! The MVP HTTP front: four routes over the existing control-plane and
//! private-data-plane operations, and the loop that drives the MVP-B worker.
//!
//! | Route | Existing operation |
//! |---|---|
//! | `PUT /v1/inputs` | F §4.1 `store_input` (commitment by the worker's own function) |
//! | `POST /v1/tasks/{task_id}/register` | F `register_task` + `delegate_issuer`, E `register_input` |
//! | `GET /v1/tasks/{task_id}` | E task record (lifecycle state), F `result_ref` |
//! | `GET /v1/tasks/{task_id}/result` | F §10 `get_result` under a single-use get-result grant |
//!
//! **Owner of record.** The data plane is in-process and keyed by local
//! signing keys (F §5.1); a remote client cannot hold one of its keys. The
//! front therefore acts as the data-plane owner of the objects it stores
//! and the get-result grants it issues never leave the process. Who may
//! act is decided by the chain, not by the front: a request is signed by
//! the client's chain key ([`super::auth`]), and only the committed task's
//! `submitter` may register it, read its status or retrieve its result.
//!
//! **What the front never decides.** The executor, the `execution_spec`
//! and the `input_commitment` come from the committed `ComputeTask`; the
//! front only refuses a task that is not the frozen MVP task, names an
//! executor this deployment does not run, or whose commitment differs from
//! the upload. It never stores a result copy of its own: result bytes are
//! read from the data plane on each request.
//!
//! **Logging.** One line per request — method, route, task-id prefix,
//! status, duration — and nothing from any body. No framework middleware
//! logs requests.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use mbongo_core::Address;
use tokio::sync::Mutex;

use crate::chain::ChainClient;
use crate::clock::Clock;
use crate::control_plane::{ControlPlane, ControlPlaneConfig, FailureClass, TaskState};
use crate::cpu_inference::InferenceObservation;
use crate::data_plane::{
    CapabilityRequest, DataPlaneError, InMemoryDataPlane, LocalKey, Operation, Presentation,
};
use crate::execution::ExecutionProfile;
use crate::identity::{ExecutorKey, IdSource, ObjectId};
use crate::inference::kaayu::{CvExtractionInputV1, INPUT_CONTRACT_V1};
use crate::worker::{AttemptOutcome, Worker};

use super::auth::{
    self, AuthError, AuthHeaders, ReplayGuard, HEADER_ADDRESS, HEADER_NONCE, HEADER_SIGNATURE,
    HEADER_TIMESTAMP,
};
use super::dto::{ErrorBody, RegisterRequest, StatusView, UploadRequest, UploadResponse};
use super::{frozen_execution_spec, MAX_BODY_BYTES, MAX_RESULT_BYTES};

/// Uploads awaiting registration, across all clients.
const MAX_PENDING_UPLOADS: usize = 64;
/// Lifetime of a get-result grant the front issues to itself.
const RESULT_GRANT_SECS: u64 = 30;

/// Deployment policy of the front. None of it is consensus.
#[derive(Debug, Clone)]
pub struct FrontConfig {
    /// The one executor this deployment runs (its worker's key).
    pub executor: Address,
    /// Data-plane lifetime of an uploaded input.
    pub input_ttl_secs: u64,
    /// Failed attempts after which the front stops offering a task.
    pub max_attempts: u32,
    /// The control plane's policy; leases must outlive an inference.
    pub control_plane: ControlPlaneConfig,
}

impl Default for FrontConfig {
    fn default() -> Self {
        Self {
            executor: Address::zero(),
            input_ttl_secs: 3_600,
            max_attempts: 3,
            control_plane: ControlPlaneConfig {
                lease_secs: 900,
                session_secs: 3_600,
                confirmation_depth: 1,
                // Also the lifetime of the result object the data plane
                // reserves under the put grant: how long a result stays
                // retrievable after it is persisted.
                capability_secs: 3_600,
            },
        }
    }
}

/// Why a tick could not run. Carries no private content.
#[derive(Debug, thiserror::Error)]
pub enum FrontError {
    /// The chain could not be observed.
    #[error("chain: {0}")]
    Chain(#[from] crate::chain::ChainError),
}

/// An uploaded input awaiting registration.
struct Upload {
    object: ObjectId,
    input_commitment: [u8; 32],
    uploaded_at: u64,
}

/// A registered task.
struct Registration {
    uploaded_at: u64,
    registered_at: u64,
}

/// The state the worker needs exclusive access to.
struct Core {
    ids: IdSource,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    worker: Worker,
    profile: Arc<dyn ExecutionProfile>,
    uploads: HashMap<([u8; 32], Address), Upload>,
    registered: HashMap<[u8; 32], Registration>,
    parked: HashSet<[u8; 32]>,
    observed: HashMap<[u8; 32], InferenceObservation>,
}

struct Snapshot {
    submitter: Address,
    view: StatusView,
}

struct Inner<C> {
    core: Mutex<Core>,
    chain: C,
    clock: Arc<dyn Clock>,
    owner: LocalKey,
    issuer: LocalKey,
    executor: ExecutorKey,
    config: FrontConfig,
    observations: Option<Arc<StdMutex<Vec<InferenceObservation>>>>,
    snapshots: StdMutex<HashMap<[u8; 32], Snapshot>>,
    replay: StdMutex<ReplayGuard>,
    busy: AtomicBool,
}

/// The front. Cheap to clone; every clone shares one state.
pub struct Front<C> {
    inner: Arc<Inner<C>>,
}

impl<C> Clone for Front<C> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// The worker's profile, shared so the worker can be rebuilt (see
/// [`Front::tick`]) without rebuilding the inference backend.
struct SharedProfile(Arc<dyn ExecutionProfile>);

impl ExecutionProfile for SharedProfile {
    fn spec_tag(&self) -> &'static [u8] {
        self.0.spec_tag()
    }
    fn spec_tags(&self) -> Vec<Vec<u8>> {
        self.0.spec_tags()
    }
    fn accepts(&self, execution_spec: &[u8]) -> bool {
        self.0.accepts(execution_spec)
    }
    fn execute(&self, input: &[u8]) -> Result<Vec<u8>, crate::execution::ExecutionError> {
        self.0.execute(input)
    }
    fn execute_task(
        &self,
        execution_spec: &[u8],
        input_commitment: &[u8; 32],
        input: &[u8],
    ) -> Result<Vec<u8>, crate::execution::ExecutionError> {
        self.0.execute_task(execution_spec, input_commitment, input)
    }
}

/// A refusal: an HTTP status and a stable code. Never carries content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiError(pub StatusCode, pub &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(ErrorBody {
                error: self.1.to_owned(),
            }),
        )
            .into_response()
    }
}

fn parse_hex32(s: &str, code: &'static str) -> Result<[u8; 32], ApiError> {
    hex::decode(s.strip_prefix("0x").unwrap_or(s))
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or(ApiError(StatusCode::BAD_REQUEST, code))
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

fn failure_name(class: FailureClass) -> &'static str {
    match class {
        FailureClass::Input => "input",
        FailureClass::Execution => "execution",
        FailureClass::Persistence => "persistence",
        FailureClass::Receipt => "receipt",
    }
}

fn ms(v: u128) -> u64 {
    u64::try_from(v).unwrap_or(u64::MAX)
}

impl<C: ChainClient + Clone + 'static> Front<C> {
    /// A front over a fresh in-process control plane and data plane, driving
    /// one worker that holds `executor` and runs `profile`.
    ///
    /// `seed` derives the front's owner key, the control plane's issuer key
    /// and every identifier; production passes 32 bytes from the OS RNG.
    /// `observations` is the inference profile's observation log, read for
    /// status timings only.
    pub fn new(
        chain: C,
        clock: Arc<dyn Clock>,
        seed: [u8; 32],
        executor: ExecutorKey,
        profile: Arc<dyn ExecutionProfile>,
        observations: Option<Arc<StdMutex<Vec<InferenceObservation>>>>,
        mut config: FrontConfig,
    ) -> Self {
        config.executor = executor.address();
        let mut ids = IdSource::new(seed);
        let owner = LocalKey::from_seed(&ids.next("mvp-front-owner"));
        let issuer = LocalKey::from_seed(&ids.next("mvp-front-issuer"));
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
                    profile,
                    uploads: HashMap::new(),
                    registered: HashMap::new(),
                    parked: HashSet::new(),
                    observed: HashMap::new(),
                }),
                chain,
                clock,
                owner,
                issuer,
                executor,
                config,
                observations,
                snapshots: StdMutex::new(HashMap::new()),
                replay: StdMutex::new(ReplayGuard::default()),
                busy: AtomicBool::new(false),
            }),
        }
    }

    /// The executor this front serves.
    pub fn executor(&self) -> Address {
        self.inner.config.executor
    }

    /// The router. Bodies above [`MAX_BODY_BYTES`] are refused before any
    /// handler reads them.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/v1/inputs", put(upload_route::<C>))
            .route("/v1/tasks/:task_id/register", post(register_route::<C>))
            .route("/v1/tasks/:task_id", get(status_route::<C>))
            .route("/v1/tasks/:task_id/result", get(result_route::<C>))
            .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
            .with_state(self.clone())
    }

    /// Exact bytes the data plane holds as the result for `task_id`, read
    /// under the same single-use grant the result route uses. For tests and
    /// operators; never served by a route other than the result route.
    pub async fn persisted_result(&self, task_id: [u8; 32]) -> Option<Vec<u8>> {
        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        let executor = core.cp.task(task_id)?.task.executor;
        self.read_result(core, task_id, executor).ok()
    }

    // ── the worker loop ──────────────────────────────────────────────

    /// One step of the worker loop: observe the chain, park tasks that used
    /// up their attempts, run one worker attempt if a registered task is
    /// open, observe again, refresh status snapshots.
    ///
    /// The worker holds the front's state for the whole attempt (inference
    /// included); status requests meanwhile are answered from the last
    /// snapshot with `worker_busy = true`.
    pub async fn tick(&self) -> Result<Option<AttemptOutcome>, FrontError> {
        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        core.cp.observe(&self.inner.chain).await?;
        // Expired leases are released here, not only when a task is offered,
        // so a receipt that never reaches a block makes its task offerable
        // again once the lease runs out.
        core.cp.sweep(&mut core.dp);
        self.park_exhausted(core);
        // Open: registered, not parked, and neither completed nor waiting
        // for its submitted receipt to reach a block (re-entering that lease
        // would only resubmit the same signed bytes).
        let open = core.registered.keys().any(|id| {
            !core.parked.contains(id)
                && core.cp.task(*id).is_some_and(|r| {
                    !matches!(r.state, TaskState::Completed | TaskState::ReceiptSubmitted)
                })
        });
        let mut outcome = None;
        if open {
            self.inner.busy.store(true, Ordering::SeqCst);
            let seen = self.observation_count();
            let attempt = core.worker.run_once(&mut core.cp, &mut core.dp, &self.inner.chain);
            // The inference is a blocking call inside the attempt. On the
            // multi-threaded runtime it runs under `block_in_place`, which
            // hands this thread's scheduler duties (its queued tasks, the I/O
            // and timer drivers) to another thread for the duration, so the
            // routes keep answering while the worker runs.
            let result = if multi_threaded() {
                tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(attempt))
            } else {
                attempt.await
            };
            self.inner.busy.store(false, Ordering::SeqCst);
            match result {
                Ok(o) => {
                    if let Some(task_id) = outcome_task(&o) {
                        if let Some(obs) = self.observation_since(seen) {
                            core.observed.insert(task_id, obs);
                        }
                    }
                    outcome = Some(o);
                }
                Err(e) => log::warn!("mvp-front: worker attempt error: {e}"),
            }
            core.cp.observe(&self.inner.chain).await?;
        }
        let ids: Vec<[u8; 32]> = core.registered.keys().copied().collect();
        for id in ids {
            self.refresh_snapshot(core, id);
        }
        Ok(outcome)
    }

    fn observation_count(&self) -> usize {
        self.inner.observations.as_ref().map_or(0, |o| o.lock().map_or(0, |v| v.len()))
    }

    fn observation_since(&self, seen: usize) -> Option<InferenceObservation> {
        let log = self.inner.observations.as_ref()?.lock().ok()?;
        log.get(seen..).and_then(|new| new.last().cloned())
    }

    /// Stops offering tasks whose attempts are exhausted. The control plane
    /// offers any non-terminal task that has an input reference, so a task
    /// that fails deterministically would be retried forever and starve the
    /// rest. Parking clears its input reference through the control plane's
    /// own durable-state restore (E §15–§16) — only when no task depends on
    /// a live lease, so no in-flight attempt or pending receipt is affected
    /// — and rebuilds the worker, whose session the restore ended.
    fn park_exhausted(&self, core: &mut Core) {
        let max = self.inner.config.max_attempts;
        let exhausted: Vec<[u8; 32]> = core
            .registered
            .keys()
            .filter(|id| !core.parked.contains(*id))
            .filter(|id| {
                core.cp.task(**id).is_some_and(|r| {
                    r.state == TaskState::Failed && r.attempt_count >= max && r.lease.is_none()
                })
            })
            .copied()
            .collect();
        if exhausted.is_empty() {
            return;
        }
        let quiet = core.registered.keys().all(|id| {
            core.cp.task(*id).map_or(true, |r| {
                matches!(
                    r.state,
                    TaskState::Discovered | TaskState::Failed | TaskState::Completed
                ) && r.lease.is_none()
            })
        });
        if !quiet {
            return;
        }
        let mut state = core.cp.snapshot();
        for rec in &mut state.tasks {
            if exhausted.contains(&rec.task_id) {
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
        core.worker = Worker::new(
            &mut core.ids,
            self.inner.executor.clone(),
            Box::new(SharedProfile(Arc::clone(&core.profile))),
        );
        for id in exhausted {
            log::warn!(
                "mvp-front: task {} parked after {max} failed attempts",
                hex::encode(&id[..8])
            );
            core.parked.insert(id);
        }
    }

    // ── views ────────────────────────────────────────────────────────

    fn build_view(&self, core: &Core, task_id: [u8; 32]) -> Option<Snapshot> {
        let rec = core.cp.task(task_id)?;
        let reg = core.registered.get(&task_id);
        let parked = core.parked.contains(&task_id);
        let state = match (reg, parked) {
            (None, _) => "not_registered",
            (Some(_), true) => "failed",
            (Some(_), false) => state_name(rec.state),
        };
        let uploaded_at = reg
            .map(|r| r.uploaded_at)
            .or_else(|| core.uploads.get(&(task_id, rec.task.submitter)).map(|u| u.uploaded_at));
        let obs = core.observed.get(&task_id);
        Some(Snapshot {
            submitter: rec.task.submitter,
            view: StatusView {
                task_id: hex::encode(task_id),
                state: state.to_owned(),
                task_height: rec.observed_height,
                attempts: rec.attempt_count,
                attempts_exhausted: parked,
                last_failure: rec.last_failure.map(|c| failure_name(c).to_owned()),
                failure_code: obs.and_then(|o| o.failure_code).map(str::to_owned),
                uploaded_at,
                registered_at: reg.map(|r| r.registered_at),
                started_at_ms: obs.map(|o| ms(o.started_at_ms)),
                completed_at_ms: obs.map(|o| ms(o.completed_at_ms)),
                inference_ms: obs.map(|o| ms(o.duration_ms)),
                input_tokens: obs.and_then(|o| o.input_tokens),
                output_tokens: obs.and_then(|o| o.output_tokens),
                finish_reason: obs.and_then(|o| o.finish_reason.clone()),
                result_available: core.dp.result_ref(task_id, rec.task.executor).is_some(),
                receipt_submitted: rec.submitted_tx.is_some() || rec.state == TaskState::Completed,
                receipt_height: rec.completed_height,
                worker_busy: false,
                as_of: self.inner.clock.now(),
            },
        })
    }

    fn refresh_snapshot(&self, core: &Core, task_id: [u8; 32]) -> Option<StatusView> {
        let snap = self.build_view(core, task_id)?;
        let view = snap.view.clone();
        if let Ok(mut s) = self.inner.snapshots.lock() {
            s.insert(task_id, snap);
        }
        Some(view)
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
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"))?;
        auth::verify_request(
            AuthHeaders {
                address: h(HEADER_ADDRESS),
                timestamp: h(HEADER_TIMESTAMP),
                nonce: h(HEADER_NONCE),
                signature: h(HEADER_SIGNATURE),
            },
            method.as_str(),
            uri.path(),
            body,
            self.inner.clock.now(),
            &mut replay,
        )
        .map_err(|e| {
            ApiError(
                StatusCode::UNAUTHORIZED,
                match e {
                    AuthError::Missing => "AUTH_MISSING",
                    AuthError::Malformed => "AUTH_MALFORMED",
                    AuthError::Stale => "AUTH_STALE",
                    AuthError::Replayed => "AUTH_REPLAYED",
                    AuthError::BadSignature => "AUTH_BAD_SIGNATURE",
                },
            )
        })
    }

    async fn upload(&self, caller: Address, body: &[u8]) -> Result<Response, ApiError> {
        let invalid = ApiError(StatusCode::BAD_REQUEST, "INVALID_INPUT_FORMAT");
        let req: UploadRequest = serde_json::from_slice(body).map_err(|_| invalid)?;
        let task_id = parse_hex32(&req.task_id, "INVALID_TASK_ID")?;
        if req.input.contract != INPUT_CONTRACT_V1 {
            return Err(invalid);
        }
        let input = CvExtractionInputV1::new(req.input.cv_text).map_err(|e| {
            if e.code() == "INPUT_TOO_LARGE" {
                ApiError(StatusCode::PAYLOAD_TOO_LARGE, "INPUT_TOO_LARGE")
            } else {
                invalid
            }
        })?;
        let bytes = input.canonical_bytes();
        let commitment = input.input_commitment();
        let input_bytes = bytes.len();
        drop(input);

        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        if core.registered.contains_key(&task_id) {
            return Err(ApiError(StatusCode::CONFLICT, "TASK_ALREADY_REGISTERED"));
        }
        let key = (task_id, caller);
        if !core.uploads.contains_key(&key) && core.uploads.len() >= MAX_PENDING_UPLOADS {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "TOO_MANY_PENDING_UPLOADS",
            ));
        }
        let now = self.inner.clock.now();
        let object = core.dp.store_input(
            &self.inner.owner,
            task_id,
            commitment,
            bytes,
            self.inner.config.input_ttl_secs,
        );
        if let Some(old) = core.uploads.insert(
            key,
            Upload {
                object,
                input_commitment: commitment,
                uploaded_at: now,
            },
        ) {
            // A re-upload before registration replaces the earlier object.
            let _ = core.dp.delete_object(&self.inner.owner, &old.object);
        }
        Ok((
            StatusCode::CREATED,
            Json(UploadResponse {
                task_id: hex::encode(task_id),
                object_ref: hex::encode(object.0),
                input_commitment: hex::encode(commitment),
                input_bytes,
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
            .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "INVALID_REQUEST"))?;
        let object_ref = parse_hex32(&req.object_ref, "INVALID_OBJECT_REF")?;
        let executor = Address(parse_hex32(&req.executor, "INVALID_EXECUTOR")?);

        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        core.cp
            .observe(&self.inner.chain)
            .await
            .map_err(|_| ApiError(StatusCode::BAD_GATEWAY, "CHAIN_UNAVAILABLE"))?;
        let task = core
            .cp
            .task(task_id)
            .ok_or(ApiError(StatusCode::NOT_FOUND, "TASK_NOT_ON_CHAIN"))?
            .task
            .clone();
        if task.submitter != caller {
            return Err(ApiError(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        if core.registered.contains_key(&task_id) {
            // Idempotent: the same task registered again changes nothing.
            let view = self.refresh_snapshot(core, task_id);
            return Ok(Json(view).into_response());
        }
        if task.execution_spec != frozen_execution_spec() {
            return Err(ApiError(
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNSUPPORTED_EXECUTION_SPEC",
            ));
        }
        if task.executor != executor {
            return Err(ApiError(
                StatusCode::UNPROCESSABLE_ENTITY,
                "EXECUTOR_MISMATCH",
            ));
        }
        if task.executor != self.inner.config.executor {
            return Err(ApiError(
                StatusCode::UNPROCESSABLE_ENTITY,
                "EXECUTOR_NOT_SERVED",
            ));
        }
        let upload = core
            .uploads
            .get(&(task_id, caller))
            .ok_or(ApiError(StatusCode::NOT_FOUND, "INPUT_NOT_UPLOADED"))?;
        if upload.object.0 != object_ref {
            return Err(ApiError(
                StatusCode::UNPROCESSABLE_ENTITY,
                "OBJECT_REF_MISMATCH",
            ));
        }
        if upload.input_commitment != task.input_commitment {
            return Err(ApiError(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INPUT_COMMITMENT_MISMATCH",
            ));
        }
        let (object, uploaded_at) = (upload.object, upload.uploaded_at);
        let internal = |_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "REGISTRATION_FAILED");
        core.dp
            .register_task(&self.inner.owner, task_id, task.executor)
            .map_err(internal)?;
        core.dp
            .delegate_issuer(&self.inner.owner, task_id, core.cp.issuer_address())
            .map_err(internal)?;
        core.cp
            .register_input(task_id, object)
            .map_err(|_| ApiError(StatusCode::INTERNAL_SERVER_ERROR, "REGISTRATION_FAILED"))?;
        core.uploads.retain(|(id, _), _| *id != task_id);
        core.registered.insert(
            task_id,
            Registration {
                uploaded_at,
                registered_at: self.inner.clock.now(),
            },
        );
        let view = self.refresh_snapshot(core, task_id);
        Ok(Json(view).into_response())
    }

    async fn status(&self, caller: Address, task_id: [u8; 32]) -> Result<Response, ApiError> {
        let snap = if let Ok(guard) = self.inner.core.try_lock() {
            self.build_view(&guard, task_id)
        } else {
            let cached = self.inner.snapshots.lock().ok().and_then(|s| {
                s.get(&task_id).map(|s| Snapshot {
                    submitter: s.submitter,
                    view: StatusView {
                        worker_busy: self.inner.busy.load(Ordering::SeqCst),
                        ..s.view.clone()
                    },
                })
            });
            if let Some(s) = cached {
                Some(s)
            } else {
                let guard = self.inner.core.lock().await;
                self.build_view(&guard, task_id)
            }
        };
        let snap = snap.ok_or(ApiError(StatusCode::NOT_FOUND, "TASK_NOT_FOUND"))?;
        if snap.submitter != caller {
            return Err(ApiError(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        Ok(Json(snap.view).into_response())
    }

    fn read_result(
        &self,
        core: &mut Core,
        task_id: [u8; 32],
        executor: Address,
    ) -> Result<Vec<u8>, ApiError> {
        let rr = core
            .dp
            .result_ref(task_id, executor)
            .ok_or(ApiError(StatusCode::CONFLICT, "RESULT_NOT_READY"))?;
        let unavailable = |e: DataPlaneError| match e {
            DataPlaneError::Expired => ApiError(StatusCode::GONE, "RESULT_EXPIRED"),
            _ => ApiError(StatusCode::INTERNAL_SERVER_ERROR, "RESULT_UNAVAILABLE"),
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
        Ok(bytes)
    }

    async fn result(&self, caller: Address, task_id: [u8; 32]) -> Result<Response, ApiError> {
        let mut guard = self.inner.core.lock().await;
        let core = &mut *guard;
        let task = core
            .cp
            .task(task_id)
            .ok_or(ApiError(StatusCode::NOT_FOUND, "TASK_NOT_FOUND"))?
            .task
            .clone();
        if task.submitter != caller {
            return Err(ApiError(StatusCode::FORBIDDEN, "NOT_TASK_SUBMITTER"));
        }
        if !core.registered.contains_key(&task_id) {
            return Err(ApiError(StatusCode::NOT_FOUND, "TASK_NOT_REGISTERED"));
        }
        let bytes = self.read_result(core, task_id, task.executor)?;
        if bytes.len() > MAX_RESULT_BYTES {
            return Err(ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "RESULT_TOO_LARGE",
            ));
        }
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

// ── handlers: authenticate, run, log one safe line ────────────────────────

fn finish(
    method: &Method,
    route: &str,
    task_id: Option<&str>,
    started: Instant,
    r: Result<Response, ApiError>,
) -> Response {
    let (status, code) = match &r {
        Ok(resp) => (resp.status(), "OK"),
        Err(e) => (e.0, e.1),
    };
    log::info!(
        "mvp-front: {method} {route} task {} -> {} {code} {}ms",
        task_id.map_or("-", |t| t.get(..16).unwrap_or("?")),
        status.as_u16(),
        started.elapsed().as_millis()
    );
    r.unwrap_or_else(IntoResponse::into_response)
}

async fn upload_route<C: ChainClient + Clone + 'static>(
    State(front): State<Front<C>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let t = Instant::now();
    let r = match front.authenticate(&method, &uri, &headers, &body) {
        Ok(caller) => front.upload(caller, &body).await,
        Err(e) => Err(e),
    };
    finish(&method, "/v1/inputs", None, t, r)
}

async fn register_route<C: ChainClient + Clone + 'static>(
    State(front): State<Front<C>>,
    Path(task_id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let t = Instant::now();
    let r = match (
        front.authenticate(&method, &uri, &headers, &body),
        parse_hex32(&task_id, "INVALID_TASK_ID"),
    ) {
        (Ok(caller), Ok(id)) => front.register(caller, id, &body).await,
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    finish(
        &method,
        "/v1/tasks/{task_id}/register",
        Some(&task_id),
        t,
        r,
    )
}

async fn status_route<C: ChainClient + Clone + 'static>(
    State(front): State<Front<C>>,
    Path(task_id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let t = Instant::now();
    let r = match (
        front.authenticate(&method, &uri, &headers, &body),
        parse_hex32(&task_id, "INVALID_TASK_ID"),
    ) {
        (Ok(caller), Ok(id)) => front.status(caller, id).await,
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    finish(&method, "/v1/tasks/{task_id}", Some(&task_id), t, r)
}

async fn result_route<C: ChainClient + Clone + 'static>(
    State(front): State<Front<C>>,
    Path(task_id): Path<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let t = Instant::now();
    let r = match (
        front.authenticate(&method, &uri, &headers, &body),
        parse_hex32(&task_id, "INVALID_TASK_ID"),
    ) {
        (Ok(caller), Ok(id)) => front.result(caller, id).await,
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    finish(&method, "/v1/tasks/{task_id}/result", Some(&task_id), t, r)
}
