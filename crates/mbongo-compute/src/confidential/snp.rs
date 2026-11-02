//! AMD SEV-SNP attestation adapter (`snp-adapter-v1`, the first backend of
//! `hardware-attestation-v1`).
//!
//! The first **hardware-backed** [`AttestationVerifier`]: real evidence is
//! the fixed 1,184-byte SEV-SNP attestation report a guest obtains from its
//! platform security processor through `/dev/sev-guest`, signed by the
//! chip's Versioned Chip Endorsement Key (VCEK), which chains through the
//! AMD SEV Signing Key (ASK) to the AMD Root Key (ARK). Parsing, chain and
//! signature verification are done by the official VirTEE/AMD `sev` crate
//! with pure-Rust cryptography; the ARK and ASK are the ones pinned in that
//! crate, referenced from the K [`TrustPolicy`] by the BLAKE3 digest of
//! their PEM, so a deployment still decides which roots it trusts and can
//! revoke one.
//!
//! What the adapter adds on top of the crate, and what makes parsing
//! different from attestation:
//!
//! - **binding**: `REPORT_DATA` must equal
//!   `SHA-512(domain || challenge_id || nonce || session_id || executor ||
//!   task_id || purpose || environment_public_key)` for the K challenge the
//!   evidence answers — the guest chose that value before the platform
//!   signed it, so the challenge, the session, the executor, the task, the
//!   purpose and the environment key are all covered by the signature;
//! - **platform policy**: guest policy bits (debug, migration), SMT state,
//!   VMPL, report version, per-component TCB minimums, guest SVN, a
//!   chip-id denylist, the K measurement allowlist over the BLAKE3 digest of
//!   the 48-byte launch measurement, a bounded validity window and, when
//!   the clock is wall time, the VCEK's X.509 validity;
//! - **normalization** into the K [`VerifiedAttestation`] with the
//!   `HARDWARE_ATTESTED` label, so J and the release authority never see a
//!   vendor structure.
//!
//! **No test root exists in this adapter.** The only roots it knows are the
//! AMD ARKs; the reference test root of [`super::reference`] is not a
//! format it understands. `production_grade()` is `true` only under a
//! strict [`SnpPolicy`].
//!
//! What this adapter does **not** decide: whether the evidence satisfies
//! the workload's policy (J), whether a release happens (the K release
//! authority), or whether the output is correct (nothing does, #52).

use std::collections::BTreeSet;
use std::fmt;

use parity_scale_codec::{Decode, Encode};
use sev::certs::snp::{builtin, ca, Certificate, Chain, Verifiable};
use sev::firmware::guest::AttestationReport;
use sev::parser::ByteParser;
use sha2::{Digest, Sha512};

use super::{
    AttestationChallenge, AttestationVerifier, EvidenceEnvelope, SecurityLabel, TrustAnchor,
    TrustPolicy, VerifiedAttestation, VerifyError,
};

/// The hardware attestation contract version (non-consensus).
pub const HARDWARE_ATTESTATION_VERSION: &str = "hardware-attestation-v1";
/// This adapter's version (non-consensus).
pub const SNP_ADAPTER_VERSION: &str = "snp-adapter-v1";
/// The evidence format this adapter understands.
pub const SNP_FORMAT: &str = "amd-sev-snp-report:v1";
/// This adapter's verifier kind.
pub const SNP_VERIFIER_KIND: &str = "amd-sev-snp-verifier";
/// Abstract environment type reported for an SNP guest.
pub const SNP_ENVIRONMENT_TYPE: &str = "amd-sev-snp-guest";
/// The size of an SEV-SNP attestation report.
pub const SNP_REPORT_LEN: usize = 1184;
/// Domain separator for `REPORT_DATA`.
const DOMAIN_REPORT_DATA: &[u8] = b"mbongo:snp-report-data:v1";

/// AMD product line whose root the adapter pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub enum SnpProduct {
    /// EPYC 7003 (Zen 3).
    Milan,
    /// EPYC 9004 (Zen 4).
    Genoa,
    /// EPYC 9005 (Zen 5).
    Turin,
}

impl SnpProduct {
    fn ark_pem(self) -> &'static [u8] {
        match self {
            Self::Milan => builtin::milan::ARK,
            Self::Genoa => builtin::genoa::ARK,
            Self::Turin => builtin::turin::ARK,
        }
    }

    fn ask_pem(self) -> &'static [u8] {
        match self {
            Self::Milan => builtin::milan::ASK,
            Self::Genoa => builtin::genoa::ASK,
            Self::Turin => builtin::turin::ASK,
        }
    }

    /// The K trust-anchor id for this product's ARK.
    pub fn anchor_id(self) -> &'static str {
        match self {
            Self::Milan => "amd-ark-milan",
            Self::Genoa => "amd-ark-genoa",
            Self::Turin => "amd-ark-turin",
        }
    }

    /// BLAKE3 of the pinned ARK PEM: the value a deployment's K
    /// [`TrustAnchor::public_key`] must carry for this product.
    pub fn ark_digest(self) -> [u8; 32] {
        *blake3::hash(self.ark_pem()).as_bytes()
    }

    /// The K trust anchor for this product.
    pub fn trust_anchor(self) -> TrustAnchor {
        TrustAnchor {
            anchor_id: self.anchor_id().to_owned(),
            public_key: self.ark_digest(),
            revoked: false,
        }
    }

    fn ca_chain(self) -> Result<ca::Chain, VerifyError> {
        let ark = Certificate::from_pem(self.ark_pem()).map_err(|_| VerifyError::UntrustedRoot)?;
        let ask = Certificate::from_pem(self.ask_pem()).map_err(|_| VerifyError::UntrustedRoot)?;
        Ok(ca::Chain { ark, ask })
    }
}

/// The payload of an [`EvidenceEnvelope`] in the [`SNP_FORMAT`] format.
/// The environment public key travels beside the report because
/// `REPORT_DATA` carries its hash, not the key.
#[derive(Clone, PartialEq, Eq, Encode, Decode)]
pub struct SnpEvidence {
    /// Product line (selects the pinned root).
    pub product: SnpProduct,
    /// The raw attestation report, exactly [`SNP_REPORT_LEN`] bytes.
    pub report: Vec<u8>,
    /// The chip's VCEK certificate, DER.
    pub vcek_der: Vec<u8>,
    /// The X25519 public key the guest generated inside itself and bound
    /// into `REPORT_DATA`.
    pub environment_key: [u8; 32],
}

impl SnpEvidence {
    /// Serialises as an envelope payload.
    pub fn to_payload(&self) -> Vec<u8> {
        self.encode()
    }

    /// Parses an envelope payload.
    pub fn from_payload(payload: &[u8]) -> Result<Self, VerifyError> {
        Self::decode(&mut &payload[..]).map_err(|_| VerifyError::Malformed)
    }
}

impl fmt::Debug for SnpEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SnpEvidence({:?}, report {} bytes ref {}, vcek {} bytes, environment key {})",
            self.product,
            self.report.len(),
            hex::encode(&blake3::hash(&self.report).as_bytes()[..8]),
            self.vcek_der.len(),
            hex::encode(&self.environment_key[..8])
        )
    }
}

/// Per-component TCB minimum. Compared component-wise; a report passes only
/// if every component of both its reported and current TCB is at least
/// the minimum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TcbMinimum {
    /// Bootloader SVN.
    pub bootloader: u8,
    /// TEE SVN.
    pub tee: u8,
    /// SNP firmware SVN.
    pub snp: u8,
    /// Microcode SVN.
    pub microcode: u8,
}

/// Deployment platform policy for SNP guests. Non-consensus. This is the
/// vendor-specific half of the trust policy; the vendor-neutral half
/// (anchors, measurement allowlist, windows) stays in the K [`TrustPolicy`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // deployment flags, each independently meaningful
pub struct SnpPolicy {
    /// Product line.
    pub product: SnpProduct,
    /// Minimum TCB, component-wise.
    pub min_tcb: TcbMinimum,
    /// Minimum guest SVN.
    pub min_guest_svn: u32,
    /// Accept a guest whose policy permits debugging. Never in production.
    pub allow_debug: bool,
    /// Accept a guest whose policy permits migration by a migration agent.
    pub allow_migration: bool,
    /// Accept a platform with SMT enabled (a side-channel consideration).
    pub allow_smt: bool,
    /// Highest VMPL accepted (0 = only the most privileged level).
    pub max_vmpl: u32,
    /// Lowest report version accepted.
    pub min_report_version: u32,
    /// Chip ids the deployment refuses (a per-chip revocation list).
    pub revoked_chips: BTreeSet<[u8; 64]>,
    /// Enforce the VCEK's X.509 validity against `now` as Unix seconds.
    /// True in production; false only where the clock is not wall time.
    pub enforce_certificate_validity: bool,
    /// Longest window an envelope may claim for a report.
    pub max_report_validity_secs: u64,
}

impl SnpPolicy {
    /// A strict production policy: no debug, no migration, no SMT, VMPL 0,
    /// report version 2 or later, certificate validity enforced, a
    /// ten-minute window.
    pub fn production(product: SnpProduct, min_tcb: TcbMinimum, min_guest_svn: u32) -> Self {
        Self {
            product,
            min_tcb,
            min_guest_svn,
            allow_debug: false,
            allow_migration: false,
            allow_smt: false,
            max_vmpl: 0,
            min_report_version: 2,
            revoked_chips: BTreeSet::new(),
            enforce_certificate_validity: true,
            max_report_validity_secs: 600,
        }
    }

    /// Whether this policy is strict enough for the verifier to report
    /// production grade: debug and migration refused, VMPL 0 only, and
    /// certificate validity enforced.
    pub fn is_strict(&self) -> bool {
        !self.allow_debug
            && !self.allow_migration
            && self.max_vmpl == 0
            && self.enforce_certificate_validity
    }
}

/// The `REPORT_DATA` a guest must place in its report to answer
/// `challenge` while offering `environment_key`:
/// `SHA-512(domain || challenge_id || nonce || session_id || executor ||
/// task_id || purpose || environment_key)`. SHA-512 fills the 64-byte field
/// exactly; no truncation.
pub fn report_data_binding(
    challenge: &AttestationChallenge,
    environment_key: &[u8; 32],
) -> [u8; 64] {
    let mut h = Sha512::new();
    h.update(DOMAIN_REPORT_DATA);
    h.update(challenge.challenge_id.0);
    h.update(challenge.nonce);
    h.update(challenge.session_id.0);
    h.update(challenge.executor.0);
    h.update(challenge.task_id);
    h.update([challenge.purpose.tag()]);
    h.update(environment_key);
    let out = h.finalize();
    let mut rd = [0u8; 64];
    rd.copy_from_slice(&out);
    rd
}

/// Parses a raw report. Length must be exact.
pub fn parse_report(bytes: &[u8]) -> Result<AttestationReport, VerifyError> {
    if bytes.len() != SNP_REPORT_LEN {
        return Err(VerifyError::Malformed);
    }
    AttestationReport::from_bytes(bytes).map_err(|_| VerifyError::Malformed)
}

/// Packs a TCB into the AMD `TCB_VERSION` u64 layout (bootloader byte 0,
/// TEE byte 1, SNP byte 6, microcode byte 7) for K's coarse
/// `security_version`. The component-wise check is the authoritative one.
fn packed_tcb(t: sev::firmware::host::TcbVersion) -> u64 {
    u64::from(t.bootloader)
        | (u64::from(t.tee) << 8)
        | (u64::from(t.snp) << 48)
        | (u64::from(t.microcode) << 56)
}

fn tcb_at_least(t: sev::firmware::host::TcbVersion, min: TcbMinimum) -> bool {
    t.bootloader >= min.bootloader
        && t.tee >= min.tee
        && t.snp >= min.snp
        && t.microcode >= min.microcode
}

/// Stage 1: chain and signature. Resolves the product's pinned root, checks
/// that the K trust policy names that root by its digest and has not revoked
/// it, verifies VCEK → ASK → ARK, then the report's signature by the VCEK.
/// Returns the parsed report and the VCEK. Establishes genuineness only;
/// nothing here is a binding or a policy check.
pub fn verify_chain_and_signature(
    evidence: &SnpEvidence,
    trust: &TrustPolicy,
) -> Result<(AttestationReport, Certificate), VerifyError> {
    let anchor = trust.live_anchor(evidence.product.anchor_id())?;
    if anchor.public_key != evidence.product.ark_digest() {
        return Err(VerifyError::UntrustedRoot);
    }
    let report = parse_report(&evidence.report)?;
    let vcek = Certificate::from_der(&evidence.vcek_der).map_err(|_| VerifyError::Malformed)?;
    let chain = Chain {
        ca: evidence.product.ca_chain()?,
        vek: vcek,
    };
    (&chain).verify().map_err(|_| VerifyError::UntrustedRoot)?;
    (&chain, &report).verify().map_err(|_| VerifyError::BadSignature)?;
    Ok((report, chain.vek))
}

/// Stage 2: binding, platform policy and normalization over an
/// already-genuine report. Pure; takes `now` explicitly.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn check_platform_policy(
    report: &AttestationReport,
    vcek: &Certificate,
    evidence: &SnpEvidence,
    envelope: &EvidenceEnvelope,
    challenge: &AttestationChallenge,
    policy: &SnpPolicy,
    trust: &TrustPolicy,
    now: u64,
) -> Result<VerifiedAttestation, VerifyError> {
    check_platform_policy_lax(
        report,
        vcek,
        evidence,
        envelope,
        challenge,
        policy,
        trust,
        now,
        SnpLaxity::STRICT,
    )
}

/// How lax a stage-2 check is. Only [`SnpLaxity::STRICT`] is used by the
/// adapter; the other forms exist for the negative-control verifiers in the
/// test group. Never a production option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnpLaxity {
    /// Skip the `REPORT_DATA` binding check.
    pub skip_binding: bool,
    /// Skip the measurement allowlist.
    pub skip_measurement: bool,
}

impl SnpLaxity {
    /// The strict check.
    pub const STRICT: Self = Self {
        skip_binding: false,
        skip_measurement: false,
    };
}

/// Stage 2 with laxity. See [`check_platform_policy`].
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn check_platform_policy_lax(
    report: &AttestationReport,
    vcek: &Certificate,
    evidence: &SnpEvidence,
    envelope: &EvidenceEnvelope,
    challenge: &AttestationChallenge,
    policy: &SnpPolicy,
    trust: &TrustPolicy,
    now: u64,
    laxity: SnpLaxity,
) -> Result<VerifiedAttestation, VerifyError> {
    if evidence.product != policy.product {
        return Err(VerifyError::UntrustedRoot);
    }
    if report.version < policy.min_report_version {
        return Err(VerifyError::PlatformStateUnacceptable);
    }
    // Binding: the guest chose REPORT_DATA before the platform signed it.
    if !laxity.skip_binding
        && report.report_data != report_data_binding(challenge, &evidence.environment_key)
    {
        return Err(VerifyError::ChallengeMismatch);
    }
    if policy.revoked_chips.contains(&report.chip_id) {
        return Err(VerifyError::RevokedPlatform);
    }
    if report.vmpl > policy.max_vmpl {
        return Err(VerifyError::PlatformStateUnacceptable);
    }
    if report.policy.debug_allowed() && !policy.allow_debug {
        return Err(VerifyError::DebugEnabled);
    }
    if report.policy.migrate_ma_allowed() && !policy.allow_migration {
        return Err(VerifyError::PlatformStateUnacceptable);
    }
    if report.plat_info.smt_enabled() && !policy.allow_smt {
        return Err(VerifyError::PlatformStateUnacceptable);
    }
    if !tcb_at_least(report.reported_tcb, policy.min_tcb)
        || !tcb_at_least(report.current_tcb, policy.min_tcb)
    {
        return Err(VerifyError::SecurityVersionTooLow);
    }
    if report.guest_svn < policy.min_guest_svn {
        return Err(VerifyError::SecurityVersionTooLow);
    }
    let measurement = *blake3::hash(&report.measurement).as_bytes();
    if !laxity.skip_measurement {
        if trust.measurement.revoked.contains(&measurement) {
            return Err(VerifyError::MeasurementRevoked);
        }
        if !trust.measurement.allowed.contains(&measurement) {
            return Err(VerifyError::MeasurementNotAllowed);
        }
    }
    // Window: the report carries no time; the envelope's claim is bounded
    // by policy, and the K challenge is the real freshness (§16 of the
    // architecture).
    if now < envelope.issued_at {
        return Err(VerifyError::EvidenceNotYetValid);
    }
    let expires_at = envelope
        .expires_at
        .min(envelope.issued_at.saturating_add(policy.max_report_validity_secs));
    if now > expires_at {
        return Err(VerifyError::EvidenceExpired);
    }
    if policy.enforce_certificate_validity {
        let x509: x509_cert::Certificate = vcek.clone().into();
        let nb = x509.tbs_certificate.validity.not_before.to_unix_duration().as_secs();
        let na = x509.tbs_certificate.validity.not_after.to_unix_duration().as_secs();
        if now < nb || now > na {
            return Err(VerifyError::ExpiredCollateral);
        }
    }
    Ok(VerifiedAttestation {
        verifier: SNP_VERIFIER_KIND.to_owned(),
        format: SNP_FORMAT.to_owned(),
        security_label: SecurityLabel::HardwareAttested,
        trust_anchor: evidence.product.anchor_id().to_owned(),
        session_id: challenge.session_id,
        executor: challenge.executor,
        task_id: challenge.task_id,
        challenge_id: challenge.challenge_id,
        purpose: challenge.purpose,
        issued_at: envelope.issued_at,
        expires_at,
        environment_type: SNP_ENVIRONMENT_TYPE.to_owned(),
        measurement,
        security_version: packed_tcb(report.reported_tcb),
        debug_disabled: !report.policy.debug_allowed(),
        environment_key: evidence.environment_key,
        evidence_reference: envelope.reference(),
    })
}

/// The SEV-SNP verifier.
pub struct SnpVerifier {
    policy: SnpPolicy,
}

impl fmt::Debug for SnpVerifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SnpVerifier({:?}, strict {}, min_tcb {:?})",
            self.policy.product,
            self.policy.is_strict(),
            self.policy.min_tcb
        )
    }
}

impl SnpVerifier {
    /// A verifier under `policy`.
    pub fn new(policy: SnpPolicy) -> Self {
        Self { policy }
    }

    /// The policy.
    pub fn policy(&self) -> &SnpPolicy {
        &self.policy
    }

    fn verify_with(
        &self,
        envelope: &EvidenceEnvelope,
        challenge: &AttestationChallenge,
        trust: &TrustPolicy,
        now: u64,
        skip_signature: bool,
        laxity: SnpLaxity,
    ) -> Result<VerifiedAttestation, VerifyError> {
        if envelope.format != SNP_FORMAT {
            return Err(VerifyError::UnknownFormat);
        }
        let evidence = SnpEvidence::from_payload(&envelope.payload)?;
        let (report, vcek) = if skip_signature {
            // Negative control only: parse without any chain or signature
            // check.
            let anchor = trust.live_anchor(evidence.product.anchor_id())?;
            if anchor.public_key != evidence.product.ark_digest() {
                return Err(VerifyError::UntrustedRoot);
            }
            (
                parse_report(&evidence.report)?,
                Certificate::from_der(&evidence.vcek_der).map_err(|_| VerifyError::Malformed)?,
            )
        } else {
            verify_chain_and_signature(&evidence, trust)?
        };
        check_platform_policy_lax(
            &report,
            &vcek,
            &evidence,
            envelope,
            challenge,
            &self.policy,
            trust,
            now,
            laxity,
        )
    }
}

impl AttestationVerifier for SnpVerifier {
    fn kind(&self) -> &str {
        SNP_VERIFIER_KIND
    }

    fn formats(&self) -> Vec<String> {
        vec![SNP_FORMAT.to_owned()]
    }

    fn security_label(&self) -> SecurityLabel {
        SecurityLabel::HardwareAttested
    }

    fn production_grade(&self) -> bool {
        self.policy.is_strict()
    }

    fn verify(
        &self,
        envelope: &EvidenceEnvelope,
        challenge: &AttestationChallenge,
        trust: &TrustPolicy,
        now: u64,
    ) -> Result<VerifiedAttestation, VerifyError> {
        self.verify_with(envelope, challenge, trust, now, false, SnpLaxity::STRICT)
    }
}

/// Deliberately unsafe SNP verifiers, so the hardware test group can prove
/// it catches them. Never registered anywhere but a test.
pub mod negative {
    use super::{
        AttestationChallenge, AttestationVerifier, EvidenceEnvelope, SecurityLabel, SnpLaxity,
        SnpPolicy, SnpVerifier, TrustPolicy, VerifiedAttestation, VerifyError, SNP_FORMAT,
        SNP_VERIFIER_KIND,
    };

    /// Skips chain and signature verification.
    #[derive(Debug)]
    pub struct SignatureBlindSnpVerifier(pub SnpVerifier);
    /// Skips the `REPORT_DATA` binding check.
    #[derive(Debug)]
    pub struct BindingBlindSnpVerifier(pub SnpVerifier);
    /// Skips the measurement allowlist.
    #[derive(Debug)]
    pub struct MeasurementBlindSnpVerifier(pub SnpVerifier);

    macro_rules! blind {
        ($t:ident, $skip_sig:expr, $lax:expr) => {
            impl $t {
                /// Under `policy`.
                pub fn new(policy: SnpPolicy) -> Self {
                    Self(SnpVerifier::new(policy))
                }
            }
            impl AttestationVerifier for $t {
                fn kind(&self) -> &str {
                    SNP_VERIFIER_KIND
                }
                fn formats(&self) -> Vec<String> {
                    vec![SNP_FORMAT.to_owned()]
                }
                fn security_label(&self) -> SecurityLabel {
                    SecurityLabel::HardwareAttested
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
                    self.0.verify_with(envelope, challenge, trust, now, $skip_sig, $lax)
                }
            }
        };
    }

    blind!(SignatureBlindSnpVerifier, true, SnpLaxity::STRICT);
    blind!(
        BindingBlindSnpVerifier,
        false,
        SnpLaxity {
            skip_binding: true,
            skip_measurement: false
        }
    );
    blind!(
        MeasurementBlindSnpVerifier,
        false,
        SnpLaxity {
            skip_binding: false,
            skip_measurement: true
        }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn products_have_distinct_pinned_roots() {
        assert_ne!(
            SnpProduct::Milan.ark_digest(),
            SnpProduct::Genoa.ark_digest()
        );
        assert_ne!(
            SnpProduct::Genoa.ark_digest(),
            SnpProduct::Turin.ark_digest()
        );
        for p in [SnpProduct::Milan, SnpProduct::Genoa, SnpProduct::Turin] {
            let chain = p.ca_chain().expect("pinned roots parse");
            assert!(
                (&chain.ark, &chain.ask).verify().is_ok(),
                "{p:?} ARK signs ASK"
            );
        }
    }

    #[test]
    fn strictness_defines_production_grade() {
        let strict = SnpPolicy::production(SnpProduct::Milan, TcbMinimum::default(), 0);
        assert!(SnpVerifier::new(strict.clone()).production_grade());
        let mut lax = strict;
        lax.allow_debug = true;
        assert!(!SnpVerifier::new(lax).production_grade());
    }
}
