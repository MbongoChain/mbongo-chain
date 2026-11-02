//! Reference provider-policy evaluator (`provider-policy-v1`).
//!
//! A pure, deterministic implementation of the eligibility semantics in
//! `docs/architecture/compute-provider-capability-policy.md` (Workstream
//! J): given an accepted task's executor, the session that proved an
//! executor key, a provider's capability advertisement, the control plane's
//! own derived facts, an effective policy and an explicit `now`, decide
//! whether the session is *eligible* to execute the task, and say why.
//!
//! What this module is not: it is not consensus, not an RPC, not wired into
//! the reference control plane (whose admission is unchanged), not a
//! registry, not a ranker and not a verifier. It never parses attestation
//! material; it consumes evidence *records* a verifier produced (J31). It
//! holds no clock: `now` is an argument (J23).
//!
//! Vocabulary, section numbers and reason codes below are the document's.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use mbongo_core::Address;

use crate::identity::SessionId;

/// The contract version this evaluator implements (§25).
pub const POLICY_VERSION: &str = "provider-policy-v1";
/// The advertisement schema this evaluator reads (§5.1).
pub const ADVERTISEMENT_SCHEMA: &str = "capability-advertisement-v1";

/// Dimension keys of the version-1 vocabulary (§5.3, §5.4).
pub mod dimension {
    /// Control-plane-local provider identifier.
    pub const PROVIDER_ID: &str = "identity.provider_id";
    /// Executors the provider operates.
    pub const EXECUTORS: &str = "identity.executors";
    /// E §5 contract version the session states.
    pub const CONTRACT_VERSION: &str = "execution.contract_version";
    /// F §4.2 representation tags the session can read.
    pub const REPRESENTATION_TAGS: &str = "execution.representation_tags";
    /// `execution_spec` tags the session can run (E §6 row 7).
    pub const PROFILE_TAGS: &str = "execution.profile_tags";
    /// The provider's confidentiality claim; never sufficient alone (§14).
    pub const CONFIDENTIAL_EXECUTION: &str = "privacy.confidential_execution";
    /// Legal jurisdiction of the provider entity.
    pub const JURISDICTION_PROVIDER: &str = "jurisdiction.provider";
    /// Physical location of the worker class or session.
    pub const JURISDICTION_WORKER_LOCATION: &str = "jurisdiction.worker_location";
    /// Where the data plane stores objects.
    pub const RESIDENCY_DATA: &str = "residency.data";
    /// Where content keys are held.
    pub const RESIDENCY_KEY: &str = "residency.key";
    /// Where results are stored.
    pub const RESIDENCY_RESULT_STORAGE: &str = "residency.result_storage";
    /// `EPHEMERAL` < `BOUNDED` < `PERSISTENT`.
    pub const RETENTION_CLASS: &str = "retention.class";
    /// Maximum retention after completion, seconds.
    pub const RETENTION_MAX_SECONDS: &str = "retention.max_seconds";
    /// `NONE` < `RESTRICTED` < `UNRESTRICTED`.
    pub const EGRESS_MODE: &str = "egress.mode";
    /// Permitted outbound destinations.
    pub const EGRESS_DESTINATIONS: &str = "egress.destinations";
    /// Abstract accelerator class (deployment vocabulary; no vendor).
    pub const HARDWARE_ACCELERATOR_CLASS: &str = "hardware.accelerator_class";
    /// Installed memory, bytes.
    pub const HARDWARE_MEMORY_BYTES: &str = "hardware.memory_bytes";
    /// Free memory now, bytes (session scope).
    pub const HARDWARE_AVAILABLE_MEMORY_BYTES: &str = "hardware.available_memory_bytes";
    /// Deployment-defined throughput unit.
    pub const HARDWARE_THROUGHPUT_UNITS: &str = "hardware.throughput_units";
    /// Runtime feature tokens.
    pub const HARDWARE_RUNTIME_FEATURES: &str = "hardware.runtime_features";
    /// Runtime family.
    pub const RUNTIME_FAMILY: &str = "runtime.family";
    /// Runtime version.
    pub const RUNTIME_VERSION: &str = "runtime.version";
    /// Result-verification class; only `NONE` is satisfiable today (#52).
    pub const VERIFICATION_RESULT: &str = "verification.result";
    /// Price claim, in `commercial.price_unit`.
    pub const COMMERCIAL_PRICE: &str = "commercial.price";
    /// Pricing reference unit.
    pub const COMMERCIAL_PRICE_UNIT: &str = "commercial.price_unit";
    /// Latency claim, milliseconds.
    pub const COMMERCIAL_LATENCY_MS: &str = "commercial.latency_ms";
    /// Session availability state.
    pub const AVAILABILITY_STATE: &str = "availability.state";
    /// Control-plane-derived reputation (§5.4); never a self-claim.
    pub const REPUTATION_SCORE: &str = "reputation.score";
}

/// Ordered token vocabularies (§18.2).
const RETENTION_ORDER: [&str; 3] = ["EPHEMERAL", "BOUNDED", "PERSISTENT"];
const EGRESS_ORDER: [&str; 3] = ["NONE", "RESTRICTED", "UNRESTRICTED"];

/// A typed claim or constraint value (§18.2). Compared canonically: tokens
/// byte for byte, numbers as integers in base units, sets as sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// An opaque token.
    Token(String),
    /// An unsigned integer in the dimension's base unit.
    Number(u64),
    /// A set of tokens.
    Set(BTreeSet<String>),
    /// A boolean.
    Bool(bool),
    /// A list of executor addresses (compared by membership).
    Addresses(Vec<Address>),
}

/// How much evidence backs a claim (§6.2). Ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceLevel {
    /// The provider asserted it.
    Claim,
    /// The control plane established it by its own check.
    ControlPlaneVerified,
    /// A party the policy trusts attested to it off-chain.
    Certified,
    /// A verifier checked remote attestation evidence (K).
    Attested,
    /// A cryptographic proof needing no trusted verifier (reserved).
    Proven,
}

impl fmt::Display for EvidenceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Claim => "CLAIM",
            Self::ControlPlaneVerified => "CONTROL_PLANE_VERIFIED",
            Self::Certified => "CERTIFIED",
            Self::Attested => "ATTESTED",
            Self::Proven => "PROVEN",
        })
    }
}

/// What an evidence record is bound to (§6.2, §19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binding {
    /// True of the provider as a whole.
    Provider,
    /// Bound to one session.
    Session(SessionId),
    /// Bound to a challenge the verifier issued for one session.
    Challenge {
        /// The session the challenge was issued for.
        session: SessionId,
        /// Opaque challenge reference (never the nonce material itself).
        challenge_ref: String,
    },
}

/// An evidence record attached to a claim. Carries the level, window and
/// binding and an opaque reference — never the underlying material (§6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// The level a verifier established.
    pub level: EvidenceLevel,
    /// When established.
    pub issued_at: u64,
    /// After this the level reverts to `Claim`.
    pub expires_at: Option<u64>,
    /// What it is bound to.
    pub binding: Binding,
    /// Who established it.
    pub verifier: String,
    /// Opaque reference to the material or the root it matched.
    pub reference: String,
    /// Revoked by the verifier or the control plane (§21).
    pub revoked: bool,
}

/// Scope of a claim (§20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// True of the operator as a whole.
    Provider,
    /// True of a pool of instances.
    WorkerClass,
    /// True of this instance, now.
    Session,
}

/// One claim (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The value.
    pub value: Value,
    /// Where it was advertised.
    pub scope: Scope,
    /// Optional evidence; absent means `Claim`.
    pub evidence: Option<Evidence>,
}

impl Claim {
    /// A plain claim at the given scope.
    pub fn plain(value: Value, scope: Scope) -> Self {
        Self {
            value,
            scope,
            evidence: None,
        }
    }
}

/// A capability advertisement (§5.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advertisement {
    /// Control-plane-local provider identifier (§5.2).
    pub provider_id: String,
    /// Executors the provider operates.
    pub executors: Vec<Address>,
    /// Issued at.
    pub issued_at: u64,
    /// Expires at (§21).
    pub expires_at: u64,
    /// Revoked by the control plane (§21).
    pub revoked: bool,
    /// Provider-scoped claims.
    pub claims: BTreeMap<String, Claim>,
    /// Worker-class-scoped claims, by class label.
    pub worker_classes: BTreeMap<String, BTreeMap<String, Claim>>,
}

/// The session under evaluation (E §5; §10.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFacts {
    /// The session.
    pub session_id: SessionId,
    /// The executor whose key this session proved (control-plane-verified).
    pub proved_executor: Address,
    /// The worker class the instance belongs to, if any.
    pub worker_class: Option<String>,
    /// Session-scoped claims (contract version, tags, availability,
    /// confidentiality, attestation evidence).
    pub claims: BTreeMap<String, Claim>,
}

/// The task facts the evaluator needs (§10.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFacts {
    /// The executor the chain committed.
    pub executor: Address,
    /// The `execution_spec` tag, as the profile the session must run.
    pub profile_tag: String,
    /// The representation tag the client registered (F §4.2).
    pub representation_tag: String,
}

/// Control-plane-derived facts (§5.4): values the provider is not the
/// authority for. Evaluated at `ControlPlaneVerified`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivedFacts {
    /// Dimension key to value.
    pub values: BTreeMap<String, Value>,
}

/// Who authored a requirement (§8.1). Declaration order is the fixed
/// ordering of soft preferences (§8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// The client / submitter.
    DataOwner,
    /// The organisation the client acts within.
    Organization,
    /// The control-plane operator.
    ControlPlane,
    /// The provider, about what it accepts.
    Provider,
}

/// Hard or soft (§9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Decides eligibility.
    Hard,
    /// Recorded only.
    Soft,
}

/// Session freshness (§20).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Any scope.
    Any,
    /// Only a session-scoped claim of this session, with evidence bound to it.
    Session,
}

/// What an unknown outcome does to a hard requirement (§11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnUnknown {
    /// Fail closed (default).
    Fail,
    /// The author allows unknown.
    Allow,
}

/// A constraint (§7, §18.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    /// Value equals.
    Equals(Value),
    /// Token is one of; for a set claim, any member is.
    OneOf(BTreeSet<String>),
    /// Token is none of.
    NoneOf(BTreeSet<String>),
    /// Number, or ordered token, is at least.
    AtLeast(Value),
    /// Number, or ordered token, is at most.
    AtMost(Value),
    /// Set contains all of.
    ContainsAll(BTreeSet<String>),
    /// Set is a subset of.
    SubsetOf(BTreeSet<String>),
    /// Boolean is true.
    IsTrue,
    /// Address set contains.
    ContainsAddress(Address),
}

/// One requirement (§7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// Unique within the effective policy; the evaluation order key.
    pub id: String,
    /// Author.
    pub source: Source,
    /// Hard or soft.
    pub kind: Kind,
    /// Dimension key.
    pub dimension: String,
    /// The constraint.
    pub constraint: Constraint,
    /// Minimum evidence level.
    pub min_evidence: EvidenceLevel,
    /// Freshness.
    pub freshness: Freshness,
    /// Unknown handling.
    pub on_unknown: OnUnknown,
    /// For `commercial.price`: the unit the cap is expressed in.
    pub price_unit: Option<String>,
}

impl Requirement {
    /// A hard requirement with the defaults of §7.
    pub fn hard(id: &str, source: Source, dimension: &str, constraint: Constraint) -> Self {
        Self {
            id: id.to_owned(),
            source,
            kind: Kind::Hard,
            dimension: dimension.to_owned(),
            constraint,
            min_evidence: EvidenceLevel::Claim,
            freshness: Freshness::Any,
            on_unknown: OnUnknown::Fail,
            price_unit: None,
        }
    }

    /// A soft preference.
    pub fn soft(id: &str, source: Source, dimension: &str, constraint: Constraint) -> Self {
        Self {
            kind: Kind::Soft,
            ..Self::hard(id, source, dimension, constraint)
        }
    }

    /// Sets the minimum evidence level.
    #[must_use]
    pub fn with_evidence(mut self, level: EvidenceLevel) -> Self {
        self.min_evidence = level;
        self
    }

    /// Requires session freshness.
    #[must_use]
    pub fn session_fresh(mut self) -> Self {
        self.freshness = Freshness::Session;
        self
    }

    /// Allows unknown.
    #[must_use]
    pub fn allow_unknown(mut self) -> Self {
        self.on_unknown = OnUnknown::Allow;
        self
    }

    /// Names the price unit a `commercial.price` cap is in.
    #[must_use]
    pub fn in_unit(mut self, unit: &str) -> Self {
        self.price_unit = Some(unit.to_owned());
        self
    }
}

/// One source's requirements (§7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyFragment {
    /// Author.
    pub source: Source,
    /// Requirements.
    pub requirements: Vec<Requirement>,
}

/// Why a policy could not be composed (§8.3, §11.2).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// Two hard requirements cannot be jointly satisfied.
    #[error("POLICY_CONFLICT on {dimension}: {first} and {second}")]
    Conflict {
        /// Dimension.
        dimension: String,
        /// First requirement id.
        first: String,
        /// Second requirement id.
        second: String,
    },
    /// Malformed.
    #[error("POLICY_INVALID: {0}")]
    Invalid(String),
}

/// The composed policy (§8.2): the union of every source's hard
/// requirements, plus soft preferences in source order. Constructed only
/// through [`compose`], which is what guarantees the union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectivePolicy {
    hard: BTreeMap<String, Requirement>,
    soft: Vec<Requirement>,
}

impl EffectivePolicy {
    /// Hard requirements, in id order.
    pub fn hard(&self) -> impl Iterator<Item = &Requirement> {
        self.hard.values()
    }

    /// Soft preferences, in source order then id order.
    pub fn soft(&self) -> &[Requirement] {
        &self.soft
    }
}

/// Composes fragments per §8.2: union of hard requirements; a source may
/// tighten and never weaken; a joint contradiction is `POLICY_CONFLICT`,
/// never resolved by precedence (§8.3).
pub fn compose(fragments: &[PolicyFragment]) -> Result<EffectivePolicy, PolicyError> {
    let mut hard: BTreeMap<String, Requirement> = BTreeMap::new();
    let mut soft: Vec<Requirement> = Vec::new();
    for fragment in fragments {
        for r in &fragment.requirements {
            if r.source != fragment.source {
                return Err(PolicyError::Invalid(format!(
                    "requirement {} claims a source other than its fragment's",
                    r.id
                )));
            }
            if r.dimension.starts_with("reputation.") && fragment.source == Source::Provider {
                return Err(PolicyError::Invalid(format!(
                    "requirement {}: a provider fragment may not constrain reputation",
                    r.id
                )));
            }
            validate_constraint(r)?;
            match r.kind {
                Kind::Hard => {
                    if hard.insert(r.id.clone(), r.clone()).is_some() {
                        return Err(PolicyError::Invalid(format!(
                            "duplicate requirement id {}",
                            r.id
                        )));
                    }
                }
                Kind::Soft => soft.push(r.clone()),
            }
        }
    }
    // Conflict detection over hard requirements on the same dimension.
    let reqs: Vec<&Requirement> = hard.values().collect();
    for (i, a) in reqs.iter().enumerate() {
        for b in &reqs[i + 1..] {
            if a.dimension == b.dimension && conflicts(a, b) {
                return Err(PolicyError::Conflict {
                    dimension: a.dimension.clone(),
                    first: a.id.clone(),
                    second: b.id.clone(),
                });
            }
        }
    }
    soft.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.id.cmp(&b.id)));
    Ok(EffectivePolicy { hard, soft })
}

fn validate_constraint(r: &Requirement) -> Result<(), PolicyError> {
    let ok = match &r.constraint {
        Constraint::AtLeast(v) | Constraint::AtMost(v) => match v {
            Value::Number(_) => true,
            Value::Token(t) => ordered_rank(&r.dimension, t).is_some(),
            _ => false,
        },
        Constraint::IsTrue => {
            r.dimension == dimension::CONFIDENTIAL_EXECUTION || r.dimension.starts_with("x.")
        }
        _ => true,
    };
    if !ok {
        return Err(PolicyError::Invalid(format!(
            "requirement {}: constraint not valid for {}",
            r.id, r.dimension
        )));
    }
    if r.dimension == dimension::COMMERCIAL_PRICE && r.price_unit.is_none() {
        return Err(PolicyError::Invalid(format!(
            "requirement {}: price cap without a unit",
            r.id
        )));
    }
    Ok(())
}

fn ordered_rank(dimension: &str, token: &str) -> Option<u64> {
    let order: &[&str] = match dimension {
        dimension::RETENTION_CLASS => &RETENTION_ORDER,
        dimension::EGRESS_MODE => &EGRESS_ORDER,
        _ => return None,
    };
    order.iter().position(|t| *t == token).and_then(|p| u64::try_from(p).ok())
}

/// Whether two hard constraints on one dimension can never both hold.
fn conflicts(a: &Requirement, b: &Requirement) -> bool {
    use Constraint::{AtLeast, AtMost, Equals, NoneOf, OneOf};
    let rank = |v: &Value, dim: &str| match v {
        Value::Number(n) => Some(*n),
        Value::Token(t) => ordered_rank(dim, t),
        _ => None,
    };
    match (&a.constraint, &b.constraint) {
        (OneOf(x), OneOf(y)) => x.is_disjoint(y),
        (OneOf(x), NoneOf(y)) | (NoneOf(y), OneOf(x)) => x.is_subset(y),
        (OneOf(x), Equals(Value::Token(t))) | (Equals(Value::Token(t)), OneOf(x)) => !x.contains(t),
        (NoneOf(x), Equals(Value::Token(t))) | (Equals(Value::Token(t)), NoneOf(x)) => {
            x.contains(t)
        }
        (Equals(x), Equals(y)) => x != y,
        (AtLeast(lo), AtMost(hi)) | (AtMost(hi), AtLeast(lo)) => {
            match (rank(lo, &a.dimension), rank(hi, &a.dimension)) {
                (Some(lo), Some(hi)) => lo > hi,
                _ => false,
            }
        }
        _ => false,
    }
}

/// Stable control-plane reason codes (§11.2). Never consensus, RPC or wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Code {
    PolicyInvalid,
    PolicyConflict,
    ExecutorMismatch,
    AdvertisementExpired,
    AdvertisementRevoked,
    ContractVersionUnsupported,
    RepresentationUnsupported,
    ProfileUnsupported,
    ProviderDenied,
    ProviderNotAllowed,
    InsufficientEvidence,
    AttestationRequired,
    EvidenceExpired,
    EvidenceNotSessionBound,
    CapabilityUnknown,
    JurisdictionNotAllowed,
    ResidencyNotSatisfied,
    RetentionTooLong,
    EgressNotAllowed,
    HardwareCapabilityMismatch,
    RuntimeCapabilityMismatch,
    ConfidentialityNotClaimed,
    VerificationStrengthUnavailable,
    PriceAboveMax,
    LatencyAboveMax,
    ReputationBelowMin,
    AvailabilityMismatch,
    ExtensionMismatch,
    /// A value mismatch on a dimension with no more specific code.
    ConstraintNotSatisfied,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PolicyInvalid => "POLICY_INVALID",
            Self::PolicyConflict => "POLICY_CONFLICT",
            Self::ExecutorMismatch => "EXECUTOR_MISMATCH",
            Self::AdvertisementExpired => "ADVERTISEMENT_EXPIRED",
            Self::AdvertisementRevoked => "ADVERTISEMENT_REVOKED",
            Self::ContractVersionUnsupported => "CONTRACT_VERSION_UNSUPPORTED",
            Self::RepresentationUnsupported => "REPRESENTATION_UNSUPPORTED",
            Self::ProfileUnsupported => "PROFILE_UNSUPPORTED",
            Self::ProviderDenied => "PROVIDER_DENIED",
            Self::ProviderNotAllowed => "PROVIDER_NOT_ALLOWED",
            Self::InsufficientEvidence => "INSUFFICIENT_EVIDENCE",
            Self::AttestationRequired => "ATTESTATION_REQUIRED",
            Self::EvidenceExpired => "EVIDENCE_EXPIRED",
            Self::EvidenceNotSessionBound => "EVIDENCE_NOT_SESSION_BOUND",
            Self::CapabilityUnknown => "CAPABILITY_UNKNOWN",
            Self::JurisdictionNotAllowed => "JURISDICTION_NOT_ALLOWED",
            Self::ResidencyNotSatisfied => "RESIDENCY_NOT_SATISFIED",
            Self::RetentionTooLong => "RETENTION_TOO_LONG",
            Self::EgressNotAllowed => "EGRESS_NOT_ALLOWED",
            Self::HardwareCapabilityMismatch => "HARDWARE_CAPABILITY_MISMATCH",
            Self::RuntimeCapabilityMismatch => "RUNTIME_CAPABILITY_MISMATCH",
            Self::ConfidentialityNotClaimed => "CONFIDENTIALITY_NOT_CLAIMED",
            Self::VerificationStrengthUnavailable => "VERIFICATION_STRENGTH_UNAVAILABLE",
            Self::PriceAboveMax => "PRICE_ABOVE_MAX",
            Self::LatencyAboveMax => "LATENCY_ABOVE_MAX",
            Self::ReputationBelowMin => "REPUTATION_BELOW_MIN",
            Self::AvailabilityMismatch => "AVAILABILITY_MISMATCH",
            Self::ExtensionMismatch => "EXTENSION_MISMATCH",
            Self::ConstraintNotSatisfied => "CONSTRAINT_NOT_SATISFIED",
        })
    }
}

/// Three-valued outcome of one requirement (§11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Met, with evidence at least the minimum.
    Satisfied,
    /// A claim or evidence exists and fails.
    NotSatisfied,
    /// No claim, wrong type, or not session-bound where required.
    Unknown,
}

/// One finding in a decision (§10.4). Carries ids, keys, codes and levels —
/// never a claim value, a policy value, evidence material or a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Requirement id.
    pub requirement_id: String,
    /// Dimension key.
    pub dimension: String,
    /// Author.
    pub source: Source,
    /// Outcome.
    pub outcome: Outcome,
    /// Reason code where the outcome is not `Satisfied`.
    pub code: Option<Code>,
    /// Evidence level the outcome rests on, where satisfied.
    pub evidence_level: Option<EvidenceLevel>,
    /// For unknown hard requirements: whether the author allowed it.
    pub allowed: bool,
}

/// The executor gate result (§10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorGate {
    /// The session proved the task's executor and the advertisement names it.
    Passed,
    /// It did not; nothing else was evaluated.
    Mismatch,
}

/// The decision (§10.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// Eligible.
    pub eligible: bool,
    /// Always [`POLICY_VERSION`].
    pub policy_version: &'static str,
    /// The gate.
    pub executor_gate: ExecutorGate,
    /// Pre-policy failures: advertisement lifetime, compatibility.
    pub preconditions: Vec<Finding>,
    /// Hard failures.
    pub hard_failures: Vec<Finding>,
    /// Unknown hard requirements (allowed or not).
    pub unknown: Vec<Finding>,
    /// Satisfied hard requirements.
    pub satisfied: Vec<Finding>,
    /// Soft findings, recorded only.
    pub soft: Vec<Finding>,
    /// The lowest evidence level among satisfied hard requirements — what
    /// eligibility actually rests on. `Claim` when nothing stronger was
    /// required.
    pub lowest_evidence: EvidenceLevel,
    /// The `now` input.
    pub evaluated_at: u64,
}

impl Decision {
    /// All codes among hard failures and preconditions, in order.
    pub fn codes(&self) -> Vec<Code> {
        self.preconditions
            .iter()
            .chain(self.hard_failures.iter())
            .filter_map(|f| f.code)
            .collect()
    }
}

/// The inputs of §10.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input<'a> {
    /// Task facts.
    pub task: &'a TaskFacts,
    /// Session facts.
    pub session: &'a SessionFacts,
    /// Advertisement.
    pub advertisement: &'a Advertisement,
    /// Control-plane-derived facts.
    pub derived: &'a DerivedFacts,
    /// Evaluation time.
    pub now: u64,
}

/// The eligibility function (§10). Pure: identical inputs give an identical
/// decision (J23). The executor gate is first and short-circuits (§10.2,
/// J44); hard requirements are evaluated completely before soft ones (J24);
/// soft findings never change `eligible` (J25).
pub fn eligible(input: &Input<'_>, policy: &EffectivePolicy) -> Decision {
    let mut decision = Decision {
        eligible: false,
        policy_version: POLICY_VERSION,
        executor_gate: ExecutorGate::Mismatch,
        preconditions: Vec::new(),
        hard_failures: Vec::new(),
        unknown: Vec::new(),
        satisfied: Vec::new(),
        soft: Vec::new(),
        lowest_evidence: EvidenceLevel::Claim,
        evaluated_at: input.now,
    };

    // 1. Executor gate (§10.2): both halves, nothing else on failure.
    let gate = input.session.proved_executor == input.task.executor
        && input.advertisement.executors.contains(&input.task.executor);
    if !gate {
        decision.preconditions.push(gate_finding(Code::ExecutorMismatch));
        return decision;
    }
    decision.executor_gate = ExecutorGate::Passed;

    // 2. Advertisement lifetime (§21).
    if input.advertisement.revoked {
        decision.preconditions.push(gate_finding(Code::AdvertisementRevoked));
    } else if input.now > input.advertisement.expires_at
        || input.now < input.advertisement.issued_at
    {
        decision.preconditions.push(gate_finding(Code::AdvertisementExpired));
    }

    // 3. Compatibility (E §5, §6 row 7): session-scoped claims only.
    let session_set = |key: &str| -> Option<&BTreeSet<String>> {
        match input.session.claims.get(key) {
            Some(Claim {
                value: Value::Set(s),
                scope: Scope::Session,
                ..
            }) => Some(s),
            _ => None,
        }
    };
    if !matches!(session_set(dimension::REPRESENTATION_TAGS), Some(s) if s.contains(&input.task.representation_tag))
    {
        decision.preconditions.push(gate_finding(Code::RepresentationUnsupported));
    }
    if !matches!(session_set(dimension::PROFILE_TAGS), Some(s) if s.contains(&input.task.profile_tag))
    {
        decision.preconditions.push(gate_finding(Code::ProfileUnsupported));
    }
    if !matches!(
        input.session.claims.get(dimension::CONTRACT_VERSION),
        Some(Claim {
            value: Value::Token(_),
            scope: Scope::Session,
            ..
        })
    ) {
        decision.preconditions.push(gate_finding(Code::ContractVersionUnsupported));
    }

    // 4. Hard requirements, completely, in id order.
    let mut lowest: Option<EvidenceLevel> = None;
    for r in policy.hard() {
        let f = evaluate_requirement(r, input);
        match (f.outcome, r.on_unknown) {
            (Outcome::Satisfied, _) => {
                let lvl = f.evidence_level.unwrap_or(EvidenceLevel::Claim);
                lowest = Some(lowest.map_or(lvl, |l: EvidenceLevel| l.min(lvl)));
                decision.satisfied.push(f);
            }
            (Outcome::NotSatisfied, _) | (Outcome::Unknown, OnUnknown::Fail) => {
                decision.hard_failures.push(f);
            }
            (Outcome::Unknown, OnUnknown::Allow) => {
                decision.unknown.push(Finding { allowed: true, ..f });
            }
        }
    }

    // 5. Soft preferences, recorded only.
    for r in policy.soft() {
        decision.soft.push(evaluate_requirement(r, input));
    }

    decision.lowest_evidence = lowest.unwrap_or(EvidenceLevel::Claim);
    decision.eligible = decision.preconditions.is_empty() && decision.hard_failures.is_empty();
    decision
}

fn gate_finding(code: Code) -> Finding {
    Finding {
        requirement_id: String::new(),
        dimension: match code {
            Code::ExecutorMismatch => dimension::EXECUTORS,
            Code::AdvertisementExpired | Code::AdvertisementRevoked => "advertisement",
            Code::RepresentationUnsupported => dimension::REPRESENTATION_TAGS,
            Code::ProfileUnsupported => dimension::PROFILE_TAGS,
            _ => dimension::CONTRACT_VERSION,
        }
        .to_owned(),
        source: Source::ControlPlane,
        outcome: Outcome::NotSatisfied,
        code: Some(code),
        evidence_level: None,
        allowed: false,
    }
}

/// Claim lookup, most specific scope first (§20): session, the session's
/// worker class, provider. Control-plane-derived dimensions come from
/// `derived` at `ControlPlaneVerified` and are never read from the
/// advertisement (§5.4).
fn lookup<'a>(
    r: &Requirement,
    input: &'a Input<'_>,
) -> Option<(Value, Scope, Option<&'a Evidence>)> {
    if r.dimension.starts_with("reputation.") {
        return input
            .derived
            .values
            .get(&r.dimension)
            .map(|v| (v.clone(), Scope::Provider, None));
    }
    if r.dimension == dimension::PROVIDER_ID {
        let c = input.advertisement.claims.get(dimension::PROVIDER_ID);
        return Some((
            Value::Token(input.advertisement.provider_id.clone()),
            Scope::Provider,
            c.and_then(|c| c.evidence.as_ref()),
        ));
    }
    if r.dimension == dimension::EXECUTORS {
        return Some((
            Value::Addresses(input.advertisement.executors.clone()),
            Scope::Provider,
            None,
        ));
    }
    if let Some(c) = input.session.claims.get(&r.dimension) {
        return Some((c.value.clone(), Scope::Session, c.evidence.as_ref()));
    }
    if let Some(class) = input.session.worker_class.as_ref() {
        if let Some(c) =
            input.advertisement.worker_classes.get(class).and_then(|m| m.get(&r.dimension))
        {
            return Some((c.value.clone(), Scope::WorkerClass, c.evidence.as_ref()));
        }
    }
    input
        .advertisement
        .claims
        .get(&r.dimension)
        .map(|c| (c.value.clone(), Scope::Provider, c.evidence.as_ref()))
}

/// §10.3, one requirement.
fn evaluate_requirement(r: &Requirement, input: &Input<'_>) -> Finding {
    let mut f = Finding {
        requirement_id: r.id.clone(),
        dimension: r.dimension.clone(),
        source: r.source,
        outcome: Outcome::Unknown,
        code: Some(Code::CapabilityUnknown),
        evidence_level: None,
        allowed: false,
    };

    // 1. No claim.
    let Some((value, scope, evidence)) = lookup(r, input) else {
        return f;
    };

    // 2. Freshness (§20): a session requirement needs a session-scoped claim
    //    of this session; provider-wide evidence never substitutes (J20).
    let bound_here = |e: &Evidence| match &e.binding {
        Binding::Provider => true,
        Binding::Session(s) | Binding::Challenge { session: s, .. } => {
            *s == input.session.session_id
        }
    };
    if r.freshness == Freshness::Session {
        // Evidence, where present, must be bound to this session. A
        // session-scoped claim with no evidence passes here and meets the
        // evidence bar, or not, in step 3.
        let evidence_bound_here = evidence.map_or(true, |e| {
            !matches!(e.binding, Binding::Provider) && bound_here(e)
        });
        let ok = scope == Scope::Session && evidence_bound_here;
        if !ok {
            f.code = Some(Code::EvidenceNotSessionBound);
            return f;
        }
    }

    // 3. Effective evidence level (§19).
    let derived = r.dimension.starts_with("reputation.");
    let (level, expired) = match evidence {
        _ if derived => (EvidenceLevel::ControlPlaneVerified, false),
        Some(e) if !bound_here(e) => (EvidenceLevel::Claim, false),
        Some(e) => {
            let live = !e.revoked
                && input.now >= e.issued_at
                && e.expires_at.map_or(true, |x| input.now <= x);
            if live {
                (e.level, false)
            } else {
                (EvidenceLevel::Claim, true)
            }
        }
        None => (EvidenceLevel::Claim, false),
    };
    if level < r.min_evidence {
        f.outcome = Outcome::NotSatisfied;
        f.code = Some(if expired {
            Code::EvidenceExpired
        } else if r.min_evidence >= EvidenceLevel::Attested {
            Code::AttestationRequired
        } else {
            Code::InsufficientEvidence
        });
        return f;
    }

    // 4–5. Type check and canonical comparison (§18.2).
    match compare(r, &value, input) {
        None => f, // type mismatch → UNKNOWN
        Some(true) => {
            f.outcome = Outcome::Satisfied;
            f.code = None;
            f.evidence_level = Some(level);
            f
        }
        Some(false) => {
            f.outcome = Outcome::NotSatisfied;
            f.code = Some(mismatch_code(r, &value));
            f
        }
    }
}

/// `Some(true)` satisfied, `Some(false)` not, `None` type mismatch.
fn compare(r: &Requirement, value: &Value, input: &Input<'_>) -> Option<bool> {
    let dim = r.dimension.as_str();
    let ordinal = |v: &Value| -> Option<u64> {
        match v {
            Value::Number(n) => Some(*n),
            Value::Token(t) => ordered_rank(dim, t),
            _ => None,
        }
    };
    match (&r.constraint, value) {
        (Constraint::Equals(want), got) => {
            if std::mem::discriminant(want) == std::mem::discriminant(got) {
                Some(want == got)
            } else {
                None
            }
        }
        (Constraint::OneOf(set), Value::Token(t)) => Some(set.contains(t)),
        (Constraint::OneOf(set), Value::Set(s)) => Some(!set.is_disjoint(s)),
        (Constraint::NoneOf(set), Value::Token(t)) => Some(!set.contains(t)),
        (Constraint::NoneOf(set), Value::Set(s)) => Some(set.is_disjoint(s)),
        (Constraint::AtLeast(bound), got) => {
            let (b, g) = (ordinal(bound)?, ordinal(got)?);
            Some(g >= b)
        }
        (Constraint::AtMost(bound), got) => {
            if dim == dimension::COMMERCIAL_PRICE {
                // A price cap applies only in its own unit (§18.2).
                let unit = input
                    .advertisement
                    .claims
                    .get(dimension::COMMERCIAL_PRICE_UNIT)
                    .map(|c| &c.value);
                match (unit, r.price_unit.as_deref()) {
                    (Some(Value::Token(u)), Some(want)) if u == want => {}
                    _ => return None,
                }
            }
            let (b, g) = (ordinal(bound)?, ordinal(got)?);
            Some(g <= b)
        }
        (Constraint::ContainsAll(set), Value::Set(s)) => Some(set.is_subset(s)),
        (Constraint::SubsetOf(set), Value::Set(s)) => Some(s.is_subset(set)),
        (Constraint::IsTrue, Value::Bool(b)) => Some(*b),
        (Constraint::ContainsAddress(a), Value::Addresses(s)) => Some(s.contains(a)),
        _ => None,
    }
}

fn mismatch_code(r: &Requirement, value: &Value) -> Code {
    let d = r.dimension.as_str();
    if d == dimension::PROVIDER_ID {
        return match r.constraint {
            Constraint::NoneOf(_) => Code::ProviderDenied,
            _ => Code::ProviderNotAllowed,
        };
    }
    if d == dimension::CONFIDENTIAL_EXECUTION {
        return Code::ConfidentialityNotClaimed;
    }
    if d == dimension::VERIFICATION_RESULT {
        return match value {
            Value::Token(t) if t == "NONE" => Code::VerificationStrengthUnavailable,
            _ => Code::ConstraintNotSatisfied,
        };
    }
    if d == dimension::COMMERCIAL_PRICE {
        return Code::PriceAboveMax;
    }
    if d == dimension::COMMERCIAL_LATENCY_MS {
        return Code::LatencyAboveMax;
    }
    if d == dimension::REPUTATION_SCORE {
        return Code::ReputationBelowMin;
    }
    match d.split('.').next().unwrap_or_default() {
        "jurisdiction" => Code::JurisdictionNotAllowed,
        "residency" => Code::ResidencyNotSatisfied,
        "retention" => Code::RetentionTooLong,
        "egress" => Code::EgressNotAllowed,
        "hardware" => Code::HardwareCapabilityMismatch,
        "runtime" | "model" => Code::RuntimeCapabilityMismatch,
        "availability" => Code::AvailabilityMismatch,
        "x" => Code::ExtensionMismatch,
        _ => Code::ConstraintNotSatisfied,
    }
}

/// The four profile templates of §12–§15, as fragments a policy author
/// composes with its other requirements. Non-normative shapes of the
/// document's tables; a deployment may write its own.
pub mod templates {
    use super::{
        compose, dimension, Constraint, EffectivePolicy, EvidenceLevel, PolicyError,
        PolicyFragment, Requirement, Source, Value,
    };
    use std::collections::BTreeSet;

    /// PUBLIC (§12): no requirement beyond the executor gate and
    /// compatibility, which the evaluator applies to every profile.
    pub fn public(source: Source) -> PolicyFragment {
        PolicyFragment {
            source,
            requirements: Vec::new(),
        }
    }

    /// One of the three strengthenings VERIFIED may use (§13).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Strengthening {
        /// `identity.provider_id` at `Certified` or better, among these.
        CertifiedIdentity(BTreeSet<String>),
        /// `reputation.score` at least this (control-plane-derived).
        Reputation(u64),
        /// `verification.result` one of these named classes (unsatisfiable
        /// today, #52).
        ResultVerification(BTreeSet<String>),
    }

    /// VERIFIED (§13): PUBLIC plus at least one strengthening. An empty
    /// list is `POLICY_INVALID` — VERIFIED must not collapse into PUBLIC.
    pub fn verified(
        source: Source,
        strengthenings: &[Strengthening],
    ) -> Result<PolicyFragment, PolicyError> {
        if strengthenings.is_empty() {
            return Err(PolicyError::Invalid(
                "VERIFIED template with no strengthening".into(),
            ));
        }
        let mut requirements = Vec::new();
        for (i, s) in strengthenings.iter().enumerate() {
            let id = format!("verified.{i}");
            requirements.push(match s {
                Strengthening::CertifiedIdentity(set) => Requirement::hard(
                    &id,
                    source,
                    dimension::PROVIDER_ID,
                    Constraint::OneOf(set.clone()),
                )
                .with_evidence(EvidenceLevel::Certified),
                Strengthening::Reputation(n) => Requirement::hard(
                    &id,
                    source,
                    dimension::REPUTATION_SCORE,
                    Constraint::AtLeast(Value::Number(*n)),
                ),
                Strengthening::ResultVerification(set) => Requirement::hard(
                    &id,
                    source,
                    dimension::VERIFICATION_RESULT,
                    Constraint::OneOf(set.clone()),
                ),
            });
        }
        Ok(PolicyFragment {
            source,
            requirements,
        })
    }

    /// CONFIDENTIAL (§14): confidential execution claimed, at `Attested`,
    /// session-fresh. The acceptable-root policy K evaluates is outside this
    /// evaluator; its verdict arrives as the evidence record on the claim.
    pub fn confidential(source: Source) -> PolicyFragment {
        PolicyFragment {
            source,
            requirements: vec![Requirement::hard(
                "confidential.execution",
                source,
                dimension::CONFIDENTIAL_EXECUTION,
                Constraint::IsTrue,
            )
            .with_evidence(EvidenceLevel::Attested)
            .session_fresh()],
        }
    }

    /// Location requirements for SOVEREIGN (§15), each at `Certified`.
    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct Locations {
        /// `jurisdiction.provider` one of.
        pub provider: Option<BTreeSet<String>>,
        /// `jurisdiction.worker_location` one of.
        pub worker_location: Option<BTreeSet<String>>,
        /// `residency.data` one of.
        pub data: Option<BTreeSet<String>>,
        /// `residency.key` one of.
        pub key: Option<BTreeSet<String>>,
        /// `residency.result_storage` one of.
        pub result_storage: Option<BTreeSet<String>>,
    }

    /// SOVEREIGN (§15): a base (CONFIDENTIAL or VERIFIED) plus certified
    /// location requirements and, optionally, egress at most a mode.
    pub fn sovereign(
        base: PolicyFragment,
        locations: &Locations,
        egress_at_most: Option<&str>,
    ) -> Result<PolicyFragment, PolicyError> {
        let source = base.source;
        let mut requirements = base.requirements;
        let mut push = |id: &str, dim: &str, set: &Option<BTreeSet<String>>| {
            if let Some(set) = set {
                requirements.push(
                    Requirement::hard(id, source, dim, Constraint::OneOf(set.clone()))
                        .with_evidence(EvidenceLevel::Certified),
                );
            }
        };
        push(
            "sovereign.jurisdiction.provider",
            dimension::JURISDICTION_PROVIDER,
            &locations.provider,
        );
        push(
            "sovereign.jurisdiction.worker_location",
            dimension::JURISDICTION_WORKER_LOCATION,
            &locations.worker_location,
        );
        push(
            "sovereign.residency.data",
            dimension::RESIDENCY_DATA,
            &locations.data,
        );
        push(
            "sovereign.residency.key",
            dimension::RESIDENCY_KEY,
            &locations.key,
        );
        push(
            "sovereign.residency.result_storage",
            dimension::RESIDENCY_RESULT_STORAGE,
            &locations.result_storage,
        );
        if requirements.iter().all(|r| !r.id.starts_with("sovereign.")) {
            return Err(PolicyError::Invalid(
                "SOVEREIGN template with no location requirement".into(),
            ));
        }
        if let Some(mode) = egress_at_most {
            requirements.push(Requirement::hard(
                "sovereign.egress.mode",
                source,
                dimension::EGRESS_MODE,
                Constraint::AtMost(Value::Token(mode.to_owned())),
            ));
        }
        let fragment = PolicyFragment {
            source,
            requirements,
        };
        // Validate as a policy in its own right so a malformed template
        // fails at construction, not at evaluation.
        let _: EffectivePolicy = compose(std::slice::from_ref(&fragment))?;
        Ok(fragment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_ladder_is_ordered() {
        assert!(EvidenceLevel::Claim < EvidenceLevel::ControlPlaneVerified);
        assert!(EvidenceLevel::ControlPlaneVerified < EvidenceLevel::Certified);
        assert!(EvidenceLevel::Certified < EvidenceLevel::Attested);
        assert!(EvidenceLevel::Attested < EvidenceLevel::Proven);
    }

    #[test]
    fn ordered_tokens_follow_the_declared_order() {
        assert_eq!(
            ordered_rank(dimension::RETENTION_CLASS, "EPHEMERAL"),
            Some(0)
        );
        assert_eq!(
            ordered_rank(dimension::EGRESS_MODE, "UNRESTRICTED"),
            Some(2)
        );
        assert_eq!(
            ordered_rank(dimension::EGRESS_MODE, "none"),
            None,
            "no case folding"
        );
        assert_eq!(ordered_rank(dimension::HARDWARE_MEMORY_BYTES, "x"), None);
    }

    #[test]
    fn source_order_is_fixed() {
        assert!(Source::DataOwner < Source::Organization);
        assert!(Source::Organization < Source::ControlPlane);
        assert!(Source::ControlPlane < Source::Provider);
    }
}
