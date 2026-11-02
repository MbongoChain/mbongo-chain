//! Mbongo Compute Confidential Authorization (`confidential-auth-v1`): the
//! named suite that proves the release path fails closed everywhere it
//! must and opens only where it should, run against the reference
//! implementation with any [`AttestationVerifier`].
//!
//! Every case plays every role in process — client, control plane,
//! release authority, key service, environment, worker — over the
//! in-memory reference components and the chain double, with a manual
//! clock. No sleeps.
//!
//! The suite is not vacuous: [`negative`] provides deliberately unsafe
//! verifiers, and the test group asserts that the suite fails them.

#![allow(clippy::many_single_char_names, clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, Once};

use mbongo_core::{
    Address, ComputeTask, Transaction, TransactionPayload, TransactionType, COMPUTE_TASK_VERSION,
};
use parity_scale_codec::Encode;

use super::reference::{
    ReferenceAttestationRoot, ReferenceAttestationVerifier, ReferenceEnvironment, REFERENCE_FORMAT,
    REFERENCE_LABEL, REFERENCE_VERIFIER_KIND,
};
use super::{
    to_policy_evidence, AttestationChallenge, AttestationVerifier, ChallengeState, ContentKey,
    EvidenceEnvelope, KeyReleaseAuthority, MeasurementPolicy, Purpose, ReleaseAuthority,
    ReleaseAuthorization, ReleaseError, ReleaseGrant, ReleaseId, ReleaseRequest, ReleaseState,
    SealedInput, SecurityLabel, TrustPolicy, VerifiedAttestation, VerifyError, WrapBinding,
    CONFIDENTIAL_AUTH_VERSION,
};
use crate::chain::testing::FakeChain;
use crate::chain::ChainClient;
use crate::clock::{Clock, ManualClock};
use crate::control_plane::{
    AttemptEvent, ControlPlane, ControlPlaneConfig, ControlPlaneError, Lease, Session,
};
use crate::data_plane::{Capability, InMemoryDataPlane, LocalKey, Presentation};
use crate::execution::{
    reference_input_commitment, reference_output_commitment, ReverseBytesProfile,
    REVERSE_BYTES_SPEC,
};
use crate::identity::{ExecutorKey, IdSource, ObjectId, WorkerInstanceId};
use crate::policy::{
    self, compose, dimension, templates, Advertisement, Binding, Claim, Code, Constraint, Decision,
    DerivedFacts, EffectivePolicy, Evidence, EvidenceLevel, PolicyFragment, Requirement, Scope,
    SessionFacts, Source, TaskFacts, Value,
};
use crate::worker::{AttemptOutcome, Worker};

/// Domain tag for session possession proofs (must match the control plane).
const DOMAIN_SESSION: &str = "mbongo:ref-session:v1";
/// The representation tag the scenario's client registers.
const REPRESENTATION: &str = "mbongo-ref:bytes:v1";

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

fn install_log_capture() -> bool {
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

// ── catalog ──────────────────────────────────────────────────────────────

/// One case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// `K01`…`K30`.
    pub id: &'static str,
    /// Name.
    pub name: &'static str,
    /// Invariants it establishes.
    pub invariants: &'static [&'static str],
}

macro_rules! case {
    ($id:literal, $name:literal, [$($inv:literal),*]) => {
        Case { id: $id, name: $name, invariants: &[$($inv),*] }
    };
}

/// The catalog, K01–K30.
pub fn catalog() -> Vec<Case> {
    vec![
        case!(
            "K01",
            "claim_only_confidential_provider_is_rejected",
            ["K4", "K5", "K25"]
        ),
        case!(
            "K02",
            "valid_attestation_satisfies_confidential",
            ["K5", "K7", "K8", "K18", "K21"]
        ),
        case!(
            "K03",
            "wrong_executor_rejected_before_release",
            ["K3", "K8"]
        ),
        case!("K04", "wrong_session_attestation_rejected", ["K7", "K11"]),
        case!("K05", "stale_attestation_rejected", ["K6"]),
        case!("K06", "expired_challenge_rejected", ["K9"]),
        case!("K07", "consumed_challenge_replay_rejected", ["K10"]),
        case!("K08", "cross_task_attestation_replay_rejected", ["K12"]),
        case!("K09", "unknown_attestation_format_rejected", ["K13"]),
        case!("K10", "untrusted_verifier_root_rejected", ["K14"]),
        case!("K11", "disallowed_measurement_rejected", ["K15"]),
        case!("K12", "revoked_root_rejected", ["K16"]),
        case!("K13", "revoked_session_rejected", ["K7"]),
        case!(
            "K14",
            "certified_evidence_does_not_satisfy_attested",
            ["K5"]
        ),
        case!(
            "K15",
            "attested_evidence_converts_into_j_evidence_record",
            ["K18"]
        ),
        case!(
            "K16",
            "j_ineligible_decision_prevents_release",
            ["K18", "K25"]
        ),
        case!("K17", "j_eligible_decision_allows_release", ["K18"]),
        case!(
            "K18",
            "data_capability_only_after_release_decision",
            ["K19", "K20", "K21"]
        ),
        case!(
            "K19",
            "keys_and_plaintext_absent_from_chain_artifacts",
            ["K1", "K2", "K31", "K32"]
        ),
        case!("K20", "keys_and_plaintext_absent_from_logs", ["K1"]),
        case!("K21", "public_flow_works_without_attestation", ["K26"]),
        case!("K22", "confidential_does_not_downgrade_to_public", ["K17"]),
        case!(
            "K23",
            "verifier_outage_fails_closed_for_confidential",
            ["K17"]
        ),
        case!(
            "K24",
            "same_evidence_cannot_authorize_another_executor",
            ["K8"]
        ),
        case!(
            "K25",
            "same_evidence_cannot_authorize_another_session",
            ["K11"]
        ),
        case!("K26", "release_authorization_expires", ["K6"]),
        case!("K27", "revoked_release_authorization_rejected", ["K16"]),
        case!(
            "K28",
            "attestation_does_not_imply_output_correctness",
            ["K30"]
        ),
        case!(
            "K29",
            "reference_verifier_is_labeled_non_production",
            ["K29"]
        ),
        case!(
            "K30",
            "compute_task_receipt_rpc_remain_unchanged",
            ["K31", "K32", "K33"]
        ),
    ]
}

/// Pass or fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Passed.
    Pass,
    /// Failed, with the first check that did not hold.
    Fail(String),
}

/// One result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    /// The case.
    pub case: Case,
    /// Its status.
    pub status: Status,
}

/// The report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// `confidential-auth-v1`.
    pub version: &'static str,
    /// The verifier under test.
    pub verifier: String,
    /// Its label.
    pub label: String,
    /// Whether it claims production grade.
    pub production_grade: bool,
    /// Results in catalog order.
    pub cases: Vec<CaseResult>,
}

impl Report {
    /// Whether every case passed.
    pub fn passed(&self) -> bool {
        self.cases.iter().all(|c| c.status == Status::Pass)
    }

    /// Machine-readable rendering.
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(
            out,
            "MBONGO COMPUTE CONFIDENTIAL AUTHORIZATION ({})
VERIFIER: {} [{}] production_grade={}",
            self.version, self.verifier, self.label, self.production_grade
        );
        let mut passed = 0;
        for c in &self.cases {
            match &c.status {
                Status::Pass => {
                    passed += 1;
                    let _ = writeln!(
                        out,
                        "  {} {:<55} PASS {:?}",
                        c.case.id, c.case.name, c.case.invariants
                    );
                }
                Status::Fail(m) => {
                    let _ = writeln!(
                        out,
                        "  {} {:<55} FAIL {:?}
      {m}",
                        c.case.id, c.case.name, c.case.invariants
                    );
                }
            }
        }
        let _ = writeln!(
            out,
            "RESULT: {} ({passed}/{})",
            if self.passed() { "PASS" } else { "FAIL" },
            self.cases.len()
        );
        out
    }
}

type CaseOutcome = Result<(), String>;

macro_rules! check {
    ($cond:expr, $($arg:tt)+) => {
        if !($cond) {
            return Err(format!($($arg)+));
        }
    };
}

// ── scenario ─────────────────────────────────────────────────────────────

const SEED_ISSUER: [u8; 32] = [0xC0u8; 32];
const SEED_CLIENT: [u8; 32] = [0xAAu8; 32];
const SEED_EXECUTOR: [u8; 32] = [0xE1u8; 32];
const SEED_OTHER: [u8; 32] = [0xE2u8; 32];
const SEED_ROOT: [u8; 32] = [0xA7u8; 32];
const SEED_ROGUE_ROOT: [u8; 32] = [0xA8u8; 32];
const SEED_ENV_KEY: [u8; 32] = [0xEEu8; 32];
const SEED_BAD_ENV_KEY: [u8; 32] = [0xEFu8; 32];
const MEASUREMENT_OK: [u8; 32] = [0x11u8; 32];
const MEASUREMENT_BAD: [u8; 32] = [0x22u8; 32];
const CHALLENGE_TTL: u64 = 60;
const RELEASE_TTL: u64 = 60;
const EVIDENCE_TTL: u64 = 120;
const MAX_EVIDENCE_AGE: u64 = 300;
const LEASE_SECS: u64 = 600;

struct Scenario {
    clock: ManualClock,
    ids: IdSource,
    chain: FakeChain,
    cp: ControlPlane,
    dp: InMemoryDataPlane,
    authority: ReleaseAuthority,
    keys: KeyReleaseAuthority,
    client: LocalKey,
    executor: ExecutorKey,
    other: ExecutorKey,
    root: ReferenceAttestationRoot,
    rogue: ReferenceAttestationRoot,
    env: ReferenceEnvironment,
    bad_env: ReferenceEnvironment,
    task: ComputeTask,
    task_id: [u8; 32],
    input: Vec<u8>,
    object: ObjectId,
    policy: EffectivePolicy,
    advertisement: Advertisement,
    derived: DerivedFacts,
    client_nonce: u64,
    secrets: Vec<Vec<u8>>,
    rendered: Vec<String>,
    capture_active: bool,
}

fn trust_policy(root: &ReferenceAttestationRoot) -> TrustPolicy {
    let mut anchors = BTreeMap::new();
    anchors.insert(root.anchor_id().to_owned(), root.anchor());
    TrustPolicy {
        accepted_formats: [REFERENCE_FORMAT.to_owned()].into_iter().collect(),
        anchors,
        measurement: MeasurementPolicy {
            allowed: [MEASUREMENT_OK].into_iter().collect(),
            revoked: BTreeSet::new(),
            min_security_version: 1,
            allow_debug: false,
        },
        max_evidence_age_secs: MAX_EVIDENCE_AGE,
        challenge_ttl_secs: CHALLENGE_TTL,
        release_ttl_secs: RELEASE_TTL,
    }
}

impl Scenario {
    async fn new(verifier: Box<dyn AttestationVerifier>) -> Self {
        let capture_active = install_log_capture();
        take_captured();
        let clock = ManualClock::starting_at(1_000);
        let clock_arc: Arc<dyn Clock> = Arc::new(clock.clone());
        let issuer = LocalKey::from_seed(&SEED_ISSUER);
        let client = LocalKey::from_seed(&SEED_CLIENT);
        let executor = ExecutorKey::from_seed(&SEED_EXECUTOR);
        let other = ExecutorKey::from_seed(&SEED_OTHER);
        let root = ReferenceAttestationRoot::from_seed(&SEED_ROOT, "test-root");
        let rogue = ReferenceAttestationRoot::from_seed(&SEED_ROGUE_ROOT, "rogue-root");
        let env = ReferenceEnvironment::new(MEASUREMENT_OK, 2, true, SEED_ENV_KEY);
        let bad_env = ReferenceEnvironment::new(MEASUREMENT_BAD, 2, true, SEED_BAD_ENV_KEY);
        let mut ids = IdSource::new([0x44u8; 32]);
        let cp = ControlPlane::new(
            Arc::clone(&clock_arc),
            IdSource::new([0x22u8; 32]),
            issuer.clone(),
            ControlPlaneConfig {
                lease_secs: LEASE_SECS,
                session_secs: 6_000,
                confirmation_depth: 1,
                capability_secs: 120,
            },
        );
        let mut dp = InMemoryDataPlane::new(Arc::clone(&clock_arc), IdSource::new([0x33u8; 32]));
        let mut authority = ReleaseAuthority::new(
            Arc::clone(&clock_arc),
            IdSource::new([0x55u8; 32]),
            trust_policy(&root),
        );
        authority.register_verifier(verifier);
        let mut keys = KeyReleaseAuthority::new(IdSource::new([0x66u8; 32]));

        let input =
            b"confidential private input: released only to an attested environment".to_vec();
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client.address(),
            executor: executor.address(),
            salt: [0x5Au8; 32],
            input_commitment: reference_input_commitment(&input),
            execution_spec: REVERSE_BYTES_SPEC.to_vec(),
        };
        let task_id = task.task_id();
        let content_key = ContentKey::from_bytes(ids.next("content-key"));
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&ids.next("seal-nonce")[..24]);
        let sealed = super::seal_input(
            &content_key,
            nonce,
            &task_id,
            &task.input_commitment,
            &input,
        );
        let object = dp.store_input(
            &client,
            task_id,
            task.input_commitment,
            sealed.to_bytes(),
            3_600,
        );
        dp.register_task(&client, task_id, executor.address())
            .expect("fresh registration");
        dp.delegate_issuer(&client, task_id, issuer.address()).expect("owner delegates");
        let key_bytes = {
            // The only place the raw key bytes are read: to list them as a
            // secret the logs must not contain.
            let k = ContentKey::from_bytes(IdSource::new([0x44u8; 32]).next("content-key"));
            assert!(k.equals(&content_key));
            IdSource::new([0x44u8; 32]).next("content-key")
        };
        keys.register_content_key(&client, task_id, object, content_key);

        let policy = compose(&[templates::confidential(Source::DataOwner)]).expect("template");
        let advertisement = Advertisement {
            provider_id: "P".into(),
            executors: vec![executor.address()],
            issued_at: 0,
            expires_at: 1_000_000_000,
            revoked: false,
            claims: BTreeMap::new(),
            worker_classes: BTreeMap::new(),
        };
        let secrets = vec![
            key_bytes.to_vec(),
            input.clone(),
            SEED_ROOT.to_vec(),
            SEED_ROGUE_ROOT.to_vec(),
            SEED_ENV_KEY.to_vec(),
            SEED_EXECUTOR.to_vec(),
            sealed.ciphertext.clone(),
        ];
        let mut s = Self {
            clock,
            ids,
            chain: FakeChain::new(),
            cp,
            dp,
            authority,
            keys,
            client,
            executor,
            other,
            root,
            rogue,
            env,
            bad_env,
            task,
            task_id,
            input,
            object,
            policy,
            advertisement,
            derived: DerivedFacts::default(),
            client_nonce: 0,
            secrets,
            rendered: Vec::new(),
            capture_active,
        };
        let task = s.task.clone();
        s.commit(&task).await;
        s.cp.register_confidential_input(task_id, object).expect("registered");
        s.render(&format!("{:?}", s.root));
        s.render(&format!("{:?}", s.env));
        s.render(&format!("{:?}", s.keys));
        s.render(&format!("{:?}", s.authority));
        s
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    fn render(&mut self, line: &str) {
        self.rendered.push(line.to_owned());
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

    /// A second confidential task for the same executor, committed and
    /// registered. Returns its id and object.
    async fn second_confidential_task(&mut self) -> ([u8; 32], ObjectId) {
        let input = b"second confidential input".to_vec();
        let task = ComputeTask {
            salt: [0x5Bu8; 32],
            input_commitment: reference_input_commitment(&input),
            ..self.task.clone()
        };
        let task_id = task.task_id();
        let key = ContentKey::from_bytes(self.ids.next("content-key-2"));
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&self.ids.next("seal-nonce-2")[..24]);
        let sealed = super::seal_input(&key, nonce, &task_id, &task.input_commitment, &input);
        let object = self.dp.store_input(
            &self.client,
            task_id,
            task.input_commitment,
            sealed.to_bytes(),
            3_600,
        );
        self.dp
            .register_task(&self.client, task_id, self.executor.address())
            .expect("registration");
        self.dp
            .delegate_issuer(&self.client, task_id, self.cp.issuer_address())
            .expect("delegate");
        self.keys.register_content_key(&self.client, task_id, object, key);
        self.commit(&task).await;
        self.cp.register_confidential_input(task_id, object).expect("registered");
        (task_id, object)
    }

    /// A PUBLIC task for the same executor with a plaintext object, the
    /// ordinary G path. The confidential task is marked completed first so
    /// the control plane offers this one.
    async fn public_task(&mut self) -> [u8; 32] {
        self.cp.mark_completed(self.task_id, 1);
        let input = b"public input".to_vec();
        let task = ComputeTask {
            salt: [0x5Cu8; 32],
            input_commitment: reference_input_commitment(&input),
            ..self.task.clone()
        };
        let task_id = task.task_id();
        let object =
            self.dp.store_input(&self.client, task_id, task.input_commitment, input, 3_600);
        self.dp
            .register_task(&self.client, task_id, self.executor.address())
            .expect("registration");
        self.dp
            .delegate_issuer(&self.client, task_id, self.cp.issuer_address())
            .expect("delegate");
        self.commit(&task).await;
        self.cp.register_input(task_id, object).expect("registered");
        task_id
    }

    fn session_for(&mut self, key: &ExecutorKey) -> Session {
        let instance = WorkerInstanceId(self.ids.next("worker-instance"));
        let executor = key.address();
        let challenge = self.cp.session_challenge(executor);
        let proof = key.prove_possession(DOMAIN_SESSION, &instance.0, &challenge);
        self.cp
            .open_session(
                instance,
                executor,
                vec![REVERSE_BYTES_SPEC.to_vec()],
                challenge,
                &proof,
            )
            .expect("session opens")
    }

    fn lease_for(&mut self, session: &Session) -> Option<Lease> {
        self.cp.offer(session.session_id, &mut self.dp).expect("offer")
    }

    fn task_facts(&self) -> TaskFacts {
        TaskFacts {
            executor: self.task.executor,
            profile_tag: String::from_utf8_lossy(&self.task.execution_spec).into_owned(),
            representation_tag: REPRESENTATION.to_owned(),
        }
    }

    /// J session facts with the confidential claim as a **claim** only.
    fn session_facts(session: &Session) -> SessionFacts {
        let mut claims = BTreeMap::new();
        claims.insert(
            dimension::CONTRACT_VERSION.to_owned(),
            Claim::plain(Value::Token("provider-policy-v1".into()), Scope::Session),
        );
        claims.insert(
            dimension::REPRESENTATION_TAGS.to_owned(),
            Claim::plain(
                Value::Set([REPRESENTATION.to_owned()].into_iter().collect()),
                Scope::Session,
            ),
        );
        claims.insert(
            dimension::PROFILE_TAGS.to_owned(),
            Claim::plain(
                Value::Set(
                    [String::from_utf8_lossy(REVERSE_BYTES_SPEC).into_owned()]
                        .into_iter()
                        .collect(),
                ),
                Scope::Session,
            ),
        );
        claims.insert(
            dimension::CONFIDENTIAL_EXECUTION.to_owned(),
            Claim::plain(Value::Bool(true), Scope::Session),
        );
        SessionFacts {
            session_id: session.session_id,
            proved_executor: session.executor,
            worker_class: None,
            claims,
        }
    }

    fn challenge(&mut self, session: &Session) -> Result<AttestationChallenge, ReleaseError> {
        let c = self.authority.issue_challenge(
            session,
            self.task_id,
            self.task.executor,
            Purpose::ConfidentialInputRelease,
        );
        if let Ok(c) = &c {
            self.secrets.push(c.nonce.to_vec());
            self.render(&format!("{c:?}"));
        }
        c
    }

    fn evidence(&mut self, challenge: &AttestationChallenge) -> EvidenceEnvelope {
        let now = self.now();
        let e = self.env.evidence(&self.root, challenge, now, EVIDENCE_TTL);
        self.secrets.push(e.payload.clone());
        self.render(&format!("{e:?}"));
        e
    }

    fn present(
        &mut self,
        session: &Session,
        envelope: &EvidenceEnvelope,
    ) -> Result<VerifiedAttestation, ReleaseError> {
        let r = self.authority.present_evidence(session, envelope);
        if let Ok(v) = &r {
            self.render(&format!("{v:?}"));
        }
        r
    }

    fn authorize(
        &mut self,
        session: &Session,
        challenge_id: super::ChallengeId,
    ) -> Result<(ReleaseAuthorization, Decision), ReleaseError> {
        self.authorize_under(session, challenge_id, None)
    }

    fn authorize_under(
        &mut self,
        session: &Session,
        challenge_id: super::ChallengeId,
        policy: Option<&EffectivePolicy>,
    ) -> Result<(ReleaseAuthorization, Decision), ReleaseError> {
        let task = self.task_facts();
        let facts = Self::session_facts(session);
        let default = self.policy.clone();
        let policy = policy.unwrap_or(&default);
        let advertisement = self.advertisement.clone();
        let derived = self.derived.clone();
        let r = self.authority.authorize_release(&ReleaseRequest {
            session,
            challenge_id,
            task: &task,
            session_facts: &facts,
            advertisement: &advertisement,
            derived: &derived,
            policy,
        });
        if let Ok((a, d)) = &r {
            self.render(&format!("{a:?}"));
            self.render(&format!("{d:?}"));
        }
        r
    }

    /// Challenge → evidence → verification → J → release.
    fn release(
        &mut self,
        session: &Session,
    ) -> Result<(ReleaseAuthorization, Decision), ReleaseError> {
        let c = self.challenge(session)?;
        let e = self.evidence(&c);
        self.present(session, &e)?;
        self.authorize(session, c.challenge_id)
    }

    fn redeem(
        &mut self,
        session: &Session,
        lease: &Lease,
        release_id: ReleaseId,
    ) -> Result<(Capability, ReleaseGrant), ControlPlaneError> {
        let r = self.cp.authorize_fetch_confidential(
            session.session_id,
            lease.lease_id,
            release_id,
            &mut self.authority,
            &mut self.dp,
        );
        if let Ok((cap, grant)) = &r {
            self.secrets.push(cap.issuer_signature.to_vec());
            self.render(&format!("{cap:?}"));
            self.render(&format!("{grant:?}"));
        }
        r
    }

    /// Fetches the sealed input under `cap` and opens it inside `env`
    /// with the key released for `grant`.
    fn open(
        &mut self,
        cap: &Capability,
        grant: &ReleaseGrant,
        object: ObjectId,
        input_commitment: [u8; 32],
        env_is_bad: bool,
    ) -> Result<Vec<u8>, String> {
        let wrapped = self.keys.release(grant, object).map_err(|e| format!("key release: {e}"))?;
        self.secrets.push(wrapped.ciphertext.clone());
        self.render(&format!("{wrapped:?}"));
        let ch = self.dp.issue_challenge(self.executor.address());
        let p = Presentation::sign(cap.clone(), ch, &self.executor);
        self.secrets.push(p.proof.to_vec());
        let sealed_bytes = self.dp.fetch_input(&p).map_err(|e| format!("fetch: {e}"))?;
        let sealed =
            SealedInput::from_bytes(sealed_bytes.as_bytes()).map_err(|e| format!("{e}"))?;
        let binding = WrapBinding {
            release_id: grant.release_id,
            task_id: grant.task_id,
            executor: grant.executor,
            session_id: grant.session_id,
        };
        let env = if env_is_bad { &self.bad_env } else { &self.env };
        let plain = env
            .open_input(
                &wrapped,
                &binding,
                &grant.task_id,
                &input_commitment,
                &sealed,
            )
            .map_err(|e| format!("open: {e}"))?;
        self.rendered.push(format!("{plain:?}"));
        Ok(plain.as_bytes().to_vec())
    }

    /// The whole confidential path for the scenario task.
    fn full_flow(
        &mut self,
        session: &Session,
        lease: &Lease,
    ) -> Result<(Vec<u8>, ReleaseGrant, Decision), String> {
        let (auth, decision) = self.release(session).map_err(|e| format!("release: {e}"))?;
        let (cap, grant) = self
            .redeem(session, lease, auth.release_id)
            .map_err(|e| format!("redeem: {e}"))?;
        let plain = self.open(&cap, &grant, self.object, self.task.input_commitment, false)?;
        Ok((plain, grant, decision))
    }

    /// Executes, persists and anchors a receipt for `output` under the
    /// lease, exactly as the reference worker would after the input step.
    async fn persist_and_anchor(
        &mut self,
        session: &Session,
        lease: &Lease,
        output: Vec<u8>,
    ) -> Result<mbongo_core::Receipt, String> {
        self.cp
            .report(
                session.session_id,
                lease.lease_id,
                AttemptEvent::InputConsumed,
                &mut self.dp,
            )
            .map_err(|e| format!("{e}"))?;
        self.cp
            .report(
                session.session_id,
                lease.lease_id,
                AttemptEvent::Started,
                &mut self.dp,
            )
            .map_err(|e| format!("{e}"))?;
        let output_commitment = reference_output_commitment(&output);
        let put = self
            .cp
            .authorize_put(session.session_id, lease.lease_id, &mut self.dp)
            .map_err(|e| format!("{e}"))?;
        let ch = self.dp.issue_challenge(self.executor.address());
        let p = Presentation::sign(put, ch, &self.executor);
        self.dp.put_result(&p, output, output_commitment).map_err(|e| format!("{e}"))?;
        let mut w_ids = IdSource::new([0x99u8; 32]);
        let worker = Worker::new(
            &mut w_ids,
            self.executor.clone(),
            Box::new(ReverseBytesProfile),
        );
        let receipt = worker.bound_receipt(&self.task, output_commitment);
        let nonce = self
            .chain
            .account_nonce(&self.executor.address())
            .await
            .map_err(|e| format!("{e}"))?;
        let tx = worker.anchor_transaction(receipt.clone(), nonce);
        self.chain.submit_transaction(&tx).await.map_err(|e| format!("{e}"))?;
        self.chain.produce_block();
        Ok(receipt)
    }

    async fn chain_bytes(&self) -> Vec<u8> {
        let latest = self.chain.latest_height().await.expect("height");
        let mut all = Vec::new();
        for h in 0..=latest {
            if let Some(b) = self.chain.block_by_height(h).await.expect("block") {
                all.extend_from_slice(&b.encode());
            }
        }
        all
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
    }

    fn diagnostics(&self) -> Vec<String> {
        let mut d = self.rendered.clone();
        d.extend(CAPTURED.lock().map(|b| b.clone()).unwrap_or_default());
        d
    }
}

/// Whether `r` is a verification refusal of exactly `want`. `Ok` is never
/// a match: a case that expected a refusal and got a release fails.
fn refused_with<T>(r: &Result<T, ReleaseError>, want: &VerifyError) -> bool {
    matches!(r, Err(ReleaseError::Verify(v)) if v == want)
}

// ── cases ────────────────────────────────────────────────────────────────

fn k01(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    // The ordinary path refuses a confidential input outright.
    let r = s.cp.authorize_fetch(session.session_id, lease.lease_id, &mut s.dp);
    check!(
        matches!(r, Err(ControlPlaneError::ConfidentialReleaseRequired)),
        "ordinary authorize_fetch must refuse a confidential input: {r:?}"
    );
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Created),
        "no capability may exist"
    );
    // J alone, with the claim only: ineligible.
    let facts = Scenario::session_facts(&session);
    let dec = policy::eligible(
        &policy::Input {
            task: &s.task_facts(),
            session: &facts,
            advertisement: &s.advertisement,
            derived: &s.derived,
            now: s.now(),
        },
        &s.policy,
    );
    check!(!dec.eligible, "claim only must be ineligible");
    check!(
        dec.codes().contains(&Code::AttestationRequired),
        "reason must be ATTESTATION_REQUIRED: {:?}",
        dec.codes()
    );
    // The authority, with a challenge that was never answered: no release.
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let r = s.authorize(&session, c.challenge_id);
    check!(
        matches!(r, Err(ReleaseError::ChallengeNotPresented)),
        "no evidence → no release: {r:?}"
    );
    Ok(())
}

fn k02(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let (plain, grant, decision) = s.full_flow(&session, &lease)?;
    check!(plain == s.input, "plaintext must equal the client's input");
    check!(decision.eligible, "decision must be eligible");
    check!(
        decision.lowest_evidence == EvidenceLevel::Attested,
        "rests on ATTESTED"
    );
    check!(
        s.authority.release_state(grant.release_id) == Some(ReleaseState::Consumed),
        "release consumed"
    );
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Consumed),
        "object consumed"
    );
    check!(
        reference_input_commitment(&plain) == s.task.input_commitment,
        "commitment verifies over plaintext"
    );
    Ok(())
}

fn k03(s: &mut Scenario) -> CaseOutcome {
    let session_b = s.session_for(&s.other.clone());
    check!(
        s.lease_for(&session_b).is_none(),
        "the control plane offers B nothing"
    );
    let r = s.authority.issue_challenge(
        &session_b,
        s.task_id,
        s.task.executor,
        Purpose::ConfidentialInputRelease,
    );
    check!(
        matches!(r, Err(ReleaseError::ExecutorMismatch)),
        "executor gate first: {r:?}"
    );
    Ok(())
}

fn k04(s: &mut Scenario) -> CaseOutcome {
    let s1 = s.session_for(&s.executor.clone());
    let s2 = s.session_for(&s.executor.clone());
    check!(s1.session_id != s2.session_id, "two sessions");
    let c = s.challenge(&s1).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    let r = s.present(&s2, &e);
    check!(
        matches!(r, Err(ReleaseError::SessionMismatch)),
        "evidence for session 1 under session 2: {r:?}"
    );
    // And an envelope relabelled for session 2 over a body for session 1.
    let mut relabelled = e.clone();
    relabelled.session_id = s2.session_id;
    let r = s.present(&s2, &relabelled);
    check!(r.is_err(), "relabelled envelope must be refused");
    Ok(())
}

fn k05(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    // Expired.
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    let e = s.root.seal(s.env.body_for("test-root", &c, now, now + 10));
    s.clock.advance(11);
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::EvidenceExpired),
        "expired: {r:?}"
    );
    // Not yet valid.
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    let e = s.root.seal(s.env.body_for("test-root", &c, now + 50, now + 100));
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::EvidenceNotYetValid),
        "not yet valid: {r:?}"
    );
    // Too old.
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    let e = s
        .root
        .seal(s.env.body_for("test-root", &c, now - MAX_EVIDENCE_AGE - 1, now + 100));
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::EvidenceTooOld),
        "too old: {r:?}"
    );
    Ok(())
}

fn k06(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    s.clock.advance(CHALLENGE_TTL + 1);
    let e = s.evidence(&c);
    let r = s.present(&session, &e);
    check!(
        matches!(r, Err(ReleaseError::ChallengeExpired)),
        "expired challenge: {r:?}"
    );
    check!(
        s.authority.challenge_state(c.challenge_id) == Some(ChallengeState::Expired),
        "state Expired"
    );
    Ok(())
}

fn k07(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    s.present(&session, &e).map_err(|e| format!("{e}"))?;
    let (auth, _) = s.authorize(&session, c.challenge_id).map_err(|e| format!("{e}"))?;
    let (_, grant) = s.redeem(&session, &lease, auth.release_id).map_err(|e| format!("{e}"))?;
    let r = s.present(&session, &e);
    check!(
        matches!(r, Err(ReleaseError::ChallengeConsumed)),
        "replayed evidence: {r:?}"
    );
    let r = s.authorize(&session, c.challenge_id);
    check!(
        matches!(r, Err(ReleaseError::ChallengeConsumed)),
        "re-authorize: {r:?}"
    );
    let r = s.redeem(&session, &lease, auth.release_id);
    check!(
        matches!(
            r,
            Err(ControlPlaneError::Release(ReleaseError::ReleaseConsumed))
        ),
        "re-redeem: {r:?}"
    );
    let w1 = s.keys.release(&grant, s.object).map_err(|e| format!("{e}"))?;
    let r = s.keys.release(&grant, s.object);
    check!(
        matches!(r, Err(ReleaseError::ReleaseConsumed)),
        "second key release: {r:?}"
    );
    s.secrets.push(w1.ciphertext);
    Ok(())
}

async fn k08(s: &mut Scenario) -> CaseOutcome {
    let (task2, _object2) = s.second_confidential_task().await;
    let session = s.session_for(&s.executor.clone());
    let lease1 = s.lease_for(&session).ok_or("no lease 1")?;
    let c1 = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let c2 = s
        .authority
        .issue_challenge(
            &session,
            task2,
            s.task.executor,
            Purpose::ConfidentialInputRelease,
        )
        .map_err(|e| format!("{e}"))?;
    // Evidence for task 1's challenge, relabelled as answering task 2's.
    let e1 = s.evidence(&c1);
    let mut relabelled = e1.clone();
    relabelled.challenge_id = c2.challenge_id;
    let r = s.present(&session, &relabelled);
    check!(
        r.is_err(),
        "evidence for task 1 must not verify for task 2: {r:?}"
    );
    check!(
        s.authority.challenge_state(c2.challenge_id) == Some(ChallengeState::Issued),
        "task 2's challenge untouched"
    );
    // A genuine release for task 1 cannot be redeemed on a lease for task 2.
    s.present(&session, &e1).map_err(|e| format!("{e}"))?;
    let (auth1, _) = s.authorize(&session, c1.challenge_id).map_err(|e| format!("{e}"))?;
    let r = s.authority.redeem(&session, auth1.release_id, task2);
    check!(
        matches!(r, Err(ReleaseError::TaskMismatch)),
        "release for task 1 on task 2: {r:?}"
    );
    // The genuine redemption still works afterwards (nothing was consumed).
    let r = s.redeem(&session, &lease1, auth1.release_id);
    check!(r.is_ok(), "genuine redemption: {r:?}");
    Ok(())
}

fn k09(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let mut e = s.evidence(&c);
    e.format = "vendor-x-quote:v9".into();
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::UnknownFormat),
        "unknown format: {r:?}"
    );
    let mut e2 = s.evidence(&c);
    e2.payload = b"not an envelope".to_vec();
    let r = s.present(&session, &e2);
    check!(
        refused_with(&r, &VerifyError::Malformed),
        "malformed: {r:?}"
    );
    Ok(())
}

fn k10(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    // Signed by a root the policy does not know.
    let e = s.rogue.seal(s.env.body_for("rogue-root", &c, now, now + EVIDENCE_TTL));
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::UntrustedRoot),
        "untrusted root: {r:?}"
    );
    // Signed by the rogue root but claiming the trusted anchor: a forgery.
    let forged = s.rogue.seal(s.env.body_for("test-root", &c, now, now + EVIDENCE_TTL));
    let r = s.present(&session, &forged);
    check!(
        refused_with(&r, &VerifyError::BadSignature),
        "forged signature: {r:?}"
    );
    let r = s.authorize(&session, c.challenge_id);
    check!(r.is_err(), "nothing released after a forgery: {r:?}");
    Ok(())
}

fn k11(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    let e = s.root.seal(s.bad_env.body_for("test-root", &c, now, now + EVIDENCE_TTL));
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::MeasurementNotAllowed),
        "bad measurement: {r:?}"
    );
    let mut body = s.env.body_for("test-root", &c, now, now + EVIDENCE_TTL);
    body.debug_disabled = false;
    let r = s.present(&session, &s.root.seal(body));
    check!(refused_with(&r, &VerifyError::DebugEnabled), "debug: {r:?}");
    let mut body = s.env.body_for("test-root", &c, now, now + EVIDENCE_TTL);
    body.security_version = 0;
    let r = s.present(&session, &s.root.seal(body));
    check!(
        refused_with(&r, &VerifyError::SecurityVersionTooLow),
        "security version: {r:?}"
    );
    // A revoked measurement fails even though it is allowed.
    s.authority.revoke_measurement(MEASUREMENT_OK);
    let e = s.evidence(&c);
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::MeasurementRevoked),
        "revoked measurement: {r:?}"
    );
    Ok(())
}

fn k12(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let (auth, _) = s.release(&session).map_err(|e| format!("{e}"))?;
    s.authority.revoke_anchor("test-root");
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::RevokedRoot),
        "revoked root: {r:?}"
    );
    // The unredeemed release that rested on it is gone too.
    let r = s.authority.redeem(&session, auth.release_id, s.task_id);
    check!(
        matches!(r, Err(ReleaseError::ReleaseRevoked)),
        "unredeemed release revoked: {r:?}"
    );
    // Rotation to a new root: old evidence fails, new evidence passes.
    let new_root = ReferenceAttestationRoot::from_seed(&[0xA9u8; 32], "test-root-2");
    s.authority.set_trust_policy(trust_policy(&new_root));
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    let old = s.root.seal(s.env.body_for("test-root", &c, now, now + EVIDENCE_TTL));
    let r = s.present(&session, &old);
    check!(
        refused_with(&r, &VerifyError::UntrustedRoot),
        "old root after rotation: {r:?}"
    );
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let fresh = new_root.seal(s.env.body_for("test-root-2", &c, now, now + EVIDENCE_TTL));
    let r = s.present(&session, &fresh);
    check!(r.is_ok(), "new root after rotation: {r:?}");
    Ok(())
}

fn k13(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let (auth, _) = s.release(&session).map_err(|e| format!("{e}"))?;
    s.authority.revoke_session(session.session_id);
    let r = s.challenge(&session);
    check!(
        matches!(r, Err(ReleaseError::SessionRevoked)),
        "challenge after revocation: {r:?}"
    );
    let r = s.redeem(&session, &lease, auth.release_id);
    check!(
        matches!(
            r,
            Err(ControlPlaneError::Release(ReleaseError::SessionRevoked))
        ),
        "redeem after revocation: {r:?}"
    );
    check!(
        s.authority.release_state(auth.release_id) == Some(ReleaseState::Revoked),
        "release revoked"
    );
    Ok(())
}

fn k14(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let mut facts = Scenario::session_facts(&session);
    facts.claims.insert(
        dimension::CONFIDENTIAL_EXECUTION.to_owned(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(Evidence {
                level: EvidenceLevel::Certified,
                issued_at: s.now(),
                expires_at: Some(s.now() + 100),
                binding: Binding::Session(session.session_id),
                verifier: "some-certifier".into(),
                reference: "cert-1".into(),
                revoked: false,
            }),
        },
    );
    let dec = policy::eligible(
        &policy::Input {
            task: &s.task_facts(),
            session: &facts,
            advertisement: &s.advertisement,
            derived: &s.derived,
            now: s.now(),
        },
        &s.policy,
    );
    check!(
        !dec.eligible && dec.codes() == vec![Code::AttestationRequired],
        "CERTIFIED < ATTESTED: {:?}",
        dec.codes()
    );
    // Injected ATTESTED evidence in the caller's facts is ignored by the
    // authority: without a verified challenge there is no release.
    let mut injected = facts.clone();
    injected.claims.insert(
        dimension::CONFIDENTIAL_EXECUTION.to_owned(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(Evidence {
                level: EvidenceLevel::Attested,
                issued_at: s.now(),
                expires_at: Some(s.now() + 100),
                binding: Binding::Session(session.session_id),
                verifier: "forged".into(),
                reference: "forged".into(),
                revoked: false,
            }),
        },
    );
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let task = s.task_facts();
    let r = s.authority.authorize_release(&ReleaseRequest {
        session: &session,
        challenge_id: c.challenge_id,
        task: &task,
        session_facts: &injected,
        advertisement: &s.advertisement,
        derived: &s.derived,
        policy: &s.policy,
    });
    check!(
        matches!(r, Err(ReleaseError::ChallengeNotPresented)),
        "injected evidence ignored: {r:?}"
    );
    Ok(())
}

fn k15(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    let v = s.present(&session, &e).map_err(|e| format!("{e}"))?;
    let ev = to_policy_evidence(&v);
    check!(ev.level == EvidenceLevel::Attested, "level");
    check!(
        ev.issued_at == v.issued_at && ev.expires_at == Some(v.expires_at),
        "window"
    );
    check!(
        ev.binding
            == Binding::Challenge {
                session: session.session_id,
                challenge_ref: hex::encode(c.challenge_id.0)
            },
        "binding"
    );
    check!(ev.verifier == v.verifier, "verifier");
    check!(
        ev.reference == hex::encode(e.reference()),
        "opaque reference"
    );
    check!(!ev.revoked, "not revoked");
    check!(
        v.security_label == SecurityLabel::ReferenceAttested,
        "label"
    );
    check!(
        v.environment_key == s.env.public_key(),
        "environment key bound"
    );
    Ok(())
}

fn k16(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let strict = compose(&[
        templates::confidential(Source::DataOwner),
        PolicyFragment {
            source: Source::Organization,
            requirements: vec![Requirement::hard(
                "org.residency",
                Source::Organization,
                dimension::RESIDENCY_DATA,
                Constraint::OneOf(["CA".to_owned()].into_iter().collect()),
            )],
        },
    ])
    .map_err(|e| format!("{e}"))?;
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    s.present(&session, &e).map_err(|e| format!("{e}"))?;
    let r = s.authorize_under(&session, c.challenge_id, Some(&strict));
    match r {
        Err(ReleaseError::PolicyIneligible { codes }) => {
            check!(
                codes.contains(&"CAPABILITY_UNKNOWN".to_owned()),
                "codes: {codes:?}"
            );
        }
        other => return Err(format!("expected PolicyIneligible: {other:?}")),
    }
    check!(
        s.authority.challenge_state(c.challenge_id) == Some(ChallengeState::Denied),
        "challenge spent"
    );
    let r = s.redeem(&session, &lease, ReleaseId([9u8; 32]));
    check!(
        matches!(
            r,
            Err(ControlPlaneError::Release(ReleaseError::UnknownRelease))
        ),
        "no release exists: {r:?}"
    );
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Created),
        "no capability"
    );
    Ok(())
}

fn k17(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let (auth, decision) = s.release(&session).map_err(|e| format!("{e}"))?;
    check!(decision.eligible, "eligible");
    check!(
        decision.satisfied.iter().any(|f| f.requirement_id == "confidential.execution"
            && f.evidence_level == Some(EvidenceLevel::Attested)),
        "confidential requirement satisfied at ATTESTED"
    );
    check!(
        auth.lowest_evidence == EvidenceLevel::Attested,
        "release rests on ATTESTED"
    );
    check!(
        s.authority.release_state(auth.release_id) == Some(ReleaseState::Released),
        "release minted"
    );
    Ok(())
}

fn k18(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let r = s.redeem(&session, &lease, ReleaseId([7u8; 32]));
    check!(
        matches!(
            r,
            Err(ControlPlaneError::Release(ReleaseError::UnknownRelease))
        ),
        "bogus release: {r:?}"
    );
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    s.present(&session, &e).map_err(|e| format!("{e}"))?;
    // Verified but not authorized: still nothing.
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Created),
        "verified ≠ released"
    );
    let r = s.cp.authorize_fetch(session.session_id, lease.lease_id, &mut s.dp);
    check!(
        matches!(r, Err(ControlPlaneError::ConfidentialReleaseRequired)),
        "ordinary path still refuses"
    );
    let (auth, _) = s.authorize(&session, c.challenge_id).map_err(|e| format!("{e}"))?;
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Created),
        "authorized ≠ capability issued"
    );
    let (cap, _) = s.redeem(&session, &lease, auth.release_id).map_err(|e| format!("{e}"))?;
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Authorized),
        "capability only now"
    );
    check!(
        cap.task_id == s.task_id && cap.presenter == s.task.executor,
        "capability bound to task and executor"
    );
    Ok(())
}

async fn k19(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let (plain, _, _) = s.full_flow(&session, &lease)?;
    let mut output = plain.clone();
    output.reverse();
    let receipt = s.persist_and_anchor(&session, &lease, output).await?;
    let bytes = s.chain_bytes().await;
    check!(
        Scenario::contains(&bytes, &s.task_id),
        "the task id is on-chain (sanity)"
    );
    check!(
        Scenario::contains(&bytes, &receipt.output_commitment),
        "the receipt is on-chain (sanity)"
    );
    for (i, secret) in s.secrets.clone().iter().enumerate() {
        check!(
            !Scenario::contains(&bytes, secret),
            "secret #{i} ({} bytes) found in chain bytes",
            secret.len()
        );
    }
    check!(
        !Scenario::contains(&bytes, &s.env.public_key()),
        "environment key not on-chain"
    );
    check!(receipt.metadata.is_empty(), "receipt metadata empty");
    check!(
        s.task.execution_spec == REVERSE_BYTES_SPEC,
        "execution_spec carries only the profile tag"
    );
    Ok(())
}

fn k20(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let _ = s.full_flow(&session, &lease)?;
    let diagnostics = s.diagnostics();
    check!(!diagnostics.is_empty(), "diagnostics captured");
    check!(s.capture_active, "log capture active");
    let input_text = String::from_utf8_lossy(&s.input).into_owned();
    for line in &diagnostics {
        check!(
            !line.contains(&input_text),
            "plaintext input in diagnostics: {line}"
        );
        for secret in &s.secrets {
            let h = hex::encode(secret);
            check!(!line.contains(&h), "secret hex in diagnostics: {line}");
        }
    }
    check!(
        diagnostics.iter().any(|l| l.contains("release") && l.contains("minted")),
        "release event logged"
    );
    Ok(())
}

async fn k21(s: &mut Scenario) -> CaseOutcome {
    let task_id = s.public_task().await;
    let mut ids = IdSource::new([0x77u8; 32]);
    let mut worker = Worker::new(&mut ids, s.executor.clone(), Box::new(ReverseBytesProfile));
    let outcome = worker
        .run_once(&mut s.cp, &mut s.dp, &s.chain)
        .await
        .map_err(|e| format!("{e}"))?;
    check!(
        matches!(outcome, AttemptOutcome::Submitted { task_id: t, .. } if t == task_id),
        "PUBLIC flow submits: {outcome:?}"
    );
    Ok(())
}

fn k22(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let now = s.now();
    let bad = s.root.seal(s.bad_env.body_for("test-root", &c, now, now + EVIDENCE_TTL));
    check!(s.present(&session, &bad).is_err(), "bad evidence refused");
    let r = s.cp.authorize_fetch(session.session_id, lease.lease_id, &mut s.dp);
    check!(
        matches!(r, Err(ControlPlaneError::ConfidentialReleaseRequired)),
        "no downgrade to ordinary: {r:?}"
    );
    let r = s.authorize_under(
        &session,
        c.challenge_id,
        Some(&compose(&[templates::public(Source::DataOwner)]).unwrap()),
    );
    check!(
        r.is_err(),
        "a PUBLIC policy handed to the authority still needs verified evidence: {r:?}"
    );
    check!(
        s.dp.object_state(&s.object) == Some(crate::data_plane::ObjectState::Created),
        "nothing served"
    );
    Ok(())
}

async fn k23(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    s.authority.unregister_verifier(REFERENCE_VERIFIER_KIND);
    let r = s.present(&session, &e);
    check!(
        refused_with(&r, &VerifyError::VerifierUnavailable),
        "outage: {r:?}"
    );
    s.authority.clear_trust_policy();
    let r = s.challenge(&session);
    check!(
        matches!(r, Err(ReleaseError::TrustPolicyMissing)),
        "no trust policy: {r:?}"
    );
    let task_id = s.public_task().await;
    let mut ids = IdSource::new([0x78u8; 32]);
    let mut worker = Worker::new(&mut ids, s.executor.clone(), Box::new(ReverseBytesProfile));
    let outcome = worker
        .run_once(&mut s.cp, &mut s.dp, &s.chain)
        .await
        .map_err(|e| format!("{e}"))?;
    check!(
        matches!(outcome, AttemptOutcome::Submitted { task_id: t, .. } if t == task_id),
        "PUBLIC continues during the outage: {outcome:?}"
    );
    Ok(())
}

fn k24(s: &mut Scenario) -> CaseOutcome {
    let session_a = s.session_for(&s.executor.clone());
    let session_b = s.session_for(&s.other.clone());
    let c = s.challenge(&session_a).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    let r = s.present(&session_b, &e);
    check!(
        matches!(
            r,
            Err(ReleaseError::SessionMismatch | ReleaseError::ExecutorMismatch)
        ),
        "A's evidence under B: {r:?}"
    );
    // B relabels the envelope as its own; the body still names A.
    let mut relabelled = e.clone();
    relabelled.session_id = session_b.session_id;
    relabelled.executor = session_b.executor;
    let r = s.present(&session_b, &relabelled);
    check!(r.is_err(), "relabelled for B must be refused: {r:?}");
    let r = s.authorize(&session_b, c.challenge_id);
    check!(r.is_err(), "no release to B: {r:?}");
    Ok(())
}

fn k25(s: &mut Scenario) -> CaseOutcome {
    let s1 = s.session_for(&s.executor.clone());
    let s2 = s.session_for(&s.executor.clone());
    let (auth, _) = s.release(&s1).map_err(|e| format!("{e}"))?;
    let r = s.authority.redeem(&s2, auth.release_id, s.task_id);
    check!(
        matches!(r, Err(ReleaseError::SessionMismatch)),
        "release for session 1 redeemed in session 2: {r:?}"
    );
    let r = s.authorize(&s2, auth.challenge_id);
    check!(
        r.is_err(),
        "session 2 cannot re-use session 1's challenge: {r:?}"
    );
    check!(
        s.authority.release_state(auth.release_id) == Some(ReleaseState::Released),
        "still redeemable by session 1"
    );
    Ok(())
}

fn k26(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let (auth, _) = s.release(&session).map_err(|e| format!("{e}"))?;
    s.clock.advance(RELEASE_TTL + 1);
    let r = s.authority.redeem(&session, auth.release_id, s.task_id);
    check!(
        matches!(r, Err(ReleaseError::ReleaseExpired)),
        "expired release: {r:?}"
    );
    check!(
        s.authority.release_state(auth.release_id) == Some(ReleaseState::Expired),
        "state Expired"
    );
    Ok(())
}

fn k27(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let (auth, _) = s.release(&session).map_err(|e| format!("{e}"))?;
    s.authority.revoke_release(auth.release_id);
    let r = s.authority.redeem(&session, auth.release_id, s.task_id);
    check!(
        matches!(r, Err(ReleaseError::ReleaseRevoked)),
        "revoked release: {r:?}"
    );
    Ok(())
}

async fn k28(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let (plain, _, _) = s.full_flow(&session, &lease)?;
    // A wrong output from an attested environment: the receipt anchors anyway.
    let mut wrong = plain.clone();
    wrong.reverse();
    wrong[0] ^= 0xFF;
    let receipt = s.persist_and_anchor(&session, &lease, wrong.clone()).await?;
    check!(
        receipt.output_commitment == reference_output_commitment(&wrong),
        "receipt commits to the wrong output"
    );
    let bytes = s.chain_bytes().await;
    check!(
        Scenario::contains(&bytes, &receipt.output_commitment),
        "anchored"
    );
    let rendered = format!(
        "{:?}",
        VerifiedAttestation {
            verifier: String::new(),
            format: String::new(),
            security_label: SecurityLabel::ReferenceAttested,
            trust_anchor: String::new(),
            session_id: session.session_id,
            executor: s.task.executor,
            task_id: s.task_id,
            challenge_id: super::ChallengeId([0; 32]),
            purpose: Purpose::ConfidentialInputRelease,
            issued_at: 0,
            expires_at: 0,
            environment_type: String::new(),
            measurement: [0; 32],
            security_version: 0,
            debug_disabled: true,
            environment_key: [0; 32],
            evidence_reference: [0; 32],
        }
    );
    check!(
        !rendered.contains("output") && !rendered.contains("correct"),
        "no correctness field exists"
    );
    Ok(())
}

fn k29(s: &mut Scenario) -> CaseOutcome {
    let v = ReferenceAttestationVerifier;
    check!(
        v.security_label() == SecurityLabel::ReferenceAttested,
        "label"
    );
    check!(!v.production_grade(), "not production grade");
    check!(
        REFERENCE_LABEL.contains("TEST ONLY")
            && REFERENCE_LABEL.contains("not hardware attestation"),
        "label text"
    );
    let session = s.session_for(&s.executor.clone());
    let c = s.challenge(&session).map_err(|e| format!("{e}"))?;
    let e = s.evidence(&c);
    let verified = s.present(&session, &e).map_err(|e| format!("{e}"))?;
    check!(
        verified.security_label == SecurityLabel::ReferenceAttested,
        "every result carries the label"
    );
    check!(
        verified.security_label.to_string() == "REFERENCE_ATTESTED",
        "label string"
    );
    Ok(())
}

async fn k30(s: &mut Scenario) -> CaseOutcome {
    let session = s.session_for(&s.executor.clone());
    let lease = s.lease_for(&session).ok_or("no lease")?;
    let (plain, grant, _) = s.full_flow(&session, &lease)?;
    let mut output = plain.clone();
    output.reverse();
    let receipt = s.persist_and_anchor(&session, &lease, output).await?;
    let receipt_bytes = receipt.encode();
    check!(
        receipt.metadata.is_empty(),
        "no attestation metadata in the receipt"
    );
    check!(
        !Scenario::contains(&receipt_bytes, &grant.release_id.0),
        "no release id in the receipt"
    );
    check!(
        !Scenario::contains(&receipt_bytes, &grant.environment_key),
        "no environment key in the receipt"
    );
    let task_bytes = s.task.encode();
    check!(
        task_bytes.len() == 1 + 32 + 32 + 32 + 32 + 1 + REVERSE_BYTES_SPEC.len(),
        "six-field envelope, unchanged"
    );
    check!(s.task.task_id() == s.task_id, "task identity unchanged");
    let bytes = s.chain_bytes().await;
    check!(
        !Scenario::contains(&bytes, &grant.release_id.0),
        "no release id on-chain"
    );
    Ok(())
}

// ── runner ───────────────────────────────────────────────────────────────

/// Runs every case, each on a fresh scenario with a fresh verifier from
/// `make_verifier`.
#[allow(clippy::too_many_lines)]
pub async fn run_all<F>(make_verifier: F) -> Report
where
    F: Fn() -> Box<dyn AttestationVerifier>,
{
    let probe = make_verifier();
    let verifier = probe.kind().to_owned();
    let label = probe.security_label().to_string();
    let production_grade = probe.production_grade();
    drop(probe);
    let mut cases = Vec::new();
    for case in catalog() {
        let mut s = Scenario::new(make_verifier()).await;
        let outcome = match case.id {
            "K01" => k01(&mut s),
            "K02" => k02(&mut s),
            "K03" => k03(&mut s),
            "K04" => k04(&mut s),
            "K05" => k05(&mut s),
            "K06" => k06(&mut s),
            "K07" => k07(&mut s),
            "K08" => k08(&mut s).await,
            "K09" => k09(&mut s),
            "K10" => k10(&mut s),
            "K11" => k11(&mut s),
            "K12" => k12(&mut s),
            "K13" => k13(&mut s),
            "K14" => k14(&mut s),
            "K15" => k15(&mut s),
            "K16" => k16(&mut s),
            "K17" => k17(&mut s),
            "K18" => k18(&mut s),
            "K19" => k19(&mut s).await,
            "K20" => k20(&mut s),
            "K21" => k21(&mut s).await,
            "K22" => k22(&mut s),
            "K23" => k23(&mut s).await,
            "K24" => k24(&mut s),
            "K25" => k25(&mut s),
            "K26" => k26(&mut s),
            "K27" => k27(&mut s),
            "K28" => k28(&mut s).await,
            "K29" => k29(&mut s),
            "K30" => k30(&mut s).await,
            other => Err(format!("no runner for {other}")),
        };
        take_captured();
        let status = match outcome {
            Ok(()) => Status::Pass,
            Err(m) => Status::Fail(m),
        };
        cases.push(CaseResult { case, status });
    }
    Report {
        version: CONFIDENTIAL_AUTH_VERSION,
        verifier,
        label,
        production_grade,
        cases,
    }
}

/// Deliberately unsafe verifiers. They exist so that the suite can prove
/// it is not vacuous: a run against either of them must fail. Never a
/// production option; not registered anywhere but the test group.
pub mod negative {
    use super::super::reference::{
        verify_reference, Laxity, REFERENCE_FORMAT, REFERENCE_VERIFIER_KIND,
    };
    use super::super::{
        AttestationChallenge, AttestationVerifier, EvidenceEnvelope, SecurityLabel, TrustPolicy,
        VerifiedAttestation, VerifyError,
    };

    /// Skips the signature check: accepts a forgery under a trusted anchor id.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct SignatureBlindVerifier;

    impl AttestationVerifier for SignatureBlindVerifier {
        fn kind(&self) -> &str {
            REFERENCE_VERIFIER_KIND
        }
        fn formats(&self) -> Vec<String> {
            vec![REFERENCE_FORMAT.to_owned()]
        }
        fn security_label(&self) -> SecurityLabel {
            SecurityLabel::ReferenceAttested
        }
        fn production_grade(&self) -> bool {
            false
        }
        fn verify(
            &self,
            envelope: &EvidenceEnvelope,
            challenge: &AttestationChallenge,
            trust: &TrustPolicy,
            now: u64,
        ) -> Result<VerifiedAttestation, VerifyError> {
            verify_reference(
                envelope,
                challenge,
                trust,
                now,
                Laxity {
                    skip_signature: true,
                    bind_from_challenge: false,
                },
            )
        }
    }

    /// Reports bindings from the challenge instead of the evidence: evidence
    /// for one task or session looks bound to whatever challenge it is
    /// presented against.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct BindingBlindVerifier;

    impl AttestationVerifier for BindingBlindVerifier {
        fn kind(&self) -> &str {
            REFERENCE_VERIFIER_KIND
        }
        fn formats(&self) -> Vec<String> {
            vec![REFERENCE_FORMAT.to_owned()]
        }
        fn security_label(&self) -> SecurityLabel {
            SecurityLabel::ReferenceAttested
        }
        fn production_grade(&self) -> bool {
            false
        }
        fn verify(
            &self,
            envelope: &EvidenceEnvelope,
            challenge: &AttestationChallenge,
            trust: &TrustPolicy,
            now: u64,
        ) -> Result<VerifiedAttestation, VerifyError> {
            verify_reference(
                envelope,
                challenge,
                trust,
                now,
                Laxity {
                    skip_signature: false,
                    bind_from_challenge: true,
                },
            )
        }
    }
}
