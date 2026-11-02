//! **REFERENCE / TEST ONLY** attestation: a signed evidence format, a test
//! root, a software "environment" and a verifier for them.
//!
//! This is **not hardware attestation** and **not production confidential
//! compute** (K29). The environment is a process holding software keys; the
//! root is an Ed25519 key a test controls. What it exercises is every
//! binding and policy check a real adapter must also pass — signature under
//! a trusted root, challenge nonce, session, executor, task and purpose
//! binding, validity window, measurement allowlist, security version and
//! debug state — and the wrap of a content key to the environment key the
//! evidence bound. What it cannot prove is that the provider operating the
//! process cannot read its memory.
//!
//! The root key is a distinct identity from every executor key and from
//! the control plane's issuer key (§15 of the architecture): it signs
//! evidence and nothing else, under its own domain tag.

use ed25519_dalek::{Signer, SigningKey};
use mbongo_core::Address;
use parity_scale_codec::{Decode, Encode};

use super::{
    open_input, unwrap_key, AttestationChallenge, AttestationVerifier, CryptoError, EnvironmentKey,
    EvidenceEnvelope, Purpose, SealedInput, SecurityLabel, TrustAnchor, TrustPolicy,
    VerifiedAttestation, VerifyError, WrapBinding, WrappedKey,
};
use crate::execution::Plaintext;
use crate::identity::{verify_signature, SessionId};

/// The reference evidence format identifier.
pub const REFERENCE_FORMAT: &str = "mbongo-ref-attestation:v1";
/// The reference verifier kind.
pub const REFERENCE_VERIFIER_KIND: &str = "mbongo-ref-verifier";
/// The label every artefact of this module carries.
pub const REFERENCE_LABEL: &str =
    "REFERENCE_ATTESTED — TEST ONLY; not hardware attestation; not production confidential compute";
/// Abstract environment type the reference environment reports.
pub const REFERENCE_ENVIRONMENT_TYPE: &str = "mbongo-ref-environment";
/// Domain tag the root signs evidence under. Never a receipt, transaction,
/// possession, lease or capability domain.
const DOMAIN_EVIDENCE: &[u8] = b"mbongo:ref-attestation-evidence:v1";

/// The signed body of a reference evidence payload. Public so a test can
/// construct a deliberately wrong one and have the root sign it.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct ReferenceEvidenceBody {
    /// Which trust anchor the signature chains to.
    pub anchor_id: String,
    /// Abstract environment type.
    pub environment_type: String,
    /// Measurement of the environment.
    pub measurement: [u8; 32],
    /// Security version.
    pub security_version: u64,
    /// Debug state.
    pub debug_disabled: bool,
    /// The environment's X25519 public key.
    pub environment_key: [u8; 32],
    /// Challenge binding.
    pub challenge_id: [u8; 32],
    /// See `challenge_id`.
    pub nonce: [u8; 32],
    /// See `challenge_id`.
    pub session_id: [u8; 32],
    /// See `challenge_id`.
    pub executor: [u8; 32],
    /// See `challenge_id`.
    pub task_id: [u8; 32],
    /// See `challenge_id`.
    pub purpose: u8,
    /// Window.
    pub issued_at: u64,
    /// See `issued_at`.
    pub expires_at: u64,
}

#[derive(Encode, Decode)]
struct ReferenceEvidence {
    body: ReferenceEvidenceBody,
    signature: [u8; 64],
}

fn signing_message(body: &ReferenceEvidenceBody) -> Vec<u8> {
    let encoded = body.encode();
    let mut m = Vec::with_capacity(DOMAIN_EVIDENCE.len() + encoded.len());
    m.extend_from_slice(DOMAIN_EVIDENCE);
    m.extend_from_slice(&encoded);
    m
}

/// A test root: the Ed25519 key that endorses reference environments.
/// Distinct from every executor and issuer key.
pub struct ReferenceAttestationRoot {
    key: SigningKey,
    anchor_id: String,
}

impl std::fmt::Debug for ReferenceAttestationRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ReferenceAttestationRoot({}, pk {}, secret redacted)",
            self.anchor_id,
            hex::encode(&self.public_key()[..8])
        )
    }
}

impl ReferenceAttestationRoot {
    /// A root from a 32-byte seed, named `anchor_id` in trust policies.
    pub fn from_seed(seed: &[u8; 32], anchor_id: &str) -> Self {
        Self {
            key: SigningKey::from_bytes(seed),
            anchor_id: anchor_id.to_owned(),
        }
    }

    /// The anchor id.
    pub fn anchor_id(&self) -> &str {
        &self.anchor_id
    }

    /// The verifying key.
    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }

    /// The trust-policy entry for this root.
    pub fn anchor(&self) -> TrustAnchor {
        TrustAnchor {
            anchor_id: self.anchor_id.clone(),
            public_key: self.public_key(),
            revoked: false,
        }
    }

    /// Signs `body` into an envelope. Public so a test can sign a wrong
    /// body; a production root would never sign on request.
    pub fn seal(&self, body: ReferenceEvidenceBody) -> EvidenceEnvelope {
        let signature = self.key.sign(&signing_message(&body)).to_bytes();
        let (challenge_id, session_id, executor, issued_at, expires_at) = (
            body.challenge_id,
            body.session_id,
            body.executor,
            body.issued_at,
            body.expires_at,
        );
        let payload = ReferenceEvidence { body, signature }.encode();
        EvidenceEnvelope {
            format: REFERENCE_FORMAT.to_owned(),
            verifier_kind: REFERENCE_VERIFIER_KIND.to_owned(),
            challenge_id: super::ChallengeId(challenge_id),
            session_id: SessionId(session_id),
            executor: Address(executor),
            issued_at,
            expires_at,
            payload,
        }
    }
}

/// A software "environment": what a real confidential environment would
/// be, minus the hardware. Holds a measurement, a security version, a
/// debug flag and the X25519 key the content key is wrapped to. Decrypts
/// inside itself.
pub struct ReferenceEnvironment {
    measurement: [u8; 32],
    security_version: u64,
    debug_disabled: bool,
    key: EnvironmentKey,
}

impl std::fmt::Debug for ReferenceEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ReferenceEnvironment(measurement {}, sv {}, debug_disabled {}, {:?})",
            hex::encode(&self.measurement[..8]),
            self.security_version,
            self.debug_disabled,
            self.key
        )
    }
}

impl ReferenceEnvironment {
    /// An environment with these properties and a key from `key_seed`.
    pub fn new(
        measurement: [u8; 32],
        security_version: u64,
        debug_disabled: bool,
        key_seed: [u8; 32],
    ) -> Self {
        Self {
            measurement,
            security_version,
            debug_disabled,
            key: EnvironmentKey::from_bytes(key_seed),
        }
    }

    /// The measurement.
    pub fn measurement(&self) -> [u8; 32] {
        self.measurement
    }

    /// The public half of the environment key.
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public()
    }

    /// The evidence body answering `challenge`, valid over the window.
    pub fn body_for(
        &self,
        anchor_id: &str,
        challenge: &AttestationChallenge,
        issued_at: u64,
        expires_at: u64,
    ) -> ReferenceEvidenceBody {
        ReferenceEvidenceBody {
            anchor_id: anchor_id.to_owned(),
            environment_type: REFERENCE_ENVIRONMENT_TYPE.to_owned(),
            measurement: self.measurement,
            security_version: self.security_version,
            debug_disabled: self.debug_disabled,
            environment_key: self.key.public(),
            challenge_id: challenge.challenge_id.0,
            nonce: challenge.nonce,
            session_id: challenge.session_id.0,
            executor: challenge.executor.0,
            task_id: challenge.task_id,
            purpose: challenge.purpose.tag(),
            issued_at,
            expires_at,
        }
    }

    /// Produces evidence answering `challenge`, endorsed by `root`, valid
    /// from `now` for `ttl_secs`.
    pub fn evidence(
        &self,
        root: &ReferenceAttestationRoot,
        challenge: &AttestationChallenge,
        now: u64,
        ttl_secs: u64,
    ) -> EvidenceEnvelope {
        root.seal(self.body_for(root.anchor_id(), challenge, now, now + ttl_secs))
    }

    /// Unwraps the released key and opens the sealed input — inside the
    /// environment. The content key exists only here.
    pub fn open_input(
        &self,
        wrapped: &WrappedKey,
        binding: &WrapBinding,
        task_id: &[u8; 32],
        input_commitment: &[u8; 32],
        sealed: &SealedInput,
    ) -> Result<Plaintext, CryptoError> {
        let key = unwrap_key(wrapped, &self.key, binding)?;
        open_input(&key, task_id, input_commitment, sealed)
    }
}

/// How lax a verification is. Only the strict form exists as a verifier;
/// the negative-control verifiers in the suite use the lax forms so that
/// the suite can prove it catches them. Never a production option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Laxity {
    /// Skip the signature check (a forging verifier).
    pub skip_signature: bool,
    /// Report bindings from the challenge instead of the evidence (a
    /// binding-blind verifier).
    pub bind_from_challenge: bool,
}

impl Laxity {
    /// The strict verification every real verifier performs.
    pub const STRICT: Self = Self {
        skip_signature: false,
        bind_from_challenge: false,
    };
}

/// Verifies a reference envelope with the given laxity. `Laxity::STRICT`
/// is the reference verifier; anything else is a test negative control.
pub fn verify_reference(
    envelope: &EvidenceEnvelope,
    challenge: &AttestationChallenge,
    trust: &TrustPolicy,
    now: u64,
    laxity: Laxity,
) -> Result<VerifiedAttestation, VerifyError> {
    if envelope.format != REFERENCE_FORMAT {
        return Err(VerifyError::UnknownFormat);
    }
    let evidence = ReferenceEvidence::decode(&mut &envelope.payload[..])
        .map_err(|_| VerifyError::Malformed)?;
    let body = evidence.body;
    let anchor = trust.live_anchor(&body.anchor_id)?;
    if !laxity.skip_signature
        && !verify_signature(
            &Address(anchor.public_key),
            &signing_message(&body),
            &evidence.signature,
        )
    {
        return Err(VerifyError::BadSignature);
    }
    let (challenge_id, session_id, executor, task_id, purpose) = if laxity.bind_from_challenge {
        (
            challenge.challenge_id,
            challenge.session_id,
            challenge.executor,
            challenge.task_id,
            challenge.purpose,
        )
    } else {
        if body.challenge_id != challenge.challenge_id.0 || body.nonce != challenge.nonce {
            return Err(VerifyError::ChallengeMismatch);
        }
        if body.session_id != challenge.session_id.0 {
            return Err(VerifyError::SessionMismatch);
        }
        if body.executor != challenge.executor.0 {
            return Err(VerifyError::ExecutorMismatch);
        }
        if body.task_id != challenge.task_id {
            return Err(VerifyError::TaskMismatch);
        }
        let purpose = Purpose::from_tag(body.purpose).ok_or(VerifyError::Malformed)?;
        if purpose != challenge.purpose {
            return Err(VerifyError::PurposeMismatch);
        }
        (
            super::ChallengeId(body.challenge_id),
            SessionId(body.session_id),
            Address(body.executor),
            body.task_id,
            purpose,
        )
    };
    if now < body.issued_at {
        return Err(VerifyError::EvidenceNotYetValid);
    }
    if now > body.expires_at {
        return Err(VerifyError::EvidenceExpired);
    }
    if now - body.issued_at > trust.max_evidence_age_secs {
        return Err(VerifyError::EvidenceTooOld);
    }
    let verified = VerifiedAttestation {
        verifier: REFERENCE_VERIFIER_KIND.to_owned(),
        format: REFERENCE_FORMAT.to_owned(),
        security_label: SecurityLabel::ReferenceAttested,
        trust_anchor: body.anchor_id.clone(),
        session_id,
        executor,
        task_id,
        challenge_id,
        purpose,
        issued_at: body.issued_at,
        expires_at: body.expires_at,
        environment_type: body.environment_type.clone(),
        measurement: body.measurement,
        security_version: body.security_version,
        debug_disabled: body.debug_disabled,
        environment_key: body.environment_key,
        evidence_reference: envelope.reference(),
    };
    trust.check_measurement(&verified)?;
    Ok(verified)
}

/// The reference verifier: strict verification of the reference format.
/// `production_grade()` is `false` and its label says why.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReferenceAttestationVerifier;

impl AttestationVerifier for ReferenceAttestationVerifier {
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
        verify_reference(envelope, challenge, trust, now, Laxity::STRICT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_debug_never_shows_the_seed() {
        let root = ReferenceAttestationRoot::from_seed(&[0x77u8; 32], "test-root");
        let s = format!("{root:?}");
        assert!(!s.contains(&hex::encode([0x77u8; 32])));
        assert!(s.contains("test-root"));
    }

    #[test]
    fn reference_label_is_explicit() {
        assert!(REFERENCE_LABEL.contains("TEST ONLY"));
        assert!(REFERENCE_LABEL.contains("not hardware attestation"));
        assert!(!ReferenceAttestationVerifier.production_grade());
    }
}
