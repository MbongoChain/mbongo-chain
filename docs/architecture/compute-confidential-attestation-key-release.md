# Confidential compute: attestation and conditional key release

> **Document type:** Architecture — authorization contract for off-chain components
> **Status:** Architectural authority for how a confidential input is
> released to an execution environment: the attestation challenge, the
> evidence envelope, the verifier boundary, the trust and measurement
> policy, the conversion of verified evidence into provider-policy
> evidence, the release authorization, and the conditional issue of the
> fetch capability and the content key. Defines no consensus rule, no RPC
> method, no SDK type, no wire format and no transport; where anything here
> appears to conflict with a normative source below, the normative source
> wins.
> **Normative sources:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md)
> (Released), [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md),
> [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md),
> [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) (FROZEN),
> [`rpc_v0.3.md`](../specs/rpc_v0.3.md) (FROZEN)
> **Parent architecture:** [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md)
> (parent §10 confidential compute, parent §11 attestation boundary, parent §17.2 CONFIDENTIAL),
> [`compute-control-plane-worker-interface.md`](compute-control-plane-worker-interface.md)
> ("E", §14 the extension point),
> [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md)
> ("F": F §4.1 step 3, F §9 conditional key release, F §14 secret handling) and
> [`compute-provider-capability-policy.md`](compute-provider-capability-policy.md)
> ("J": J §6 evidence, J §14 CONFIDENTIAL, J §19 freshness, J §20 session
> binding, J §22.3 the handoff). This document implements exactly the extension point
> those documents reserved, reuses every identity, grant and policy they
> define, and yields to all of them on any conflict.
> **Contract versions:** `confidential-auth-v1` (challenge, verifier
> boundary, release authorization, key release) and
> `attestation-envelope-v1` (the evidence envelope) — architecture /
> control-plane versions, non-consensus (§26).

This is Workstream K ([#142](https://github.com/MbongoChain/mbongo-chain/issues/142)),
on J ([#140](https://github.com/MbongoChain/mbongo-chain/issues/140)) and I.
It answers one question:

> **How does a worker that claims to run confidentially come to hold the
> content key of a confidential input — and how is it kept from holding it
> on the strength of the claim alone?**

The answer, in one line:

```
valid worker session
  + fresh, purpose-bound challenge
  + acceptable attestation evidence
  + verified session, executor and task binding
  + trust policy and measurement policy satisfied
  + J effective policy: eligible, ATTESTED requirements satisfied at ATTESTED
        ↓
one single-use release authorization
        ↓
fetch capability for the sealed input  +  content key wrapped to the attested key
```

and any failure anywhere: **no key, no plaintext, no capability, no
execution start.**

**What this document proves and does not.** The reference path (§24) is
**REFERENCE / TEST ONLY**: its "environment" is a process holding software
keys, endorsed by a root a test controls. It proves the *mechanism* — the
data plane stores ciphertext; the key is released only to the key a
verified attestation bound, only after J says eligible, once per challenge,
within a window; every replay, binding, freshness and trust failure fails
closed; the PUBLIC path is untouched. It does **not** prove
*provider-resistant confidentiality*: nothing here stops the operator of
the reference process from reading its memory. That property needs a
hardware-backed verifier, which is the follow-up gate (§29). No sentence
below should be read as claiming it.

---

## 1. Purpose and the extension point

The parent architecture describes confidential compute as an optional,
stronger profile (§10) and fixes where each step lives (§11): generation
in plane 4, verification in plane 2, policy evaluation in plane 2 or by
the client, key release in plane 3, and **no on-chain reference today**. E
§14 says exactly one point moves: "step 4 of §7 gains a precondition — the
worker environment produces attestation evidence, a verifier … checks it
against policy, and only then is the content key released to that
environment." F §9 says the same from the data plane's side: "key release
becomes conditional … wrapped to a key the attested environment proves it
holds"; encrypted input is *required* in the confidential profile. J §14
says what the policy requires — `privacy.confidential_execution` IS_TRUE at
`min_evidence = ATTESTED`, session-fresh — and J §22.3 names what K must
supply.

This document is that precondition, that conditional release, and that
supply. Nothing else in the lifecycle changes: the task, the lease, the
attempt, the session, the capability model, the result path and the
receipt are exactly as E, F and RFC 0005 have them.

---

## 2. Authority boundary and trust boundary

| Component | Plane | Gains here | Never |
|---|---|---|---|
| **Control plane** — the release authority (§13) | 2 | asks for attestation (issues challenges); invokes a verifier; runs the J evaluator; mints and consumes release authorizations | chooses or changes the executor; decides receipt validity; holds a content key |
| **Attestation verifier** (§7) | 2 | says whether evidence is genuine and what it says | says whether that is acceptable — that is trust policy (§8) plus J (§12) |
| **Private data plane / key release authority** (§14, §15) | 3 | stores the sealed input; releases the content key, wrapped, against a redeemed release | anything about tasks, leases or receipts; signing anything the chain sees |
| **Worker session** | 2/4 | proves executor possession (E §5) and presents evidence | substitutes another executor |
| **Execution environment** | 4 | produces evidence; holds the environment key; decrypts inside itself | judges its own acceptability |
| **Chain** | 1 | nothing | parses a challenge, an envelope, a trust policy, a release, a key |

No component gains authority it did not have. The control plane already
decided admission (E §6) and obtained capabilities (E §7); it now has one
more precondition to satisfy before the confidential capability. The data
plane already enforced capabilities (F §5); it now also holds ciphertext
and a delegated key service. The chain already knew nothing of any of this
and still does (E22, K2).

---

## 3. Exact authority map

| K requirement | Authority | Required behaviour | Plane | Testable observable |
|---|---|---|---|---|
| attestation challenge | parent §10 flow step 1; E §14 ("produces attestation evidence"); J §19 `CHALLENGE(nonce, session)` binding | a fresh, unpredictable, session-, executor-, task- and purpose-bound challenge, issued by the control plane, echoed in the evidence | 2 | K06, K07, K08 |
| freshness | J §19 (windows; "expired → level CLAIM"); J §14 (session-fresh); parent §10 ("ephemeral key … destruction at the end of the session") | evidence has a window; the authority evaluates at an explicit `now`; expired, not-yet-valid and too-old evidence fail | 2 | K05, K26 |
| session binding | E §5 (session bound to one instance and one executor); J §20 (session-scoped claims; provider-wide evidence never substitutes); J20, J21 | evidence names the session; a release is redeemable only inside that session | 2 | K04, K13, K25 |
| executor binding | RFC 0005 rule s; E §6 row 2; J §10.2 (gate first) | the challenge is issued only to a session that proved `task.executor`; evidence names that executor; nothing else is evaluated first | 2 | K03, K24 |
| provider-policy integration | J §22.3; J §14; J18 | verified evidence becomes a J evidence record at ATTESTED; `policy::eligible` decides; no bypass | 2 | K15, K16, K17 |
| trust policy | parent §10 ("verifies the evidence against the expected environment: measured code, configuration, hardware class"); parent §11 (policy evaluation); P13 (vendor-neutral) | a deployment policy of accepted formats, trusted roots, measurements, minimum security version, debug policy, freshness bounds; non-consensus | 2 | K09, K10, K12 |
| measurement policy | parent §10 ("measured code, configuration") | an allowlist with revocation; approving a measurement says nothing about output correctness (P16) | 2 | K11, K28 |
| evidence conversion | J §6.2 evidence record; J §22.3 | `VerifiedAttestation` → `Evidence { ATTESTED, CHALLENGE binding, window, verifier, opaque reference }` | 2 | K15 |
| conditional release | F §9 ("only then is the key released"); parent §11 ("key release … conditional on the two steps above"); J §22.3 `release_decision` | fetch capability and wrapped key only against a redeemed, single-use, TTL-bound release | 2 → 3 | K02, K18, K21 |
| failure behaviour | J §11 (fail closed); parent §10 limits | every missing or failing condition yields no release; availability failure over policy bypass | 2 | K09, K16, K22, K23 |
| logging / privacy | F §13, E §18, J §24, F19, E23, C30 | identifiers and hashes only; no key, plaintext, evidence payload, wrapped key or proof in logs | all | K19, K20 |
| ordinary-provider fallback | parent §7, §22; F §9 ordinary profile; F14 | the PUBLIC path is unchanged and attestation-free; a CONFIDENTIAL requirement never downgrades | 2 | K21, K22, K23 |

Two things authority does not supply and this document does not add: an
on-chain attestation commitment (parent §11: "not today"; a normative
decision needing its own RFC) and any vendor as a protocol or policy
constant (P13, E28, F18, J30).

---

## 4. Terminology

| Term | Meaning |
|---|---|
| **challenge** | a control-plane-issued, single-use, short-lived binding the evidence must answer (§5) |
| **evidence envelope** | format, verifier kind, bindings, window and an opaque vendor payload (§6) |
| **verifier** | the component that turns an envelope into a `VerifiedAttestation` or a failure class (§7) |
| **trust policy** | the deployment's accepted formats, roots, measurements and bounds (§8) |
| **verified attestation** | the normalized, vendor-neutral result of verification (§10) |
| **release authority** | the control plane's confidential authorization service: challenges, verification, J evaluation, release authorizations (§13) |
| **release authorization** | the single-use, TTL-bound, session-bound record that a release may happen (§13) |
| **release grant** | what a redeemed authorization yields, once: the facts needed to issue the capability and wrap the key (§13) |
| **key release authority** | the client's delegated key service (parent §11, F §14): holds content keys, releases each wrapped, once per redeemed release (§15) |
| **environment key** | the X25519 key the environment holds and its attestation binds; the content key is wrapped to it (F §9) |
| **sealed input** | the client's input encrypted under the content key with the task id and input commitment as associated data (§15) |
| **REFERENCE_ATTESTED** | the assurance label of the reference verifier: software keys, no hardware root; test only (§24) |

Identities are E §2's and J §4's, unchanged. The reference root (§24) is a
distinct identity from every executor key and from the control plane's
issuer key; it signs evidence under its own domain tag and nothing else.

---

## 5. Challenge model

A challenge is issued by the release authority (§13), never by the worker,
and never stored on the chain:

```
AttestationChallenge {
  challenge_id     opaque, fresh
  nonce            32 unpredictable bytes the evidence must echo
  session_id       the session it was issued to
  executor         the executor that session proved — equal to task.executor
  task_id          the task the release would serve
  purpose          CONFIDENTIAL_INPUT_RELEASE | RESULT_RELEASE (reserved)
  issued_at, expires_at
}
```

| Property | Rule |
|---|---|
| fresh, unpredictable | id and nonce from the authority's id source; never reused |
| single-session | issued to one session; presenting under another is `SessionMismatch` before any verifier runs |
| short-lived | `expires_at = issued_at + trust.challenge_ttl_secs`; expired before use → `ChallengeExpired`, state `Expired` |
| single-use | one release authorization at most; a refusal spends it too (state `Denied`); replay → `ChallengeConsumed` |
| executor-bound | issued only if `session.executor == task.executor`; otherwise `ExecutorMismatch` and nothing else happens (§9 of J; K3) |
| task-bound | names one task; evidence for it cannot serve another task (§17) |
| purpose-bound | `CONFIDENTIAL_INPUT_RELEASE` never authorizes a result release, and vice versa; the tag is signed into the reference evidence body |

**Lifecycle:** `Issued → Presented → Consumed`, or `Issued → Expired`,
`Issued | Presented → Revoked` (session revocation), `Presented → Denied`
(policy refusal). Ephemeral off-chain state; §23 says what survives a
restart.

---

## 6. Evidence envelope

```
EvidenceEnvelope (attestation-envelope-v1) {
  format           evidence format identifier (deployment vocabulary)
  verifier_kind    which verifier should evaluate it
  challenge_id, session_id, executor
  issued_at, expires_at
  payload          opaque vendor bytes — potentially sensitive; never logged
}
reference() = BLAKE3(payload)     — what logs, decisions and releases carry
```

The payload is opaque to everything but the named verifier. The normalized
facts live in the `VerifiedAttestation` (§10), not in the envelope. No
vendor-specific evidence is a protocol object, and none appears in
`execution_spec`, `Receipt.metadata` or any chain field (K19, K30).

---

## 7. Verifier interface

```
trait AttestationVerifier {
  kind()             -> verifier kind
  formats()          -> the formats it understands
  security_label()   -> REFERENCE_ATTESTED | HARDWARE_ATTESTED
  production_grade() -> bool     (the reference answers false)
  verify(envelope, challenge, trust_policy, now) -> VerifiedAttestation | VerifyError
}
```

A verifier establishes **genuineness and content**: that the payload is
what it claims under a trusted root, and what it says about the
environment and its bindings. It does not decide acceptability; that is
the trust policy (§8) and J (§12). The release authority re-checks the
normalized result against its own challenge record and the policy (§13),
so a permissive verifier cannot widen what is accepted beyond what only a
signature check can establish — which is why a signature-blind verifier is
caught by the conformance suite (§26).

Future adapters — a confidential-VM verifier, a GPU vendor's, a CPU
vendor's — implement this trait, normalize into the same
`VerifiedAttestation`, and change nothing in J, F or the protocol (K34,
K40). None exists in this repository (§29).

---

## 8. Trust policy

Deployment policy, held by the release authority, non-consensus:

```
TrustPolicy {
  accepted_formats        set of evidence formats the deployment accepts at all
  anchors                 anchor_id -> { public_key, revoked }
  measurement             MeasurementPolicy (§9)
  max_evidence_age_secs   evidence older than this at presentation is refused
  challenge_ttl_secs
  release_ttl_secs
}
```

No root is hard-coded anywhere in protocol code; a policy without a root
accepts nothing. **Rotation (§18):** replacing the policy affects every
subsequent verification; unredeemed release authorizations whose anchor is
now missing or revoked are revoked; redeemed releases cannot be undone. A
missing trust policy fails every confidential request closed
(`TrustPolicyMissing`).

---

## 9. Measurement policy

```
MeasurementPolicy {
  allowed                 set of 32-byte measurements (runtime / image / policy-version digests)
  revoked                 measurements that fail even if listed as allowed
  min_security_version
  allow_debug             whether an environment with debug enabled may be accepted
}
```

Matching is exact on bytes; no partial or pattern match. An approved
measurement means the environment is one the policy expects. It says
**nothing** about whether the output is correct (P16, RFC 0005 §9.2, K30):
K28 anchors a wrong output from an accepted environment to make the point.

---

## 10. VerifiedAttestation

The normalized, vendor-neutral result:

```
VerifiedAttestation {
  verifier, format, security_label, trust_anchor
  session_id, executor, task_id, challenge_id, purpose      — bindings as the evidence states them
  issued_at, expires_at                                       — the evidence window
  environment_type                                           — abstract; no vendor name in the core grammar
  measurement, security_version, debug_disabled
  environment_key                                            — the X25519 public key the environment proved it holds
  evidence_reference                                         — BLAKE3 of the payload
}
```

It carries no raw evidence, no key material and no field about output. Its
`security_label` is the verifier's; the reference verifier's results are
always `REFERENCE_ATTESTED`.

---

## 11. Conversion into J evidence

The most important integration, and the only place the two contracts
touch:

```
VerifiedAttestation  →  J Evidence {
  level        = ATTESTED
  issued_at    = attestation.issued_at
  expires_at   = Some(attestation.expires_at)
  binding      = CHALLENGE(session_id, challenge_ref = hex(challenge_id))
  verifier     = attestation.verifier
  reference    = hex(evidence_reference)      — opaque
  revoked      = false
}
```

The release authority attaches that record to the session-scoped claim
`privacy.confidential_execution = true` **itself**, replacing whatever the
caller supplied for that dimension. A caller therefore cannot inject an
ATTESTED evidence record (K14 proves it): the only route to ATTESTED is a
verified challenge. Everything else about J is untouched: the evaluator,
the composition rule, the reason codes, the other dimensions.

---

## 12. CONFIDENTIAL policy evaluation

J's CONFIDENTIAL template (J §14) requires `privacy.confidential_execution`
IS_TRUE with `min_evidence = ATTESTED` and `freshness = SESSION`. With the
record of §11 attached, `policy::eligible` evaluates that requirement as it
evaluates any other: the evidence must be unexpired at `now`, bound to this
session, and at least ATTESTED. The release authority then requires, in
addition, that the decision is eligible **and** that every requirement with
`min_evidence ≥ ATTESTED` appears in `decision.satisfied` at ATTESTED or
above — J §22.3's `release_decision` rule, implemented.

| Presented | Result |
|---|---|
| claim only (`confidential_execution = true`, no evidence) | ineligible, `ATTESTATION_REQUIRED` (K01) |
| CERTIFIED evidence | ineligible, `ATTESTATION_REQUIRED` (K14) |
| ATTESTED, expired | ineligible, `EVIDENCE_EXPIRED` (K05) |
| ATTESTED, bound to another session | ineligible, `EVIDENCE_NOT_SESSION_BOUND` — and refused by the authority before J is reached (K04, K25) |
| ATTESTED, fresh, this session | eligible; `lowest_evidence = ATTESTED` (K02, K17) |
| ATTESTED, but another hard requirement unknown or failed | ineligible; the challenge is spent (`Denied`); no release (K16) |

J remains the final policy decision (K18). K neither reinterprets nor
weakens it: a policy that says ATTESTED + fresh + session-bound is not
satisfied by a claim, a certificate, an expired attestation or a provider
assertion (K01, K05, K14).

---

## 13. Release authorization

The release authority (`ReleaseAuthority`, control plane) owns the state
machine:

```
Requested ─issue_challenge─▶ ChallengeIssued ─present_evidence─▶ EvidencePresented (= Verified)
   ─authorize_release─▶ PolicySatisfied ─▶ Released ─redeem─▶ Consumed
                          │                    ├─▶ Expired   (release_ttl, or the evidence window, whichever first)
                          ▼                    └─▶ Revoked   (release, session or anchor revocation; rotation)
                        Denied  (challenge spent; a new challenge is needed)
```

`Requested → Released` without `Verified` and `PolicySatisfied` does not
exist as a transition. The record:

```
ReleaseAuthorization {
  release_id
  task_id, executor, session_id, challenge_id
  trust_anchor, evidence_reference, environment_key
  lowest_evidence         (ATTESTED for any release)
  issued_at, expires_at   = min(now + release_ttl, evidence.expires_at)
}
```

**Not a bearer token.** A release is redeemed only through the authority,
inside the authenticated session it names, for its task, once. Presenting
its id from another session is `SessionMismatch`; for another task,
`TaskMismatch`; twice, `ReleaseConsumed`; after `expires_at`,
`ReleaseExpired`; after revocation, `ReleaseRevoked`. Redemption yields a
`ReleaseGrant` — `release_id, task_id, executor, session_id,
environment_key, expires_at` — consumed by the control plane and the key
service in process.

**Idempotence.** A retried `authorize_release` on a consumed challenge
returns `ChallengeConsumed`; a retried `redeem` returns `ReleaseConsumed`;
a retried key release returns `ReleaseConsumed`. No path mints a second key
release for the same challenge; a genuine retry is a new challenge, new
evidence, new release — exactly as a consumed data capability is never
reopened (F §11, E10).

---

## 14. Data capability and key release

F §9 defines the confidential profile as **key release becomes
conditional**, over encrypted input, wrapped to the attested environment's
key. This document implements both halves as **separate abstractions**
because they are separate grants (E §3,
J §22.1):

| Half | Where | Rule |
|---|---|---|
| **A. the fetch capability** | control plane → data plane | `ControlPlane::authorize_fetch_confidential(session, lease, release_id, authority, dp)` redeems the release (§13) and only then issues the ordinary F fetch capability. The ordinary `authorize_fetch` refuses a confidential object outright with `ConfidentialReleaseRequired`. Lease first, then release, then capability (E §7 order extended). |
| **B. the content key** | key release authority (plane 3) | `KeyReleaseAuthority::release(grant, object)` wraps the client-registered content key to `grant.environment_key` under a binding of `(release_id, task_id, executor, session_id)`, once per release. |

Neither half replaces the other. The capability yields the **sealed** bytes
(useless without the key); the wrapped key is useless without the
environment's secret. The layering the K issue requires is preserved
exactly: task authority (chain) · execution lease (control plane) · policy
eligibility (J) · attestation evidence (verifier) · release authorization
(authority) · data-plane capability (F). None is collapsed into another;
`task_id`, a locator, a lease and `provider_id` each still grant nothing
(K22–K24; F3, F9, E6, J §5.2).

**Release ordering (the confidential path):**

```
task accepted (chain)  →  session authenticated (E §5)  →  lease (E §7 step 3)
  →  challenge (§5)  →  attestation (plane 4)  →  verifier (§7)  →  J policy PASS (§12)
  →  release authorization (§13)  →  fetch capability + wrapped key (§14)
  →  fetch sealed input  →  unwrap and open inside the environment (§15)
  →  commitment verification (F §7)  →  execution start
```

No private-data capability for a confidential object exists before the
release authorization (K18); the verified-but-not-authorized and the
authorized-but-not-redeemed states both leave the object `Created`.

---

## 15. Encryption and key handling

**Sealing.** The client generates a 32-byte content key, seals the input
with XChaCha20-Poly1305 under a fresh 24-byte nonce, with associated data
`domain || task_id || input_commitment`, and stores the ciphertext as the
data-plane object (F §4.1 step 3; the commitment is over the plaintext
exactly as RFC 0005 §2.4 has it). The data plane never holds the key.

**Wrapping.** The key release authority wraps the content key to the
environment key from the attestation: X25519 agreement between a one-time
ephemeral key and the environment key (rejecting a non-contributory
result), BLAKE3 `derive_key` over `shared || ephemeral_pk || recipient_pk`
under a fixed context, XChaCha20-Poly1305 over the 32 key bytes with the
release binding `(release_id, task_id, executor, session_id, ephemeral_pk,
recipient_pk)` as associated data. Changing any binding, the recipient, the
ephemeral key or a ciphertext byte makes the wrap unopenable (tested).

**Opening** happens inside the environment: unwrap with the environment
secret, then open the sealed input with the task id and commitment as
associated data. The content key exists in plaintext only there and in the
client's own store (F §14).

**Classification.** The primitives are standard and already in the
workspace (the libp2p noise stack); nothing is invented. The *wrapping
model* is classified with the reference verifier: the environment key is a
software key in a process, endorsed by a test root. Binding the wrap to that
key is exactly what a hardware adapter would do with a key the hardware
attests; the cryptography does not change, the assurance behind the key
does (§29).

**Rules for key material** (F §14): never logged; never
on-chain; never in `Receipt.metadata` or `execution_spec`; never returned
through any RPC; `Debug` on every key-bearing type redacts. K19 scans every
block byte for the content key, the wrapped key, the plaintext, the sealed
ciphertext, the nonce, the root seed and the environment secret; K20 scans
runtime logs and `Debug` renderings for the same.

---

## 16. Freshness and session binding

| Check | Where | Outcome on failure |
|---|---|---|
| challenge window | authority, before the verifier | `ChallengeExpired` |
| evidence `issued_at ≤ now ≤ expires_at` | verifier and authority | `EvidenceNotYetValid` / `EvidenceExpired` |
| evidence age `now − issued_at ≤ max_evidence_age_secs` | verifier and authority | `EvidenceTooOld` |
| challenge belongs to the presenting session | authority, before the verifier | `SessionMismatch` |
| evidence names the challenge's session, executor, task, purpose, id and nonce | verifier; re-checked by the authority | `SessionMismatch` / `ExecutorMismatch` / `TaskMismatch` / `PurposeMismatch` / `ChallengeMismatch` |
| release redeemed inside its session | authority | `SessionMismatch` |
| release window | authority | `ReleaseExpired` |

Time is an explicit input everywhere (the components take a `Clock`; the
suite uses a manual one). Evidence for session A never satisfies session B,
even for the same provider, machine and executor, unless re-attested under
B's own challenge (K04, K25; J20, J21).

---

## 17. Replay protection

| Replay | Stops at | Code |
|---|---|---|
| same envelope presented twice | challenge state `Presented`/`Consumed` | `ChallengeConsumed` (K07) |
| same release redeemed twice | release state `Consumed` | `ReleaseConsumed` (K07) |
| same grant to the key service twice | the key service's released set | `ReleaseConsumed` (K07) |
| evidence for task A presented for task B's challenge | evidence body names task A and challenge A's id and nonce | `ChallengeMismatch` / `TaskMismatch` (K08) |
| release for task A redeemed for task B | release binding | `TaskMismatch` (K08) |
| evidence from session A presented in session B | challenge–session binding | `SessionMismatch` (K04, K24) |
| envelope relabelled with another session or executor | body still names the original | verifier mismatch (K04, K24) |
| evidence for executor A under executor B's session | executor gate | `ExecutorMismatch` / `SessionMismatch` (K24) |
| provider B presenting provider A's evidence | evidence binds session and executor, not a provider string; B's session cannot obtain A's challenge | refused before any provider identity is consulted |

Replay protection rests on the authority's own state (challenge and release
records) and on the evidence's signed bindings; it never relies on
`provider_id` string equality.

---

## 18. Revocation

| Revoked | Future effect | Already released |
|---|---|---|
| a trust anchor | evidence under it fails `RevokedRoot`; unredeemed releases resting on it are `Revoked` (K12) | a redeemed release cannot be recalled |
| a measurement | evidence carrying it fails `MeasurementRevoked` (K11) | as above |
| a session | no challenge, verification, release or redemption for it; its challenges and unredeemed releases are `Revoked` (K13) | as above |
| a release | `ReleaseRevoked` on redemption (K27) | — |
| the trust policy replaced (rotation) | new evidence evaluated against the new policy; unredeemed releases with a missing or revoked anchor are `Revoked` (K12) | as above |

**Honesty about released keys.** Once a wrapped key has been released to an
environment, revocation cannot make that environment "unlearn" it.
Revocation blocks future releases and redemptions; it does not erase a
disclosure that already happened, and no hardware-mediated key
invalidation exists in the reference path. A stricter policy after release
has the same limit (J §21.1): future accesses denied, the running
attempt handled by explicit E §13 policy.

**In-flight leases** (a K decision, since E §13 and §15 do not name
attestation): a lease whose release was redeemed continues under E §13
semantics; a lease whose release was not yet redeemed must re-attest under
a new challenge when the anchor, measurement or policy changed. Nothing
already anchored is rewritten (E18, J22).

---

## 19. Failure semantics

For a CONFIDENTIAL requirement, every one of these yields **no release**:

| Condition | Code |
|---|---|
| no verifier registered for the evidence kind (outage) | `VerifierUnavailable` (K23) |
| evidence format not accepted by the deployment, or unknown to the verifier | `UnknownFormat` (K09) |
| malformed payload | `Malformed` (K09) |
| no trust policy configured | `TrustPolicyMissing` (K23) |
| no trusted root, or root revoked | `UntrustedRoot` / `RevokedRoot` (K10, K12) |
| signature invalid | `BadSignature` (K10) |
| measurement not allowed / revoked / version too low / debug enabled | (K11) |
| any binding or window failure | §16, §17 |
| J ineligible | `PolicyIneligible { codes }` (K16) |

Availability failure is preferable to a privacy-policy bypass: **during a
verifier outage CONFIDENTIAL tasks fail closed and PUBLIC tasks continue**
(K23). A task whose effective policy requires CONFIDENTIAL is never
downgraded to VERIFIED or PUBLIC because attestation failed: the ordinary
fetch path refuses its object regardless (K22).

---

## 20. PUBLIC and VERIFIED coexistence

- **PUBLIC** is unchanged: `register_input` + `authorize_fetch` + the
  reference worker's `run_once`, no challenge, no verifier, no key service
  (K21). `compute-conformance-v1` (38/38), the reference harness and the
  v0.4 vertical all run this path and are the regression proof. TEE is not
  required network-wide; no attestation is required unless a policy
  requires it (K26).
- **VERIFIED** is unchanged and is not CONFIDENTIAL (J §13, J15, K27 of
  the invariants). A VERIFIED template does not require ATTESTED evidence
  unless the policy author adds a confidential requirement.
- The confidential path is opt-in per task: the client seals the input,
  registers the content key with the key service, and registers the object
  as confidential with the control plane.

---

## 21. SOVEREIGN compatibility

A SOVEREIGN policy (J §15) composes CONFIDENTIAL with CERTIFIED location
requirements. K makes the confidential half satisfiable; it does **not**
make any location dimension ATTESTED: the reference attestation carries no
location, and an unknown `jurisdiction.worker_location` stays UNKNOWN
under a policy that requires it (J §16). A future hardware adapter that
binds an environment to certified hardware (parent §19) may raise location
evidence; nothing here claims it.

---

## 22. Logging and privacy

F §13, E §18 and J §24 apply. For K in addition:

| Item | In logs by default? |
|---|---|
| challenge id, session id, executor, task id, release id, anchor id, verifier kind, security label, reason class | yes |
| evidence reference (BLAKE3 of the payload), measurement | yes |
| the challenge nonce, the evidence payload, the wrapped key, the content key, the environment secret, the root secret, plaintext, capability proofs | **never** |

Raw evidence is treated as potentially sensitive: logs carry its hash. The
suite's K20 captures runtime logs and every `Debug` rendering the scenario
touched and checks them against the scenario's secret list; K19 does the
same over every block byte.

---

## 23. Security limitations and persistence

**K protects against:** claim-only spoofing (K01), stale evidence (K05),
cross-session replay (K04, K25), cross-task replay (K08), the wrong
executor (K03, K24), an untrusted or revoked root (K10, K12), an unapproved
measurement (K11), policy bypass (K14, K16), release before authorization
(K18), verifier outage downgrade (K22, K23).

**K does not protect against:** compromise of the hardware or firmware a
future adapter trusts; TEE side channels; malicious firmware a trust
policy accepts; host denial of service; incorrect output (P16, #52);
traffic analysis and metadata observation (parent §13); a provider reading
plaintext in the **reference** environment, which is a software process
(§24).

**Confidentiality level actually implemented:** *policy-gated,
encrypted-at-rest in the data plane, key released only to the attested
key*. With the reference verifier that is `REFERENCE_ATTESTED`, not
"attested-environment-only plaintext" in the hardware sense; the claim
"provider-resistant" is reserved for a hardware-backed verifier (§29).

**Persistence.** The reference release authority and key service are in
memory. Trust policy, measurement policy and revocation state are
deployment configuration that must be supplied at start; consumed
challenges, release authorizations and the key service's released set do
**not** survive a restart. Consequently replay protection is scoped to one
authority process lifetime: after a restart, a challenge is unknown
(`UnknownChallenge`) rather than consumed, which fails closed — an old
envelope cannot be re-verified because its challenge record is gone — but
durable replay protection across restarts is **not claimed**. A production
authority must persist consumed-challenge and release state, or bind
challenges to an epoch that changes at restart.

**Crash windows** (E §9.3 style): after challenge issuance — nothing
granted, re-issue; after verification — the challenge is `Presented`,
re-authorize or let it expire; after policy PASS before redemption — the
release is `Released`, redeem or let it expire; after redemption before the
capability is issued — the release is `Consumed`, and a fresh challenge is
required (no second release for that challenge); after the capability —
F's consumed-capability rules. No path mints a second key release for the
same challenge.

---

## 24. Reference verifier

`confidential::reference` — **REFERENCE_ATTESTED, TEST ONLY; not hardware
attestation; not production confidential compute** (`REFERENCE_LABEL`,
`production_grade() == false`, K29):

- **format** `mbongo-ref-attestation:v1`, **verifier kind**
  `mbongo-ref-verifier`;
- **root** `ReferenceAttestationRoot`: an Ed25519 key from a test seed,
  named by an anchor id, signing `domain || SCALE(body)` under the
  evidence domain tag; distinct from every executor and issuer key;
- **environment** `ReferenceEnvironment`: a measurement, a security
  version, a debug flag and an X25519 environment key; produces the signed
  body (anchor id, environment type, measurement, security version, debug
  state, environment public key, challenge id, nonce, session id, executor,
  task id, purpose tag, window) and opens the sealed input inside itself;
- **verifier** `ReferenceAttestationVerifier`: decodes, resolves the anchor
  (untrusted / revoked), verifies the signature, checks every binding and
  window, checks the measurement policy, and normalizes.

Real cryptographic verification (Ed25519, X25519, XChaCha20-Poly1305,
BLAKE3) over a software root: what it cannot supply is a hardware root of
trust, and it says so in every artefact.

---

## 25. Vendor adapter extension

An adapter for a real environment implements `AttestationVerifier` and
maps its evidence into the same `VerifiedAttestation`: `environment_type`
(abstract), `measurement` (the adapter's canonical digest), security
version, debug state, and the environment key the evidence binds. Its trust
anchors are entries in the same `TrustPolicy`; its format is one more
accepted format. Nothing in J, F, E or the protocol changes; the suite
(§26) runs unchanged against it. Vendor names may appear as values inside
the adapter's own namespace, never as keys of this contract (P13, E28,
J §18.1).

---

## 26. Conformance

`confidential-auth-v1` — **Mbongo Compute Confidential Authorization** — is
the named suite: 30 cases K01–K30 (`crates/mbongo-compute/src/confidential/suite.rs`),
run by `cargo run -p mbongo-compute --bin confidential_authorization`
(prints `CONFIDENTIAL AUTHORIZATION: PASS`) and by the test group
`confidential_authorization`, as the CI step **Mbongo Compute Confidential
Authorization**. It is not vacuous: two deliberately unsafe verifiers — one
that skips the signature check, one that reports bindings from the
challenge instead of the evidence — fail it (K10 and K08 respectively), and
the test group asserts that they do. It composes with, and does not modify,
`compute-conformance-v1` and `provider_policy`: a confidential worker must
pass all three.

| Id | Case | Invariants |
|---|---|---|
| K01 | claim-only confidential provider is rejected | K4, K5, K25 |
| K02 | valid attestation satisfies CONFIDENTIAL | K5, K7, K8, K18, K21 |
| K03 | wrong executor rejected before release | K3, K8 |
| K04 | wrong-session attestation rejected | K7, K11 |
| K05 | stale attestation rejected | K6 |
| K06 | expired challenge rejected | K9 |
| K07 | consumed challenge replay rejected | K10 |
| K08 | cross-task attestation replay rejected | K12 |
| K09 | unknown attestation format rejected | K13 |
| K10 | untrusted verifier root rejected (and a forged signature) | K14 |
| K11 | disallowed measurement rejected | K15 |
| K12 | revoked root rejected; rotation | K16 |
| K13 | revoked session rejected | K7 |
| K14 | CERTIFIED does not satisfy ATTESTED; injected evidence ignored | K5 |
| K15 | ATTESTED evidence converts into the J record | K18 |
| K16 | J ineligible prevents release | K18, K25 |
| K17 | J eligible allows release | K18 |
| K18 | capability only after the release decision | K19, K20, K21 |
| K19 | keys and plaintext absent from chain artifacts | K1, K2, K31, K32 |
| K20 | keys and plaintext absent from logs | K1 |
| K21 | PUBLIC flow works without attestation | K26 |
| K22 | CONFIDENTIAL does not downgrade to PUBLIC | K17 |
| K23 | verifier outage fails closed for CONFIDENTIAL; PUBLIC continues | K17 |
| K24 | same evidence cannot authorize another executor | K8 |
| K25 | same evidence cannot authorize another session | K11 |
| K26 | release authorization expires | K6 |
| K27 | revoked release authorization rejected | K16 |
| K28 | attestation does not imply output correctness | K30 |
| K29 | reference verifier is labelled non-production | K29 |
| K30 | ComputeTask, Receipt and RPC remain unchanged | K31–K33 |

---

## 27. Invariants

| # | Invariant | Status |
|---|---|---|
| K1 | confidential authorization is off-chain | ALREADY_TRUE (parent §11 "not today"); DEFINED_BY_THIS_GATE |
| K2 | consensus does not parse attestation | ALREADY_TRUE (E22) |
| K3 | `task.executor` remains authoritative | ALREADY_TRUE (rule s); executor gate first (§5) ★ |
| K4 | claim-only confidentiality is insufficient | DEFINED_BY_THIS_GATE (§12) ★ |
| K5 | CONFIDENTIAL requires ATTESTED evidence | ALREADY_TRUE as policy (J §14); made satisfiable here ★ |
| K6 | evidence must be fresh | DEFINED_BY_THIS_GATE (§16) ★ |
| K7 | evidence must bind to the session | DEFINED_BY_THIS_GATE (§16) ★ |
| K8 | evidence must bind to the executor | DEFINED_BY_THIS_GATE (§5, §16) ★ |
| K9 | the challenge must be fresh | DEFINED_BY_THIS_GATE (§5) ★ |
| K10 | challenge replay fails | DEFINED_BY_THIS_GATE (§17) ★ |
| K11 | cross-session replay fails | DEFINED_BY_THIS_GATE (§17) ★ |
| K12 | cross-task replay fails | DEFINED_BY_THIS_GATE (§17) ★ |
| K13 | an unknown evidence format fails closed | DEFINED_BY_THIS_GATE (§19) ★ |
| K14 | an untrusted verifier root fails closed | DEFINED_BY_THIS_GATE (§8, §19) ★ |
| K15 | a disallowed measurement fails closed | DEFINED_BY_THIS_GATE (§9) ★ |
| K16 | a revoked trust root fails closed | DEFINED_BY_THIS_GATE (§18) ★ |
| K17 | verifier outage does not downgrade privacy | DEFINED_BY_THIS_GATE (§19) ★ |
| K18 | the J policy evaluator remains the final policy decision | DEFINED_BY_THIS_GATE (§11, §12) ★ |
| K19 | eligibility does not itself release data | ALREADY_TRUE (J6–J9); §14 ★ |
| K20 | a release authorization is not a data capability | DEFINED_BY_THIS_GATE (§13, §14) ★ |
| K21 | a confidential data capability is issued only after release authorization | DEFINED_BY_THIS_GATE (§14) ★ |
| K22 | `task_id` alone grants nothing | ALREADY_TRUE (F3) |
| K23 | `provider_id` alone grants nothing | ALREADY_TRUE (J §5.2) |
| K24 | a lease alone grants no confidential data | ALREADY_TRUE (E6); §14 ★ |
| K25 | confidential release never occurs before policy PASS | DEFINED_BY_THIS_GATE (§13) ★ |
| K26 | the PUBLIC flow remains attestation-free unless policy requires otherwise | ALREADY_TRUE (parent §22, F14); §20 ★ |
| K27 | VERIFIED remains distinct from CONFIDENTIAL | ALREADY_TRUE (J §13) |
| K28 | SOVEREIGN may compose confidential requirements | ALREADY_TRUE (J §15); §21 |
| K29 | the reference verifier is non-production | DEFINED_BY_THIS_GATE (§24) ★ |
| K30 | K does not prove output correctness | ALREADY_TRUE (P16) ★ |
| K31 | K adds no `Receipt` field | ALREADY_TRUE ★ |
| K32 | K adds no `ComputeTask` field | ALREADY_TRUE ★ |
| K33 | K adds no RPC version | ALREADY_TRUE |
| K34 | K is vendor-neutral | ALREADY_TRUE (P13, E28, J29); §25 |
| K35 | K adds no GPU execution | ALREADY_TRUE |
| K36 | K adds no AI inference | ALREADY_TRUE |
| K37 | K adds no metering | ALREADY_TRUE |
| K38 | K adds no pricing | ALREADY_TRUE |
| K39 | K adds no MBO settlement | ALREADY_TRUE |
| K40 | a future real hardware verifier plugs in without protocol change | DEFINED_BY_THIS_GATE (§7, §25) |
| K41 | a caller cannot inject ATTESTED evidence; only a verified challenge produces it | DEFINED_BY_THIS_GATE (§11) ★ |
| K42 | a refused policy evaluation spends the challenge | DEFINED_BY_THIS_GATE (§13) ★ |
| K43 | the key release authority signs no receipt, names no executor and touches no chain state | DEFINED_BY_THIS_GATE (§15) |
| K44 | replay protection across an authority restart is not claimed by the reference | DEFINED_BY_THIS_GATE (§23) |

★ = exercised by the suite (§26). Conflicts with existing authority:
**none.**

---

## 28. Non-goals

GPU execution, AI inference, metering, pricing, marketplace, MBO
settlement; any vendor adapter (NVIDIA, AMD, Intel, cloud confidential VM);
hardware attestation of any kind; proof of output correctness (#52);
payment or metadata privacy; an on-chain attestation commitment; a change
to `ComputeTask`, `Receipt`, rules (k)–(s), `rpc_v0.3` or the SDK wire; any
modification of `compute-conformance-v1` or of J's semantics; durable
replay protection across restarts in the reference; location evidence.

---

## 29. Future hardware adapters

**Follow-up gate — Hardware Attestation Adapter, first production
backend.** The first real verifier: a confidential-VM, CPU-vendor or
GPU-vendor adapter implementing `AttestationVerifier`, with its own
evidence format, its own trust-root distribution (certificate chains,
revocation), a `HARDWARE_ATTESTED` label, and the same suite run against
it plus the adapter's own tests. Until it exists, K is
`REFERENCE_ATTESTED`: the mechanism is proven, the provider-resistant
property is not, and nothing in this repository should be described as
production hardware-confidential.

---

## See also

- [`compute-provider-capability-policy.md`](compute-provider-capability-policy.md) — J: the policy this document makes satisfiable
- [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md) — parent architecture (§10, §11, §17)
- [`compute-control-plane-worker-interface.md`](compute-control-plane-worker-interface.md) — E (§14)
- [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md) — F (§9, §14)
- [`compute-conformance.md`](../development/compute-conformance.md) — `compute-conformance-v1` (C37)
- [`reference-worker.md`](../development/reference-worker.md) — the reference implementation
- [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md) — normative, Released
- [#142](https://github.com/MbongoChain/mbongo-chain/issues/142) — Workstream K
- [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (future)
