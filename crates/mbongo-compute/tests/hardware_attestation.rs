//! Mbongo Compute Hardware Attestation: the named test group for
//! `hardware-attestation-v1` / `snp-adapter-v1`.
//!
//! Three layers, kept apart on purpose:
//!
//! 1. **Captured real evidence** (`test-vectors/attestation/snp/`): a real
//!    AMD EPYC Milan report and its VCEK, published by Google under
//!    Apache-2.0. These prove parsing, the real VCEK → ASK → ARK chain
//!    against the pinned AMD root, signature tamper rejection, real platform
//!    policy rejection (the report allows debug), and — decisively — that
//!    real evidence whose `REPORT_DATA` was not chosen for a Mbongo
//!    challenge is **refused as unbound**. Parsing is not attestation.
//! 2. **Stage-2 unit tests** on an *unsigned* copy of the same report whose
//!    `REPORT_DATA` is set to a Mbongo binding: binding, platform policy and
//!    normalization logic, in isolation from the chain. These are mocks by
//!    construction and prove nothing about hardware.
//! 3. **Live tests** (`#[ignore]`): run only on a SEV-SNP guest through
//!    `snp_live_test`; reported as ignored, never as passed, elsewhere.
//!
//! Negative-control verifiers (signature-, binding- and measurement-blind)
//! must accept what the strict adapter rejects, or the group is vacuous.

use std::collections::{BTreeMap, BTreeSet};

use mbongo_compute::confidential::reference::{
    ReferenceAttestationRoot, ReferenceAttestationVerifier, REFERENCE_FORMAT,
};
use mbongo_compute::confidential::snp::negative::{
    BindingBlindSnpVerifier, MeasurementBlindSnpVerifier, SignatureBlindSnpVerifier,
};
use mbongo_compute::confidential::snp::{
    check_platform_policy, parse_report, report_data_binding, verify_chain_and_signature,
    SnpEvidence, SnpPolicy, SnpProduct, SnpVerifier, TcbMinimum, HARDWARE_ATTESTATION_VERSION,
    SNP_ADAPTER_VERSION, SNP_ENVIRONMENT_TYPE, SNP_FORMAT, SNP_REPORT_LEN, SNP_VERIFIER_KIND,
};
use mbongo_compute::confidential::{
    seal_input, to_policy_evidence, unwrap_key, wrap_key, AttestationChallenge,
    AttestationVerifier, ChallengeId, ContentKey, EnvironmentKey, EvidenceEnvelope,
    MeasurementPolicy, Purpose, ReleaseId, SecurityLabel, TrustPolicy, VerifyError, WrapBinding,
};
use mbongo_compute::identity::SessionId;
use mbongo_compute::policy::{
    self, compose, dimension, templates, Advertisement, Claim, DerivedFacts, EvidenceLevel, Scope,
    SessionFacts, Source, TaskFacts, Value,
};
use mbongo_core::Address;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../test-vectors/attestation/snp"
);
const NOW: u64 = 1_000;
/// Byte offset of `REPORT_DATA` in an SEV-SNP report (ABI §7.3, table 23).
const REPORT_DATA_OFFSET: usize = 0x50;
/// Byte offset of `MEASUREMENT`.
const MEASUREMENT_OFFSET: usize = 0x90;
/// Byte offset of `SIGNATURE`.
const SIGNATURE_OFFSET: usize = 0x2A0;

fn read(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn real_report() -> Vec<u8> {
    let r = read("milan-go-sev-guest/attestation-report.bin");
    assert_eq!(r.len(), SNP_REPORT_LEN);
    r
}

fn real_vcek() -> Vec<u8> {
    read("milan-go-sev-guest/vcek.der")
}

fn other_chip_vcek() -> Vec<u8> {
    read("vcek-milan-other-chip-virtee.der")
}

fn env_key() -> EnvironmentKey {
    EnvironmentKey::from_bytes([0x5Eu8; 32])
}

fn challenge(session: u8, executor: u8, task: u8) -> AttestationChallenge {
    AttestationChallenge {
        challenge_id: ChallengeId([0xC1u8; 32]),
        nonce: [0x4Eu8; 32],
        session_id: SessionId([session; 32]),
        executor: Address([executor; 32]),
        task_id: [task; 32],
        purpose: Purpose::ConfidentialInputRelease,
        issued_at: NOW - 10,
        expires_at: NOW + 50,
    }
}

fn evidence(report: Vec<u8>, vcek: Vec<u8>) -> SnpEvidence {
    SnpEvidence {
        product: SnpProduct::Milan,
        report,
        vcek_der: vcek,
        environment_key: env_key().public(),
    }
}

fn envelope(ev: &SnpEvidence, c: &AttestationChallenge) -> EvidenceEnvelope {
    EvidenceEnvelope {
        format: SNP_FORMAT.into(),
        verifier_kind: SNP_VERIFIER_KIND.into(),
        challenge_id: c.challenge_id,
        session_id: c.session_id,
        executor: c.executor,
        issued_at: NOW - 5,
        expires_at: NOW + 60,
        payload: ev.to_payload(),
    }
}

/// The real report's measurement digest, as K sees it.
fn real_measurement_digest() -> [u8; 32] {
    let r = parse_report(&real_report()).unwrap();
    *blake3::hash(&r.measurement).as_bytes()
}

fn trust(allow_measurement: bool) -> TrustPolicy {
    let mut anchors = BTreeMap::new();
    anchors.insert(
        SnpProduct::Milan.anchor_id().to_owned(),
        SnpProduct::Milan.trust_anchor(),
    );
    let allowed: BTreeSet<[u8; 32]> = if allow_measurement {
        [real_measurement_digest()].into_iter().collect()
    } else {
        BTreeSet::new()
    };
    TrustPolicy {
        accepted_formats: [SNP_FORMAT.to_owned()].into_iter().collect(),
        anchors,
        measurement: MeasurementPolicy {
            allowed,
            revoked: BTreeSet::new(),
            min_security_version: 0,
            allow_debug: true,
        },
        max_evidence_age_secs: 300,
        challenge_ttl_secs: 60,
        release_ttl_secs: 60,
    }
}

/// A policy that the captured report can pass stage 2 under: debug and SMT
/// allowed (the fixture has both), certificate validity not enforced (the
/// clock is not wall time). Explicitly **not** production grade.
fn lenient() -> SnpPolicy {
    let mut p = SnpPolicy::production(SnpProduct::Milan, TcbMinimum::default(), 0);
    p.allow_debug = true;
    p.allow_smt = true;
    p.enforce_certificate_validity = false;
    p
}

/// The real report with `REPORT_DATA` replaced by a Mbongo binding. Its
/// signature is then invalid: a stage-2 mock, never a stage-1 fixture.
fn bound_copy(c: &AttestationChallenge, key: &[u8; 32]) -> Vec<u8> {
    let mut r = real_report();
    r[REPORT_DATA_OFFSET..REPORT_DATA_OFFSET + 64].copy_from_slice(&report_data_binding(c, key));
    r
}

fn stage2(
    report: &[u8],
    ev: &SnpEvidence,
    c: &AttestationChallenge,
    policy: &SnpPolicy,
    t: &TrustPolicy,
    now: u64,
) -> Result<mbongo_compute::confidential::VerifiedAttestation, VerifyError> {
    let parsed = parse_report(report).unwrap();
    let vcek = sev::certs::snp::Certificate::from_der(&ev.vcek_der).unwrap();
    check_platform_policy(&parsed, &vcek, ev, &envelope(ev, c), c, policy, t, now)
}

// ── layer 1: captured real evidence ───────────────────────────────────────

#[test]
fn hw01_real_milan_report_parses_and_chains_to_the_pinned_amd_root() {
    let ev = evidence(real_report(), real_vcek());
    let (report, _vcek) =
        verify_chain_and_signature(&ev, &trust(true)).expect("real chain and signature");
    assert_eq!(report.version, 2);
    assert_eq!(report.vmpl, 0);
    assert!(
        report.policy.debug_allowed(),
        "the captured guest allowed debug"
    );
    assert!(!report.policy.migrate_ma_allowed());
    assert_eq!(
        (
            report.reported_tcb.bootloader,
            report.reported_tcb.tee,
            report.reported_tcb.snp,
            report.reported_tcb.microcode
        ),
        (2, 0, 5, 68)
    );
    assert_eq!(
        &report.report_data[..5],
        &[1, 2, 3, 4, 5],
        "REPORT_DATA chosen by the capturing party"
    );
    assert_eq!(HARDWARE_ATTESTATION_VERSION, "hardware-attestation-v1");
    assert_eq!(SNP_ADAPTER_VERSION, "snp-adapter-v1");
}

#[test]
fn hw02_real_report_is_refused_as_unbound_to_any_mbongo_challenge() {
    // Genuine, chained, signed — and useless to Mbongo: its REPORT_DATA does
    // not answer this challenge. Parsing is not attestation.
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let v = SnpVerifier::new(lenient());
    let r = v.verify(&envelope(&ev, &c), &c, &trust(true), NOW);
    assert_eq!(r, Err(VerifyError::ChallengeMismatch));
}

#[test]
fn hw08_tampered_signature_and_signed_fields_are_rejected() {
    let t = trust(true);
    for (name, offset) in [
        ("signature", SIGNATURE_OFFSET),
        ("report_data", REPORT_DATA_OFFSET),
        ("measurement", MEASUREMENT_OFFSET),
        ("policy", 0x08),
        ("reported_tcb", 0x180),
    ] {
        let mut r = real_report();
        r[offset] ^= 0x01;
        let ev = evidence(r, real_vcek());
        assert_eq!(
            verify_chain_and_signature(&ev, &t).err(),
            Some(VerifyError::BadSignature),
            "{name}"
        );
    }
    let mut short = real_report();
    short.pop();
    assert_eq!(
        verify_chain_and_signature(&evidence(short, real_vcek()), &t).err(),
        Some(VerifyError::Malformed)
    );
}

#[test]
fn hw09_untrusted_root_and_unknown_format_are_rejected() {
    let ev = evidence(real_report(), real_vcek());
    // No anchor at all.
    let mut none = trust(true);
    none.anchors.clear();
    assert_eq!(
        verify_chain_and_signature(&ev, &none).err(),
        Some(VerifyError::UntrustedRoot)
    );
    // An anchor entry under the Milan id whose digest is another root.
    let mut wrong = trust(true);
    wrong.anchors.get_mut("amd-ark-milan").unwrap().public_key = SnpProduct::Genoa.ark_digest();
    assert_eq!(
        verify_chain_and_signature(&ev, &wrong).err(),
        Some(VerifyError::UntrustedRoot)
    );
    // Evidence labelled Genoa: the chain to the Genoa root fails.
    let mut genoa = trust(true);
    genoa.anchors.insert(
        SnpProduct::Genoa.anchor_id().into(),
        SnpProduct::Genoa.trust_anchor(),
    );
    let mut mislabelled = ev.clone();
    mislabelled.product = SnpProduct::Genoa;
    assert_eq!(
        verify_chain_and_signature(&mislabelled, &genoa).err(),
        Some(VerifyError::UntrustedRoot)
    );
    // The reference format is not an SNP format.
    let c = challenge(1, 0xA1, 7);
    let mut env = envelope(&ev, &c);
    env.format = REFERENCE_FORMAT.into();
    assert_eq!(
        SnpVerifier::new(lenient()).verify(&env, &c, &trust(true), NOW),
        Err(VerifyError::UnknownFormat)
    );
    // Garbage payload.
    let mut garbage = envelope(&ev, &c);
    garbage.payload = b"not an snp payload".to_vec();
    assert_eq!(
        SnpVerifier::new(lenient()).verify(&garbage, &c, &trust(true), NOW),
        Err(VerifyError::Malformed)
    );
}

#[test]
fn hw10_revoked_root_other_chip_and_revoked_chip_are_rejected() {
    let ev = evidence(real_report(), real_vcek());
    let mut revoked = trust(true);
    revoked.anchors.get_mut("amd-ark-milan").unwrap().revoked = true;
    assert_eq!(
        verify_chain_and_signature(&ev, &revoked).err(),
        Some(VerifyError::RevokedRoot)
    );
    // Another Milan chip's VCEK chains to the same root but did not sign this report.
    let other = evidence(real_report(), other_chip_vcek());
    assert_eq!(
        verify_chain_and_signature(&other, &trust(true)).err(),
        Some(VerifyError::BadSignature)
    );
    // Chip denylist (stage 2).
    let c = challenge(1, 0xA1, 7);
    let bound = bound_copy(&c, &env_key().public());
    let mut p = lenient();
    p.revoked_chips.insert(parse_report(&bound).unwrap().chip_id);
    assert_eq!(
        stage2(&bound, &ev, &c, &p, &trust(true), NOW).err(),
        Some(VerifyError::RevokedPlatform)
    );
}

#[test]
fn hw13_real_report_with_debug_allowed_fails_a_production_policy() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    let production = SnpPolicy::production(SnpProduct::Milan, TcbMinimum::default(), 0);
    assert!(SnpVerifier::new(production.clone()).production_grade());
    assert_eq!(
        stage2(&bound, &ev, &c, &production, &trust(true), NOW).err(),
        Some(VerifyError::DebugEnabled)
    );
    // Debug allowed but SMT refused: the platform has SMT enabled.
    let mut no_smt = lenient();
    no_smt.allow_smt = false;
    assert_eq!(
        stage2(&bound, &ev, &c, &no_smt, &trust(true), NOW).err(),
        Some(VerifyError::PlatformStateUnacceptable)
    );
    // VMPL policy.
    let mut vmpl = lenient();
    vmpl.max_vmpl = 0;
    assert!(
        stage2(&bound, &ev, &c, &vmpl, &trust(true), NOW).is_ok(),
        "the report is VMPL 0"
    );
    // Report version policy.
    let mut version = lenient();
    version.min_report_version = 3;
    assert_eq!(
        stage2(&bound, &ev, &c, &version, &trust(true), NOW).err(),
        Some(VerifyError::PlatformStateUnacceptable)
    );
}

#[test]
fn hw11_measurement_allowlist_is_enforced() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    assert_eq!(
        stage2(&bound, &ev, &c, &lenient(), &trust(false), NOW).err(),
        Some(VerifyError::MeasurementNotAllowed)
    );
    let mut revoked = trust(true);
    revoked.measurement.revoked.insert(real_measurement_digest());
    assert_eq!(
        stage2(&bound, &ev, &c, &lenient(), &revoked, NOW).err(),
        Some(VerifyError::MeasurementRevoked)
    );
    assert!(stage2(&bound, &ev, &c, &lenient(), &trust(true), NOW).is_ok());
}

#[test]
fn hw12_tcb_and_guest_svn_minimums_are_enforced_component_wise() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    for (name, min) in [
        (
            "bootloader",
            TcbMinimum {
                bootloader: 3,
                ..TcbMinimum::default()
            },
        ),
        (
            "tee",
            TcbMinimum {
                tee: 1,
                ..TcbMinimum::default()
            },
        ),
        (
            "snp",
            TcbMinimum {
                snp: 6,
                ..TcbMinimum::default()
            },
        ),
        (
            "microcode",
            TcbMinimum {
                microcode: 69,
                ..TcbMinimum::default()
            },
        ),
    ] {
        let mut p = lenient();
        p.min_tcb = min;
        assert_eq!(
            stage2(&bound, &ev, &c, &p, &trust(true), NOW).err(),
            Some(VerifyError::SecurityVersionTooLow),
            "{name}"
        );
    }
    let mut exact = lenient();
    exact.min_tcb = TcbMinimum {
        bootloader: 2,
        tee: 0,
        snp: 5,
        microcode: 68,
    };
    assert!(
        stage2(&bound, &ev, &c, &exact, &trust(true), NOW).is_ok(),
        "equal to the report's TCB passes"
    );
    let mut svn = lenient();
    svn.min_guest_svn = 1;
    assert_eq!(
        stage2(&bound, &ev, &c, &svn, &trust(true), NOW).err(),
        Some(VerifyError::SecurityVersionTooLow)
    );
}

#[test]
fn hw14_stale_windows_and_expired_collateral_are_rejected() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    let parsed = parse_report(&bound).unwrap();
    let vcek = sev::certs::snp::Certificate::from_der(&ev.vcek_der).unwrap();
    let mut expired = envelope(&ev, &c);
    expired.expires_at = NOW - 1;
    assert_eq!(
        check_platform_policy(
            &parsed,
            &vcek,
            &ev,
            &expired,
            &c,
            &lenient(),
            &trust(true),
            NOW
        )
        .err(),
        Some(VerifyError::EvidenceExpired)
    );
    let mut future = envelope(&ev, &c);
    future.issued_at = NOW + 1;
    assert_eq!(
        check_platform_policy(
            &parsed,
            &vcek,
            &ev,
            &future,
            &c,
            &lenient(),
            &trust(true),
            NOW
        )
        .err(),
        Some(VerifyError::EvidenceNotYetValid)
    );
    // The policy caps a generous envelope window.
    let mut capped = lenient();
    capped.max_report_validity_secs = 1;
    let mut generous = envelope(&ev, &c);
    generous.issued_at = NOW - 10;
    generous.expires_at = NOW + 1_000;
    assert_eq!(
        check_platform_policy(
            &parsed,
            &vcek,
            &ev,
            &generous,
            &c,
            &capped,
            &trust(true),
            NOW
        )
        .err(),
        Some(VerifyError::EvidenceExpired)
    );
    // Certificate validity, enforced against a clock outside the VCEK window.
    let mut strict_time = lenient();
    strict_time.enforce_certificate_validity = true;
    assert_eq!(
        stage2(&bound, &ev, &c, &strict_time, &trust(true), NOW).err(),
        Some(VerifyError::ExpiredCollateral)
    );
    // And inside it.
    let x: x509_cert::Certificate = vcek.clone().into();
    let inside = x.tbs_certificate.validity.not_before.to_unix_duration().as_secs() + 1;
    let mut env_inside = envelope(&ev, &c);
    env_inside.issued_at = inside - 5;
    env_inside.expires_at = inside + 60;
    assert!(check_platform_policy(
        &parsed,
        &vcek,
        &ev,
        &env_inside,
        &c,
        &strict_time,
        &trust(true),
        inside
    )
    .is_ok());
}

// ── layer 2: binding and normalization on an unsigned bound copy ─────────

#[test]
fn hw03_binding_derivation_covers_every_field_and_fills_report_data() {
    let base = challenge(1, 0xA1, 7);
    let key = env_key().public();
    let rd = report_data_binding(&base, &key);
    assert_eq!(rd.len(), 64);
    let variants: Vec<(&str, AttestationChallenge, [u8; 32])> = vec![
        (
            "challenge_id",
            AttestationChallenge {
                challenge_id: ChallengeId([0xC2; 32]),
                ..base.clone()
            },
            key,
        ),
        (
            "nonce",
            AttestationChallenge {
                nonce: [0x4F; 32],
                ..base.clone()
            },
            key,
        ),
        ("session", challenge(2, 0xA1, 7), key),
        ("executor", challenge(1, 0xB2, 7), key),
        ("task", challenge(1, 0xA1, 8), key),
        (
            "purpose",
            AttestationChallenge {
                purpose: Purpose::ResultRelease,
                ..base.clone()
            },
            key,
        ),
        ("environment_key", base.clone(), [0x99; 32]),
    ];
    for (name, c, k) in variants {
        assert_ne!(
            report_data_binding(&c, &k),
            rd,
            "{name} must change REPORT_DATA"
        );
    }
}

#[test]
fn hw04_to_hw07_bindings_are_enforced_by_stage_two() {
    let c = challenge(1, 0xA1, 7);
    let key = env_key().public();
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &key);
    let ok =
        stage2(&bound, &ev, &c, &lenient(), &trust(true), NOW).expect("bound copy passes stage 2");
    assert_eq!(
        (ok.session_id, ok.executor, ok.task_id, ok.challenge_id),
        (c.session_id, c.executor, c.task_id, c.challenge_id)
    );
    // Same report presented for another session, executor or task: refused.
    for (name, other) in [
        ("session", challenge(2, 0xA1, 7)),
        ("executor", challenge(1, 0xB2, 7)),
        ("task", challenge(1, 0xA1, 8)),
    ] {
        assert_eq!(
            stage2(&bound, &ev, &other, &lenient(), &trust(true), NOW).err(),
            Some(VerifyError::ChallengeMismatch),
            "{name}"
        );
    }
    // Environment key substitution after quoting: the host swaps the key
    // beside the report; REPORT_DATA still hashes the original.
    let mut swapped = ev.clone();
    swapped.environment_key = EnvironmentKey::from_bytes([0x77; 32]).public();
    assert_eq!(
        stage2(&bound, &swapped, &c, &lenient(), &trust(true), NOW).err(),
        Some(VerifyError::ChallengeMismatch)
    );
    // A fresh challenge with a new nonce: the old bound report is stale.
    let fresh = AttestationChallenge {
        nonce: [0x4F; 32],
        challenge_id: ChallengeId([0xC3; 32]),
        ..c.clone()
    };
    assert_eq!(
        stage2(&bound, &ev, &fresh, &lenient(), &trust(true), NOW).err(),
        Some(VerifyError::ChallengeMismatch)
    );
}

#[test]
fn hw16_normalization_carries_provenance_and_no_vendor_structure() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    let v = stage2(&bound, &ev, &c, &lenient(), &trust(true), NOW).unwrap();
    assert_eq!(v.security_label, SecurityLabel::HardwareAttested);
    assert_eq!(v.verifier, SNP_VERIFIER_KIND);
    assert_eq!(v.format, SNP_FORMAT);
    assert_eq!(v.trust_anchor, "amd-ark-milan");
    assert_eq!(v.environment_type, SNP_ENVIRONMENT_TYPE);
    assert_eq!(v.measurement, real_measurement_digest());
    assert_eq!(v.security_version, 2 | (5u64 << 48) | (68u64 << 56));
    assert!(
        !v.debug_disabled,
        "the fixture allows debug, faithfully reported"
    );
    assert_eq!(v.environment_key, env_key().public());
    assert_eq!(v.purpose, Purpose::ConfidentialInputRelease);
    let rendered = format!("{v:?}");
    assert!(
        !rendered.contains("AttestationReport"),
        "no vendor structure leaks through"
    );
}

#[test]
fn hw17_normalized_evidence_satisfies_j_confidential_and_nothing_weaker_does() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    let v = stage2(&bound, &ev, &c, &lenient(), &trust(true), NOW).unwrap();
    let evidence_record = to_policy_evidence(&v);
    assert_eq!(evidence_record.level, EvidenceLevel::Attested);
    let mut claims = BTreeMap::new();
    claims.insert(
        dimension::CONTRACT_VERSION.to_owned(),
        Claim::plain(Value::Token("provider-policy-v1".into()), Scope::Session),
    );
    claims.insert(
        dimension::REPRESENTATION_TAGS.to_owned(),
        Claim::plain(
            Value::Set(["r".to_owned()].into_iter().collect()),
            Scope::Session,
        ),
    );
    claims.insert(
        dimension::PROFILE_TAGS.to_owned(),
        Claim::plain(
            Value::Set(["p".to_owned()].into_iter().collect()),
            Scope::Session,
        ),
    );
    claims.insert(
        dimension::CONFIDENTIAL_EXECUTION.to_owned(),
        Claim {
            value: Value::Bool(true),
            scope: Scope::Session,
            evidence: Some(evidence_record),
        },
    );
    let facts = SessionFacts {
        session_id: c.session_id,
        proved_executor: c.executor,
        worker_class: None,
        claims,
    };
    let task = TaskFacts {
        executor: c.executor,
        profile_tag: "p".into(),
        representation_tag: "r".into(),
    };
    let ad = Advertisement {
        provider_id: "P".into(),
        executors: vec![c.executor],
        issued_at: 0,
        expires_at: u64::MAX,
        revoked: false,
        claims: BTreeMap::new(),
        worker_classes: BTreeMap::new(),
    };
    let policy = compose(&[templates::confidential(Source::DataOwner)]).unwrap();
    let dec = policy::eligible(
        &policy::Input {
            task: &task,
            session: &facts,
            advertisement: &ad,
            derived: &DerivedFacts::default(),
            now: NOW,
        },
        &policy,
    );
    assert!(dec.eligible, "{dec:?}");
    assert_eq!(dec.lowest_evidence, EvidenceLevel::Attested);
    // The same claim without the record: ATTESTATION_REQUIRED.
    let mut bare = facts.clone();
    bare.claims.insert(
        dimension::CONFIDENTIAL_EXECUTION.to_owned(),
        Claim::plain(Value::Bool(true), Scope::Session),
    );
    let dec = policy::eligible(
        &policy::Input {
            task: &task,
            session: &bare,
            advertisement: &ad,
            derived: &DerivedFacts::default(),
            now: NOW,
        },
        &policy,
    );
    assert!(!dec.eligible);
}

#[test]
fn hw20_hw21_content_key_wraps_only_to_the_attested_environment_key() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let bound = bound_copy(&c, &env_key().public());
    let v = stage2(&bound, &ev, &c, &lenient(), &trust(true), NOW).unwrap();
    let key = ContentKey::from_bytes([0x10; 32]);
    let binding = WrapBinding {
        release_id: ReleaseId([1; 32]),
        task_id: c.task_id,
        executor: c.executor,
        session_id: c.session_id,
    };
    let wrapped = wrap_key(
        &key,
        v.environment_key,
        &EnvironmentKey::from_bytes([0x70; 32]),
        [0x80; 24],
        &binding,
    )
    .unwrap();
    assert!(
        unwrap_key(&wrapped, &env_key(), &binding).unwrap().equals(&key),
        "the attested environment unwraps"
    );
    assert!(
        unwrap_key(&wrapped, &EnvironmentKey::from_bytes([0x71; 32]), &binding).is_err(),
        "a host-held key cannot"
    );
    let sealed = seal_input(&key, [0x40; 24], &c.task_id, &[0x30; 32], b"secret input");
    let opened = mbongo_compute::confidential::open_input(
        &unwrap_key(&wrapped, &env_key(), &binding).unwrap(),
        &c.task_id,
        &[0x30; 32],
        &sealed,
    )
    .unwrap();
    assert_eq!(opened.as_bytes(), b"secret input");
}

#[test]
fn hw24_hw25_evidence_renderings_carry_references_not_reports() {
    let c = challenge(1, 0xA1, 7);
    let ev = evidence(real_report(), real_vcek());
    let shown = format!(
        "{ev:?} {:?} {:?}",
        envelope(&ev, &c),
        SnpVerifier::new(lenient())
    );
    assert!(
        !shown.contains(&hex::encode(&real_report()[..32])),
        "no report bytes"
    );
    assert!(
        !shown.contains(&hex::encode(&real_vcek()[..32])),
        "no certificate bytes"
    );
    assert!(
        shown.contains("ref "),
        "a hash reference is what logs carry"
    );
}

#[test]
fn hw27_hw28_reference_root_is_never_accepted_and_production_grade_is_strictness() {
    let root = ReferenceAttestationRoot::from_seed(&[0xA7; 32], "test-root");
    let mut t = trust(true);
    t.anchors.insert("test-root".into(), root.anchor());
    t.accepted_formats.insert(REFERENCE_FORMAT.into());
    let c = challenge(1, 0xA1, 7);
    let mut env = envelope(&evidence(real_report(), real_vcek()), &c);
    env.format = REFERENCE_FORMAT.into();
    assert_eq!(
        SnpVerifier::new(lenient()).verify(&env, &c, &t, NOW),
        Err(VerifyError::UnknownFormat)
    );
    assert!(!SnpVerifier::new(lenient()).production_grade());
    assert!(SnpVerifier::new(SnpPolicy::production(
        SnpProduct::Milan,
        TcbMinimum::default(),
        0
    ))
    .production_grade());
    assert!(!ReferenceAttestationVerifier.production_grade());
}

#[test]
fn hw23_reference_verifier_and_confidential_auth_v1_are_unchanged() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let report = rt.block_on(mbongo_compute::confidential::suite::run_all(|| {
        Box::new(ReferenceAttestationVerifier)
    }));
    assert!(report.passed(), "{}", report.render());
    assert_eq!(report.cases.len(), 30);
}

// ── negative controls ─────────────────────────────────────────────────────

#[test]
fn negative_controls_are_caught() {
    let c = challenge(1, 0xA1, 7);
    let t = trust(true);
    // A forged report (unsigned bound copy) under a trusted anchor id.
    let forged = evidence(bound_copy(&c, &env_key().public()), real_vcek());
    let env = envelope(&forged, &c);
    assert_eq!(
        SnpVerifier::new(lenient()).verify(&env, &c, &t, NOW),
        Err(VerifyError::BadSignature),
        "strict rejects"
    );
    assert!(
        SignatureBlindSnpVerifier::new(lenient()).verify(&env, &c, &t, NOW).is_ok(),
        "signature-blind accepts — caught"
    );
    // The real, unbound report.
    let real = evidence(real_report(), real_vcek());
    let env = envelope(&real, &c);
    assert_eq!(
        SnpVerifier::new(lenient()).verify(&env, &c, &t, NOW),
        Err(VerifyError::ChallengeMismatch)
    );
    assert!(
        BindingBlindSnpVerifier::new(lenient()).verify(&env, &c, &t, NOW).is_ok(),
        "binding-blind accepts — caught"
    );
    // A bound copy the strict verifier would still reject on signature; use
    // the signature-blind path's output shape through the measurement-blind
    // verifier with an empty allowlist on the real report.
    let empty = trust(false);
    assert_eq!(
        BindingBlindSnpVerifier::new(lenient()).verify(&env, &c, &empty, NOW),
        Err(VerifyError::MeasurementNotAllowed)
    );
    let both = MeasurementBlindSnpVerifier::new(lenient());
    assert_eq!(
        both.verify(&env, &c, &empty, NOW),
        Err(VerifyError::ChallengeMismatch),
        "measurement-blind still binds"
    );
}

// ── layer 3: live hardware (ignored unless run on an SEV-SNP guest) ───────

/// Runs only on a SEV-SNP guest: `cargo test -p mbongo-compute --test
/// hardware_attestation -- --ignored`. Reads the results file written by
/// `snp_live_test` from `MBONGO_SNP_LIVE_RESULT` and asserts every REAL_*
/// line is YES. Elsewhere it is reported as ignored, never as passed.
#[test]
#[ignore = "requires a live AMD SEV-SNP guest; see docs/architecture/compute-hardware-attestation-adapter.md §16"]
fn hw_live_snp_guest_end_to_end() {
    let path = std::env::var("MBONGO_SNP_LIVE_RESULT")
        .expect("MBONGO_SNP_LIVE_RESULT must point at the snp_live_test results file");
    let text = std::fs::read_to_string(&path).expect("results file");
    let required = [
        "REAL_QUOTE_GENERATED=YES",
        "REAL_QUOTE_VERIFIED=YES",
        "REAL_CHALLENGE_BOUND=YES",
        "REAL_SESSION_BOUND=YES",
        "REAL_TASK_BOUND=YES",
        "REAL_EXECUTOR_BOUND=YES",
        "REAL_ENVIRONMENT_KEY_BOUND=YES",
        "REAL_MEASUREMENT_CHECKED=YES",
        "REAL_SECURITY_STATE_CHECKED=YES",
        "REAL_J_POLICY_PASS=YES",
        "REAL_KEY_RELEASE=YES",
        "REAL_PRIVATE_INPUT_DECRYPTED_INSIDE_PROTECTED_ENV=YES",
        "RESULT=PASS_HARDWARE",
    ];
    for line in required {
        assert!(
            text.lines().any(|l| l.trim() == line),
            "missing {line} in {path}"
        );
    }
}
