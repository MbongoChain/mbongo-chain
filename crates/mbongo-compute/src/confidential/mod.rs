//! Confidential authorization (`confidential-auth-v1`): attestation
//! challenges, evidence, verification, conversion into J evidence, and
//! conditional release of a confidential input to an attested environment.
//!
//! This is the implementation of the one extension point the architecture
//! reserved (privacy §10–§11, E §14, F §9): **key release becomes
//! conditional.** A worker that merely advertises
//! `privacy.confidential_execution = true` receives nothing. It receives the
//! fetch capability for a confidential input, and the content key wrapped
//! to the key its attestation bound, only after
//!
//! ```text
//! valid session + fresh purpose-bound challenge + acceptable evidence
//!   + verified session / executor / task binding + trust and measurement
//!   policy + J policy eligible            ->  one single-use release
//! ```
//!
//! and any failure anywhere means no key, no plaintext, no capability, no
//! execution start. Nothing here is consensus: no validator parses a
//! challenge, an envelope, a trust policy or a release (E22, K2); the
//! `ComputeTask`, the `Receipt`, the RPC surface and the SDK wire are
//! untouched (K31–K33).
//!
//! # Components and what each may decide
//!
//! | Component | Decides | Never decides |
//! |---|---|---|
//! | [`ReleaseAuthority`] (control plane) | issues challenges, invokes a verifier, runs the J evaluator, mints and consumes release authorizations | who the executor is; whether a receipt is valid |
//! | [`AttestationVerifier`] | whether evidence is genuine and what it says | whether that is acceptable — that is trust policy plus J |
//! | [`KeyReleaseAuthority`] (the client's delegated key service, F §11, §14) | releases the content key, wrapped, against a redeemed release | anything about tasks, leases or receipts |
//! | the execution environment (plane 4) | produces evidence; decrypts inside itself | its own acceptability |
//! | the chain | nothing here | everything here |
//!
//! # What the reference path is, and is not
//!
//! [`reference`] provides a **REFERENCE / TEST ONLY** attestation format,
//! root and verifier. It is not hardware attestation and not production
//! confidential compute (K29): the "environment" is a process holding
//! software keys, and the property it cannot prove is that the provider
//! cannot read plaintext. What it does prove is the mechanism — the data
//! plane stores ciphertext, the key is released only to the key a verified
//! attestation bound, only after J says eligible, once per challenge,
//! within a window — and that every replay, binding, freshness and trust
//! failure fails closed. See `docs/architecture/compute-confidential-attestation-key-release.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use mbongo_core::Address;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::clock::Clock;
use crate::control_plane::Session;
use crate::data_plane::LocalKey;
use crate::execution::Plaintext;
use crate::identity::{IdSource, ObjectId, SessionId};
use crate::policy::{
    self, dimension, Advertisement, Binding, Claim, Decision, DerivedFacts, EffectivePolicy,
    Evidence, EvidenceLevel, Scope, SessionFacts, TaskFacts, Value,
};

pub mod reference;
pub mod suite;

/// The contract version of everything in this module (architecture /
/// control-plane version; non-consensus).
pub const CONFIDENTIAL_AUTH_VERSION: &str = "confidential-auth-v1";
/// The evidence envelope schema version (non-consensus).
pub const ATTESTATION_ENVELOPE_VERSION: &str = "attestation-envelope-v1";

/// Domain tag for sealing a confidential input under its content key.
const DOMAIN_SEAL: &[u8] = b"mbongo:ref-confidential-input:v1";
/// Domain tag for wrapping a content key to an environment key.
const DOMAIN_WRAP: &[u8] = b"mbongo:ref-key-wrap:v1";
/// BLAKE3 `derive_key` context for the wrap key.
const KDF_CONTEXT: &str = "mbongo:ref-key-wrap-kdf:v1";

macro_rules! opaque_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub [u8; 32]);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", hex::encode(&self.0[..8]))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), hex::encode(self.0))
            }
        }
    };
}

opaque_id!(
    /// One attestation challenge. Off-chain; an identifier, not a credential.
    ChallengeId
);
opaque_id!(
    /// One release authorization. Off-chain; redeemable only inside the
    /// session it names, once.
    ReleaseId
);

// ── challenge ─────────────────────────────────────────────────────────────

/// What a challenge authorises evidence *for* (§5 of the architecture).
/// Domain-separated: evidence for one purpose never serves another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Release of a confidential input (fetch capability + content key).
    ConfidentialInputRelease,
    /// Reserved: release of an encrypted result to a client. Not used by
    /// the reference path; named so that the domain exists.
    ResultRelease,
}

impl Purpose {
    /// Stable tag byte.
    pub fn tag(self) -> u8 {
        match self {
            Self::ConfidentialInputRelease => 1,
            Self::ResultRelease => 2,
        }
    }

    /// From a tag byte.
    pub fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::ConfidentialInputRelease),
            2 => Some(Self::ResultRelease),
            _ => None,
        }
    }
}

/// A fresh, unpredictable, single-use, short-lived challenge bound to one
/// session, one executor, one task and one purpose. Ephemeral off-chain
/// state; never on the chain.
#[derive(Clone, PartialEq, Eq)]
pub struct AttestationChallenge {
    /// The challenge.
    pub challenge_id: ChallengeId,
    /// Unpredictable bytes the evidence must echo.
    pub nonce: [u8; 32],
    /// The session it was issued to.
    pub session_id: SessionId,
    /// The executor that session proved — equal to `task.executor`.
    pub executor: Address,
    /// The task the release would serve.
    pub task_id: [u8; 32],
    /// What the evidence would authorise.
    pub purpose: Purpose,
    /// Issued at.
    pub issued_at: u64,
    /// Not valid after.
    pub expires_at: u64,
}

impl fmt::Debug for AttestationChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AttestationChallenge({}, session {}, executor {}, task {}, {:?}, until {}, nonce redacted)",
            self.challenge_id,
            self.session_id,
            self.executor,
            hex::encode(&self.task_id[..8]),
            self.purpose,
            self.expires_at
        )
    }
}

/// Lifecycle of a challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChallengeState {
    /// Issued; no evidence yet.
    Issued,
    /// Evidence verified against it; not yet used for a release.
    Presented,
    /// Used for exactly one release authorization.
    Consumed,
    /// Past its window before use.
    Expired,
    /// Withdrawn.
    Revoked,
    /// Evidence presented and the policy refused; the challenge is spent.
    Denied,
}

// ── evidence ──────────────────────────────────────────────────────────────

/// An implementation-neutral evidence envelope (`attestation-envelope-v1`).
/// The vendor payload stays opaque; the normalized result is
/// [`VerifiedAttestation`]. Logs carry [`EvidenceEnvelope::reference`], never
/// the payload.
#[derive(Clone, PartialEq, Eq)]
pub struct EvidenceEnvelope {
    /// Evidence format identifier (deployment vocabulary; the reference is
    /// [`reference::REFERENCE_FORMAT`]).
    pub format: String,
    /// Which verifier kind should evaluate it.
    pub verifier_kind: String,
    /// The challenge it answers.
    pub challenge_id: ChallengeId,
    /// The session presenting it.
    pub session_id: SessionId,
    /// The executor that session proved.
    pub executor: Address,
    /// Evidence validity window, as the environment states it.
    pub issued_at: u64,
    /// See `issued_at`.
    pub expires_at: u64,
    /// The opaque vendor payload. Potentially sensitive; never logged.
    pub payload: Vec<u8>,
}

impl EvidenceEnvelope {
    /// BLAKE3 of the payload: the reference logs and decisions carry.
    pub fn reference(&self) -> [u8; 32] {
        *blake3::hash(&self.payload).as_bytes()
    }
}

impl fmt::Debug for EvidenceEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EvidenceEnvelope({}, {}, challenge {}, session {}, executor {}, ref {}, payload redacted)",
            self.format,
            self.verifier_kind,
            self.challenge_id,
            self.session_id,
            self.executor,
            hex::encode(&self.reference()[..8])
        )
    }
}

// ── trust policy ──────────────────────────────────────────────────────────

/// A verifier root the deployment trusts. Deployment policy, non-consensus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustAnchor {
    /// Deployment-local name.
    pub anchor_id: String,
    /// The root's Ed25519 verifying key (reference format) or an opaque
    /// identifier a vendor adapter interprets.
    pub public_key: [u8; 32],
    /// Revoked roots fail closed.
    pub revoked: bool,
}

/// Which environments are acceptable. Approving a measurement says the
/// environment is one the policy expects; it says nothing about whether
/// the output is correct (K30).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MeasurementPolicy {
    /// Allowed measurements (runtime / image / policy-version digests).
    pub allowed: BTreeSet<[u8; 32]>,
    /// Revoked measurements fail closed even if also listed as allowed.
    pub revoked: BTreeSet<[u8; 32]>,
    /// Minimum firmware / security version.
    pub min_security_version: u64,
    /// Whether an environment with debug enabled may be accepted.
    pub allow_debug: bool,
}

/// Deployment trust policy (§8, §9 of the architecture). Non-consensus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustPolicy {
    /// Evidence formats the deployment accepts at all.
    pub accepted_formats: BTreeSet<String>,
    /// Trusted roots by id.
    pub anchors: BTreeMap<String, TrustAnchor>,
    /// Measurement policy.
    pub measurement: MeasurementPolicy,
    /// Evidence older than this at presentation is refused.
    pub max_evidence_age_secs: u64,
    /// Challenge lifetime.
    pub challenge_ttl_secs: u64,
    /// Release authorization lifetime (also capped by the evidence window).
    pub release_ttl_secs: u64,
}

impl TrustPolicy {
    fn live_anchor(&self, id: &str) -> Result<&TrustAnchor, VerifyError> {
        let a = self.anchors.get(id).ok_or(VerifyError::UntrustedRoot)?;
        if a.revoked {
            return Err(VerifyError::RevokedRoot);
        }
        Ok(a)
    }

    fn check_measurement(&self, v: &VerifiedAttestation) -> Result<(), VerifyError> {
        if self.measurement.revoked.contains(&v.measurement) {
            return Err(VerifyError::MeasurementRevoked);
        }
        if !self.measurement.allowed.contains(&v.measurement) {
            return Err(VerifyError::MeasurementNotAllowed);
        }
        if v.security_version < self.measurement.min_security_version {
            return Err(VerifyError::SecurityVersionTooLow);
        }
        if !v.debug_disabled && !self.measurement.allow_debug {
            return Err(VerifyError::DebugEnabled);
        }
        Ok(())
    }
}

// ── verified attestation ──────────────────────────────────────────────────

/// How strong the verifier's own assurance is. The reference verifier is
/// always [`SecurityLabel::ReferenceAttested`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLabel {
    /// Test-only: software keys, no hardware root of trust.
    ReferenceAttested,
    /// A hardware-backed verifier (none exists in this repository).
    HardwareAttested,
}

impl fmt::Display for SecurityLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReferenceAttested => "REFERENCE_ATTESTED",
            Self::HardwareAttested => "HARDWARE_ATTESTED",
        })
    }
}

/// The normalized result of a successful verification (§10). Carries no
/// raw evidence and nothing about output correctness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAttestation {
    /// Verifier kind.
    pub verifier: String,
    /// Evidence format.
    pub format: String,
    /// Assurance label.
    pub security_label: SecurityLabel,
    /// The trust anchor the evidence chained to.
    pub trust_anchor: String,
    /// Bindings, as the evidence states them.
    pub session_id: SessionId,
    /// See `session_id`.
    pub executor: Address,
    /// See `session_id`.
    pub task_id: [u8; 32],
    /// See `session_id`.
    pub challenge_id: ChallengeId,
    /// See `session_id`.
    pub purpose: Purpose,
    /// Validity window.
    pub issued_at: u64,
    /// See `issued_at`.
    pub expires_at: u64,
    /// Abstract environment type (no vendor name in the core grammar).
    pub environment_type: String,
    /// The measurement the policy matched.
    pub measurement: [u8; 32],
    /// Firmware / security version.
    pub security_version: u64,
    /// Debug state.
    pub debug_disabled: bool,
    /// The X25519 public key the environment proved it holds; the content
    /// key is wrapped to it (F §9: "a key the attested environment proves
    /// it holds").
    pub environment_key: [u8; 32],
    /// BLAKE3 of the envelope payload.
    pub evidence_reference: [u8; 32],
}

/// Why verification failed. Classes only; never evidence content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(missing_docs)]
pub enum VerifyError {
    #[error("unknown evidence format")]
    UnknownFormat,
    #[error("no verifier available for this evidence")]
    VerifierUnavailable,
    #[error("malformed evidence")]
    Malformed,
    #[error("untrusted verifier root")]
    UntrustedRoot,
    #[error("revoked verifier root")]
    RevokedRoot,
    #[error("evidence signature invalid")]
    BadSignature,
    #[error("evidence does not answer this challenge")]
    ChallengeMismatch,
    #[error("evidence bound to another session")]
    SessionMismatch,
    #[error("evidence bound to another executor")]
    ExecutorMismatch,
    #[error("evidence bound to another task")]
    TaskMismatch,
    #[error("evidence issued for another purpose")]
    PurposeMismatch,
    #[error("evidence expired")]
    EvidenceExpired,
    #[error("evidence not yet valid")]
    EvidenceNotYetValid,
    #[error("evidence older than the policy allows")]
    EvidenceTooOld,
    #[error("measurement not allowed by policy")]
    MeasurementNotAllowed,
    #[error("measurement revoked")]
    MeasurementRevoked,
    #[error("security version below the policy minimum")]
    SecurityVersionTooLow,
    #[error("environment has debug enabled")]
    DebugEnabled,
}

/// The vendor-neutral verifier boundary (§7). A future hardware adapter
/// implements this; nothing in J or the protocol changes (K40).
pub trait AttestationVerifier: Send + Sync {
    /// Verifier kind identifier (matches `EvidenceEnvelope::verifier_kind`).
    fn kind(&self) -> &str;
    /// Evidence formats this verifier understands.
    fn formats(&self) -> Vec<String>;
    /// Assurance label of everything this verifier produces.
    fn security_label(&self) -> SecurityLabel;
    /// Whether this verifier may be called production confidential
    /// attestation. The reference verifier answers `false`.
    fn production_grade(&self) -> bool;
    /// Verifies `envelope` against the challenge it answers and the trust
    /// policy, at `now`. Returns the normalized result or a class of
    /// failure. Must not log the payload.
    fn verify(
        &self,
        envelope: &EvidenceEnvelope,
        challenge: &AttestationChallenge,
        trust: &TrustPolicy,
        now: u64,
    ) -> Result<VerifiedAttestation, VerifyError>;
}

// ── conversion into J evidence ────────────────────────────────────────────

/// The J evidence record a verified attestation becomes (§11): level
/// `ATTESTED`, bound to the challenge and session, valid for the evidence
/// window, carrying only an opaque reference.
pub fn to_policy_evidence(v: &VerifiedAttestation) -> Evidence {
    Evidence {
        level: EvidenceLevel::Attested,
        issued_at: v.issued_at,
        expires_at: Some(v.expires_at),
        binding: Binding::Challenge {
            session: v.session_id,
            challenge_ref: hex::encode(v.challenge_id.0),
        },
        verifier: v.verifier.clone(),
        reference: hex::encode(v.evidence_reference),
        revoked: false,
    }
}

// ── release authorization ─────────────────────────────────────────────────

/// Lifecycle of a release authorization (§13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseState {
    /// Minted; redeemable once inside its session before `expires_at`.
    Released,
    /// Redeemed.
    Consumed,
    /// Past its window before redemption.
    Expired,
    /// Withdrawn before redemption.
    Revoked,
}

/// A minted release authorization. Not a bearer token: it is redeemed only
/// through the authority, inside the authenticated session it names, once.
/// Carries references and identifiers only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAuthorization {
    /// The release.
    pub release_id: ReleaseId,
    /// Bindings.
    pub task_id: [u8; 32],
    /// See `task_id`.
    pub executor: Address,
    /// See `task_id`.
    pub session_id: SessionId,
    /// The challenge it consumed.
    pub challenge_id: ChallengeId,
    /// The anchor the evidence chained to.
    pub trust_anchor: String,
    /// BLAKE3 of the evidence payload.
    pub evidence_reference: [u8; 32],
    /// The key the content key will be wrapped to.
    pub environment_key: [u8; 32],
    /// What the J decision rested on (always `Attested` for a release).
    pub lowest_evidence: EvidenceLevel,
    /// Window.
    pub issued_at: u64,
    /// See `issued_at`.
    pub expires_at: u64,
}

/// What a redeemed release yields: the facts the control plane needs to
/// issue the confidential fetch capability and the key service needs to
/// wrap the key. Produced once per release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseGrant {
    /// The release redeemed.
    pub release_id: ReleaseId,
    /// Bindings.
    pub task_id: [u8; 32],
    /// See `task_id`.
    pub executor: Address,
    /// See `task_id`.
    pub session_id: SessionId,
    /// Wrap target.
    pub environment_key: [u8; 32],
    /// The release's expiry, inherited by the wrapped key's usefulness.
    pub expires_at: u64,
}

/// Why the authority refused. Classes and identifiers only.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(missing_docs)]
pub enum ReleaseError {
    #[error("session executor is not the task executor")]
    ExecutorMismatch,
    #[error("not this session's challenge or release")]
    SessionMismatch,
    #[error("release does not serve this task")]
    TaskMismatch,
    #[error("session revoked")]
    SessionRevoked,
    #[error("session expired")]
    SessionExpired,
    #[error("unknown challenge")]
    UnknownChallenge,
    #[error("challenge expired")]
    ChallengeExpired,
    #[error("challenge already consumed")]
    ChallengeConsumed,
    #[error("challenge denied")]
    ChallengeDenied,
    #[error("challenge revoked")]
    ChallengeRevoked,
    #[error("no verified evidence for this challenge")]
    ChallengeNotPresented,
    #[error("no trust policy configured")]
    TrustPolicyMissing,
    #[error("verification: {0}")]
    Verify(#[from] VerifyError),
    #[error("policy ineligible: {codes:?}")]
    PolicyIneligible {
        /// Reason codes from the J decision.
        codes: Vec<String>,
    },
    #[error("unknown release")]
    UnknownRelease,
    #[error("release expired")]
    ReleaseExpired,
    #[error("release already consumed")]
    ReleaseConsumed,
    #[error("release revoked")]
    ReleaseRevoked,
    #[error("no content key registered for this object")]
    NoContentKey,
}

struct ChallengeRecord {
    challenge: AttestationChallenge,
    state: ChallengeState,
    verified: Option<VerifiedAttestation>,
}

struct ReleaseRecord {
    auth: ReleaseAuthorization,
    state: ReleaseState,
}

/// Everything the authority needs to run the J evaluator for a release
/// (§12). The confidential claim's evidence is **not** taken from
/// `session_facts`: the authority replaces that dimension with its own
/// verification, so a caller cannot inject ATTESTED evidence.
pub struct ReleaseRequest<'a> {
    /// The authenticated control-plane session.
    pub session: &'a Session,
    /// The challenge whose verified evidence backs the request.
    pub challenge_id: ChallengeId,
    /// Task facts for J.
    pub task: &'a TaskFacts,
    /// Session facts for J (compatibility claims); see above.
    pub session_facts: &'a SessionFacts,
    /// The provider advertisement.
    pub advertisement: &'a Advertisement,
    /// Control-plane-derived facts.
    pub derived: &'a DerivedFacts,
    /// The effective policy. Consumed as is; never reinterpreted (K18).
    pub policy: &'a EffectivePolicy,
}

/// The control plane's confidential authorization service (§13). Holds
/// the trust policy, the registered verifiers, ephemeral challenges and
/// release authorizations, and the session revocation list. In memory:
/// consumed-challenge and release state does not survive a restart, so
/// replay protection is scoped to one process lifetime (§23 of the
/// architecture states this rather than claiming durability).
pub struct ReleaseAuthority {
    clock: Arc<dyn Clock>,
    ids: IdSource,
    trust: Option<TrustPolicy>,
    verifiers: BTreeMap<String, Box<dyn AttestationVerifier>>,
    challenges: BTreeMap<ChallengeId, ChallengeRecord>,
    releases: BTreeMap<ReleaseId, ReleaseRecord>,
    revoked_sessions: BTreeSet<SessionId>,
}

impl fmt::Debug for ReleaseAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReleaseAuthority(verifiers {:?}, challenges {}, releases {})",
            self.verifiers.keys().collect::<Vec<_>>(),
            self.challenges.len(),
            self.releases.len()
        )
    }
}

impl ReleaseAuthority {
    /// A fresh authority under `trust`.
    pub fn new(clock: Arc<dyn Clock>, ids: IdSource, trust: TrustPolicy) -> Self {
        Self {
            clock,
            ids,
            trust: Some(trust),
            verifiers: BTreeMap::new(),
            challenges: BTreeMap::new(),
            releases: BTreeMap::new(),
            revoked_sessions: BTreeSet::new(),
        }
    }

    fn now(&self) -> u64 {
        self.clock.now()
    }

    /// Registers a verifier by its kind.
    pub fn register_verifier(&mut self, verifier: Box<dyn AttestationVerifier>) {
        self.verifiers.insert(verifier.kind().to_owned(), verifier);
    }

    /// Removes a verifier (an outage). Confidential requests for its
    /// evidence then fail closed with `VerifierUnavailable` (K17).
    pub fn unregister_verifier(&mut self, kind: &str) {
        self.verifiers.remove(kind);
    }

    /// Replaces the trust policy (rotation, §18). Future verifications use
    /// the new policy. Unredeemed releases whose anchor is now missing or
    /// revoked are revoked; redeemed releases cannot be undone.
    pub fn set_trust_policy(&mut self, trust: TrustPolicy) {
        for rec in self.releases.values_mut() {
            if rec.state == ReleaseState::Released
                && trust.live_anchor(&rec.auth.trust_anchor).is_err()
            {
                rec.state = ReleaseState::Revoked;
            }
        }
        self.trust = Some(trust);
    }

    /// Removes the trust policy entirely: every confidential request fails
    /// closed with `TrustPolicyMissing`.
    pub fn clear_trust_policy(&mut self) {
        self.trust = None;
    }

    /// The current trust policy.
    pub fn trust_policy(&self) -> Option<&TrustPolicy> {
        self.trust.as_ref()
    }

    /// Revokes a root: future evidence under it fails; unredeemed releases
    /// resting on it are revoked.
    pub fn revoke_anchor(&mut self, anchor_id: &str) {
        if let Some(t) = self.trust.as_mut() {
            if let Some(a) = t.anchors.get_mut(anchor_id) {
                a.revoked = true;
            }
        }
        for rec in self.releases.values_mut() {
            if rec.state == ReleaseState::Released && rec.auth.trust_anchor == anchor_id {
                rec.state = ReleaseState::Revoked;
            }
        }
    }

    /// Revokes a measurement: future evidence carrying it fails.
    pub fn revoke_measurement(&mut self, measurement: [u8; 32]) {
        if let Some(t) = self.trust.as_mut() {
            t.measurement.revoked.insert(measurement);
        }
    }

    /// Revokes a session: every challenge and unredeemed release of that
    /// session is revoked, and nothing new is issued to it.
    pub fn revoke_session(&mut self, session_id: SessionId) {
        self.revoked_sessions.insert(session_id);
        for rec in self.challenges.values_mut() {
            if rec.challenge.session_id == session_id
                && matches!(
                    rec.state,
                    ChallengeState::Issued | ChallengeState::Presented
                )
            {
                rec.state = ChallengeState::Revoked;
            }
        }
        for rec in self.releases.values_mut() {
            if rec.state == ReleaseState::Released && rec.auth.session_id == session_id {
                rec.state = ReleaseState::Revoked;
            }
        }
    }

    /// Revokes one unredeemed release.
    pub fn revoke_release(&mut self, release_id: ReleaseId) {
        if let Some(rec) = self.releases.get_mut(&release_id) {
            if rec.state == ReleaseState::Released {
                rec.state = ReleaseState::Revoked;
            }
        }
    }

    /// Current state of a challenge (after expiry sweep).
    pub fn challenge_state(&mut self, id: ChallengeId) -> Option<ChallengeState> {
        let now = self.now();
        let rec = self.challenges.get_mut(&id)?;
        if matches!(
            rec.state,
            ChallengeState::Issued | ChallengeState::Presented
        ) && now > rec.challenge.expires_at
        {
            rec.state = ChallengeState::Expired;
        }
        Some(rec.state)
    }

    /// Current state of a release (after expiry sweep).
    pub fn release_state(&mut self, id: ReleaseId) -> Option<ReleaseState> {
        let now = self.now();
        let rec = self.releases.get_mut(&id)?;
        if rec.state == ReleaseState::Released && now > rec.auth.expires_at {
            rec.state = ReleaseState::Expired;
        }
        Some(rec.state)
    }

    fn live_session(&self, session: &Session) -> Result<(), ReleaseError> {
        if self.revoked_sessions.contains(&session.session_id) {
            return Err(ReleaseError::SessionRevoked);
        }
        if self.now() > session.expires_at {
            return Err(ReleaseError::SessionExpired);
        }
        Ok(())
    }

    /// Issues a challenge to `session` for `task_id` (§5). The executor
    /// gate is first: the session must have proved `task_executor` (K3,
    /// K8), and no policy or evidence reaches past it.
    pub fn issue_challenge(
        &mut self,
        session: &Session,
        task_id: [u8; 32],
        task_executor: Address,
        purpose: Purpose,
    ) -> Result<AttestationChallenge, ReleaseError> {
        self.live_session(session)?;
        if session.executor != task_executor {
            log::warn!(
                "confidential: challenge refused — session {} proved {} but the task names {task_executor}",
                session.session_id,
                session.executor
            );
            return Err(ReleaseError::ExecutorMismatch);
        }
        let trust = self.trust.as_ref().ok_or(ReleaseError::TrustPolicyMissing)?;
        let now = self.now();
        let challenge = AttestationChallenge {
            challenge_id: ChallengeId(self.ids.next("attestation-challenge")),
            nonce: self.ids.next("attestation-nonce"),
            session_id: session.session_id,
            executor: session.executor,
            task_id,
            purpose,
            issued_at: now,
            expires_at: now + trust.challenge_ttl_secs,
        };
        self.challenges.insert(
            challenge.challenge_id,
            ChallengeRecord {
                challenge: challenge.clone(),
                state: ChallengeState::Issued,
                verified: None,
            },
        );
        log::info!("confidential: issued {challenge:?}");
        Ok(challenge)
    }

    /// Verifies `envelope` against the challenge it names, presented by
    /// `session` (§7, §16, §17). Fails closed on a missing verifier, an
    /// unaccepted format, a missing trust policy, and every binding,
    /// freshness, root and measurement failure; the authority re-checks
    /// the normalized result against its own challenge record and policy
    /// so that a permissive verifier cannot widen what is accepted.
    pub fn present_evidence(
        &mut self,
        session: &Session,
        envelope: &EvidenceEnvelope,
    ) -> Result<VerifiedAttestation, ReleaseError> {
        self.live_session(session)?;
        let now = self.now();
        let trust = self.trust.as_ref().ok_or(ReleaseError::TrustPolicyMissing)?;
        let rec = self
            .challenges
            .get(&envelope.challenge_id)
            .ok_or(ReleaseError::UnknownChallenge)?;
        match rec.state {
            ChallengeState::Issued => {}
            ChallengeState::Presented | ChallengeState::Consumed => {
                return Err(ReleaseError::ChallengeConsumed)
            }
            ChallengeState::Expired => return Err(ReleaseError::ChallengeExpired),
            ChallengeState::Revoked => return Err(ReleaseError::ChallengeRevoked),
            ChallengeState::Denied => return Err(ReleaseError::ChallengeDenied),
        }
        let challenge = rec.challenge.clone();
        if now > challenge.expires_at {
            self.challenges.get_mut(&envelope.challenge_id).expect("present").state =
                ChallengeState::Expired;
            return Err(ReleaseError::ChallengeExpired);
        }
        if challenge.session_id != session.session_id || envelope.session_id != session.session_id {
            return Err(ReleaseError::SessionMismatch);
        }
        if challenge.executor != session.executor || envelope.executor != session.executor {
            return Err(ReleaseError::ExecutorMismatch);
        }
        if !trust.accepted_formats.contains(&envelope.format) {
            return Err(VerifyError::UnknownFormat.into());
        }
        let engine = self
            .verifiers
            .get(&envelope.verifier_kind)
            .ok_or(VerifyError::VerifierUnavailable)?;
        if !engine.formats().contains(&envelope.format) {
            return Err(VerifyError::UnknownFormat.into());
        }
        let verified = engine.verify(envelope, &challenge, trust, now)?;
        // Defence in depth: the normalized result must agree with the
        // authority's own record and policy, whatever the verifier did.
        if verified.challenge_id != challenge.challenge_id {
            return Err(VerifyError::ChallengeMismatch.into());
        }
        if verified.session_id != challenge.session_id {
            return Err(VerifyError::SessionMismatch.into());
        }
        if verified.executor != challenge.executor {
            return Err(VerifyError::ExecutorMismatch.into());
        }
        if verified.task_id != challenge.task_id {
            return Err(VerifyError::TaskMismatch.into());
        }
        if verified.purpose != challenge.purpose {
            return Err(VerifyError::PurposeMismatch.into());
        }
        if now < verified.issued_at {
            return Err(VerifyError::EvidenceNotYetValid.into());
        }
        if now > verified.expires_at {
            return Err(VerifyError::EvidenceExpired.into());
        }
        if now - verified.issued_at > trust.max_evidence_age_secs {
            return Err(VerifyError::EvidenceTooOld.into());
        }
        trust.live_anchor(&verified.trust_anchor)?;
        trust.check_measurement(&verified)?;
        if verified.evidence_reference != envelope.reference() {
            return Err(VerifyError::Malformed.into());
        }
        let rec = self.challenges.get_mut(&envelope.challenge_id).expect("present");
        rec.state = ChallengeState::Presented;
        rec.verified = Some(verified.clone());
        log::info!(
            "confidential: evidence ref {} verified for {} by {} ({})",
            hex::encode(&verified.evidence_reference[..8]),
            challenge.challenge_id,
            verified.verifier,
            verified.security_label
        );
        Ok(verified)
    }

    /// Converts the challenge's verified evidence into J evidence, runs the
    /// J evaluator, and — only if eligible with every ATTESTED-level
    /// requirement satisfied at ATTESTED — mints a single-use release and
    /// consumes the challenge (§12, §13). A refusal also spends the
    /// challenge: a new one is needed for another try.
    pub fn authorize_release(
        &mut self,
        req: &ReleaseRequest<'_>,
    ) -> Result<(ReleaseAuthorization, Decision), ReleaseError> {
        self.live_session(req.session)?;
        let trust = self.trust.as_ref().ok_or(ReleaseError::TrustPolicyMissing)?;
        let now = self.now();
        let rec = self.challenges.get(&req.challenge_id).ok_or(ReleaseError::UnknownChallenge)?;
        let verified = match rec.state {
            ChallengeState::Presented => {
                rec.verified.clone().ok_or(ReleaseError::ChallengeNotPresented)?
            }
            ChallengeState::Issued => return Err(ReleaseError::ChallengeNotPresented),
            ChallengeState::Consumed => return Err(ReleaseError::ChallengeConsumed),
            ChallengeState::Expired => return Err(ReleaseError::ChallengeExpired),
            ChallengeState::Revoked => return Err(ReleaseError::ChallengeRevoked),
            ChallengeState::Denied => return Err(ReleaseError::ChallengeDenied),
        };
        if now > rec.challenge.expires_at {
            self.challenges.get_mut(&req.challenge_id).expect("present").state =
                ChallengeState::Expired;
            return Err(ReleaseError::ChallengeExpired);
        }
        if verified.session_id != req.session.session_id {
            return Err(ReleaseError::SessionMismatch);
        }
        if verified.executor != req.session.executor || req.task.executor != req.session.executor {
            return Err(ReleaseError::ExecutorMismatch);
        }
        if verified.purpose != Purpose::ConfidentialInputRelease {
            return Err(VerifyError::PurposeMismatch.into());
        }
        // The claim the evidence backs, injected by the authority: the
        // caller's own confidential claim and any evidence on it are
        // discarded (K18: J is the policy decision; K4: claims are not
        // proofs; callers cannot manufacture ATTESTED evidence).
        let mut facts = req.session_facts.clone();
        facts.session_id = req.session.session_id;
        facts.proved_executor = req.session.executor;
        facts.claims.insert(
            dimension::CONFIDENTIAL_EXECUTION.to_owned(),
            Claim {
                value: Value::Bool(true),
                scope: Scope::Session,
                evidence: Some(to_policy_evidence(&verified)),
            },
        );
        let decision = policy::eligible(
            &policy::Input {
                task: req.task,
                session: &facts,
                advertisement: req.advertisement,
                derived: req.derived,
                now,
            },
            req.policy,
        );
        let attested_satisfied = req
            .policy
            .hard()
            .filter(|r| r.min_evidence >= EvidenceLevel::Attested)
            .all(|r| {
                decision.satisfied.iter().any(|f| {
                    f.requirement_id == r.id
                        && f.evidence_level.is_some_and(|l| l >= EvidenceLevel::Attested)
                })
            });
        let rec = self.challenges.get_mut(&req.challenge_id).expect("present");
        if !decision.eligible || !attested_satisfied {
            rec.state = ChallengeState::Denied;
            let codes: Vec<String> = decision.codes().iter().map(ToString::to_string).collect();
            log::warn!(
                "confidential: release refused for {} — policy {codes:?}",
                req.challenge_id
            );
            return Err(ReleaseError::PolicyIneligible { codes });
        }
        rec.state = ChallengeState::Consumed;
        let auth = ReleaseAuthorization {
            release_id: ReleaseId(self.ids.next("release")),
            task_id: verified.task_id,
            executor: verified.executor,
            session_id: verified.session_id,
            challenge_id: verified.challenge_id,
            trust_anchor: verified.trust_anchor.clone(),
            evidence_reference: verified.evidence_reference,
            environment_key: verified.environment_key,
            lowest_evidence: decision.lowest_evidence,
            issued_at: now,
            expires_at: (now + trust.release_ttl_secs).min(verified.expires_at),
        };
        self.releases.insert(
            auth.release_id,
            ReleaseRecord {
                auth: auth.clone(),
                state: ReleaseState::Released,
            },
        );
        log::info!(
            "confidential: release {} minted for task {} session {} (until {})",
            auth.release_id,
            hex::encode(&auth.task_id[..8]),
            auth.session_id,
            auth.expires_at
        );
        Ok((auth, decision))
    }

    /// Redeems a release once, inside its session, for its task (§13,
    /// §17). The grant it yields is what the control plane turns into the
    /// fetch capability and the key service turns into a wrapped key.
    pub fn redeem(
        &mut self,
        session: &Session,
        release_id: ReleaseId,
        task_id: [u8; 32],
    ) -> Result<ReleaseGrant, ReleaseError> {
        self.live_session(session)?;
        let now = self.now();
        let rec = self.releases.get_mut(&release_id).ok_or(ReleaseError::UnknownRelease)?;
        match rec.state {
            ReleaseState::Released => {}
            ReleaseState::Consumed => return Err(ReleaseError::ReleaseConsumed),
            ReleaseState::Expired => return Err(ReleaseError::ReleaseExpired),
            ReleaseState::Revoked => return Err(ReleaseError::ReleaseRevoked),
        }
        if now > rec.auth.expires_at {
            rec.state = ReleaseState::Expired;
            return Err(ReleaseError::ReleaseExpired);
        }
        if rec.auth.session_id != session.session_id {
            return Err(ReleaseError::SessionMismatch);
        }
        if rec.auth.executor != session.executor {
            return Err(ReleaseError::ExecutorMismatch);
        }
        if rec.auth.task_id != task_id {
            return Err(ReleaseError::TaskMismatch);
        }
        rec.state = ReleaseState::Consumed;
        log::info!("confidential: release {release_id} redeemed");
        Ok(ReleaseGrant {
            release_id,
            task_id,
            executor: rec.auth.executor,
            session_id: rec.auth.session_id,
            environment_key: rec.auth.environment_key,
            expires_at: rec.auth.expires_at,
        })
    }
}

// ── content keys, sealing and wrapping ────────────────────────────────────

/// Why a cryptographic operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[allow(missing_docs)]
pub enum CryptoError {
    #[error("authentication failed")]
    Authentication,
    #[error("malformed ciphertext")]
    Malformed,
    #[error("non-contributory key agreement")]
    NonContributory,
}

/// A 32-byte content-encryption key (F §14). Overwritten on drop; never
/// rendered.
pub struct ContentKey([u8; 32]);

impl ContentKey {
    /// From 32 bytes the client generated.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Constant-time-irrelevant equality for tests.
    pub fn equals(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Clone for ContentKey {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl Drop for ContentKey {
    fn drop(&mut self) {
        for b in &mut self.0 {
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl fmt::Debug for ContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ContentKey(redacted)")
    }
}

/// A confidential input as the data plane stores it: ciphertext under the
/// content key, authenticated with the task id and the input commitment.
/// The data plane holds this and never the key (F §5, §9).
#[derive(Clone, PartialEq, Eq)]
pub struct SealedInput {
    /// XChaCha20-Poly1305 nonce.
    pub nonce: [u8; 24],
    /// Ciphertext plus tag.
    pub ciphertext: Vec<u8>,
}

impl SealedInput {
    /// Serialises for storage as an opaque data-plane payload.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24 + self.ciphertext.len());
        v.extend_from_slice(&self.nonce);
        v.extend_from_slice(&self.ciphertext);
        v
    }

    /// Parses a stored payload.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        if bytes.len() < 24 + 16 {
            return Err(CryptoError::Malformed);
        }
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&bytes[..24]);
        Ok(Self {
            nonce,
            ciphertext: bytes[24..].to_vec(),
        })
    }
}

impl fmt::Debug for SealedInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SealedInput({} bytes ciphertext)", self.ciphertext.len())
    }
}

fn seal_aad(task_id: &[u8; 32], input_commitment: &[u8; 32]) -> Vec<u8> {
    let mut a = Vec::with_capacity(DOMAIN_SEAL.len() + 64);
    a.extend_from_slice(DOMAIN_SEAL);
    a.extend_from_slice(task_id);
    a.extend_from_slice(input_commitment);
    a
}

/// Seals a private input for `task_id` under `key` (F §4.1 step 3,
/// confidential profile). The commitment is over the plaintext, exactly as
/// RFC 0005 §2.4 has it; the ciphertext is what the data plane stores.
pub fn seal_input(
    key: &ContentKey,
    nonce: [u8; 24],
    task_id: &[u8; 32],
    input_commitment: &[u8; 32],
    plaintext: &[u8],
) -> SealedInput {
    let cipher = XChaCha20Poly1305::new((&key.0).into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &seal_aad(task_id, input_commitment),
            },
        )
        .expect("XChaCha20-Poly1305 encryption is infallible for in-memory buffers");
    SealedInput { nonce, ciphertext }
}

/// Opens a sealed input. Fails on any tampering or wrong binding.
pub fn open_input(
    key: &ContentKey,
    task_id: &[u8; 32],
    input_commitment: &[u8; 32],
    sealed: &SealedInput,
) -> Result<Plaintext, CryptoError> {
    let cipher = XChaCha20Poly1305::new((&key.0).into());
    cipher
        .decrypt(
            XNonce::from_slice(&sealed.nonce),
            Payload {
                msg: &sealed.ciphertext,
                aad: &seal_aad(task_id, input_commitment),
            },
        )
        .map(Plaintext::new)
        .map_err(|_| CryptoError::Authentication)
}

/// An X25519 key an execution environment holds (the "key the attested
/// environment proves it holds", F §9). The secret never leaves the
/// environment; the attestation binds the public half.
pub struct EnvironmentKey(StaticSecret);

impl EnvironmentKey {
    /// From 32 secret bytes.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(StaticSecret::from(bytes))
    }

    /// The public half.
    pub fn public(&self) -> [u8; 32] {
        PublicKey::from(&self.0).to_bytes()
    }
}

impl fmt::Debug for EnvironmentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EnvironmentKey(pk {}, secret redacted)",
            hex::encode(&self.public()[..8])
        )
    }
}

/// What a wrapped key is bound to: changing any of these makes the wrap
/// unopenable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapBinding {
    /// The release the wrap was made for.
    pub release_id: ReleaseId,
    /// Bindings copied from the release grant.
    pub task_id: [u8; 32],
    /// See `task_id`.
    pub executor: Address,
    /// See `task_id`.
    pub session_id: SessionId,
}

impl WrapBinding {
    fn aad(&self, ephemeral_public: &[u8; 32], recipient_public: &[u8; 32]) -> Vec<u8> {
        let mut a = Vec::with_capacity(DOMAIN_WRAP.len() + 32 * 6);
        a.extend_from_slice(DOMAIN_WRAP);
        a.extend_from_slice(&self.release_id.0);
        a.extend_from_slice(&self.task_id);
        a.extend_from_slice(&self.executor.0);
        a.extend_from_slice(&self.session_id.0);
        a.extend_from_slice(ephemeral_public);
        a.extend_from_slice(recipient_public);
        a
    }
}

/// A content key wrapped to an environment key: X25519 agreement between
/// a fresh ephemeral key and the recipient, BLAKE3 `derive_key` to a wrap
/// key, XChaCha20-Poly1305 with the release binding as associated data.
/// Useless to anyone without the recipient's secret.
#[derive(Clone, PartialEq, Eq)]
pub struct WrappedKey {
    /// The wrapper's ephemeral public key.
    pub ephemeral_public: [u8; 32],
    /// The recipient the wrap was made for.
    pub recipient_public: [u8; 32],
    /// Nonce.
    pub nonce: [u8; 24],
    /// Ciphertext plus tag (48 bytes).
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for WrappedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WrappedKey(to {}, ciphertext redacted)",
            hex::encode(&self.recipient_public[..8])
        )
    }
}

fn wrap_key_material(
    shared: &[u8; 32],
    ephemeral_public: &[u8; 32],
    recipient_public: &[u8; 32],
) -> [u8; 32] {
    let mut m = Vec::with_capacity(96);
    m.extend_from_slice(shared);
    m.extend_from_slice(ephemeral_public);
    m.extend_from_slice(recipient_public);
    blake3::derive_key(KDF_CONTEXT, &m)
}

/// Wraps `key` to `recipient_public` under `binding`, using `ephemeral` as
/// the wrapper's one-time X25519 secret and `nonce` for the AEAD.
pub fn wrap_key(
    key: &ContentKey,
    recipient_public: [u8; 32],
    ephemeral: &EnvironmentKey,
    nonce: [u8; 24],
    binding: &WrapBinding,
) -> Result<WrappedKey, CryptoError> {
    let ephemeral_public = ephemeral.public();
    let shared = ephemeral.0.diffie_hellman(&PublicKey::from(recipient_public));
    if !shared.was_contributory() {
        return Err(CryptoError::NonContributory);
    }
    let wk = wrap_key_material(shared.as_bytes(), &ephemeral_public, &recipient_public);
    let cipher = XChaCha20Poly1305::new((&wk).into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &key.0,
                aad: &binding.aad(&ephemeral_public, &recipient_public),
            },
        )
        .map_err(|_| CryptoError::Authentication)?;
    Ok(WrappedKey {
        ephemeral_public,
        recipient_public,
        nonce,
        ciphertext,
    })
}

/// Unwraps inside the environment holding `recipient`.
pub fn unwrap_key(
    wrapped: &WrappedKey,
    recipient: &EnvironmentKey,
    binding: &WrapBinding,
) -> Result<ContentKey, CryptoError> {
    if recipient.public() != wrapped.recipient_public {
        return Err(CryptoError::Authentication);
    }
    let shared = recipient.0.diffie_hellman(&PublicKey::from(wrapped.ephemeral_public));
    if !shared.was_contributory() {
        return Err(CryptoError::NonContributory);
    }
    let wk = wrap_key_material(
        shared.as_bytes(),
        &wrapped.ephemeral_public,
        &wrapped.recipient_public,
    );
    let cipher = XChaCha20Poly1305::new((&wk).into());
    let bytes = cipher
        .decrypt(
            XNonce::from_slice(&wrapped.nonce),
            Payload {
                msg: &wrapped.ciphertext,
                aad: &binding.aad(&wrapped.ephemeral_public, &wrapped.recipient_public),
            },
        )
        .map_err(|_| CryptoError::Authentication)?;
    let mut k = [0u8; 32];
    if bytes.len() != 32 {
        return Err(CryptoError::Malformed);
    }
    k.copy_from_slice(&bytes);
    Ok(ContentKey(k))
}

/// The client's delegated key service (privacy §11 "key release", F §14):
/// holds content keys the owner registered and releases each, wrapped,
/// exactly once per redeemed release. It signs no receipt, names no
/// executor and touches no chain state.
pub struct KeyReleaseAuthority {
    ids: IdSource,
    keys: BTreeMap<([u8; 32], ObjectId), (Address, ContentKey)>,
    released: BTreeSet<ReleaseId>,
}

impl fmt::Debug for KeyReleaseAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KeyReleaseAuthority({} keys, {} releases; keys redacted)",
            self.keys.len(),
            self.released.len()
        )
    }
}

impl KeyReleaseAuthority {
    /// A fresh key service.
    pub fn new(ids: IdSource) -> Self {
        Self {
            ids,
            keys: BTreeMap::new(),
            released: BTreeSet::new(),
        }
    }

    /// The owner registers the content key of `object` for `task_id`.
    pub fn register_content_key(
        &mut self,
        owner: &LocalKey,
        task_id: [u8; 32],
        object: ObjectId,
        key: ContentKey,
    ) {
        self.keys.insert((task_id, object), (owner.address(), key));
        log::info!(
            "key-service: content key registered for task {} object {object}",
            hex::encode(&task_id[..8])
        );
    }

    /// Releases the content key of `object`, wrapped to the environment
    /// key the redeemed `grant` carries. One wrapped key per release: a
    /// second call for the same release is refused (§13 idempotence).
    pub fn release(
        &mut self,
        grant: &ReleaseGrant,
        object: ObjectId,
    ) -> Result<WrappedKey, ReleaseError> {
        if self.released.contains(&grant.release_id) {
            return Err(ReleaseError::ReleaseConsumed);
        }
        let (_, key) = self.keys.get(&(grant.task_id, object)).ok_or(ReleaseError::NoContentKey)?;
        let ephemeral = EnvironmentKey::from_bytes(self.ids.next("wrap-ephemeral"));
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&self.ids.next("wrap-nonce")[..24]);
        let wrapped = wrap_key(
            key,
            grant.environment_key,
            &ephemeral,
            nonce,
            &WrapBinding {
                release_id: grant.release_id,
                task_id: grant.task_id,
                executor: grant.executor,
                session_id: grant.session_id,
            },
        )
        .map_err(|_| ReleaseError::NoContentKey)?;
        self.released.insert(grant.release_id);
        log::info!(
            "key-service: content key released under {} to environment key {}",
            grant.release_id,
            hex::encode(&grant.environment_key[..8])
        );
        Ok(wrapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purpose_tags_round_trip() {
        for p in [Purpose::ConfidentialInputRelease, Purpose::ResultRelease] {
            assert_eq!(Purpose::from_tag(p.tag()), Some(p));
        }
        assert_eq!(Purpose::from_tag(0), None);
    }

    #[test]
    fn content_key_debug_is_redacted() {
        let k = ContentKey::from_bytes([0x42u8; 32]);
        let s = format!("{k:?}");
        assert!(!s.contains("4242"));
    }
}
