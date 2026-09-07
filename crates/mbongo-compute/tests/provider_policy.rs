//! Mbongo Compute Provider Policy: the named test group for the reference
//! evaluator of `docs/architecture/compute-provider-capability-policy.md`
//! (`provider-policy-v1`). Drives the decision matrix of §28 and the ★
//! invariants of §27. No clock, no sleep: `now` is an argument.

use std::collections::{BTreeMap, BTreeSet};

use mbongo_compute::identity::SessionId;
use mbongo_compute::policy::templates::{self, Locations, Strengthening};
use mbongo_compute::policy::{
    compose, dimension as d, eligible, Advertisement, Binding, Claim, Code, Constraint,
    DerivedFacts, Evidence, EvidenceLevel, ExecutorGate, Input, Kind, Outcome, PolicyError,
    PolicyFragment, Requirement, Scope, SessionFacts, Source, TaskFacts, Value,
};
use mbongo_core::Address;

const NOW: u64 = 1_000_000;
const PROFILE: &str = "mbongo-ref:reverse-bytes:v1";
const REPR: &str = "app:bytes:v1";

fn addr(b: u8) -> Address {
    Address([b; 32])
}
fn sid(b: u8) -> SessionId {
    SessionId([b; 32])
}
fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}
fn tok(s: &str) -> Value {
    Value::Token(s.to_owned())
}

fn task(executor: Address) -> TaskFacts {
    TaskFacts {
        executor,
        profile_tag: PROFILE.into(),
        representation_tag: REPR.into(),
    }
}

/// A session that proved `executor`, with the REQUIRED_NOW claims.
fn session(executor: Address, id: SessionId, class: Option<&str>) -> SessionFacts {
    let mut claims = BTreeMap::new();
    claims.insert(
        d::CONTRACT_VERSION.to_owned(),
        Claim::plain(tok("provider-policy-v1"), Scope::Session),
    );
    claims.insert(
        d::REPRESENTATION_TAGS.to_owned(),
        Claim::plain(Value::Set(set(&[REPR])), Scope::Session),
    );
    claims.insert(
        d::PROFILE_TAGS.to_owned(),
        Claim::plain(Value::Set(set(&[PROFILE])), Scope::Session),
    );
    claims.insert(
        d::CONFIDENTIAL_EXECUTION.to_owned(),
        Claim::plain(Value::Bool(false), Scope::Session),
    );
    SessionFacts {
        session_id: id,
        proved_executor: executor,
        worker_class: class.map(str::to_owned),
        claims,
    }
}

/// An ordinary provider "P" operating `executors`.
fn advert(executors: &[Address]) -> Advertisement {
    Advertisement {
        provider_id: "P".into(),
        executors: executors.to_vec(),
        issued_at: NOW - 100,
        expires_at: NOW + 100,
        revoked: false,
        claims: BTreeMap::new(),
        worker_classes: BTreeMap::new(),
    }
}

fn evidence(level: EvidenceLevel, binding: Binding, expires_at: Option<u64>) -> Evidence {
    Evidence {
        level,
        issued_at: NOW - 10,
        expires_at,
        binding,
        verifier: "test-verifier".into(),
        reference: "root-ref".into(),
        revoked: false,
    }
}

fn policy(reqs: Vec<Requirement>) -> mbongo_compute::policy::EffectivePolicy {
    compose(&[PolicyFragment {
        source: Source::DataOwner,
        requirements: reqs,
    }])
    .expect("valid policy")
}

fn decide(
    t: &TaskFacts,
    s: &SessionFacts,
    a: &Advertisement,
    p: &mbongo_compute::policy::EffectivePolicy,
) -> mbongo_compute::policy::Decision {
    let derived = DerivedFacts::default();
    eligible(
        &Input {
            task: t,
            session: s,
            advertisement: a,
            derived: &derived,
            now: NOW,
        },
        p,
    )
}

// ---------------------------------------------------------------- §28 #1, #2

#[test]
fn public_task_with_correct_executor_and_ordinary_provider_is_eligible() {
    let a = addr(0xA1);
    let dec = decide(
        &task(a),
        &session(a, sid(1), None),
        &advert(&[a]),
        &policy(vec![]),
    );
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(dec.executor_gate, ExecutorGate::Passed);
    assert_eq!(
        dec.lowest_evidence,
        EvidenceLevel::Claim,
        "PUBLIC rests on claims only"
    );
    assert_eq!(dec.policy_version, "provider-policy-v1");
}

#[test]
fn wrong_executor_is_ineligible_and_nothing_else_is_evaluated() {
    let (a, b) = (addr(0xA1), addr(0xB2));
    // Provider Q proved B and has every capability; the task names A.
    let mut q = advert(&[b]);
    q.provider_id = "Q".into();
    q.claims.insert(
        d::JURISDICTION_PROVIDER.into(),
        Claim::plain(tok("CA"), Scope::Provider),
    );
    let p = policy(vec![Requirement::hard(
        "j",
        Source::DataOwner,
        d::JURISDICTION_PROVIDER,
        Constraint::OneOf(set(&["CA"])),
    )]);
    let dec = decide(&task(a), &session(b, sid(1), None), &q, &p);
    assert!(!dec.eligible);
    assert_eq!(dec.executor_gate, ExecutorGate::Mismatch);
    assert_eq!(dec.codes(), vec![Code::ExecutorMismatch]);
    assert!(
        dec.satisfied.is_empty()
            && dec.hard_failures.is_empty()
            && dec.soft.is_empty()
            && dec.unknown.is_empty()
    );
}

#[test]
fn executor_proved_but_absent_from_advertisement_fails_the_gate() {
    let (a, b) = (addr(0xA1), addr(0xB2));
    // The session proved A, but the advertisement under evaluation names only B.
    let dec = decide(
        &task(a),
        &session(a, sid(1), None),
        &advert(&[b]),
        &policy(vec![]),
    );
    assert_eq!(dec.codes(), vec![Code::ExecutorMismatch]);
}

// ---------------------------------------------------------- compatibility

#[test]
fn unsupported_profile_or_representation_is_ineligible() {
    let a = addr(0xA1);
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::PROFILE_TAGS.into(),
        Claim::plain(Value::Set(set(&["other:v9"])), Scope::Session),
    );
    let dec = decide(&task(a), &s, &advert(&[a]), &policy(vec![]));
    assert_eq!(dec.codes(), vec![Code::ProfileUnsupported]);

    let mut s = session(a, sid(1), None);
    s.claims.remove(d::REPRESENTATION_TAGS);
    let dec = decide(&task(a), &s, &advert(&[a]), &policy(vec![]));
    assert_eq!(dec.codes(), vec![Code::RepresentationUnsupported]);
}

#[test]
fn expired_or_revoked_advertisement_is_ineligible() {
    let a = addr(0xA1);
    let mut ad = advert(&[a]);
    ad.expires_at = NOW - 1;
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &policy(vec![])).codes(),
        vec![Code::AdvertisementExpired]
    );
    let mut ad = advert(&[a]);
    ad.revoked = true;
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &policy(vec![])).codes(),
        vec![Code::AdvertisementRevoked]
    );
}

// ------------------------------------------------------- §28 #8, #9, #14

#[test]
fn required_capability_missing_fails_closed() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "res",
        Source::DataOwner,
        d::RESIDENCY_DATA,
        Constraint::OneOf(set(&["CA"])),
    )]);
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    assert!(!dec.eligible);
    assert_eq!(dec.codes(), vec![Code::CapabilityUnknown]);
    assert_eq!(dec.hard_failures[0].outcome, Outcome::Unknown);
}

#[test]
fn unknown_jurisdiction_cannot_satisfy_an_explicit_jurisdiction_requirement() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "j",
        Source::DataOwner,
        d::JURISDICTION_WORKER_LOCATION,
        Constraint::OneOf(set(&["CA"])),
    )]);
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    assert!(!dec.eligible);
    assert_eq!(dec.codes(), vec![Code::CapabilityUnknown]);
}

#[test]
fn unknown_allowed_by_the_requirement_author_is_eligible_and_recorded() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "res",
        Source::DataOwner,
        d::RESIDENCY_DATA,
        Constraint::OneOf(set(&["CA"])),
    )
    .allow_unknown()]);
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(dec.unknown.len(), 1);
    assert!(dec.unknown[0].allowed);
}

#[test]
fn a_weaker_source_cannot_allow_unknown_for_a_stronger_requirement() {
    let a = addr(0xA1);
    let p = compose(&[
        PolicyFragment {
            source: Source::DataOwner,
            requirements: vec![Requirement::hard(
                "owner.res",
                Source::DataOwner,
                d::RESIDENCY_DATA,
                Constraint::OneOf(set(&["CA"])),
            )],
        },
        PolicyFragment {
            source: Source::Provider,
            requirements: vec![Requirement::hard(
                "prov.res",
                Source::Provider,
                d::RESIDENCY_DATA,
                Constraint::OneOf(set(&["CA"])),
            )
            .allow_unknown()],
        },
    ])
    .unwrap();
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    assert!(!dec.eligible);
    assert_eq!(dec.hard_failures.len(), 1);
    assert_eq!(dec.hard_failures[0].requirement_id, "owner.res");
    assert_eq!(
        dec.unknown.len(),
        1,
        "the provider's own relaxation applies only to its own requirement"
    );
}

// ------------------------------------------------------------- §28 #3

#[test]
fn verified_identity_requires_certified_evidence_not_a_claim() {
    let a = addr(0xA1);
    let frag = templates::verified(
        Source::DataOwner,
        &[Strengthening::CertifiedIdentity(set(&["P"]))],
    )
    .unwrap();
    let p = compose(&[frag]).unwrap();
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    assert!(!dec.eligible);
    assert_eq!(dec.codes(), vec![Code::InsufficientEvidence]);

    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::PROVIDER_ID.into(),
        Claim {
            value: tok("P"),
            scope: Scope::Provider,
            evidence: Some(evidence(EvidenceLevel::Certified, Binding::Provider, None)),
        },
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(dec.lowest_evidence, EvidenceLevel::Certified);
}

#[test]
fn verified_without_a_strengthening_is_invalid() {
    assert!(matches!(
        templates::verified(Source::DataOwner, &[]),
        Err(PolicyError::Invalid(_))
    ));
}

#[test]
fn result_verification_requirement_is_unsatisfiable_today() {
    let a = addr(0xA1);
    let frag = templates::verified(
        Source::DataOwner,
        &[Strengthening::ResultVerification(set(&["REDUNDANT"]))],
    )
    .unwrap();
    let p = compose(&[frag]).unwrap();
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::VERIFICATION_RESULT.into(),
        Claim::plain(tok("NONE"), Scope::Provider),
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    assert_eq!(dec.codes(), vec![Code::VerificationStrengthUnavailable]);
}

// ------------------------------------------------------- §28 #4, #5, #6

#[test]
fn confidential_claim_alone_does_not_satisfy_the_attestation_requirement() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim::plain(Value::Bool(true), Scope::Session),
    );
    let dec = decide(&task(a), &s, &advert(&[a]), &p);
    assert!(!dec.eligible);
    assert_eq!(dec.codes(), vec![Code::AttestationRequired]);
}

#[test]
fn reference_worker_does_not_claim_confidentiality_and_is_not_confidential_eligible() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    // `false` at level CLAIM against `min_evidence = ATTESTED`: the evidence
    // bar is checked before the value, so the reason is the missing
    // attestation, not the value.
    assert_eq!(dec.codes(), vec![Code::AttestationRequired]);
}

#[test]
fn certified_is_not_attested_for_confidential() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence(
                EvidenceLevel::Certified,
                Binding::Session(sid(1)),
                None,
            )),
        },
    );
    let dec = decide(&task(a), &s, &advert(&[a]), &p);
    assert_eq!(dec.codes(), vec![Code::AttestationRequired]);
}

#[test]
fn confidential_with_session_bound_attested_evidence_is_eligible() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence(
                EvidenceLevel::Attested,
                Binding::Challenge {
                    session: sid(1),
                    challenge_ref: "ch-1".into(),
                },
                Some(NOW + 60),
            )),
        },
    );
    let dec = decide(&task(a), &s, &advert(&[a]), &p);
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(dec.lowest_evidence, EvidenceLevel::Attested);
    assert_eq!(
        dec.satisfied[0].evidence_level,
        Some(EvidenceLevel::Attested)
    );
}

#[test]
fn attested_evidence_bound_to_another_session_is_not_fresh() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence(
                EvidenceLevel::Attested,
                Binding::Session(sid(2)),
                Some(NOW + 60),
            )),
        },
    );
    let dec = decide(&task(a), &s, &advert(&[a]), &p);
    assert_eq!(dec.codes(), vec![Code::EvidenceNotSessionBound]);
}

#[test]
fn expired_or_revoked_attested_evidence_cannot_satisfy_a_fresh_requirement() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence(
                EvidenceLevel::Attested,
                Binding::Session(sid(1)),
                Some(NOW - 1),
            )),
        },
    );
    assert_eq!(
        decide(&task(a), &s, &advert(&[a]), &p).codes(),
        vec![Code::EvidenceExpired]
    );

    let mut ev = evidence(
        EvidenceLevel::Attested,
        Binding::Session(sid(1)),
        Some(NOW + 60),
    );
    ev.revoked = true;
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(ev),
        },
    );
    assert_eq!(
        decide(&task(a), &s, &advert(&[a]), &p).codes(),
        vec![Code::EvidenceExpired]
    );
}

#[test]
fn provider_wide_evidence_does_not_substitute_for_session_freshness() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    // The provider attested "once, somewhere": a provider-scoped claim with
    // provider-bound ATTESTED evidence. The session itself claims nothing.
    let mut s = session(a, sid(1), None);
    s.claims.remove(d::CONFIDENTIAL_EXECUTION);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Provider,
            evidence: Some(evidence(EvidenceLevel::Attested, Binding::Provider, None)),
        },
    );
    let dec = decide(&task(a), &s, &ad, &p);
    assert_eq!(dec.codes(), vec![Code::EvidenceNotSessionBound]);
}

// ------------------------------------------------------------- §28 #7

#[test]
fn sovereign_with_wrong_worker_location_is_ineligible_despite_provider_jurisdiction() {
    let a = addr(0xA1);
    let base = templates::verified(
        Source::DataOwner,
        &[Strengthening::CertifiedIdentity(set(&["P"]))],
    )
    .unwrap();
    let locations = Locations {
        worker_location: Some(set(&["CA"])),
        data: Some(set(&["CA"])),
        ..Locations::default()
    };
    let frag = templates::sovereign(base, &locations, Some("NONE")).unwrap();
    let p = compose(&[frag]).unwrap();

    let certified = |v: &str| Claim {
        value: tok(v),
        scope: Scope::WorkerClass,
        evidence: Some(evidence(EvidenceLevel::Certified, Binding::Provider, None)),
    };
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::PROVIDER_ID.into(),
        Claim {
            value: tok("P"),
            scope: Scope::Provider,
            evidence: Some(evidence(EvidenceLevel::Certified, Binding::Provider, None)),
        },
    );
    ad.claims.insert(
        d::JURISDICTION_PROVIDER.into(),
        Claim {
            value: tok("CA"),
            scope: Scope::Provider,
            evidence: Some(evidence(EvidenceLevel::Certified, Binding::Provider, None)),
        },
    );
    let mut class = BTreeMap::new();
    class.insert(d::JURISDICTION_WORKER_LOCATION.to_owned(), certified("US"));
    class.insert(d::RESIDENCY_DATA.to_owned(), certified("CA"));
    class.insert(
        d::EGRESS_MODE.to_owned(),
        Claim::plain(tok("NONE"), Scope::WorkerClass),
    );
    ad.worker_classes.insert("pool-1".into(), class);

    let dec = decide(&task(a), &session(a, sid(1), Some("pool-1")), &ad, &p);
    assert!(!dec.eligible);
    assert_eq!(dec.codes(), vec![Code::JurisdictionNotAllowed]);
    assert_eq!(
        dec.hard_failures[0].dimension,
        d::JURISDICTION_WORKER_LOCATION
    );

    // Moving the workers to CA — still certified — makes it eligible.
    ad.worker_classes
        .get_mut("pool-1")
        .unwrap()
        .insert(d::JURISDICTION_WORKER_LOCATION.to_owned(), certified("CA"));
    let dec = decide(&task(a), &session(a, sid(1), Some("pool-1")), &ad, &p);
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(
        dec.lowest_evidence,
        EvidenceLevel::Claim,
        "egress is a plain claim; that is what this decision rests on"
    );
}

#[test]
fn sovereign_location_claim_without_certification_is_insufficient() {
    let a = addr(0xA1);
    let base = templates::confidential(Source::DataOwner);
    let frag = templates::sovereign(
        base,
        &Locations {
            data: Some(set(&["CA"])),
            ..Locations::default()
        },
        None,
    )
    .unwrap();
    let p = compose(&[frag]).unwrap();
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::RESIDENCY_DATA.into(),
        Claim::plain(tok("CA"), Scope::Provider),
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    let codes = dec.codes();
    assert!(codes.contains(&Code::InsufficientEvidence), "{codes:?}");
    assert!(codes.contains(&Code::AttestationRequired), "{codes:?}");
}

#[test]
fn sovereign_without_any_location_is_invalid() {
    let base = templates::confidential(Source::DataOwner);
    assert!(matches!(
        templates::sovereign(base, &Locations::default(), None),
        Err(PolicyError::Invalid(_))
    ));
}

// ---------------------------------------------- §28 #10–#15: hard caps

#[test]
fn retention_exceeding_the_maximum_is_ineligible() {
    let a = addr(0xA1);
    let p = policy(vec![
        Requirement::hard(
            "r.class",
            Source::DataOwner,
            d::RETENTION_CLASS,
            Constraint::AtMost(tok("BOUNDED")),
        ),
        Requirement::hard(
            "r.max",
            Source::DataOwner,
            d::RETENTION_MAX_SECONDS,
            Constraint::AtMost(Value::Number(3600)),
        ),
    ]);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::RETENTION_CLASS.into(),
        Claim::plain(tok("BOUNDED"), Scope::Provider),
    );
    ad.claims.insert(
        d::RETENTION_MAX_SECONDS.into(),
        Claim::plain(Value::Number(86_400), Scope::Provider),
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    assert_eq!(dec.codes(), vec![Code::RetentionTooLong]);
    assert_eq!(dec.satisfied.len(), 1);
}

#[test]
fn egress_violating_the_policy_is_ineligible() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "e",
        Source::DataOwner,
        d::EGRESS_MODE,
        Constraint::AtMost(tok("RESTRICTED")),
    )]);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::EGRESS_MODE.into(),
        Claim::plain(tok("UNRESTRICTED"), Scope::Provider),
    );
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &p).codes(),
        vec![Code::EgressNotAllowed]
    );
    ad.claims.insert(
        d::EGRESS_MODE.into(),
        Claim::plain(tok("NONE"), Scope::Provider),
    );
    assert!(decide(&task(a), &session(a, sid(1), None), &ad, &p).eligible);
}

#[test]
fn price_above_a_hard_cap_is_ineligible_and_a_unit_mismatch_is_unknown() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "price",
        Source::DataOwner,
        d::COMMERCIAL_PRICE,
        Constraint::AtMost(Value::Number(100)),
    )
    .in_unit("U")]);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::COMMERCIAL_PRICE.into(),
        Claim::plain(Value::Number(120), Scope::Provider),
    );
    ad.claims.insert(
        d::COMMERCIAL_PRICE_UNIT.into(),
        Claim::plain(tok("U"), Scope::Provider),
    );
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &p).codes(),
        vec![Code::PriceAboveMax]
    );

    ad.claims.insert(
        d::COMMERCIAL_PRICE.into(),
        Claim::plain(Value::Number(50), Scope::Provider),
    );
    ad.claims.insert(
        d::COMMERCIAL_PRICE_UNIT.into(),
        Claim::plain(tok("V"), Scope::Provider),
    );
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &p).codes(),
        vec![Code::CapabilityUnknown]
    );
}

#[test]
fn latency_above_a_hard_max_is_ineligible_but_a_soft_preference_is_only_recorded() {
    let a = addr(0xA1);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::COMMERCIAL_LATENCY_MS.into(),
        Claim::plain(Value::Number(250), Scope::Provider),
    );

    let hard = policy(vec![Requirement::hard(
        "lat",
        Source::DataOwner,
        d::COMMERCIAL_LATENCY_MS,
        Constraint::AtMost(Value::Number(100)),
    )]);
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &hard).codes(),
        vec![Code::LatencyAboveMax]
    );

    let soft = policy(vec![Requirement::soft(
        "lat",
        Source::DataOwner,
        d::COMMERCIAL_LATENCY_MS,
        Constraint::AtMost(Value::Number(50)),
    )]);
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &soft);
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(dec.soft.len(), 1);
    assert_eq!(dec.soft[0].outcome, Outcome::NotSatisfied);
    assert_eq!(dec.soft[0].code, Some(Code::LatencyAboveMax));
}

#[test]
fn soft_preferences_cannot_override_a_hard_failure() {
    let a = addr(0xA1);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::COMMERCIAL_LATENCY_MS.into(),
        Claim::plain(Value::Number(5), Scope::Provider),
    );
    let p = compose(&[PolicyFragment {
        source: Source::DataOwner,
        requirements: vec![
            Requirement::hard(
                "res",
                Source::DataOwner,
                d::RESIDENCY_DATA,
                Constraint::OneOf(set(&["CA"])),
            ),
            Requirement::soft(
                "lat",
                Source::DataOwner,
                d::COMMERCIAL_LATENCY_MS,
                Constraint::AtMost(Value::Number(50)),
            ),
        ],
    }])
    .unwrap();
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    assert!(!dec.eligible);
    assert_eq!(
        dec.soft[0].outcome,
        Outcome::Satisfied,
        "the soft preference is met — and changes nothing"
    );
    assert_eq!(dec.codes(), vec![Code::CapabilityUnknown]);
}

#[test]
fn hard_constraints_are_evaluated_before_soft_and_completely() {
    let a = addr(0xA1);
    let p = compose(&[PolicyFragment {
        source: Source::DataOwner,
        requirements: vec![
            Requirement::hard(
                "a.res",
                Source::DataOwner,
                d::RESIDENCY_DATA,
                Constraint::OneOf(set(&["CA"])),
            ),
            Requirement::hard(
                "b.jur",
                Source::DataOwner,
                d::JURISDICTION_PROVIDER,
                Constraint::OneOf(set(&["CA"])),
            ),
            Requirement::soft(
                "z.lat",
                Source::DataOwner,
                d::COMMERCIAL_LATENCY_MS,
                Constraint::AtMost(Value::Number(50)),
            ),
        ],
    }])
    .unwrap();
    let dec = decide(&task(a), &session(a, sid(1), None), &advert(&[a]), &p);
    assert_eq!(
        dec.hard_failures.len(),
        2,
        "every hard requirement is reported, not just the first"
    );
    assert_eq!(dec.hard_failures[0].requirement_id, "a.res");
    assert_eq!(dec.hard_failures[1].requirement_id, "b.jur");
    assert_eq!(dec.soft.len(), 1);
}

// -------------------------------------------------- §8: composition

#[test]
fn a_provider_fragment_cannot_weaken_a_data_owner_requirement() {
    let a = addr(0xA1);
    let p = compose(&[
        PolicyFragment {
            source: Source::Provider,
            requirements: vec![Requirement::hard(
                "prov.loc",
                Source::Provider,
                d::JURISDICTION_WORKER_LOCATION,
                Constraint::OneOf(set(&["CA", "US"])),
            )],
        },
        PolicyFragment {
            source: Source::DataOwner,
            requirements: vec![Requirement::hard(
                "owner.loc",
                Source::DataOwner,
                d::JURISDICTION_WORKER_LOCATION,
                Constraint::OneOf(set(&["CA"])),
            )],
        },
    ])
    .unwrap();
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::JURISDICTION_WORKER_LOCATION.into(),
        Claim::plain(tok("US"), Scope::Provider),
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    assert!(!dec.eligible);
    assert_eq!(dec.hard_failures.len(), 1);
    assert_eq!(dec.hard_failures[0].requirement_id, "owner.loc");
    assert_eq!(dec.satisfied[0].requirement_id, "prov.loc");
}

#[test]
fn a_client_fragment_cannot_weaken_an_organization_requirement() {
    let a = addr(0xA1);
    let p = compose(&[
        PolicyFragment {
            source: Source::Organization,
            requirements: vec![Requirement::hard(
                "org.ret",
                Source::Organization,
                d::RETENTION_MAX_SECONDS,
                Constraint::AtMost(Value::Number(60)),
            )],
        },
        PolicyFragment {
            source: Source::DataOwner,
            requirements: vec![Requirement::hard(
                "owner.ret",
                Source::DataOwner,
                d::RETENTION_MAX_SECONDS,
                Constraint::AtMost(Value::Number(86_400)),
            )],
        },
    ])
    .unwrap();
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::RETENTION_MAX_SECONDS.into(),
        Claim::plain(Value::Number(3600), Scope::Provider),
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    assert!(!dec.eligible);
    assert_eq!(dec.hard_failures[0].requirement_id, "org.ret");
}

#[test]
fn contradictory_hard_requirements_are_a_policy_conflict_never_a_tie_break() {
    let r = compose(&[
        PolicyFragment {
            source: Source::Organization,
            requirements: vec![Requirement::hard(
                "org",
                Source::Organization,
                d::JURISDICTION_PROVIDER,
                Constraint::OneOf(set(&["CA"])),
            )],
        },
        PolicyFragment {
            source: Source::ControlPlane,
            requirements: vec![Requirement::hard(
                "cp",
                Source::ControlPlane,
                d::JURISDICTION_PROVIDER,
                Constraint::OneOf(set(&["US"])),
            )],
        },
    ]);
    assert!(matches!(r, Err(PolicyError::Conflict { .. })), "{r:?}");

    let r = compose(&[PolicyFragment {
        source: Source::DataOwner,
        requirements: vec![
            Requirement::hard(
                "lo",
                Source::DataOwner,
                d::RETENTION_MAX_SECONDS,
                Constraint::AtLeast(Value::Number(60)),
            ),
            Requirement::hard(
                "hi",
                Source::DataOwner,
                d::RETENTION_MAX_SECONDS,
                Constraint::AtMost(Value::Number(0)),
            ),
        ],
    }]);
    assert!(matches!(r, Err(PolicyError::Conflict { .. })), "{r:?}");
}

#[test]
fn fragment_order_does_not_change_the_effective_policy_or_the_decision() {
    let a = addr(0xA1);
    let f1 = PolicyFragment {
        source: Source::DataOwner,
        requirements: vec![Requirement::hard(
            "owner.loc",
            Source::DataOwner,
            d::JURISDICTION_WORKER_LOCATION,
            Constraint::OneOf(set(&["CA"])),
        )],
    };
    let f2 = PolicyFragment {
        source: Source::ControlPlane,
        requirements: vec![
            Requirement::hard(
                "cp.price",
                Source::ControlPlane,
                d::COMMERCIAL_PRICE,
                Constraint::AtMost(Value::Number(100)),
            )
            .in_unit("U"),
            Requirement::soft(
                "cp.lat",
                Source::ControlPlane,
                d::COMMERCIAL_LATENCY_MS,
                Constraint::AtMost(Value::Number(50)),
            ),
        ],
    };
    let p1 = compose(&[f1.clone(), f2.clone()]).unwrap();
    let p2 = compose(&[f2, f1]).unwrap();
    assert_eq!(p1, p2);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::JURISDICTION_WORKER_LOCATION.into(),
        Claim::plain(tok("CA"), Scope::Provider),
    );
    ad.claims.insert(
        d::COMMERCIAL_PRICE.into(),
        Claim::plain(Value::Number(90), Scope::Provider),
    );
    ad.claims.insert(
        d::COMMERCIAL_PRICE_UNIT.into(),
        Claim::plain(tok("U"), Scope::Provider),
    );
    let s = session(a, sid(1), None);
    let d1 = decide(&task(a), &s, &ad, &p1);
    let d2 = decide(&task(a), &s, &ad, &p2);
    assert_eq!(d1, d2);
    assert!(d1.eligible);
}

#[test]
fn identical_inputs_give_identical_decisions() {
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence(
                EvidenceLevel::Attested,
                Binding::Session(sid(1)),
                Some(NOW + 60),
            )),
        },
    );
    let ad = advert(&[a]);
    let first = decide(&task(a), &s, &ad, &p);
    for _ in 0..10 {
        assert_eq!(decide(&task(a), &s, &ad, &p), first);
    }
    // Time is an input: the same inputs at a later `now` are different inputs.
    let derived = DerivedFacts::default();
    let later = eligible(
        &Input {
            task: &task(a),
            session: &s,
            advertisement: &ad,
            derived: &derived,
            now: NOW + 90,
        },
        &p,
    );
    assert!(first.eligible && !later.eligible);
    assert_eq!(later.codes(), vec![Code::EvidenceExpired]);
}

// ------------------------------------------------- §5.4: reputation

#[test]
fn reputation_comes_from_the_control_plane_never_from_the_advertisement() {
    let a = addr(0xA1);
    let frag = templates::verified(Source::DataOwner, &[Strengthening::Reputation(80)]).unwrap();
    let p = compose(&[frag]).unwrap();
    let mut ad = advert(&[a]);
    // A self-claimed score is ignored entirely.
    ad.claims.insert(
        d::REPUTATION_SCORE.into(),
        Claim::plain(Value::Number(100), Scope::Provider),
    );
    let s = session(a, sid(1), None);
    let t = task(a);
    let none = DerivedFacts::default();
    let dec = eligible(
        &Input {
            task: &t,
            session: &s,
            advertisement: &ad,
            derived: &none,
            now: NOW,
        },
        &p,
    );
    assert_eq!(dec.codes(), vec![Code::CapabilityUnknown]);

    let mut derived = DerivedFacts::default();
    derived.values.insert(d::REPUTATION_SCORE.into(), Value::Number(50));
    let dec = eligible(
        &Input {
            task: &t,
            session: &s,
            advertisement: &ad,
            derived: &derived,
            now: NOW,
        },
        &p,
    );
    assert_eq!(dec.codes(), vec![Code::ReputationBelowMin]);

    derived.values.insert(d::REPUTATION_SCORE.into(), Value::Number(90));
    let dec = eligible(
        &Input {
            task: &t,
            session: &s,
            advertisement: &ad,
            derived: &derived,
            now: NOW,
        },
        &p,
    );
    assert!(dec.eligible);
    assert_eq!(dec.lowest_evidence, EvidenceLevel::ControlPlaneVerified);
}

#[test]
fn a_provider_fragment_may_not_constrain_reputation() {
    let r = compose(&[PolicyFragment {
        source: Source::Provider,
        requirements: vec![Requirement::hard(
            "p.rep",
            Source::Provider,
            d::REPUTATION_SCORE,
            Constraint::AtLeast(Value::Number(0)),
        )],
    }]);
    assert!(matches!(r, Err(PolicyError::Invalid(_))));
}

// ---------------------------------------------- §20: scope shadowing

#[test]
fn the_most_specific_scope_shadows_the_provider_wide_claim() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "mem",
        Source::DataOwner,
        d::HARDWARE_AVAILABLE_MEMORY_BYTES,
        Constraint::AtLeast(Value::Number(64 << 30)),
    )]);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::HARDWARE_AVAILABLE_MEMORY_BYTES.into(),
        Claim::plain(Value::Number(80 << 30), Scope::Provider),
    );
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::HARDWARE_AVAILABLE_MEMORY_BYTES.into(),
        Claim::plain(Value::Number(8 << 30), Scope::Session),
    );
    let dec = decide(&task(a), &s, &ad, &p);
    assert_eq!(dec.codes(), vec![Code::HardwareCapabilityMismatch]);
}

#[test]
fn a_type_mismatch_is_unknown_never_a_coercion() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "mem",
        Source::DataOwner,
        d::HARDWARE_MEMORY_BYTES,
        Constraint::AtLeast(Value::Number(1)),
    )]);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::HARDWARE_MEMORY_BYTES.into(),
        Claim::plain(tok("80 GiB"), Scope::Provider),
    );
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &p).codes(),
        vec![Code::CapabilityUnknown]
    );
}

#[test]
fn tokens_compare_byte_for_byte_without_case_folding() {
    let a = addr(0xA1);
    let p = policy(vec![Requirement::hard(
        "j",
        Source::DataOwner,
        d::JURISDICTION_PROVIDER,
        Constraint::OneOf(set(&["CA"])),
    )]);
    let mut ad = advert(&[a]);
    ad.claims.insert(
        d::JURISDICTION_PROVIDER.into(),
        Claim::plain(tok("ca"), Scope::Provider),
    );
    assert_eq!(
        decide(&task(a), &session(a, sid(1), None), &ad, &p).codes(),
        vec![Code::JurisdictionNotAllowed]
    );
}

// ---------------------------------------------- §24: safe decisions

#[test]
fn a_decision_carries_no_claim_or_policy_values() {
    let a = addr(0xA1);
    const CLAIM_VALUE: &str = "CLAIM-VALUE-7f3a9c";
    const POLICY_VALUE: &str = "POLICY-VALUE-51e2bd";
    const EVIDENCE_REF: &str = "EVIDENCE-REF-0c44a1";
    let p = policy(vec![
        Requirement::hard(
            "j",
            Source::DataOwner,
            d::JURISDICTION_PROVIDER,
            Constraint::OneOf(set(&[POLICY_VALUE])),
        )
        .with_evidence(EvidenceLevel::Certified),
        Requirement::soft(
            "lat",
            Source::DataOwner,
            d::COMMERCIAL_LATENCY_MS,
            Constraint::AtMost(Value::Number(424_242)),
        ),
    ]);
    let mut ad = advert(&[a]);
    let mut ev = evidence(EvidenceLevel::Certified, Binding::Provider, None);
    ev.reference = EVIDENCE_REF.into();
    ad.claims.insert(
        d::JURISDICTION_PROVIDER.into(),
        Claim {
            value: tok(CLAIM_VALUE),
            scope: Scope::Provider,
            evidence: Some(ev),
        },
    );
    ad.claims.insert(
        d::COMMERCIAL_LATENCY_MS.into(),
        Claim::plain(Value::Number(999_999), Scope::Provider),
    );
    let dec = decide(&task(a), &session(a, sid(1), None), &ad, &p);
    let rendered = format!("{dec:?}");
    for forbidden in [CLAIM_VALUE, POLICY_VALUE, EVIDENCE_REF, "424242", "999999"] {
        assert!(
            !rendered.contains(forbidden),
            "decision leaked {forbidden}: {rendered}"
        );
    }
    assert!(
        rendered.contains("JurisdictionNotAllowed") && rendered.contains(d::JURISDICTION_PROVIDER)
    );
}

// ---------------------------------------------- §22.3: the K handoff

#[test]
fn k_handoff_the_key_release_precondition_is_expressible_from_the_decision() {
    // What K's release_decision needs from J: eligible, and every
    // requirement at or above ATTESTED satisfied at or above ATTESTED.
    fn release(
        dec: &mbongo_compute::policy::Decision,
        policy: &mbongo_compute::policy::EffectivePolicy,
    ) -> bool {
        dec.eligible
            && policy.hard().filter(|r| r.min_evidence >= EvidenceLevel::Attested).all(|r| {
                dec.satisfied.iter().any(|f| {
                    f.requirement_id == r.id
                        && f.evidence_level.is_some_and(|l| l >= EvidenceLevel::Attested)
                })
            })
    }
    let a = addr(0xA1);
    let p = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let ad = advert(&[a]);
    let mut s = session(a, sid(1), None);
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim::plain(Value::Bool(true), Scope::Session),
    );
    assert!(
        !release(&decide(&task(a), &s, &ad, &p), &p),
        "claim only: withhold"
    );
    s.claims.insert(
        d::CONFIDENTIAL_EXECUTION.into(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence(
                EvidenceLevel::Attested,
                Binding::Challenge {
                    session: sid(1),
                    challenge_ref: "ch".into(),
                },
                Some(NOW + 30),
            )),
        },
    );
    assert!(
        release(&decide(&task(a), &s, &ad, &p), &p),
        "attested, fresh, bound: release"
    );
    // And a PUBLIC decision, which needs nothing, never needs a key-release gate to be false either:
    assert!(release(
        &decide(&task(a), &s, &ad, &policy(vec![])),
        &policy(vec![])
    ));
}

#[test]
fn hard_and_soft_kinds_are_preserved_through_composition() {
    let p = compose(&[PolicyFragment {
        source: Source::ControlPlane,
        requirements: vec![
            Requirement::soft(
                "s",
                Source::ControlPlane,
                d::COMMERCIAL_LATENCY_MS,
                Constraint::AtMost(Value::Number(1)),
            ),
            Requirement::hard(
                "h",
                Source::ControlPlane,
                d::COMMERCIAL_LATENCY_MS,
                Constraint::AtMost(Value::Number(10)),
            ),
        ],
    }])
    .unwrap();
    assert_eq!(p.hard().count(), 1);
    assert_eq!(p.soft().len(), 1);
    assert_eq!(p.soft()[0].kind, Kind::Soft);
}
