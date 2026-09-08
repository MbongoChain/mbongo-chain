//! Mbongo Compute Confidential Authorization: the named test group for
//! `confidential-auth-v1`. Runs the K01–K30 suite against the reference
//! verifier, proves the suite is not vacuous against two deliberately
//! unsafe verifiers, and exercises the sealing and key-wrapping primitives
//! directly (round trip and tamper).

use mbongo_compute::confidential::reference::ReferenceAttestationVerifier;
use mbongo_compute::confidential::suite::negative::{BindingBlindVerifier, SignatureBlindVerifier};
use mbongo_compute::confidential::suite::{catalog, run_all, Status};
use mbongo_compute::confidential::{
    open_input, seal_input, unwrap_key, wrap_key, ContentKey, EnvironmentKey, ReleaseId,
    SealedInput, WrapBinding, CONFIDENTIAL_AUTH_VERSION,
};
use mbongo_compute::identity::SessionId;
use mbongo_core::Address;

/// The suite captures process-wide logs; runs in one test binary must not
/// interleave.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn reference_verifier_passes_every_case() {
    let _serial = SERIAL.lock().await;
    let report = run_all(|| Box::new(ReferenceAttestationVerifier)).await;
    print!("{}", report.render());
    assert_eq!(report.version, CONFIDENTIAL_AUTH_VERSION);
    assert!(
        !report.production_grade,
        "the reference verifier must never claim production grade"
    );
    assert_eq!(report.cases.len(), 30);
    let failed: Vec<_> = report
        .cases
        .iter()
        .filter(|c| c.status != Status::Pass)
        .map(|c| format!("{} {:?}", c.case.id, c.status))
        .collect();
    assert!(failed.is_empty(), "failed: {failed:?}");
}

#[test]
fn catalog_is_complete_and_unique() {
    let cases = catalog();
    assert_eq!(cases.len(), 30);
    for (i, c) in cases.iter().enumerate() {
        assert_eq!(c.id, format!("K{:02}", i + 1));
        assert!(!c.invariants.is_empty(), "{} names no invariant", c.id);
    }
    let mut names: Vec<_> = cases.iter().map(|c| c.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 30, "case names unique");
}

#[tokio::test]
async fn a_signature_blind_verifier_is_caught() {
    let _serial = SERIAL.lock().await;
    let report = run_all(|| Box::new(SignatureBlindVerifier)).await;
    assert!(
        !report.passed(),
        "a forging verifier must not pass the suite"
    );
    let k10 = report.cases.iter().find(|c| c.case.id == "K10").unwrap();
    assert!(
        matches!(k10.status, Status::Fail(_)),
        "K10 must catch a skipped signature check: {:?}",
        k10.status
    );
}

#[tokio::test]
async fn a_binding_blind_verifier_is_caught() {
    let _serial = SERIAL.lock().await;
    let report = run_all(|| Box::new(BindingBlindVerifier)).await;
    assert!(
        !report.passed(),
        "a binding-blind verifier must not pass the suite"
    );
    let failed: Vec<_> = report
        .cases
        .iter()
        .filter(|c| c.status != Status::Pass)
        .map(|c| c.case.id)
        .collect();
    assert!(
        failed.contains(&"K08"),
        "K08 (cross-task replay) must catch it: {failed:?}"
    );
}

fn binding() -> WrapBinding {
    WrapBinding {
        release_id: ReleaseId([1u8; 32]),
        task_id: [2u8; 32],
        executor: Address([3u8; 32]),
        session_id: SessionId([4u8; 32]),
    }
}

#[test]
fn seal_and_open_round_trip_and_tamper() {
    let key = ContentKey::from_bytes([0x10u8; 32]);
    let task = [0x20u8; 32];
    let commitment = [0x30u8; 32];
    let sealed = seal_input(&key, [0x40u8; 24], &task, &commitment, b"hello");
    assert!(
        !sealed.ciphertext.windows(5).any(|w| w == b"hello"),
        "ciphertext hides the plaintext"
    );
    let plain = open_input(&key, &task, &commitment, &sealed).unwrap();
    assert_eq!(plain.as_bytes(), b"hello");
    // Storage round trip.
    let restored = SealedInput::from_bytes(&sealed.to_bytes()).unwrap();
    assert_eq!(restored, sealed);
    // Tampered ciphertext.
    let mut t = sealed.clone();
    t.ciphertext[0] ^= 1;
    assert!(open_input(&key, &task, &commitment, &t).is_err());
    // Wrong task binding, wrong commitment binding, wrong key.
    assert!(open_input(&key, &[0x21u8; 32], &commitment, &sealed).is_err());
    assert!(open_input(&key, &task, &[0x31u8; 32], &sealed).is_err());
    assert!(open_input(
        &ContentKey::from_bytes([0x11u8; 32]),
        &task,
        &commitment,
        &sealed
    )
    .is_err());
    assert!(SealedInput::from_bytes(&[0u8; 10]).is_err());
}

#[test]
fn wrap_and_unwrap_round_trip_and_tamper() {
    let key = ContentKey::from_bytes([0x50u8; 32]);
    let recipient = EnvironmentKey::from_bytes([0x60u8; 32]);
    let other = EnvironmentKey::from_bytes([0x61u8; 32]);
    let b = binding();
    let wrapped = wrap_key(
        &key,
        recipient.public(),
        &EnvironmentKey::from_bytes([0x70u8; 32]),
        [0x80u8; 24],
        &b,
    )
    .unwrap();
    assert_eq!(wrapped.ciphertext.len(), 48);
    let unwrapped = unwrap_key(&wrapped, &recipient, &b).unwrap();
    assert!(unwrapped.equals(&key));
    // Wrong recipient.
    assert!(unwrap_key(&wrapped, &other, &b).is_err());
    // Wrong binding: another release, task, executor or session.
    for wrong in [
        WrapBinding {
            release_id: ReleaseId([9u8; 32]),
            ..b
        },
        WrapBinding {
            task_id: [9u8; 32],
            ..b
        },
        WrapBinding {
            executor: Address([9u8; 32]),
            ..b
        },
        WrapBinding {
            session_id: SessionId([9u8; 32]),
            ..b
        },
    ] {
        assert!(
            unwrap_key(&wrapped, &recipient, &wrong).is_err(),
            "{wrong:?}"
        );
    }
    // Tampered ciphertext and ephemeral key.
    let mut t = wrapped.clone();
    t.ciphertext[5] ^= 1;
    assert!(unwrap_key(&t, &recipient, &b).is_err());
    let mut t = wrapped.clone();
    t.ephemeral_public[0] ^= 1;
    assert!(unwrap_key(&t, &recipient, &b).is_err());
    // Nothing renders the key.
    let shown = format!("{wrapped:?} {key:?} {recipient:?}");
    assert!(!shown.contains(&hex::encode([0x50u8; 32])));
    assert!(!shown.contains(&hex::encode([0x60u8; 32])));
    assert!(!shown.contains(&hex::encode(&wrapped.ciphertext)));
}
