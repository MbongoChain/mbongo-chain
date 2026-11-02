# Hardware attestation adapter: the first production backend (AMD SEV-SNP)

> **Document type:** Architecture — backend contract for off-chain components
> **Status:** Architectural authority for how real hardware attestation
> evidence enters the K confidential authorization path: the selected
> backend, its trust chain, the binding of the K challenge and the
> environment key into signed platform evidence, the platform and TCB
> policy, the normalization into K's `VerifiedAttestation`, the live-guest
> test, and the exact confidentiality claim the backend supports. Defines
> no consensus rule, no RPC method, no SDK type, no wire format and no
> transport; where anything here appears to conflict with a normative
> source below, the normative source wins.
> **Normative sources:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md)
> (Released), [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md),
> [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md),
> [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) (FROZEN),
> [`rpc_v0.3.md`](../specs/rpc_v0.3.md) (FROZEN)
> **Parent architecture:** [`compute-confidential-attestation-key-release.md`](compute-confidential-attestation-key-release.md)
> ("K": K §5 challenge, K §6 envelope, K §7 verifier interface, K §8 trust
> policy, K §10 normalized result, K §13 release, K §24 reference verifier,
> K §25 adapter extension, K §29 this gate),
> [`compute-provider-capability-policy.md`](compute-provider-capability-policy.md)
> ("J": J §6 evidence ladder, J §14 CONFIDENTIAL) and
> [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md)
> (parent §10 confidential compute and its limits, parent §11 attestation
> boundary, P13 vendor neutrality). This document implements K §25 for one
> backend, changes nothing in K, J, E, F or the protocol, and yields to all
> of them on any conflict.
> **Contract versions:** `hardware-attestation-v1` (what any hardware
> adapter must establish) and `snp-adapter-v1` (this backend) —
> architecture / control-plane versions, non-consensus (§22).

This is K-HW ([#144](https://github.com/MbongoChain/mbongo-chain/issues/144)),
on K ([#142](https://github.com/MbongoChain/mbongo-chain/issues/142)).

**Status of the gate, stated first.** The adapter, its trust chain, its
binding, its policy and its normalization are implemented and proven on
**real, captured** AMD EPYC Milan evidence (§16.1). The **live** run — a
SEV-SNP guest answering a fresh Mbongo challenge, verified, released,
decrypted inside the guest — has **not** been performed: no SEV-SNP guest
is reachable from the development environment, and provisioning one is a
paid action reserved for the maintainer (§16.3). Until that run is
recorded, the gate is `BLOCKED_NO_ACCESSIBLE_HARDWARE`, the operational
confidential path remains `REFERENCE_ATTESTED`, and
`PROVIDER_RESISTANT_CONFIDENTIALITY_PROVEN = NO` (§20). Nothing below
should be read as claiming otherwise.

---

## 1. Purpose

K left one thing outstanding: every artefact of its confidential path is
`REFERENCE_ATTESTED` — a software environment endorsed by a test root — so
the property the whole path exists for, that the provider operating the
environment cannot read the plaintext or the content key, was not proven
(K §23). This document adds the first real, hardware-backed
`AttestationVerifier` behind the boundary K reserved (K §7, K §25), so
that the same challenge → evidence → verification → J → release → key path
runs on platform-generated evidence, and states precisely what that
evidence does and does not establish.

---

## 2. Scope

In scope: one backend, AMD SEV-SNP (§3); its evidence format, trust
chain, binding, policy, normalization, failure classes, tests, and the
live-guest harness. Preserved unchanged: the K challenge, envelope,
`VerifiedAttestation`, `TrustPolicy`, `ReleaseAuthority`,
`KeyReleaseAuthority` and release state machine; J; E; F; `ComputeTask`;
`Receipt`; rpc_v0.3; the SDK wire. Out of scope (§21): every other
backend, GPU execution and GPU memory confidentiality, AI inference,
economics, durable authority state, and any claim about output
correctness.

---

## 3. Selected backend

| Backend | Reachable hardware | Evidence | Challenge binding | Measurement and security state | Offline verification | Decision |
|---|---|---|---|---|---|---|
| **AMD SEV-SNP** | none from the development host; any SNP guest (EPYC Milan / Genoa / Turin bare metal; GCP n2d/c2d/c3d, Azure DCasv5/ECasv5 confidential VMs) | fixed 1,184-byte attestation report from the platform security processor, ECDSA P-384 by the per-chip VCEK, chained VCEK → ASK → ARK | 64-byte guest-chosen `REPORT_DATA`, inside the signed report | 48-byte launch measurement; guest policy (debug, migration, SMT, single socket); reported, current, committed and launch TCB (bootloader, TEE, SNP, microcode); VMPL; platform info (SMT, TSME) | **yes** — ARK/ASK pinned in the official `sev` crate; VCEK carried by the guest (extended report) or cached from AMD KDS | **selected** |
| Intel TDX | none; TDX guests (Sapphire Rapids+) | DCAP TD quote, ECDSA P-256, PCK chain to the Intel SGX root | 64-byte `REPORTDATA` | MRTD, RTMR0–3, TCB status via collateral | hybrid: Intel PCS/PCCS collateral (TCB info, QE identity, CRLs) required | deferred |
| NVIDIA confidential GPU | none (H100/H200 in CC mode plus a CPU TEE) | GPU quote through NRAS or a local verifier | yes | GPU firmware measurements | online by default | deferred; composes with a CPU TEE; GPU memory is its own boundary (§4) |
| AWS Nitro Enclaves | none | CBOR/COSE attestation document to the AWS Nitro root | 64-byte user data plus nonce | PCR0–8 | yes | deferred; cloud-specific format |
| TPM 2.0 on the development laptop | present | TPM quote over PCRs, EK chain | qualifying data | **host** boot PCRs | yes | rejected: attests the host's boot state; the host operator still reads memory (mission §66) |

**Selection reason.** SEV-SNP is the only candidate with a fully offline
verification path against vendor roots pinned in an official, maintained
library (VirTEE/AMD `sev` 8.0.0 with pure-Rust cryptography — it builds on
the Windows development host without OpenSSL), a guest-chosen 64-byte
binding field inside the signature, explicit debug and migration policy
bits, per-component TCB, and real captured reports available publicly for
parser and chain fixtures. Alternatives are deferred, not rejected: each
implements the same trait (§23).

`FIRST_HARDWARE_BACKEND = AMD SEV-SNP` · `VERIFIER_LIBRARY = sev 8.0.0
(VirTEE/AMD), features snp + crypto_nossl` · `VERIFIER_SOURCE =
crates.io, pinned in Cargo.lock` · `VERIFICATION_MODE = OFFLINE` (§11).

---

## 4. Security boundary

What SEV-SNP protects, under AMD's documented threat model: guest memory
is encrypted with a per-VM key held by the platform security processor,
integrity-protected against replay and remapping by the reverse map table,
and the hypervisor and host operator cannot read or undetectably modify
guest memory or registers. Attestation lets a remote party verify which
guest image booted (measurement), under which policy, on which platform
firmware (TCB).

| Boundary | SEV-SNP guest | Note |
|---|---|---|
| encrypted at rest (data plane) | K, unchanged | independent of the backend |
| encrypted in transit | K, unchanged | independent of the backend |
| protected CPU memory (guest RAM) | **yes** | the property this backend supplies |
| protected accelerator / GPU memory | **no** | a CPU confidential VM does not protect a GPU; `CONFIDENTIAL_GPU_PATH = NO` |
| protection from the guest's own software | no | code inside the guest sees plaintext; measurement is what makes that code known |

What SEV-SNP does **not** protect against, and this document does not
claim: compromise of AMD hardware or firmware; side channels (timing,
cache, power, ciphertext side channels — `ciphertext_hiding` is a
platform flag the policy may require in a later revision); malicious
firmware a trust policy accepts; host denial of service (the host can
stop, pause or refuse to schedule the guest); application-level
exfiltration by code inside the measured guest; traffic analysis.

---

## 5. Hardware evidence format

An `EvidenceEnvelope` (K §6) with `format = amd-sev-snp-report:v1`,
`verifier_kind = amd-sev-snp-verifier` and a SCALE-encoded payload:

```
SnpEvidence {
  product          Milan | Genoa | Turin      — selects the pinned root
  report           1,184 bytes                — the platform-signed attestation report
  vcek_der         DER                        — the chip's VCEK certificate
  environment_key  32 bytes                   — the guest's X25519 public key, bound in REPORT_DATA
}
```

The report is parsed by the official crate (`AttestationReport`,
ABI-versioned). Fields the adapter reads: version, guest SVN, guest
policy, VMPL, platform info, reported and current TCB, `REPORT_DATA`,
measurement, chip id, signature. Successful parsing establishes
nothing; every claim below is a separate check (§12).

---

## 6. Trust chain

```
attestation report  ──signed by──▶  VCEK (per chip, per TCB)
                                     ──signed by──▶  ASK (AMD SEV Signing Key, per product)
                                                     ──signed by──▶  ARK (AMD Root Key, per product, self-signed)
```

- The ARK and ASK are the ones pinned in the `sev` crate for each product
  line. The adapter never accepts a root it is handed in evidence.
- The K `TrustPolicy` names the root the deployment trusts as an anchor
  entry whose id is `amd-ark-<product>` and whose 32-byte `public_key` is
  the **BLAKE3 digest of the pinned ARK PEM** (`SnpProduct::ark_digest`).
  The adapter refuses evidence whose product has no anchor entry
  (`UntrustedRoot`), whose entry carries a different digest
  (`UntrustedRoot`), or whose entry is revoked (`RevokedRoot`). The
  release authority re-checks the same entry (K §13), so a deployment
  revokes a root in one place.
- The VCEK arrives with the evidence: the guest obtains it from the
  extended report (the hypervisor's certificate table) or from AMD KDS. It
  is never trusted by presence: VCEK → ASK → ARK is verified by the crate,
  then the report's signature by the VCEK. A VCEK of another chip chains
  correctly and fails the report signature (HW10).
- No self-declared root id, string label or provider-controlled
  certificate participates in trust (HW2, HW9).

---

## 7. Challenge and `REPORT_DATA` binding

The guest places, in the 64-byte `REPORT_DATA` the platform signs:

```
REPORT_DATA = SHA-512( "mbongo:snp-report-data:v1"
                       || challenge_id (32) || nonce (32)
                       || session_id (32)   || executor (32)
                       || task_id (32)      || purpose (1)
                       || environment_public_key (32) )
```

SHA-512 fills the field exactly; there is no truncation. The adapter
recomputes the value from the K challenge record and the evidence's
environment key and compares it byte for byte; a mismatch is
`ChallengeMismatch`. Because the platform signs `REPORT_DATA`, the
challenge nonce (freshness), the session, the executor, the task, the
purpose and the environment key are all covered by the hardware signature
(HW3–HW7). A report whose `REPORT_DATA` was chosen for anything else —
including every captured fixture — cannot satisfy any Mbongo challenge
(HW02). The K challenge (single-use, TTL-bound, executor-gated, K §5) is
the real freshness; §10 says which other freshness dimensions are
enforced.

---

## 8. Environment key binding

The guest generates its X25519 environment key **inside the guest** and
binds the public half into `REPORT_DATA` before requesting the report. The
key travels beside the report in `SnpEvidence.environment_key`; the
adapter's binding check (§7) ties it to the signed report, so a host that
substitutes a public key after quoting produces a `ChallengeMismatch`
(HW07). The `VerifiedAttestation.environment_key` the release authority
records is that bound key, and the key release authority wraps the content
key to exactly it (K §14, §15): a host-held key cannot unwrap (HW21).

The private half never leaves the guest: it lives in encrypted guest
memory, which is the property §4 supplies. With the reference environment
(K §24) the same construction holds mechanically but the memory is not
protected — which is why the claim in §20 depends on the live run.

---

## 9. Measurement mapping

| SNP field | K field | Rule |
|---|---|---|
| `MEASUREMENT` (48 bytes, launch digest of the guest image) | `VerifiedAttestation.measurement` = BLAKE3 of the 48 bytes | must be in `TrustPolicy.measurement.allowed` and not in `revoked` (`MeasurementNotAllowed` / `MeasurementRevoked`) |
| reported TCB (bootloader, TEE, SNP, microcode) | `security_version` = AMD `TCB_VERSION` u64 packing | component-wise ≥ `SnpPolicy.min_tcb` for **both** reported and current TCB (`SecurityVersionTooLow`); the packed value is informational |
| guest SVN | — | ≥ `min_guest_svn` |
| guest policy `DEBUG_ALLOWED` | `debug_disabled` = !debug | refused unless `allow_debug` (`DebugEnabled`) |
| guest policy `MIGRATE_MA_ALLOWED` | — | refused unless `allow_migration` (`PlatformStateUnacceptable`) |
| platform info `SMT_ENABLED` | — | refused unless `allow_smt` (`PlatformStateUnacceptable`) |
| VMPL | — | ≤ `max_vmpl` (`PlatformStateUnacceptable`) |
| report version | — | ≥ `min_report_version` (`PlatformStateUnacceptable`) |
| chip id | — | not in `revoked_chips` (`RevokedPlatform`) |
| `environment_type` | `amd-sev-snp-guest` | abstract; no vendor name is a policy key (P13, J §18) |

Meaningful vendor state is not flattened into one boolean: debug,
migration, SMT, VMPL and each TCB component are separate policy inputs.
An approved measurement means the guest image is one the deployment
expects; it says nothing about output correctness (P16, §21).

---

## 10. TCB and security policy; freshness dimensions

`SnpPolicy::production(product, min_tcb, min_guest_svn)`: no debug, no
migration, no SMT, VMPL 0 only, report version ≥ 2, certificate validity
enforced, a ten-minute report window. `production_grade()` is `true` only
when the policy is strict in this sense (`is_strict`); a lenient policy —
the one the fixture tests must use, because the captured guest allowed
debug — reports `false`. No wildcard-accept-all exists: an empty
measurement allowlist accepts nothing.

| Freshness dimension | Enforced by | How |
|---|---|---|
| K challenge freshness | release authority + adapter | single-use challenge, TTL, nonce in `REPORT_DATA` |
| report window | adapter | the envelope's `issued_at ≤ now ≤ expires_at`, with `expires_at` capped at `issued_at + max_report_validity_secs`; the report itself carries no time |
| VCEK certificate validity | adapter, when `enforce_certificate_validity` | `now` within the VCEK's X.509 `notBefore..notAfter` (`ExpiredCollateral`); requires a wall-clock `now` |
| ARK/ASK validity | pinned; rotated by a crate update (§11) | — |
| TCB status | adapter | component-wise minimums; AMD publishes TCB updates, the deployment raises `min_tcb` |
| revocation | adapter + K trust policy | chip denylist; measurement revocation; anchor revocation |

Not enforced (stated): AMD publishes no VCEK CRL; "revocation" of a chip
is a deployment denylist; `reported_tcb` lower than `current_tcb` is
permitted by AMD and accepted here when both meet the minimum.

---

## 11. Collateral and revocation

`VERIFICATION_MODE = OFFLINE`: the verifier needs no network. Roots are
pinned; the VCEK is part of the evidence and is validated by chain, so a
stale or forged VCEK fails signature verification rather than a freshness
check. A deployment that prefers to fetch VCEKs itself from AMD KDS caches
them per (chip id, reported TCB) — the certificate is deterministic for
that pair — and the adapter's certificate-validity check bounds the cache.
Root update: a new ARK/ASK for a product line arrives with a crate update
and a new anchor digest in the trust policy; the old digest is removed or
revoked. Intermediate rotation: AMD does not rotate the ASK independently
of a product line. Verifier outage cannot occur offline; the K
`VerifierUnavailable` and `TrustPolicyMissing` classes still apply when
the adapter is not registered or the policy is absent, and fail closed
(K §19).

---

## 12. Verifier adapter

`SnpVerifier` implements K's `AttestationVerifier` in two stages, both
required:

1. **`verify_chain_and_signature`** — anchor resolution and digest match
   (§6), report parse (exact length), VCEK parse, VCEK → ASK → ARK, report
   signature by the VCEK. Establishes genuineness only.
2. **`check_platform_policy`** — product matches policy, report version,
   `REPORT_DATA` binding (§7), chip denylist, VMPL, debug, migration, SMT,
   TCB and guest SVN minimums, measurement allowlist and revocation,
   report window, certificate validity, then normalization (§13). Pure;
   takes `now` explicitly.

Vendor-specific parsing lives entirely here (HW29). Neither J nor the
release authority sees a report; the authority re-checks the normalized
result against its own challenge record and policy exactly as it does for
the reference verifier (K §13), so a permissive adapter cannot widen what
is accepted beyond what only a signature can establish — which is why the
signature-blind negative control is caught (§16.2).

### 12.1 Failure taxonomy

K's `VerifyError` classes, with five added for hardware backends
(additive; no existing class or rule changed):

| Class | Raised when |
|---|---|
| `UnknownFormat` | not `amd-sev-snp-report:v1` |
| `Malformed` | payload, report length or VCEK DER does not parse |
| `UntrustedRoot` | no anchor for the product, digest mismatch, chain failure, product mislabelled |
| `RevokedRoot` | anchor revoked |
| `BadSignature` | the VCEK does not sign the report (tamper, other chip) |
| `ChallengeMismatch` | `REPORT_DATA` ≠ the binding for this challenge and environment key (covers session, executor, task, purpose, nonce and key substitution) |
| `RevokedPlatform` | chip id on the denylist |
| `PlatformStateUnacceptable` | migration allowed, SMT enabled, VMPL too high, report version too old, against policy |
| `DebugEnabled` | guest policy allows debug, against policy |
| `SecurityVersionTooLow` | a TCB component or the guest SVN below the minimum |
| `MeasurementNotAllowed` / `MeasurementRevoked` | allowlist |
| `EvidenceNotYetValid` / `EvidenceExpired` | report window |
| `ExpiredCollateral` | VCEK outside its validity, when enforced |
| `VerifierUnavailable` / `TrustPolicyMissing` | K, unchanged |
| `VendorServiceUnavailable` | reserved for online backends; never raised here |

---

## 13. Normalization to K

```
VerifiedAttestation {
  verifier            "amd-sev-snp-verifier"
  format              "amd-sev-snp-report:v1"
  security_label      HARDWARE_ATTESTED
  trust_anchor        "amd-ark-<product>"
  session_id, executor, task_id, challenge_id, purpose     — from the challenge the binding proved
  issued_at, expires_at                                     — the bounded report window
  environment_type    "amd-sev-snp-guest"
  measurement         BLAKE3(launch measurement)
  security_version    packed reported TCB
  debug_disabled      !DEBUG_ALLOWED
  environment_key     the bound X25519 public key
  evidence_reference  BLAKE3(envelope payload)
}
```

Backend provenance is carried by `verifier`, `format` and `trust_anchor`;
the label `HARDWARE_ATTESTED` is K's assurance label and is distinct from
J's evidence level, which remains `ATTESTED` (§14). No vendor structure
reaches J or the authority (HW16).

---

## 14. J integration

Unchanged from K §11–§12: `to_policy_evidence` turns the normalized
result into a J evidence record at `ATTESTED`, bound to the challenge and
session, valid for the report window; `policy::eligible` decides; the
release authority requires eligibility with every ATTESTED-level
requirement satisfied at ATTESTED. J does not know the vendor (HW17,
HW18). A deployment that wants to pin a backend does so in the K trust
policy — accepted formats and anchors — not in J: policy requires
properties, not vendors (mission §50, J §18).

---

## 15. Key-release integration

Unchanged from K §13–§15. The release authority mints one single-use
release from the verified attestation; the control plane issues the fetch
capability only against its redemption; the key release authority wraps
the content key to `environment_key` — the key the platform signature
bound (§8) — with X25519, BLAKE3 `derive_key` and XChaCha20-Poly1305 under
the release binding; the guest unwraps inside its protected memory and
opens the sealed input. Tampering with the wrapped key, the associated
data, the recipient or the release binding fails (K tests; HW20, HW21).

---

## 16. Real-hardware test

### 16.1 What has been run: captured real evidence

`test-vectors/attestation/snp/` holds a real Milan report (1,184 bytes)
with its VCEK, captured by Google and published in `go-sev-guest` under
Apache-2.0, plus a second Milan VCEK from `virtee/sev`. Provenance and
digests are in its README. Against them the `hardware_attestation` test
group proves, on this repository's own verifier:

| Proven on real evidence | Test |
|---|---|
| parse; version 2; VMPL 0; TCB 2/0/5/68; chain VCEK → ASK → pinned Milan ARK; report signature | HW01 |
| the real report is refused as unbound (`ChallengeMismatch`): parsing is not attestation | HW02 |
| any tampered signed byte (signature, `REPORT_DATA`, measurement, policy, TCB) → `BadSignature`; wrong length → `Malformed` | HW08, HW29 |
| no anchor / wrong digest / mislabelled product → `UntrustedRoot`; reference format → `UnknownFormat`; garbage → `Malformed` | HW09 |
| revoked anchor → `RevokedRoot`; another chip's VCEK → `BadSignature`; chip denylist → `RevokedPlatform` | HW10 |
| the captured guest allowed debug → a production policy refuses it (`DebugEnabled`); SMT policy; VMPL; report version | HW13 |
| measurement allowlist and revocation | HW11 |
| component-wise TCB and guest SVN minimums | HW12 |
| report window, cap, not-yet-valid, VCEK validity against a clock outside and inside its window | HW14 |

Binding, normalization and J integration (HW03–HW07, HW16, HW17, HW20,
HW21) are proven on an **unsigned copy** of the same report whose
`REPORT_DATA` is set to a Mbongo binding: stage-2 logic in isolation,
mocks by construction, proving nothing about hardware.

### 16.2 Negative controls

Three deliberately unsafe adapters — signature-blind, binding-blind,
measurement-blind — each accept an input the strict adapter rejects
(`BadSignature`, `ChallengeMismatch`, `MeasurementNotAllowed`); the group
asserts it. They exist only in tests and are never registered.

### 16.3 What has not been run: the live guest

The core of this gate is a SEV-SNP guest answering a fresh challenge.
`snp_live_test` (Linux only; prints `RESULT=SKIPPED_NO_HARDWARE` and exits
3 elsewhere) performs it in one process inside the guest:

```
session (proof of possession) → lease → K challenge
  → X25519 environment key generated in the guest
  → REPORT_DATA = binding(challenge, key)  → /dev/sev-guest extended report (report + VCEK)
  → SnpVerifier (strict policy: operator-supplied measurement allowlist and TCB minimum)
  → J CONFIDENTIAL → release → confidential fetch capability + wrapped key
  → unwrap and open inside the guest → commitment verifies
```

and prints `REAL_QUOTE_GENERATED`, `REAL_QUOTE_VERIFIED`,
`REAL_CHALLENGE_BOUND`, `REAL_SESSION_BOUND`, `REAL_TASK_BOUND`,
`REAL_EXECUTOR_BOUND`, `REAL_ENVIRONMENT_KEY_BOUND`,
`REAL_MEASUREMENT_CHECKED`, `REAL_SECURITY_STATE_CHECKED`,
`REAL_J_POLICY_PASS`, `REAL_KEY_RELEASE`,
`REAL_PRIVATE_INPUT_DECRYPTED_INSIDE_PROTECTED_ENV` and
`RESULT=PASS_HARDWARE`. The `#[ignore]`d test
`hw_live_snp_guest_end_to_end` reads that result and asserts every line;
elsewhere it is reported as *ignored*, never as passed. The CI job
"Mbongo Compute Hardware Attestation (live, SEV-SNP)" runs it only on a
self-hosted runner inside a guest (labels `self-hosted`, `sev-snp`) when
the repository variable `MBONGO_SNP_LIVE_RUNNER` is `true`; otherwise it
is skipped, and a skipped job is `SKIPPED_NO_HARDWARE`, not proof.

**Why it has not run.** The development host is an Intel Coffee Lake
laptop with no SEV-SNP, TDX or confidential-GPU capability; WSL exposes no
guest attestation device; the only cloud credential is expired and would
require an interactive login and, to create a confidential VM, a paid
action the maintainer must take. **How to run it:** an SNP guest (GCP
`n2d`/`c2d`/`c3d` with `--confidential-compute-type SEV_SNP`, Azure
`DCasv5`/`ECasv5`, or bare-metal EPYC), Linux with `/dev/sev-guest`, then
`cargo run -p mbongo-compute --bin snp_live_test -- --print-report` to
learn the measurement and TCB, then the gated run with `--measurement`,
`--min-tcb` and `--out`, then the ignored test with
`MBONGO_SNP_LIVE_RESULT` set. Record platform, firmware/TCB, kernel and
crate versions in #144 with the result file.

**Type-checking of the live harness.** The Linux-only module cannot be
cross-checked from the Windows host through cargo (the workspace's
`reqwest` pulls `openssl-sys` on Linux); it is checked in a Linux
container (§16.4 records the result).

### 16.4 Verification record

| Item | Value |
|---|---|
| captured-evidence group | `cargo test -p mbongo-compute --test hardware_attestation`: 18 passed, 1 ignored (the live test) |
| live SEV-SNP run | **not performed** (§16.3) |
| Linux type-check of `snp_live_test` | `cargo check` and `cargo clippy -D warnings` of the crate with the Linux-only module compiled, in a `rust:1.94-bookworm` container: clean (2026-09-08) |

---

## 17. Failure semantics

Every class in §12.1 yields, for a CONFIDENTIAL requirement, no
`VerifiedAttestation`, no J evidence, no release, no capability, no key
(K §19). A verification failure never downgrades the task to
`REFERENCE_ATTESTED`, `CLAIM`, VERIFIED or PUBLIC (HW27): the reference
verifier is a different verifier kind under a different format, and a
deployment that does not register it cannot fall back to it; a
deployment that registers both accepts the reference format only where
its trust policy lists that format, which a production policy does not.

---

## 18. Privacy and logging

K §22 applies. In addition: the raw report and the VCEK are never
logged; `SnpEvidence` and the envelope render a BLAKE3 reference and
sizes (HW24, HW25); the measurement digest, chip id prefix, anchor id,
verifier kind and failure class may be logged. `REPORT_DATA` is a hash and
reveals nothing beyond what the challenge record already holds. Vendor
certificates are public. Nothing here reaches the chain (HW22, HW23).

---

## 19. Known limitations

- **No live run yet** (§16.3). Everything proven is on captured evidence
  and unsigned stage-2 copies.
- **Durability.** K's release authority keeps consumed-challenge and
  release state in memory (K §23). With real hardware this matters more,
  not less: after an authority restart an old report is unknown rather
  than consumed — fail-closed — but durable replay protection across
  restarts is not provided. `PRODUCTION_AUTHORITY_DURABILITY_GAP = YES`;
  follow-up "Persistent Confidential Authority State" (§23).
- **Certificate validity needs wall time.** The fixture tests run on a
  manual clock and disable it; production enables it.
- **Report time.** SNP reports carry no timestamp; the window is the
  envelope's claim bounded by policy, and freshness rests on the K
  challenge nonce.
- **Ciphertext hiding, alias check and other platform flags** are parsed
  by the crate but not yet policy inputs; a later `snp-adapter-v2` may add
  them.
- **Harness randomness.** `snp_live_test` seeds keys from time and pid
  through BLAKE3, adequate for a test run; a production guest uses the OS
  RNG.

---

## 20. Production-readiness classification

| Question | Answer |
|---|---|
| classification | **REFERENCE_ONLY** operationally (no live run), with a hardware verifier implemented and proven on captured real evidence |
| `PROVIDER_RESISTANT_CONFIDENTIALITY_PROVEN` | **NO** — every mechanism is in place, but the live guest run that shows the private key generated and the plaintext decrypted inside SEV-SNP-protected memory has not been performed |
| `CONFIDENTIAL_CPU_PATH` | implemented, unproven live |
| `CONFIDENTIAL_GPU_PATH` | NO |
| `production_grade()` of `SnpVerifier` | `true` under a strict policy — a statement about the verifier (no test root, real chain, binding, policy), not about a live run |

**The claim this backend will support, once the live run is recorded:**
the host or provider cannot obtain the plaintext input or the content key
through the normal Mbongo release path unless the configured hardware
attestation policy is satisfied — because the key is wrapped to a key
generated in, and never leaving, SEV-SNP-protected memory, and released
only after the signed report binding that key, the challenge, the session,
the executor and the task verifies under the pinned AMD root and the
measurement, TCB and policy checks. Excluded from that claim: everything
in §4's "does not protect against" list.

---

## 21. Invariants

| # | Invariant | Status |
|---|---|---|
| HW1 | the real backend uses real hardware-generated evidence | DEFINED_BY_THIS_GATE (§5); proven on captured evidence; live run outstanding |
| HW2 | no reference root is accepted in hardware mode | DEFINED_BY_THIS_GATE (§6) ★ |
| HW3 | hardware evidence binds the K challenge | DEFINED_BY_THIS_GATE (§7) ★ |
| HW4 | evidence binds the current session | ★ |
| HW5 | evidence binds the current executor | ★ |
| HW6 | evidence binds the current task | ★ |
| HW7 | evidence binds the environment public key | DEFINED_BY_THIS_GATE (§8) ★ |
| HW8 | the environment private key stays inside the protected environment | SEV-SNP property (§4); live run outstanding |
| HW9 | untrusted chain fails closed | ★ |
| HW10 | revoked chain fails closed | ★ |
| HW11 | invalid signature fails closed | ★ |
| HW12 | disallowed measurement fails closed | ★ |
| HW13 | insufficient security version fails closed | ★ |
| HW14 | debug / insecure mode fails closed | ★ |
| HW15 | expired collateral fails closed | ★ |
| HW16 | verifier outage fails closed | ALREADY_TRUE (K §19) |
| HW17 | hardware verification does not bypass J | ALREADY_TRUE (K §11); ★ |
| HW18 | J remains the final workload-policy authority | ALREADY_TRUE (K18) |
| HW19 | release authorization remains separate | ALREADY_TRUE (K §13) |
| HW20 | the data capability remains separate | ALREADY_TRUE (K §14) |
| HW21 | content-key release happens only after J PASS | ALREADY_TRUE (K §13) ★ |
| HW22 | hardware evidence is never on-chain | ALREADY_TRUE (K19; nothing here touches the chain) |
| HW23 | the raw content key is never on-chain | ALREADY_TRUE (K19) |
| HW24 | the raw content key is never logged | ALREADY_TRUE (K20) ★ |
| HW25 | PUBLIC does not require hardware attestation | ALREADY_TRUE (K21) |
| HW26 | VERIFIED does not imply hardware confidentiality | ALREADY_TRUE (J §13) |
| HW27 | a failed CONFIDENTIAL does not downgrade | ALREADY_TRUE (K22); §17 ★ |
| HW28 | hardware attestation does not prove output correctness | ALREADY_TRUE (P16) |
| HW29 | the vendor-specific parser stays inside the adapter | DEFINED_BY_THIS_GATE (§12) ★ |
| HW30 | `ComputeTask` remains unchanged | ALREADY_TRUE |
| HW31 | `Receipt` remains unchanged | ALREADY_TRUE |
| HW32 | RPC remains unchanged | ALREADY_TRUE |
| HW33 | the SDK wire remains unchanged | ALREADY_TRUE |
| HW34 | no executor reassignment | ALREADY_TRUE (rule s; K §5) ★ |
| HW35 | no marketplace logic | ALREADY_TRUE |
| HW36 | no MBO settlement | ALREADY_TRUE |
| HW37 | no metering | ALREADY_TRUE |
| HW38 | no AI runtime added | ALREADY_TRUE |
| HW39 | additional backends can be added behind the same interface | DEFINED_BY_THIS_GATE (§23) |
| HW40 | the provider-resistant claim is allowed only if the private key and plaintext stay within the backend's protected boundary | DEFINED_BY_THIS_GATE (§20); **not yet claimable** |
| HW41 | each TCB component, debug, migration, SMT and VMPL are separate policy inputs, never one boolean | DEFINED_BY_THIS_GATE (§9) ★ |
| HW42 | the measurement allowlist has no wildcard; an empty allowlist accepts nothing | DEFINED_BY_THIS_GATE (§10) ★ |
| HW43 | `production_grade()` is a function of policy strictness, never of a label | DEFINED_BY_THIS_GATE (§10) ★ |
| HW44 | a skipped hardware job is never reported as a pass | DEFINED_BY_THIS_GATE (§16.3) |

★ = exercised by the `hardware_attestation` group on captured or
stage-2 evidence. Conflicts with existing authority: **none**; K's error
taxonomy gained five classes additively.

---

## 22. Non-goals and versioning

Non-goals: any second backend (TDX, NVIDIA, Nitro); GPU execution or GPU
memory confidentiality; AI inference; metering, pricing, marketplace, MBO
settlement; durable authority state; output correctness (#52); an
on-chain attestation commitment; any change to `ComputeTask`, `Receipt`,
rules (k)–(s), rpc_v0.3, the SDK wire, `compute-conformance-v1`, J or K's
release state machine.

Versioning: `hardware-attestation-v1` names what every hardware adapter
must establish (§6–§13 in backend-neutral terms); `snp-adapter-v1` names
this backend's evidence format, binding and policy. Both are architecture
/ control-plane versions, non-consensus. Adding a policy input from an
already-parsed field is a minor extension; changing the binding formula or
the evidence format is `snp-adapter-v2`.

---

## 23. Future adapters and follow-ups

- **Intel TDX** (`tdx-adapter-v1`): `REPORTDATA` binding identical in
  shape; MRTD and RTMR0–3 map to the measurement allowlist (a set per
  register or a digest over them); TCB status through Intel collateral
  with a cache TTL policy — the first `HYBRID` verification mode.
- **NVIDIA confidential GPU**: composes with a CPU TEE adapter; the GPU
  quote binds the same challenge; `CONFIDENTIAL_GPU_PATH` becomes its own
  gate with its own memory-boundary statement.
- **AWS Nitro Enclaves**: COSE attestation document to the Nitro root;
  PCRs map to the allowlist.
- **Persistent Confidential Authority State**: durable consumed-challenge
  and release records, or epoch-bound challenges, so that replay
  protection survives an authority restart (K §23, §19 here).
- **`snp-adapter-v2`**: ciphertext hiding, alias check and other platform
  flags as policy inputs; ID-block and author-key digests for
  deployments that sign guest images.

---

## See also

- [`compute-confidential-attestation-key-release.md`](compute-confidential-attestation-key-release.md) — K: the path this backend plugs into
- [`compute-provider-capability-policy.md`](compute-provider-capability-policy.md) — J: the policy that decides
- [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md) — parent architecture (§10 confidential compute and its limits)
- [`../../test-vectors/attestation/snp/README.md`](../../test-vectors/attestation/snp/README.md) — captured evidence, provenance and digests
- [#144](https://github.com/MbongoChain/mbongo-chain/issues/144) — K-HW; the live-run record belongs there
- [#142](https://github.com/MbongoChain/mbongo-chain/issues/142) — Workstream K
- [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (future)
