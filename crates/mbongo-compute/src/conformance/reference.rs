//! The reference implementation's adapter to the conformance suite.
//!
//! Wires the in-memory control plane and data plane, the reference worker
//! and the in-memory chain double into [`Subject`], exactly as the
//! integration tests and the live harness wire them. Everything the suite
//! asks for in the contracts' vocabulary is answered here with the
//! reference types; nothing in the suite sees them.
//!
//! Diagnostics are captured at runtime: a process-wide `log` sink records
//! every line the components emit, and every grant, presentation, key,
//! lease and plaintext the scenario touches is rendered with `Debug` into
//! the same buffer, so that C30 checks what an operator would actually
//! see rather than what the source claims.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Once};

use mbongo_core::{
    Address, ComputeTask, Receipt, Transaction, TransactionPayload, TransactionType,
    COMPUTE_TASK_VERSION,
};
use mbongo_verification::RECEIPT_VERSION;

use super::{
    Actor, Claims, FailureKind, FaultPoint, GrantOp, Outcome, Refusal, Retarget, Subject,
    SubjectInfo,
};
use crate::chain::testing::FakeChain;
use crate::chain::ChainClient;
use crate::clock::{Clock, ManualClock};
use crate::control_plane::{AttemptEvent, ControlPlane, ControlPlaneConfig, Lease};
use crate::data_plane::{
    Capability, CapabilityRequest, InMemoryDataPlane, LocalKey, Operation, Presentation,
};
use crate::execution::{
    reference_input_commitment, reference_output_commitment, ExecutionProfile, ReverseBytesProfile,
    REVERSE_BYTES_SPEC,
};
use crate::identity::{CapabilityId, ExecutorKey, IdSource, ObjectId, SessionId};
use crate::worker::{AttemptOutcome, Fault, Worker};

// ── runtime log capture ──────────────────────────────────────────────────

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static INSTALL: Once = Once::new();
static CAPTURE: Capture = Capture;

struct Capture;

impl log::Log for Capture {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }
    fn log(&self, record: &log::Record<'_>) {
        if let Ok(mut buf) = CAPTURED.lock() {
            buf.push(format!(
                "{} {} {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
    }
    fn flush(&self) {}
}

/// Installs the capturing logger once per process. Returns whether this
/// process's logger is the capture (another logger may have been set
/// first, in which case captured output is empty and the report says so).
pub fn install_log_capture() -> bool {
    INSTALL.call_once(|| {
        if log::set_logger(&CAPTURE).is_ok() {
            log::set_max_level(log::LevelFilter::Trace);
        }
    });
    log::max_level() == log::LevelFilter::Trace
}

fn take_captured() -> Vec<String> {
    CAPTURED.lock().map(|mut b| std::mem::take(&mut *b)).unwrap_or_default()
}

// ── the subject ──────────────────────────────────────────────────────────

/// A lease as the reference worker uses it: the session it was offered in
/// plus the lease itself.
#[derive(Debug, Clone)]
pub struct RefLease {
    session: SessionId,
    lease: Lease,
}

/// One scenario over the reference implementation.
pub struct ReferenceSubject {
    clock: ManualClock,
    ids: IdSource,
    chain: FakeChain,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    issuer: LocalKey,
    client: LocalKey,
    executor: ExecutorKey,
    other: ExecutorKey,
    stranger: ExecutorKey,
    input: Vec<u8>,
    task: ComputeTask,
    input_object: ObjectId,
    client_nonce: u64,
    presentations: HashMap<CapabilityId, Presentation>,
    rendered: Vec<String>,
    secrets: Vec<Vec<u8>>,
    capture_active: bool,
}

const SEED_ISSUER: [u8; 32] = [0xC0u8; 32];
const SEED_CLIENT: [u8; 32] = [0xAAu8; 32];
const SEED_EXECUTOR: [u8; 32] = [0xE1u8; 32];
const SEED_OTHER: [u8; 32] = [0xE2u8; 32];
const SEED_STRANGER: [u8; 32] = [0x5Eu8; 32];
const LEASE_SECS: u64 = 60;
const GRANT_SECS: u64 = 120;

impl Default for ReferenceSubject {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceSubject {
    /// A fresh scenario: one private input stored, one task built for the
    /// executor, nothing committed yet.
    pub fn new() -> Self {
        let capture_active = install_log_capture();
        take_captured();
        let clock = ManualClock::starting_at(1_000);
        let clock_arc: Arc<dyn Clock> = Arc::new(clock.clone());
        let issuer = LocalKey::from_seed(&SEED_ISSUER);
        let client = LocalKey::from_seed(&SEED_CLIENT);
        let executor = ExecutorKey::from_seed(&SEED_EXECUTOR);
        let other = ExecutorKey::from_seed(&SEED_OTHER);
        let stranger = ExecutorKey::from_seed(&SEED_STRANGER);
        let cp = ControlPlane::new(
            Arc::clone(&clock_arc),
            IdSource::new([0x22u8; 32]),
            issuer.clone(),
            ControlPlaneConfig {
                lease_secs: LEASE_SECS,
                session_secs: 600,
                confirmation_depth: 1,
                capability_secs: GRANT_SECS,
            },
        );
        let mut dp = InMemoryDataPlane::new(Arc::clone(&clock_arc), IdSource::new([0x33u8; 32]));
        let input = b"conformance private input: never on the chain, never in a log".to_vec();
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client.address(),
            executor: executor.address(),
            salt: [0x5Au8; 32],
            input_commitment: reference_input_commitment(&input),
            execution_spec: REVERSE_BYTES_SPEC.to_vec(),
        };
        let task_id = task.task_id();
        let input_object = dp.store_input(
            &client,
            task_id,
            task.input_commitment,
            input.clone(),
            3_600,
        );
        dp.register_task(&client, task_id, executor.address())
            .expect("fresh registration");
        dp.delegate_issuer(&client, task_id, issuer.address()).expect("owner delegates");
        let mut secrets = vec![
            SEED_ISSUER.to_vec(),
            SEED_CLIENT.to_vec(),
            SEED_EXECUTOR.to_vec(),
            SEED_OTHER.to_vec(),
            SEED_STRANGER.to_vec(),
        ];
        secrets.dedup();
        let mut s = Self {
            clock,
            ids: IdSource::new([0x11u8; 32]),
            chain: FakeChain::new(),
            cp,
            dp,
            issuer,
            client,
            executor,
            other,
            stranger,
            input,
            task,
            input_object,
            client_nonce: 0,
            presentations: HashMap::new(),
            rendered: Vec::new(),
            secrets,
            capture_active,
        };
        s.render(&format!("{:?}", s.executor));
        s.render(&format!("{:?}", s.other));
        s
    }

    fn render(&mut self, line: &str) {
        self.rendered.push(line.to_string());
    }

    fn key(&self, actor: Actor) -> &ExecutorKey {
        match actor {
            Actor::Executor => &self.executor,
            Actor::OtherExecutor => &self.other,
            Actor::Client => &self.client,
            Actor::Stranger => &self.stranger,
        }
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    fn signed_task_tx(&mut self, task: &ComputeTask) -> Transaction {
        let mut tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: self.client.address(),
            receiver: Address::zero(),
            amount: 0,
            nonce: self.client_nonce,
            payload: TransactionPayload::ComputeTask(Box::new(task.clone())),
            signature: [0u8; 64],
        };
        self.client_nonce += 1;
        tx.signature = self.client.sign(&tx.signing_payload());
        tx
    }

    async fn commit(&mut self, task: &ComputeTask) {
        let tx = self.signed_task_tx(task);
        self.chain.submit_transaction(&tx).await.expect("chain double accepts");
        self.chain.produce_block();
        self.chain.produce_block();
        self.cp.observe(&self.chain).await.expect("observation");
    }

    /// Presents `cap` as `actor`, recording the presentation and the proof.
    fn present(&mut self, actor: Actor, cap: &Capability) -> Presentation {
        let key = self.key(actor).clone();
        let ch = self.dp.issue_challenge(key.address());
        let p = Presentation::sign(cap.clone(), ch, &key);
        self.note_presentation(&p);
        p
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
        op: Operation,
        resource: ObjectId,
        id: [u8; 32],
    ) -> Capability {
        let now = self.now();
        Capability {
            capability_id: CapabilityId(id),
            task_id,
            presenter: self.address(actor),
            operation: op,
            resource,
            not_before: now,
            not_after: now + 60,
            max_uses: 1,
            issuer: self.issuer.address(),
            issuer_signature: [0u8; 64],
        }
    }

    fn fetch(&mut self, p: &Presentation) -> Result<Vec<u8>, Refusal> {
        match self.dp.fetch_input(p) {
            Ok(plain) => {
                self.render(&format!("{plain:?}"));
                Ok(plain.as_bytes().to_vec())
            }
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn map_outcome(o: Result<AttemptOutcome, crate::worker::WorkerError>) -> Outcome {
        match o {
            Ok(AttemptOutcome::Idle) => Outcome::Idle,
            Ok(AttemptOutcome::Submitted { task_id, .. }) => Outcome::Submitted(task_id),
            Ok(AttemptOutcome::Completed { task_id, .. }) => Outcome::Completed(task_id),
            Ok(AttemptOutcome::Failed { task_id, class }) => Outcome::Failed(
                task_id,
                match class {
                    crate::control_plane::FailureClass::Input => FailureKind::Input,
                    crate::control_plane::FailureClass::Execution => FailureKind::Execution,
                    crate::control_plane::FailureClass::Persistence => FailureKind::Persistence,
                    crate::control_plane::FailureClass::Receipt => FailureKind::Receipt,
                },
            ),
            Ok(AttemptOutcome::Crashed { task_id, .. }) => Outcome::Crashed(task_id),
            Err(e) => Outcome::Aborted(format!("{e}")),
        }
    }
}

impl Subject for ReferenceSubject {
    type Chain = FakeChain;
    type Grant = Capability;
    type Lease = RefLease;
    type Locator = ObjectId;
    type Instance = Worker;

    fn info(&self) -> SubjectInfo {
        SubjectInfo {
            name: "mbongo-compute reference (in-memory control plane and data plane, chain double)"
                .to_string(),
            profile_tag: String::from_utf8_lossy(REVERSE_BYTES_SPEC).into_owned(),
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
        self.input.clone()
    }

    fn expected_output(&self) -> Vec<u8> {
        ReverseBytesProfile.execute(&self.input).expect("reference profile")
    }

    fn task(&self) -> ComputeTask {
        self.task.clone()
    }

    fn input_locator(&self) -> ObjectId {
        self.input_object
    }

    fn address(&self, actor: Actor) -> Address {
        self.key(actor).address()
    }

    async fn commit_task(&mut self) {
        let task = self.task.clone();
        self.commit(&task).await;
        self.cp.register_input(task.task_id(), self.input_object).expect("observed");
    }

    async fn commit_task_carrying_input_in_spec(&mut self) -> [u8; 32] {
        let probe = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: self.client.address(),
            executor: self.executor.address(),
            salt: [0x5Bu8; 32],
            input_commitment: reference_input_commitment(&self.input),
            execution_spec: self.input.clone(),
        };
        self.commit(&probe).await;
        probe.task_id()
    }

    fn new_instance(&mut self, actor: Actor) -> Worker {
        let key = self.key(actor).clone();
        let w = Worker::new(&mut self.ids, key, Box::new(ReverseBytesProfile));
        self.render(&format!("{w:?}"));
        w
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
        self.dp.inject(crate::data_plane::DataPlaneFault::PutResultFails);
        true
    }

    fn inject_lost_submission_response(&mut self) -> bool {
        self.chain.lose_next_response();
        true
    }

    async fn run(&mut self, instance: &mut Worker) -> Outcome {
        let r = instance.run_once(&mut self.cp, &mut self.dp, &self.chain).await;
        let o = Self::map_outcome(r);
        self.render(&format!("{o:?}"));
        o
    }

    fn obtain_lease(&mut self, instance: &mut Worker) -> Option<RefLease> {
        let session = instance.ensure_session(&mut self.cp).ok()?;
        let lease = self.cp.offer(session.session_id, &mut self.dp).ok()??;
        self.secrets.push(lease.issuer_signature.to_vec());
        self.render(&format!("{lease:?}"));
        Some(RefLease {
            session: session.session_id,
            lease,
        })
    }

    fn grant_under_lease(
        &mut self,
        _: &Worker,
        lease: &RefLease,
        op: GrantOp,
    ) -> Result<Capability, Refusal> {
        let r =
            match op {
                GrantOp::FetchInput => {
                    self.cp.authorize_fetch(lease.session, lease.lease.lease_id, &mut self.dp)
                }
                GrantOp::PutResult => {
                    self.cp.authorize_put(lease.session, lease.lease.lease_id, &mut self.dp)
                }
                GrantOp::GetResult => return Err(Refusal::NoSuchPath(
                    "a worker lease never yields a get-result grant; only the owner may retrieve"
                        .to_string(),
                )),
            };
        match r {
            Ok(cap) => {
                self.note_grant(&cap);
                Ok(cap)
            }
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn report_started(&mut self, _: &Worker, lease: &RefLease) -> Result<(), Refusal> {
        self.cp
            .report(
                lease.session,
                lease.lease.lease_id,
                AttemptEvent::Started,
                &mut self.dp,
            )
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn heartbeat(&mut self, _: &Worker, lease: &RefLease) -> Result<(), Refusal> {
        self.cp
            .heartbeat(lease.session, lease.lease.lease_id)
            .map(|_| ())
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn attempt_count(&self, task_id: [u8; 32]) -> u32 {
        self.cp.task(task_id).map_or(0, |t| t.attempt_count)
    }

    fn executor_of_record(&self, task_id: [u8; 32]) -> Option<Address> {
        self.cp.task(task_id).map(|t| t.task.executor)
    }

    fn owner_side_grant(&mut self, presenter: Actor, op: GrantOp) -> Result<Capability, Refusal> {
        let task_id = self.task.task_id();
        let (operation, resource) = match op {
            GrantOp::FetchInput => (Operation::FetchInput, Some(self.input_object)),
            GrantOp::PutResult => (Operation::PutResult, None),
            GrantOp::GetResult => {
                let r = self
                    .dp
                    .result_ref(task_id, self.executor.address())
                    .ok_or_else(|| Refusal::Denied("no durable result to grant".to_string()))?;
                (Operation::GetResult, Some(r.object))
            }
        };
        let issuer = self.issuer.clone();
        let cap = self
            .dp
            .issue_capability(
                &issuer,
                &CapabilityRequest {
                    task_id,
                    operation,
                    resource,
                    ttl_secs: GRANT_SECS,
                    max_uses: 1,
                },
            )
            .map_err(|e| Refusal::Denied(format!("{e:?}")))?;
        self.note_grant(&cap);
        if cap.presenter != self.address(presenter) {
            return Err(Refusal::NoSuchPath(format!(
                "the issuer cannot name {presenter:?}: grants name the registered party ({})",
                cap.presenter
            )));
        }
        Ok(cap)
    }

    fn present_fetch(&mut self, actor: Actor, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let p = self.present(actor, grant);
        self.fetch(&p)
    }

    fn replay_last_presentation(&mut self, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let p = self.presentations.get(&grant.capability_id).cloned().ok_or_else(|| {
            Refusal::NoSuchPath("no presentation of this grant was observed".to_string())
        })?;
        self.fetch(&p)
    }

    fn present_fetch_unproven(&mut self, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let ch = self.dp.issue_challenge(grant.presenter);
        let p = Presentation {
            capability: grant.clone(),
            challenge: ch,
            proof: [0u8; 64],
        };
        self.fetch(&p)
    }

    fn present_put(
        &mut self,
        actor: Actor,
        grant: &Capability,
        bytes: Vec<u8>,
    ) -> Result<(), Refusal> {
        let p = self.present(actor, grant);
        let commitment = reference_output_commitment(&bytes);
        self.dp
            .put_result(&p, bytes, commitment)
            .map(|_| ())
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn present_get(&mut self, actor: Actor, grant: &Capability) -> Result<Vec<u8>, Refusal> {
        let p = self.present(actor, grant);
        match self.dp.get_result(&p) {
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
        // The best anyone can do with a task id: name it everywhere a grant
        // has a field, and present that.
        let cap = self.forged(
            actor,
            task_id,
            Operation::FetchInput,
            ObjectId(task_id),
            task_id,
        );
        let p = self.present(actor, &cap);
        self.fetch(&p)
    }

    fn fetch_with_locator_only(
        &mut self,
        actor: Actor,
        locator: &ObjectId,
    ) -> Result<Vec<u8>, Refusal> {
        let cap = self.forged(
            actor,
            self.task.task_id(),
            Operation::FetchInput,
            *locator,
            locator.0,
        );
        let p = self.present(actor, &cap);
        self.fetch(&p)
    }

    fn fetch_with_lease_only(
        &mut self,
        actor: Actor,
        lease: &RefLease,
    ) -> Result<Vec<u8>, Refusal> {
        // A lease carries a real issuer signature; presenting it as if it
        // were a grant is the closest a lease can come to a credential.
        let mut cap = self.forged(
            actor,
            lease.lease.task_id,
            Operation::FetchInput,
            self.input_object,
            lease.lease.lease_id.0,
        );
        cap.issuer = lease.lease.issuer;
        cap.issuer_signature = lease.lease.issuer_signature;
        let p = self.present(actor, &cap);
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
        let p = self.present(actor, &cap);
        match self.dp.get_result(&p) {
            Ok(plain) => Ok(plain.as_bytes().to_vec()),
            Err(e) => Err(Refusal::Denied(format!("{e:?}"))),
        }
    }

    fn revoke(&mut self, grant: &Capability) -> Result<(), Refusal> {
        let issuer = self.issuer.clone();
        self.dp
            .revoke(&issuer, grant.task_id, grant.capability_id)
            .map_err(|e| Refusal::Denied(format!("{e:?}")))
    }

    fn retarget(&self, grant: &Capability, how: Retarget) -> Capability {
        let mut c = grant.clone();
        match how {
            Retarget::Task(id) => c.task_id = id,
            Retarget::Presenter(actor) => c.presenter = self.address(actor),
        }
        c
    }

    fn input_consumed(&self, locator: &ObjectId) -> bool {
        self.dp.object_state(locator) == Some(crate::data_plane::ObjectState::Consumed)
    }

    fn result_exists(&self, task_id: [u8; 32]) -> bool {
        self.dp.result_ref(task_id, self.executor.address()).is_some()
    }

    fn result_commitment(&self, task_id: [u8; 32]) -> Option<[u8; 32]> {
        self.dp
            .result_ref(task_id, self.executor.address())
            .map(|r| r.output_commitment)
    }

    fn owner_retrieve_result(&mut self, task_id: [u8; 32]) -> Result<Vec<u8>, Refusal> {
        let r = self
            .dp
            .result_ref(task_id, self.executor.address())
            .ok_or_else(|| Refusal::Denied("no durable result".to_string()))?;
        let client = self.client.clone();
        let cap = self
            .dp
            .issue_capability(
                &client,
                &CapabilityRequest {
                    task_id,
                    operation: Operation::GetResult,
                    resource: Some(r.object),
                    ttl_secs: GRANT_SECS,
                    max_uses: 1,
                },
            )
            .map_err(|e| Refusal::Denied(format!("{e:?}")))?;
        self.note_grant(&cap);
        self.present_get(Actor::Client, &cap)
    }

    fn control_plane_receipt(&mut self, output_commitment: [u8; 32]) -> Receipt {
        let mut receipt = Receipt {
            version: RECEIPT_VERSION,
            task_id: self.task.task_id(),
            input_commitment: self.task.input_commitment,
            output_commitment,
            executor: self.task.executor,
            metadata: Vec::new(),
            signature: [0u8; 64],
        };
        receipt.signature = self.issuer.sign(&receipt.receipt_hash().0);
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
            signature: [0u8; 64],
        };
        tx.signature = self.issuer.sign(&tx.signing_payload());
        tx
    }

    async fn executor_anchor_arbitrary(
        &mut self,
        output_commitment: [u8; 32],
    ) -> Result<(), Refusal> {
        let w = Worker::new(
            &mut self.ids,
            self.executor.clone(),
            Box::new(ReverseBytesProfile),
        );
        let receipt = w.bound_receipt(&self.task, output_commitment);
        let nonce = self
            .chain
            .account_nonce(&self.executor.address())
            .await
            .map_err(|e| Refusal::Denied(e.to_string()))?;
        let tx = w.anchor_transaction(receipt, nonce);
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
        for line in CAPTURED.lock().map(|b| b.clone()).unwrap_or_default() {
            out.push_str(&line);
            out.push('\n');
        }
        out.push_str("# debug renderings\n");
        for line in &self.rendered {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("# durable coordination state\n");
        out.push_str(&serde_json::to_string(&self.cp.snapshot()).unwrap_or_default());
        out.push('\n');
        out
    }

    fn secrets(&self) -> Vec<Vec<u8>> {
        self.secrets.clone()
    }
}
