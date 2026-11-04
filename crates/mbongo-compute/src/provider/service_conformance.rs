//! `compute-conformance-v1` against the provider service (S2-10, #191).
//!
//! Tests only. The subject is the real [`ProviderService`]: its durable store,
//! its task loop, its admission path, and an external runner speaking
//! `mbongo-runner:v1`. What the suite asks of coordination and the data plane
//! goes to the service's own control plane and data plane, which have no public
//! route, so this sits inside the crate instead of calling HTTP.
//!
//! **Actor mapping.** On the data plane the service holds a private owner key
//! and acts for the client it authenticated, so `Actor::Client` presents with
//! that key. The client's own identity stays the task's submitter, and
//! [`Subject::owner_retrieve_result`] goes through the service's result route
//! as that submitter.
//!
//! What a green run means is stated in [`crate::conformance`]: behaviour, not
//! confidentiality, correctness or exactly-once execution. What this subject
//! leaves out is in `docs/development/compute-conformance.md`.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::response::IntoResponse;
use mbongo_core::{
    Address, ComputeTask, Receipt, Transaction, TransactionPayload, TransactionType,
    COMPUTE_TASK_VERSION,
};
use mbongo_verification::RECEIPT_VERSION;
use tokio::sync::MutexGuard;

use super::{
    Core, MeteringLog, ProviderConfig, ProviderLimits, ProviderParts, ProviderService,
    ProviderStore, RegisterRequest, SharedProfile, UploadRequest, UploadResponse,
};
use crate::chain::testing::FakeChain;
use crate::chain::ChainClient;
use crate::clock::{Clock, ManualClock};
use crate::conformance::reference::{captured_lines, install_log_capture, take_captured};
use crate::conformance::{
    catalog, run_all, Actor, Claims, FailureKind, FaultPoint, GrantOp, Outcome, Refusal, Retarget,
    Status, Subject, SubjectInfo,
};
use crate::control_plane::{AttemptEvent, ControlPlaneConfig, FailureClass, Lease};
use crate::data_plane::{
    Capability, CapabilityRequest, DataPlaneFault, LocalKey, ObjectState, Operation, Presentation,
};
use crate::execution::{reference_input_commitment, reference_output_commitment};
use crate::identity::{CapabilityId, ExecutorKey, IdSource, ObjectId, SessionId};
use crate::job::{job_spec_digest, JobSource, RuntimeAdapter, RuntimeIdentity, JOB_TAG};
use crate::mvp::client::random_bytes;
use crate::provider::durable::{DurableStore, RetentionPolicy};
use crate::provider::runner::{RunnerConfig, SubprocessRuntime};
use crate::worker::{AttemptOutcome, Fault, Worker, WorkerError};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const SEED_SERVICE: [u8; 32] = [0xC0; 32];
const SEED_CLIENT: [u8; 32] = [0xAA; 32];
const SEED_EXECUTOR: [u8; 32] = [0xE1; 32];
const SEED_OTHER: [u8; 32] = [0xE2; 32];
const SEED_STRANGER: [u8; 32] = [0x5E; 32];
const STATE_KEY: [u8; 32] = [0x57; 32];
const LEASE_SECS: u64 = 60;
const GRANT_SECS: u64 = 120;
/// Canonical JSON, for the generic `fixture-sum` job of the contract vectors.
const INPUT: &[u8] = b"[3141592,6535897,9323846,2643383,2795028]";
const OUTPUT: &[u8] = b"{\"count\":5,\"sum\":24439746}";
/// Cases that cannot apply to a provider service, by id, with the reason. Each
/// must be reported as unsupported: none is skipped silently.
const NOT_APPLICABLE: [(&str, &str); 0] = [];

fn block_on<F: Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

fn node() -> String {
    let name = format!("node{}", std::env::consts::EXE_SUFFIX);
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join(&name))
        .find(|path| path.is_file())
        .expect("node on PATH")
        .display()
        .to_string()
}

/// The canonical `fixture-sum` job spec of the contract vectors.
fn job_spec() -> Vec<u8> {
    let path = Path::new(ROOT).join("test-vectors/job/job-contract-v1.json");
    let vectors: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("vectors")).expect("vectors parse");
    vectors["specs"]
        .as_array()
        .and_then(|specs| specs.iter().find(|s| s["name"] == "fixture-sum"))
        .and_then(|spec| spec["canonical"].as_str())
        .expect("fixture-sum")
        .as_bytes()
        .to_vec()
}

/// A lease as a worker uses it: the session it was offered in, and the lease.
#[derive(Debug, Clone)]
struct ProviderLease {
    session: SessionId,
    lease: Lease,
}

/// One scenario over a real provider service.
struct ProviderSubject {
    dir: PathBuf,
    service: ProviderService<FakeChain>,
    chain: FakeChain,
    clock: ManualClock,
    metering: Arc<MeteringLog>,
    ids: IdSource,
    client: LocalKey,
    executor: ExecutorKey,
    other: ExecutorKey,
    stranger: ExecutorKey,
    spec: Vec<u8>,
    task: ComputeTask,
    client_nonce: u64,
    presentations: HashMap<CapabilityId, Presentation>,
    rendered: Vec<String>,
    secrets: Vec<Vec<u8>>,
    capture_active: bool,
}

impl Drop for ProviderSubject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl ProviderSubject {
    fn new() -> Self {
        let capture_active = install_log_capture();
        take_captured();
        let dir = std::env::temp_dir().join(format!(
            "mbongo-provider-conformance-{}",
            hex::encode(random_bytes::<8>())
        ));
        std::fs::create_dir_all(dir.join("artifacts")).expect("artifact directory");
        let clock = ManualClock::starting_at(1_000);
        let chain = FakeChain::new();
        let executor = ExecutorKey::from_seed(&SEED_EXECUTOR);
        let store = Arc::new(ProviderStore::new(dir.join("artifacts")));
        let runtime = SubprocessRuntime::new(
            RunnerConfig {
                identity: RuntimeIdentity {
                    family: "test-fixture".to_owned(),
                    revision: "1".to_owned(),
                    binary_sha256: None,
                    accelerator_class: "cpu".to_owned(),
                },
                command: vec![
                    node(),
                    format!("{ROOT}/scripts/fixtures/mbongo-runner-fixture.mjs"),
                ],
                pass_env: vec!["SystemRoot".to_owned()],
                work_dir: dir.join("work"),
            },
            Arc::clone(&store) as Arc<dyn JobSource>,
        );
        let durable =
            DurableStore::open(&dir.join("state"), &STATE_KEY, RetentionPolicy::default())
                .expect("durable state");
        let metering = Arc::new(MeteringLog::default());
        let defaults = ProviderConfig::default();
        let config = ProviderConfig {
            control_plane: ControlPlaneConfig {
                lease_secs: LEASE_SECS,
                session_secs: 600,
                confirmation_depth: 1,
                capability_secs: GRANT_SECS,
            },
            limits: ProviderLimits {
                max_wall_ms: 30_000,
                ..defaults.limits
            },
            ..defaults
        };
        let service = block_on(ProviderService::open(
            chain.clone(),
            Arc::new(clock.clone()) as Arc<dyn Clock>,
            SEED_SERVICE,
            executor.clone(),
            ProviderParts {
                store,
                runtimes: vec![Arc::new(runtime) as Arc<dyn RuntimeAdapter>],
                metering: Arc::clone(&metering) as Arc<dyn crate::job::MeteringSink>,
            },
            Arc::new(durable),
            config,
        ))
        .expect("service");
        let client = LocalKey::from_seed(&SEED_CLIENT);
        let spec = job_spec();
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client.address(),
            executor: executor.address(),
            salt: [0x5A; 32],
            input_commitment: reference_input_commitment(INPUT),
            execution_spec: [JOB_TAG, job_spec_digest(&spec).as_slice()].concat(),
        };
        let mut subject = Self {
            dir,
            service,
            chain,
            clock,
            metering,
            ids: IdSource::new([0x11; 32]),
            client,
            executor,
            other: ExecutorKey::from_seed(&SEED_OTHER),
            stranger: ExecutorKey::from_seed(&SEED_STRANGER),
            spec,
            task,
            client_nonce: 0,
            presentations: HashMap::new(),
            rendered: Vec::new(),
            secrets: vec![
                SEED_SERVICE.to_vec(),
                SEED_CLIENT.to_vec(),
                SEED_EXECUTOR.to_vec(),
                SEED_OTHER.to_vec(),
                SEED_STRANGER.to_vec(),
                STATE_KEY.to_vec(),
            ],
            capture_active,
        };
        subject.render(&format!("{:?}", subject.executor));
        subject.render(&format!("{:?}", subject.other));
        subject
    }

    fn core(&self) -> MutexGuard<'_, Core> {
        self.service.inner.core.try_lock().expect("the service is idle")
    }

    fn render(&mut self, line: &str) {
        self.rendered.push(line.to_owned());
    }

    /// The key an actor holds on the data plane.
    fn key(&self, actor: Actor) -> ExecutorKey {
        match actor {
            Actor::Executor => self.executor.clone(),
            Actor::OtherExecutor => self.other.clone(),
            Actor::Client => self.service.inner.owner.clone(),
            Actor::Stranger => self.stranger.clone(),
        }
    }

    fn issuer(&self) -> LocalKey {
        self.service.inner.issuer.clone()
    }

    fn task_id(&self) -> [u8; 32] {
        self.task.task_id()
    }

    async fn commit(&mut self, task: &ComputeTask) {
        let mut tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: self.client.address(),
            receiver: Address::zero(),
            amount: 0,
            nonce: self.client_nonce,
            payload: TransactionPayload::ComputeTask(Box::new(task.clone())),
            signature: [0; 64],
        };
        self.client_nonce += 1;
        tx.signature = self.client.sign(&tx.signing_payload());
        self.chain.submit_transaction(&tx).await.expect("chain double accepts");
        self.chain.produce_block();
        self.chain.produce_block();
    }

    fn presented(&mut self, actor: Actor, cap: &Capability) -> Presentation {
        let key = self.key(actor);
        let challenge = self.core().dp.issue_challenge(key.address());
        let presentation = Presentation::sign(cap.clone(), challenge, &key);
        self.note_presentation(&presentation);
        presentation
    }

    fn note_presentation(&mut self, p: &Presentation) {
        self.secrets.push(p.proof.to_vec());
        self.secrets.push(p.capability.issuer_signature.to_vec());
        self.render(&format!("{p:?}"));
        self.presentations.insert(p.capability.capability_id, p.clone());
    }

    fn note_grant(&mut self, cap: &Capability) {
        self.secrets.push(cap.issuer_signature.to_vec());
        self.render(&format!("{cap:?}"));
    }

    fn forged(
        &self,
        actor: Actor,
        task_id: [u8; 32],
        operation: Operation,
        resource: ObjectId,
        id: [u8; 32],
    ) -> Capability {
        let now = self.clock.now();
        Capability {
            capability_id: CapabilityId(id),
            task_id,
            presenter: self.key(actor).address(),
            operation,
            resource,
            not_before: now,
            not_after: now + 60,
            max_uses: 1,
            issuer: self.issuer().address(),
            issuer_signature: [0; 64],
        }
    }

    fn fetch(&mut self, p: &Presentation) -> Result<Vec<u8>, Refusal> {
        let fetched = self.core().dp.fetch_input(p);
        match fetched {
            Ok(plain) => {
                self.render(&format!("{plain:?}"));
                Ok(plain.as_bytes().to_vec())
            }
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn outcome(result: Option<AttemptOutcome>) -> Outcome {
        Self::mapped(result.map_or(Ok(AttemptOutcome::Idle), Ok::<_, WorkerError>))
    }

    fn mapped(o: Result<AttemptOutcome, WorkerError>) -> Outcome {
        match o {
            Ok(AttemptOutcome::Idle) => Outcome::Idle,
            Ok(AttemptOutcome::Submitted { task_id, .. }) => Outcome::Submitted(task_id),
            Ok(AttemptOutcome::Completed { task_id, .. }) => Outcome::Completed(task_id),
            Ok(AttemptOutcome::Failed { task_id, class }) => Outcome::Failed(
                task_id,
                match class {
                    FailureClass::Input => FailureKind::Input,
                    FailureClass::Execution => FailureKind::Execution,
                    FailureClass::Persistence => FailureKind::Persistence,
                    FailureClass::Receipt => FailureKind::Receipt,
                },
            ),
            Ok(AttemptOutcome::Crashed { task_id, .. }) => Outcome::Crashed(task_id),
            Err(e) => Outcome::Aborted(format!("{e}")),
        }
    }
}

impl Subject for ProviderSubject {
    type Chain = FakeChain;
    type Grant = Capability;
    type Lease = ProviderLease;
    type Locator = ObjectId;
    type Instance = Worker;

    fn info(&self) -> SubjectInfo {
        SubjectInfo {
            name: "mbongo-compute provider service (durable store, external runner, chain double)"
                .to_owned(),
            profile_tag: String::from_utf8_lossy(JOB_TAG).into_owned(),
            claims: Claims {
                exactly_once: false,
                confidential_execution: false,
            },
        }
    }

    fn chain(&self) -> &FakeChain {
        &self.chain
    }

    fn produce_block(&mut self) {
        self.chain.produce_block();
    }

    fn advance_time(&mut self, secs: u64) {
        self.clock.advance(secs);
    }

    fn lease_ttl_secs(&self) -> u64 {
        LEASE_SECS
    }

    fn grant_ttl_secs(&self) -> u64 {
        GRANT_SECS
    }

    fn private_input(&self) -> Vec<u8> {
        INPUT.to_vec()
    }

    fn expected_output(&self) -> Vec<u8> {
        OUTPUT.to_vec()
    }

    fn task(&self) -> ComputeTask {
        self.task.clone()
    }

    fn input_locator(&self) -> ObjectId {
        let id = self.task_id();
        self.core()
            .cp
            .task(id)
            .and_then(|record| record.input_object)
            .expect("the task is admitted")
    }

    fn address(&self, actor: Actor) -> Address {
        match actor {
            Actor::Client => self.client.address(),
            other => self.key(other).address(),
        }
    }

    /// The client's path: the job spec, the task on-chain, the input, the
    /// registration. A first tick lets the worker's session learn the job spec.
    async fn commit_task(&mut self) {
        let accepted = self.service.put_job_spec(&self.spec).await;
        assert!(accepted.is_ok(), "job spec refused");
        self.service.tick().await.expect("tick");
        let task = self.task.clone();
        self.commit(&task).await;
        let task_id = task.task_id();
        let client = self.client.address();
        let request = UploadRequest {
            task_id: hex::encode(task_id),
            input: hex::encode(INPUT),
        };
        let body = serde_json::to_vec(&request).expect("request");
        let Ok(uploaded) = self.service.upload(client, &body) else {
            panic!("input refused");
        };
        let bytes = axum::body::to_bytes(uploaded.into_body(), usize::MAX).await.expect("body");
        let uploaded: UploadResponse = serde_json::from_slice(&bytes).expect("upload response");
        let request = RegisterRequest {
            object_ref: uploaded.object_ref,
        };
        let body = serde_json::to_vec(&request).expect("request");
        let registered = self.service.register(client, task_id, &body).await;
        assert!(registered.is_ok(), "registration refused");
        let mut core = self.core();
        self.service.admit_next(&mut core).expect("admission");
    }

    /// No special case: the client tries to register it like any task.
    async fn commit_task_carrying_input_in_spec(&mut self) -> [u8; 32] {
        let probe = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: self.client.address(),
            executor: self.executor.address(),
            salt: [0x5B; 32],
            input_commitment: reference_input_commitment(INPUT),
            execution_spec: INPUT.to_vec(),
        };
        self.commit(&probe).await;
        let request = RegisterRequest {
            object_ref: hex::encode([0u8; 32]),
        };
        let body = serde_json::to_vec(&request).expect("request");
        let registered = self.service.register(self.client.address(), probe.task_id(), &body).await;
        assert!(
            registered.is_err(),
            "a task with no job spec was registered"
        );
        probe.task_id()
    }

    fn new_instance(&mut self, actor: Actor) -> Worker {
        let key = self.key(actor);
        let profile = Arc::clone(&self.service.inner.profile);
        let worker = Worker::new(&mut self.ids, key, Box::new(SharedProfile(profile)));
        self.render(&format!("{worker:?}"));
        worker
    }

    fn inject(&mut self, instance: &mut Worker, fault: FaultPoint) -> bool {
        instance.inject(match fault {
            FaultPoint::CrashBeforeFetch => Fault::CrashBeforeFetch,
            FaultPoint::CrashAfterFetch => Fault::CrashAfterFetch,
            FaultPoint::CrashDuringExecution => Fault::CrashDuringExecution,
            FaultPoint::CrashAfterPersist => Fault::CrashAfterPersist,
            FaultPoint::CrashAfterSubmit => Fault::CrashAfterSubmit,
            FaultPoint::CorruptInput => Fault::CorruptInput,
        });
        true
    }

    fn inject_result_store_failure(&mut self) -> bool {
        self.core().dp.inject(DataPlaneFault::PutResultFails);
        true
    }

    fn inject_lost_submission_response(&mut self) -> bool {
        self.chain.lose_next_response();
        true
    }

    /// One pass of the service's own task loop, with this instance as its
    /// worker.
    async fn run(&mut self, instance: &mut Worker) -> Outcome {
        std::mem::swap(&mut self.core().worker, instance);
        let result = self.service.tick().await;
        std::mem::swap(&mut self.core().worker, instance);
        let outcome = match result {
            Ok(attempt) => Self::outcome(attempt),
            Err(e) => Outcome::Aborted(e.to_string()),
        };
        self.render(&format!("{outcome:?}"));
        outcome
    }

    fn obtain_lease(&mut self, instance: &mut Worker) -> Option<ProviderLease> {
        let lease = {
            let mut guard = self.core();
            let core = &mut *guard;
            let session = instance.ensure_session(&mut core.cp).ok()?;
            let lease = core.cp.offer(session.session_id, &mut core.dp).ok()??;
            ProviderLease {
                session: session.session_id,
                lease,
            }
        };
        self.secrets.push(lease.lease.issuer_signature.to_vec());
        self.render(&format!("{:?}", lease.lease));
        Some(lease)
    }

    fn grant_under_lease(
        &mut self,
        _: &Worker,
        lease: &ProviderLease,
        op: GrantOp,
    ) -> Result<Capability, Refusal> {
        let granted = {
            let mut guard = self.core();
            let core = &mut *guard;
            let id = lease.lease.lease_id;
            match op {
                GrantOp::FetchInput => core.cp.authorize_fetch(lease.session, id, &mut core.dp),
                GrantOp::PutResult => core.cp.authorize_put(lease.session, id, &mut core.dp),
                GrantOp::GetResult => {
                    return Err(Refusal::NoSuchPath(
                        "a worker lease never yields a get-result grant".to_owned(),
                    ))
                }
            }
        };
        match granted {
            Ok(cap) => {
                self.note_grant(&cap);
                Ok(cap)
            }
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn report_started(&mut self, _: &Worker, lease: &ProviderLease) -> Result<(), Refusal> {
        let mut guard = self.core();
        let core = &mut *guard;
        core.cp
            .report(
                lease.session,
                lease.lease.lease_id,
                AttemptEvent::Started,
                &mut core.dp,
            )
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn heartbeat(&mut self, _: &Worker, lease: &ProviderLease) -> Result<(), Refusal> {
        self.core()
            .cp
            .heartbeat(lease.session, lease.lease.lease_id)
            .map(|_| ())
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn attempt_count(&self, task_id: [u8; 32]) -> u32 {
        self.core().cp.task(task_id).map_or(0, |t| t.attempt_count)
    }

    fn executor_of_record(&self, task_id: [u8; 32]) -> Option<Address> {
        self.core().cp.task(task_id).map(|t| t.task.executor)
    }

    fn owner_side_grant(&mut self, presenter: Actor, op: GrantOp) -> Result<Capability, Refusal> {
        let task_id = self.task_id();
        let executor = self.executor.address();
        let (operation, resource) = match op {
            GrantOp::FetchInput => (Operation::FetchInput, Some(self.input_locator())),
            GrantOp::PutResult => (Operation::PutResult, None),
            GrantOp::GetResult => {
                let result = self
                    .core()
                    .dp
                    .result_ref(task_id, executor)
                    .ok_or_else(|| Refusal::Denied("no durable result to grant".to_owned()))?;
                (Operation::GetResult, Some(result.object))
            }
        };
        let issuer = self.issuer();
        let granted = self.core().dp.issue_capability(
            &issuer,
            &CapabilityRequest {
                task_id,
                operation,
                resource,
                ttl_secs: GRANT_SECS,
                max_uses: 1,
            },
        );
        let cap = granted.map_err(|e| Refusal::Denied(format!("{e:?}")))?;
        self.note_grant(&cap);
        if cap.presenter != self.key(presenter).address() {
            return Err(Refusal::NoSuchPath(format!(
                "the issuer cannot name {presenter:?}: grants name the registered party"
            )));
        }
        Ok(cap)
    }

    fn present_fetch(&mut self, actor: Actor, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let p = self.presented(actor, grant);
        self.fetch(&p)
    }

    fn replay_last_presentation(&mut self, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let p = self.presentations.get(&grant.capability_id).cloned().ok_or_else(|| {
            Refusal::NoSuchPath("no presentation of this grant was observed".to_owned())
        })?;
        self.fetch(&p)
    }

    fn present_fetch_unproven(&mut self, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let challenge = self.core().dp.issue_challenge(grant.presenter);
        let p = Presentation {
            capability: grant.clone(),
            challenge,
            proof: [0; 64],
        };
        self.fetch(&p)
    }

    fn present_put(
        &mut self,
        actor: Actor,
        grant: &Capability,
        bytes: Vec<u8>,
    ) -> Result<(), Refusal> {
        let p = self.presented(actor, grant);
        let commitment = reference_output_commitment(&bytes);
        self.core()
            .dp
            .put_result(&p, bytes, commitment)
            .map(|_| ())
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn present_get(&mut self, actor: Actor, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let p = self.presented(actor, grant);
        let read = self.core().dp.get_result(&p);
        match read {
            Ok(plain) => {
                self.render(&format!("{plain:?}"));
                Ok(plain.as_bytes().to_vec())
            }
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn fetch_with_task_id_only(
        &mut self,
        actor: Actor,
        task_id: [u8; 32],
    ) -> Result<Vec<u8>, Refusal> {
        let cap = self.forged(
            actor,
            task_id,
            Operation::FetchInput,
            ObjectId(task_id),
            task_id,
        );
        let p = self.presented(actor, &cap);
        self.fetch(&p)
    }

    fn fetch_with_locator_only(
        &mut self,
        actor: Actor,
        locator: &ObjectId,
    ) -> Result<Vec<u8>, Refusal> {
        let cap = self.forged(
            actor,
            self.task_id(),
            Operation::FetchInput,
            *locator,
            locator.0,
        );
        let p = self.presented(actor, &cap);
        self.fetch(&p)
    }

    fn fetch_with_lease_only(
        &mut self,
        actor: Actor,
        lease: &ProviderLease,
    ) -> Result<Vec<u8>, Refusal> {
        // A lease carries a real issuer signature: the closest it can come to
        // a credential is to be presented as one.
        let mut cap = self.forged(
            actor,
            lease.lease.task_id,
            Operation::FetchInput,
            self.input_locator(),
            lease.lease.lease_id.0,
        );
        cap.issuer = lease.lease.issuer;
        cap.issuer_signature = lease.lease.issuer_signature;
        let p = self.presented(actor, &cap);
        self.fetch(&p)
    }

    fn result_with_task_id_only(
        &mut self,
        actor: Actor,
        task_id: [u8; 32],
    ) -> Result<Vec<u8>, Refusal> {
        let cap = self.forged(
            actor,
            task_id,
            Operation::GetResult,
            ObjectId(task_id),
            task_id,
        );
        let p = self.presented(actor, &cap);
        let read = self.core().dp.get_result(&p);
        match read {
            Ok(plain) => Ok(plain.as_bytes().to_vec()),
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn revoke(&mut self, grant: &Capability) -> Result<(), Refusal> {
        let issuer = self.issuer();
        self.core()
            .dp
            .revoke(&issuer, grant.task_id, grant.capability_id)
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn retarget(&self, grant: &Capability, how: Retarget) -> Capability {
        let mut cap = grant.clone();
        match how {
            Retarget::Task(id) => cap.task_id = id,
            Retarget::Presenter(actor) => cap.presenter = self.key(actor).address(),
        }
        cap
    }

    fn input_consumed(&self, locator: &ObjectId) -> bool {
        self.core().dp.object_state(locator) == Some(ObjectState::Consumed)
    }

    fn result_exists(&self, task_id: [u8; 32]) -> bool {
        self.core().dp.result_ref(task_id, self.executor.address()).is_some()
    }

    fn result_commitment(&self, task_id: [u8; 32]) -> Option<[u8; 32]> {
        self.core()
            .dp
            .result_ref(task_id, self.executor.address())
            .map(|r| r.output_commitment)
    }

    /// The service's result route, as the task's submitter, after a tick has
    /// bound the receipt to the durable output.
    fn owner_retrieve_result(&mut self, task_id: [u8; 32]) -> Result<Vec<u8>, Refusal> {
        let client = self.client.address();
        let service = self.service.clone();
        let read = block_on(async move {
            service.tick().await.map_err(|e| e.to_string())?;
            let response = match service.result(client, task_id).await {
                Ok(response) => response,
                Err(refused) => {
                    return Err(refused.into_response().status().to_string());
                }
            };
            let body = axum::body::to_bytes(response.into_body(), usize::MAX).await;
            body.map(|b| b.to_vec()).map_err(|e| e.to_string())
        });
        read.map_err(Refusal::Denied)
    }

    fn control_plane_receipt(&mut self, output_commitment: [u8; 32]) -> Receipt {
        let mut receipt = Receipt {
            version: RECEIPT_VERSION,
            task_id: self.task_id(),
            input_commitment: self.task.input_commitment,
            output_commitment,
            executor: self.task.executor,
            metadata: Vec::new(),
            signature: [0; 64],
        };
        receipt.signature = self.issuer().sign(&receipt.receipt_hash().0);
        receipt
    }

    fn control_plane_anchor_transaction(&mut self, receipt: Receipt) -> Transaction {
        let mut tx = Transaction {
            tx_type: TransactionType::AnchorReceipt,
            sender: self.task.executor,
            receiver: Address::zero(),
            amount: 0,
            nonce: 0,
            payload: TransactionPayload::AnchorReceipt(Box::new(receipt)),
            signature: [0; 64],
        };
        tx.signature = self.issuer().sign(&tx.signing_payload());
        tx
    }

    async fn executor_anchor_arbitrary(
        &mut self,
        output_commitment: [u8; 32],
    ) -> Result<(), Refusal> {
        let worker = self.new_instance(Actor::Executor);
        let receipt = worker.bound_receipt(&self.task, output_commitment);
        let nonce = self
            .chain
            .account_nonce(&self.executor.address())
            .await
            .map_err(|e| Refusal::Denied(e.to_string()))?;
        let tx = worker.anchor_transaction(receipt, nonce);
        self.chain
            .submit_transaction(&tx)
            .await
            .map(|_| ())
            .map_err(|e| Refusal::Denied(e.to_string()))
    }

    fn diagnostics(&self) -> String {
        let mut out = String::new();
        out.push_str(if self.capture_active {
            "# log capture: active\n"
        } else {
            "# log capture: inactive (another logger owns this process)\n"
        });
        for line in captured_lines() {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("# debug renderings\n");
        for line in &self.rendered {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("# durable coordination state\n");
        out.push_str(&serde_json::to_string(&self.core().cp.snapshot()).unwrap_or_default());
        out.push('\n');
        // The journal as it sits on disk: the input and the result are in it,
        // encrypted, and neither may show.
        out.push_str("# durable state file\n");
        let journal = self.dir.join("state").join("provider-state.journal");
        out.push_str(&hex::encode(std::fs::read(journal).unwrap_or_default()));
        out.push('\n');
        out
    }

    fn secrets(&self) -> Vec<Vec<u8>> {
        self.secrets.clone()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_provider_service_passes_conformance() {
    let report = run_all(ProviderSubject::new).await;
    println!("{}", report.render());
    assert_eq!(report.cases.len(), catalog().len());
    assert_eq!(report.cases.len(), 38);
    for result in &report.cases {
        let exception = NOT_APPLICABLE.iter().find(|(id, _)| *id == result.case.id);
        match (&result.status, exception) {
            (Status::Pass, None) | (Status::Unsupported(_), Some(_)) => {}
            (status, _) => panic!("{} {}: {status:?}", result.case.id, result.case.name),
        }
    }
    let passed = report.cases.iter().filter(|c| c.status == Status::Pass).count();
    assert_eq!(passed + NOT_APPLICABLE.len(), 38);
}

/// The control: a green suite means nothing if the scenario never reached the
/// runner, the journal or the result route.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_scenario_goes_through_the_runner_and_the_journal() {
    let mut s = ProviderSubject::new();
    s.commit_task().await;
    let mut worker = s.new_instance(Actor::Executor);
    let outcome = s.run(&mut worker).await;
    assert_eq!(outcome, Outcome::Submitted(s.task_id()));
    assert_eq!(s.metering.len(), 1, "the external runner did not run");
    s.produce_block();
    let id = s.task_id();
    assert_eq!(s.owner_retrieve_result(id).unwrap(), OUTPUT);
    let journal = s.dir.join("state").join("provider-state.journal");
    let bytes = std::fs::read(journal).unwrap();
    assert!(!bytes.is_empty());
    assert!(!bytes.windows(INPUT.len()).any(|w| w == INPUT));
    assert!(!bytes.windows(OUTPUT.len()).any(|w| w == OUTPUT));
}
