//! Reference private data plane (F), in memory.
//!
//! Holds private objects, issues and enforces capabilities, and is the
//! authority for whether an object exists, whether a capability has been
//! consumed, and whether a result exists (F §11). It is **not** protocol
//! authority and it is **not durable**: everything here lives in process
//! memory and is lost on restart. That is out of scope for the first
//! worker by design; a durable backend must keep the same invariants.
//!
//! # Five things this module keeps apart (F §3)
//!
//! - an **identifier** ([`ObjectId`]) names an object and grants nothing;
//! - a **locator** is not modelled — the data plane *is* the locator here —
//!   and would grant nothing either;
//! - a **capability** ([`Capability`]) is a signed, bounded, single-use
//!   grant naming one task, one presenter, one operation and one object;
//! - a **secret** is the presenter's private key, which never enters this
//!   module: presentations carry a signature, never the key;
//! - a **commitment** is a public 32-byte value the data plane stores and
//!   never derives.
//!
//! # Who is trusted for what
//!
//! Objects and task registrations are created by their **owner** (the
//! client) under the owner's signature. The owner may **delegate** issuance
//! for a task to another key — the control plane's issuer service in the
//! reference deployment. Before serving an input or accepting a result the
//! data plane checks that the presenting executor equals the executor the
//! owner registered for that task (F §5.1): a compromised control plane
//! can issue grants, but only to the executor the client already named.
//!
//! A `task_id` is public the moment the task is in a block. It is an
//! identifier and nothing else: no method here accepts a bare `task_id` as
//! authorization (F3).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use mbongo_core::Address;

use crate::clock::Clock;
use crate::execution::Plaintext;
use crate::identity::{
    verify_possession, verify_signature, CapabilityId, Challenge, ExecutorKey, IdSource, ObjectId,
};

/// A local-custody Ed25519 key used by a client (owner) or by the control
/// plane's issuer service: the same type as [`ExecutorKey`]. Naming the role
/// keeps it visible which key is acting where; the executor's key is never
/// used as an issuer, and an issuer key can never sign a receipt that the
/// chain would accept for a task it did not name.
pub type LocalKey = ExecutorKey;

/// Domain tag for capability issuer signatures.
const DOMAIN_CAPABILITY: &str = "mbongo:ref-capability:v1";
/// Domain tag for presentation proofs of possession.
const DOMAIN_PRESENT: &str = "mbongo:ref-present:v1";
/// Domain tag for owner registrations.
const DOMAIN_REGISTER: &str = "mbongo:ref-register:v1";
/// How long a challenge stays valid, in seconds.
const CHALLENGE_TTL_SECS: u64 = 120;

/// One operation per capability (F §5.2, least privilege).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Fetch the private input. Single-use by default.
    FetchInput,
    /// Store the private result. Single-use.
    PutResult,
    /// Retrieve the private result. Bounded use.
    GetResult,
}

impl Operation {
    fn tag(self) -> u8 {
        match self {
            Operation::FetchInput => 1,
            Operation::PutResult => 2,
            Operation::GetResult => 3,
        }
    }
}

/// A capability (F §5.3): everything it binds is under the issuer's
/// signature, so changing any field invalidates the grant.
///
/// `presenter` is the key that must prove possession when the capability
/// is presented: the task's executor for `FetchInput` and `PutResult`, the
/// object's owner for `GetResult`.
#[derive(Clone, PartialEq, Eq)]
pub struct Capability {
    /// Unique; the unit of consumption and revocation.
    pub capability_id: CapabilityId,
    /// The task this grant serves.
    pub task_id: [u8; 32],
    /// The only party who may present it, with proof of possession.
    pub presenter: Address,
    /// The one operation it permits.
    pub operation: Operation,
    /// The one object it applies to.
    pub resource: ObjectId,
    /// Not valid before (seconds).
    pub not_before: u64,
    /// Not valid after (seconds).
    pub not_after: u64,
    /// How many presentations are allowed. `1` for fetch and put.
    pub max_uses: u32,
    /// The owner, or a delegate the owner authorised for this task.
    pub issuer: Address,
    /// Issuer signature over every field above.
    pub issuer_signature: [u8; 64],
}

impl std::fmt::Debug for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Logs may carry the capability id and its shape, never the grant
        // itself in a form that could be replayed (F §13).
        write!(
            f,
            "Capability({}, task {}, {:?}, resource {}, presenter {}, until {})",
            self.capability_id,
            hex::encode(&self.task_id[..8]),
            self.operation,
            self.resource,
            self.presenter,
            self.not_after
        )
    }
}

impl Capability {
    /// The bytes the issuer signs.
    fn signing_message(&self) -> Vec<u8> {
        let mut m = Vec::with_capacity(200);
        m.extend_from_slice(DOMAIN_CAPABILITY.as_bytes());
        m.extend_from_slice(&self.capability_id.0);
        m.extend_from_slice(&self.task_id);
        m.extend_from_slice(&self.presenter.0);
        m.push(self.operation.tag());
        m.extend_from_slice(&self.resource.0);
        m.extend_from_slice(&self.not_before.to_le_bytes());
        m.extend_from_slice(&self.not_after.to_le_bytes());
        m.extend_from_slice(&self.max_uses.to_le_bytes());
        m.extend_from_slice(&self.issuer.0);
        m
    }
}

/// What an issuer asks for.
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    /// The task the grant serves.
    pub task_id: [u8; 32],
    /// The operation.
    pub operation: Operation,
    /// The object, for `FetchInput` and `GetResult`. Ignored for
    /// `PutResult`, where the data plane reserves the result object.
    pub resource: Option<ObjectId>,
    /// Lifetime in seconds from now.
    pub ttl_secs: u64,
    /// Presentations allowed. Fetch and put are forced to `1`.
    pub max_uses: u32,
}

/// A capability presented with proof of possession (F §5.3). The proof is
/// bound to the capability id and a fresh data-plane challenge, so an
/// observed presentation cannot be replayed.
#[derive(Clone)]
pub struct Presentation {
    /// The grant.
    pub capability: Capability,
    /// The challenge the data plane issued to the presenter.
    pub challenge: Challenge,
    /// Presenter's signature over `(DOMAIN_PRESENT || capability_id || challenge)`.
    pub proof: [u8; 64],
}

impl std::fmt::Debug for Presentation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Presentation({:?}, challenge {}, proof redacted)",
            self.capability, self.challenge
        )
    }
}

impl Presentation {
    /// Builds a presentation by proving possession of `presenter`'s key.
    pub fn sign(capability: Capability, challenge: Challenge, presenter: &ExecutorKey) -> Self {
        let proof =
            presenter.prove_possession(DOMAIN_PRESENT, &capability.capability_id.0, &challenge);
        Self {
            capability,
            challenge,
            proof,
        }
    }
}

/// Reference to a persisted private result (F §10). Metadata only: no
/// payload, no key, no locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultRef {
    /// The task answered.
    pub task_id: [u8; 32],
    /// The result object.
    pub object: ObjectId,
    /// The commitment the receipt must carry.
    pub output_commitment: [u8; 32],
    /// The executor that produced it.
    pub executor: Address,
    /// The owner who may retrieve it.
    pub owner: Address,
    /// Lifetime end (seconds).
    pub expires_at: u64,
}

/// Object lifecycle (F §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectState {
    /// Stored, not yet referenced by any capability.
    Created,
    /// At least one live capability exists for it.
    Authorized,
    /// The single-use fetch happened.
    Consumed,
    /// Reserved for a result not yet stored.
    Reserved,
    /// Access withdrawn by the owner.
    Revoked,
    /// Content destroyed; identifier kept as a tombstone.
    Deleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObjectKind {
    Input,
    Result,
}

struct PrivateObject {
    id: ObjectId,
    kind: ObjectKind,
    task_id: [u8; 32],
    /// `input_commitment` for inputs, `output_commitment` for results.
    commitment: [u8; 32],
    owner: Address,
    /// The producing executor, for results.
    executor: Option<Address>,
    payload: Option<Vec<u8>>,
    state: ObjectState,
    expires_at: u64,
}

impl std::fmt::Debug for PrivateObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PrivateObject({}, {:?}, task {}, {:?}, payload redacted)",
            self.id,
            self.kind,
            hex::encode(&self.task_id[..8]),
            self.state
        )
    }
}

#[derive(Debug, Clone)]
struct TaskRegistration {
    executor: Address,
    owner: Address,
    delegates: Vec<Address>,
}

#[derive(Debug, Default, Clone)]
struct CapabilityRecord {
    uses: u32,
    revoked: bool,
}

/// Why the data plane refused. Carries classes and identifiers only, never
/// content (F §6, §13).
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum DataPlaneError {
    /// No task registered under this id.
    #[error("unknown task")]
    UnknownTask,
    /// No such object, or not this task's object.
    #[error("unknown object")]
    UnknownObject,
    /// The capability's window has not opened or has closed.
    #[error("capability expired or not yet valid")]
    Expired,
    /// The issuer revoked it.
    #[error("capability revoked")]
    Revoked,
    /// Already presented the permitted number of times.
    #[error("capability consumed")]
    Consumed,
    /// The presenter is not the executor the task names (or not the owner).
    #[error("wrong presenter for this task")]
    WrongPresenter,
    /// The capability names a different task than the object serves.
    #[error("capability task does not match object")]
    WrongTask,
    /// The capability's operation does not fit the object.
    #[error("capability operation does not match object")]
    WrongOperation,
    /// The signer is neither the owner nor a delegate for the task.
    #[error("issuer not authorised for this task")]
    BadIssuer,
    /// The issuer signature does not verify.
    #[error("issuer signature invalid")]
    BadIssuerSignature,
    /// Unknown, expired or already-used challenge.
    #[error("bad challenge")]
    BadChallenge,
    /// The proof of possession does not verify for the presenter.
    #[error("possession proof invalid")]
    BadPossessionProof,
    /// A durable result already exists for this task and executor.
    #[error("a result already exists for this task and executor")]
    ResultAlreadyExists(Box<ResultRef>),
    /// The registration or store is not signed by the owner.
    #[error("owner signature invalid")]
    BadOwnerSignature,
    /// The task was already registered with a different executor.
    #[error("task executor already registered and immutable")]
    ExecutorImmutable,
    /// Transient storage failure; nothing was consumed.
    #[error("storage failure")]
    StorageFailure,
}

/// Fault injection for tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataPlaneFault {
    /// The next `put_result` fails transiently.
    PutResultFails,
}

/// The in-memory reference data plane.
pub struct InMemoryDataPlane {
    clock: Arc<dyn Clock>,
    ids: IdSource,
    objects: BTreeMap<ObjectId, PrivateObject>,
    tasks: BTreeMap<[u8; 32], TaskRegistration>,
    capabilities: BTreeMap<CapabilityId, CapabilityRecord>,
    challenges: BTreeMap<Challenge, (Address, u64)>,
    results: HashMap<([u8; 32], Address), ObjectId>,
    fault: Option<DataPlaneFault>,
}

impl InMemoryDataPlane {
    /// A fresh, empty data plane.
    pub fn new(clock: Arc<dyn Clock>, ids: IdSource) -> Self {
        Self {
            clock,
            ids,
            objects: BTreeMap::new(),
            tasks: BTreeMap::new(),
            capabilities: BTreeMap::new(),
            challenges: BTreeMap::new(),
            results: HashMap::new(),
            fault: None,
        }
    }

    /// Arms a fault for the next matching operation.
    pub fn inject(&mut self, fault: DataPlaneFault) {
        self.fault = Some(fault);
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    // ── owner operations ──────────────────────────────────────────────

    /// Registers, under the owner's signature, which executor the chain
    /// commits for `task_id` (F §5.1, client registration). Immutable once
    /// set: a second registration naming a different executor is refused,
    /// which is what stops any party — the control plane included — from
    /// authorising a worker the client did not name.
    pub fn register_task(
        &mut self,
        owner: &LocalKey,
        task_id: [u8; 32],
        executor: Address,
    ) -> Result<(), DataPlaneError> {
        let mut m = Vec::new();
        m.extend_from_slice(DOMAIN_REGISTER.as_bytes());
        m.extend_from_slice(&task_id);
        m.extend_from_slice(&executor.0);
        let sig = owner.sign(&m);
        if !verify_signature(&owner.address(), &m, &sig) {
            return Err(DataPlaneError::BadOwnerSignature);
        }
        match self.tasks.get(&task_id) {
            Some(existing) if existing.executor != executor => {
                Err(DataPlaneError::ExecutorImmutable)
            }
            Some(_) => Ok(()),
            None => {
                self.tasks.insert(
                    task_id,
                    TaskRegistration {
                        executor,
                        owner: owner.address(),
                        delegates: Vec::new(),
                    },
                );
                log::info!(
                    "data-plane: task {} registered for executor {executor}",
                    hex::encode(&task_id[..8])
                );
                Ok(())
            }
        }
    }

    /// Lets `issuer` issue capabilities for `task_id` on the owner's behalf
    /// (the control plane's issuer service, F §5.1 / E §3).
    pub fn delegate_issuer(
        &mut self,
        owner: &LocalKey,
        task_id: [u8; 32],
        issuer: Address,
    ) -> Result<(), DataPlaneError> {
        let reg = self.tasks.get_mut(&task_id).ok_or(DataPlaneError::UnknownTask)?;
        if reg.owner != owner.address() {
            return Err(DataPlaneError::BadOwnerSignature);
        }
        if !reg.delegates.contains(&issuer) {
            reg.delegates.push(issuer);
        }
        Ok(())
    }

    /// Stores a private input for `task_id`, owned by `owner`, committed
    /// under `input_commitment`. The data plane records the commitment and
    /// never derives it (F §7: verification is the worker's job).
    pub fn store_input(
        &mut self,
        owner: &LocalKey,
        task_id: [u8; 32],
        input_commitment: [u8; 32],
        payload: Vec<u8>,
        ttl_secs: u64,
    ) -> ObjectId {
        let id = ObjectId(self.ids.next("object"));
        self.objects.insert(
            id,
            PrivateObject {
                id,
                kind: ObjectKind::Input,
                task_id,
                commitment: input_commitment,
                owner: owner.address(),
                executor: None,
                payload: Some(payload),
                state: ObjectState::Created,
                expires_at: self.now() + ttl_secs,
            },
        );
        log::info!(
            "data-plane: input object {id} stored for task {}",
            hex::encode(&task_id[..8])
        );
        id
    }

    /// The current state of an object, if it exists.
    pub fn object_state(&self, id: &ObjectId) -> Option<ObjectState> {
        self.objects.get(id).map(|o| o.state)
    }

    /// Destroys an object's content (F §12). Only the owner may.
    pub fn delete_object(&mut self, owner: &LocalKey, id: &ObjectId) -> Result<(), DataPlaneError> {
        let obj = self.objects.get_mut(id).ok_or(DataPlaneError::UnknownObject)?;
        if obj.owner != owner.address() {
            return Err(DataPlaneError::BadOwnerSignature);
        }
        obj.payload = None;
        obj.state = ObjectState::Deleted;
        Ok(())
    }

    // ── issuance ──────────────────────────────────────────────────────

    /// Issues a capability signed by `issuer`, who must be the task's owner
    /// or an owner-delegated issuer. Fetch and put are single-use.
    pub fn issue_capability(
        &mut self,
        issuer: &LocalKey,
        req: &CapabilityRequest,
    ) -> Result<Capability, DataPlaneError> {
        let reg = self.tasks.get(&req.task_id).ok_or(DataPlaneError::UnknownTask)?.clone();
        let issuer_addr = issuer.address();
        if issuer_addr != reg.owner && !reg.delegates.contains(&issuer_addr) {
            return Err(DataPlaneError::BadIssuer);
        }
        let (presenter, resource, max_uses) = match req.operation {
            Operation::FetchInput => {
                let id = req.resource.ok_or(DataPlaneError::UnknownObject)?;
                let obj = self.objects.get(&id).ok_or(DataPlaneError::UnknownObject)?;
                if obj.task_id != req.task_id || obj.kind != ObjectKind::Input {
                    return Err(DataPlaneError::UnknownObject);
                }
                (reg.executor, id, 1)
            }
            Operation::PutResult => {
                // Reserve the result object now, so the grant names one
                // resource like every other grant.
                let id = ObjectId(self.ids.next("object"));
                self.objects.insert(
                    id,
                    PrivateObject {
                        id,
                        kind: ObjectKind::Result,
                        task_id: req.task_id,
                        commitment: [0u8; 32],
                        owner: reg.owner,
                        executor: Some(reg.executor),
                        payload: None,
                        state: ObjectState::Reserved,
                        expires_at: self.now() + req.ttl_secs,
                    },
                );
                (reg.executor, id, 1)
            }
            Operation::GetResult => {
                let id = req.resource.ok_or(DataPlaneError::UnknownObject)?;
                let obj = self.objects.get(&id).ok_or(DataPlaneError::UnknownObject)?;
                if obj.task_id != req.task_id || obj.kind != ObjectKind::Result {
                    return Err(DataPlaneError::UnknownObject);
                }
                (reg.owner, id, req.max_uses.max(1))
            }
        };
        let now = self.now();
        let mut cap = Capability {
            capability_id: CapabilityId(self.ids.next("capability")),
            task_id: req.task_id,
            presenter,
            operation: req.operation,
            resource,
            not_before: now,
            not_after: now + req.ttl_secs,
            max_uses,
            issuer: issuer_addr,
            issuer_signature: [0u8; 64],
        };
        cap.issuer_signature = issuer.sign(&cap.signing_message());
        self.capabilities.insert(cap.capability_id, CapabilityRecord::default());
        if let Some(obj) = self.objects.get_mut(&resource) {
            if obj.state == ObjectState::Created {
                obj.state = ObjectState::Authorized;
            }
        }
        log::info!("data-plane: issued {cap:?}");
        Ok(cap)
    }

    /// Revokes a capability. The issuer or the task owner may.
    pub fn revoke(
        &mut self,
        by: &LocalKey,
        task_id: [u8; 32],
        capability_id: CapabilityId,
    ) -> Result<(), DataPlaneError> {
        let reg = self.tasks.get(&task_id).ok_or(DataPlaneError::UnknownTask)?;
        let who = by.address();
        if who != reg.owner && !reg.delegates.contains(&who) {
            return Err(DataPlaneError::BadIssuer);
        }
        let rec = self.capabilities.get_mut(&capability_id).ok_or(DataPlaneError::Revoked)?;
        rec.revoked = true;
        log::info!("data-plane: capability {capability_id} revoked");
        Ok(())
    }

    /// A fresh single-use challenge for `presenter`.
    pub fn issue_challenge(&mut self, presenter: Address) -> Challenge {
        let ch = Challenge(self.ids.next("challenge"));
        self.challenges.insert(ch, (presenter, self.now() + CHALLENGE_TTL_SECS));
        ch
    }

    // ── presentation ──────────────────────────────────────────────────

    /// Validates a presentation end to end (F §6 step 5) and, on success,
    /// records the use and the spent challenge. Order: issuer signature,
    /// issuer authority, window, revocation, consumption, presenter is the
    /// registered executor (or owner), resource fits, challenge, proof.
    fn validate(
        &mut self,
        p: &Presentation,
        expected_operation: Operation,
        expected_kind: ObjectKind,
    ) -> Result<(), DataPlaneError> {
        let cap = &p.capability;
        if !verify_signature(&cap.issuer, &cap.signing_message(), &cap.issuer_signature) {
            log::warn!(
                "data-plane: refused {} — bad issuer signature",
                cap.capability_id
            );
            return Err(DataPlaneError::BadIssuerSignature);
        }
        let reg = self.tasks.get(&cap.task_id).ok_or(DataPlaneError::UnknownTask)?.clone();
        if cap.issuer != reg.owner && !reg.delegates.contains(&cap.issuer) {
            return Err(DataPlaneError::BadIssuer);
        }
        let now = self.now();
        if now < cap.not_before || now > cap.not_after {
            return Err(DataPlaneError::Expired);
        }
        let rec = self
            .capabilities
            .get(&cap.capability_id)
            .cloned()
            .ok_or(DataPlaneError::Revoked)?;
        if rec.revoked {
            return Err(DataPlaneError::Revoked);
        }
        if rec.uses >= cap.max_uses {
            log::warn!(
                "data-plane: refused {} — already consumed",
                cap.capability_id
            );
            return Err(DataPlaneError::Consumed);
        }
        if cap.operation != expected_operation {
            return Err(DataPlaneError::WrongOperation);
        }
        let expected_presenter = match expected_operation {
            Operation::FetchInput | Operation::PutResult => reg.executor,
            Operation::GetResult => reg.owner,
        };
        if cap.presenter != expected_presenter {
            log::warn!(
                "data-plane: refused {} — presenter is not the task's party",
                cap.capability_id
            );
            return Err(DataPlaneError::WrongPresenter);
        }
        let obj = self.objects.get(&cap.resource).ok_or(DataPlaneError::UnknownObject)?;
        if obj.task_id != cap.task_id {
            return Err(DataPlaneError::WrongTask);
        }
        if obj.kind != expected_kind {
            return Err(DataPlaneError::WrongOperation);
        }
        if matches!(obj.state, ObjectState::Revoked | ObjectState::Deleted) || now > obj.expires_at
        {
            return Err(DataPlaneError::Expired);
        }
        let (for_whom, expires) =
            self.challenges.get(&p.challenge).copied().ok_or(DataPlaneError::BadChallenge)?;
        if for_whom != cap.presenter || now > expires {
            return Err(DataPlaneError::BadChallenge);
        }
        if !verify_possession(
            &cap.presenter,
            DOMAIN_PRESENT,
            &cap.capability_id.0,
            &p.challenge,
            &p.proof,
        ) {
            log::warn!(
                "data-plane: refused {} — possession proof invalid",
                cap.capability_id
            );
            return Err(DataPlaneError::BadPossessionProof);
        }
        // The challenge is spent by a valid proof, whatever happens next.
        self.challenges.remove(&p.challenge);
        Ok(())
    }

    fn record_use(&mut self, id: CapabilityId) {
        if let Some(rec) = self.capabilities.get_mut(&id) {
            rec.uses += 1;
        }
    }

    /// Serves the private input once (F §6). The capability is consumed
    /// even if the caller then crashes: a consumed grant is never reopened
    /// (F §11), and a retry needs a fresh one.
    pub fn fetch_input(&mut self, p: &Presentation) -> Result<Plaintext, DataPlaneError> {
        self.validate(p, Operation::FetchInput, ObjectKind::Input)?;
        let obj = self
            .objects
            .get_mut(&p.capability.resource)
            .ok_or(DataPlaneError::UnknownObject)?;
        let payload = obj.payload.clone().ok_or(DataPlaneError::UnknownObject)?;
        obj.state = ObjectState::Consumed;
        self.record_use(p.capability.capability_id);
        log::info!(
            "data-plane: input served under {}",
            p.capability.capability_id
        );
        Ok(Plaintext::new(payload))
    }

    /// Stores the private result and returns its reference only once the
    /// store is complete: the caller may build a receipt only after this
    /// returns `Ok` (F §10, E §11). A second put for the same
    /// `(task_id, executor)` is refused with the existing reference, so at
    /// most one durable result exists per executor per task. A transient
    /// storage failure consumes nothing.
    pub fn put_result(
        &mut self,
        p: &Presentation,
        payload: Vec<u8>,
        output_commitment: [u8; 32],
    ) -> Result<ResultRef, DataPlaneError> {
        self.validate(p, Operation::PutResult, ObjectKind::Result)?;
        let cap = &p.capability;
        if let Some(existing) = self.results.get(&(cap.task_id, cap.presenter)).copied() {
            let obj = &self.objects[&existing];
            return Err(DataPlaneError::ResultAlreadyExists(Box::new(ResultRef {
                task_id: cap.task_id,
                object: existing,
                output_commitment: obj.commitment,
                executor: cap.presenter,
                owner: obj.owner,
                expires_at: obj.expires_at,
            })));
        }
        if self.fault.take() == Some(DataPlaneFault::PutResultFails) {
            log::warn!(
                "data-plane: storage failure injected for {}",
                cap.capability_id
            );
            return Err(DataPlaneError::StorageFailure);
        }
        let obj = self.objects.get_mut(&cap.resource).ok_or(DataPlaneError::UnknownObject)?;
        if obj.executor != Some(cap.presenter) {
            return Err(DataPlaneError::WrongPresenter);
        }
        obj.payload = Some(payload);
        obj.commitment = output_commitment;
        obj.state = ObjectState::Created;
        let r = ResultRef {
            task_id: cap.task_id,
            object: cap.resource,
            output_commitment,
            executor: cap.presenter,
            owner: obj.owner,
            expires_at: obj.expires_at,
        };
        self.results.insert((cap.task_id, cap.presenter), cap.resource);
        self.record_use(cap.capability_id);
        log::info!(
            "data-plane: result persisted for task {} under {}",
            hex::encode(&cap.task_id[..8]),
            cap.capability_id
        );
        Ok(r)
    }

    /// Serves the private result to its owner under a get-result grant.
    pub fn get_result(&mut self, p: &Presentation) -> Result<Plaintext, DataPlaneError> {
        self.validate(p, Operation::GetResult, ObjectKind::Result)?;
        let obj = self.objects.get(&p.capability.resource).ok_or(DataPlaneError::UnknownObject)?;
        let payload = obj.payload.clone().ok_or(DataPlaneError::UnknownObject)?;
        self.record_use(p.capability.capability_id);
        Ok(Plaintext::new(payload))
    }

    /// The durable result for `(task_id, executor)`, if one exists.
    /// Metadata only; retrieving the bytes needs a get-result grant.
    pub fn result_ref(&self, task_id: [u8; 32], executor: Address) -> Option<ResultRef> {
        let id = *self.results.get(&(task_id, executor))?;
        let obj = self.objects.get(&id)?;
        Some(ResultRef {
            task_id,
            object: id,
            output_commitment: obj.commitment,
            executor,
            owner: obj.owner,
            expires_at: obj.expires_at,
        })
    }

    /// The executor registered for `task_id`, if any.
    pub fn registered_executor(&self, task_id: [u8; 32]) -> Option<Address> {
        self.tasks.get(&task_id).map(|r| r.executor)
    }
}
