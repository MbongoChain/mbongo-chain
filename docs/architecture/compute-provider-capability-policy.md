# Provider capability and policy model: the eligibility contract

> **Document type:** Architecture — policy contract for off-chain components
> **Status:** Architectural authority for how the Compute control plane
> decides whether a provider's worker is *eligible* to execute an accepted
> `ComputeTask`: what a provider may claim, what a policy may require, how
> claims and requirements are compared, and what the decision may and may
> not do. Defines no consensus rule, no RPC method, no SDK type, no wire
> format and no transport; where anything here appears to conflict with a
> normative source below, the normative source wins.
> **Normative sources:** [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md)
> (Released), [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md),
> [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md),
> [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) (FROZEN),
> [`rpc_v0.3.md`](../specs/rpc_v0.3.md) (FROZEN),
> [`VISION_v1.md`](../VISION_v1.md)
> **Parent architecture:** [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md)
> (the four-plane model; §8, §17 and §19 are the dimensions this document
> gives semantics to),
> [`compute-control-plane-worker-interface.md`](compute-control-plane-worker-interface.md)
> ("E": identities, sessions, admission, leases) and
> [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md)
> ("F": capabilities, key release). This document refines the two
> CONTROL-PLANE POLICY rows of E §6 and the confidential extension point of
> E §14 / F §9, reuses every identity and grant those documents define, and
> yields to all three on any conflict.
> **Contract version:** `provider-policy-v1` — an architecture / control-plane
> version, non-consensus (§25).

This is Workstream J, the first architecture gate after the first Compute
vertical ([#126](https://github.com/MbongoChain/mbongo-chain/issues/126),
closed) and the prerequisite for Workstream K, confidential-compute
integration. It answers one question:

> **Given an accepted `ComputeTask`, its committed executor, a provider's
> capability advertisement, the worker session presenting it, an effective
> policy and whatever evidence exists — may this worker execute this task?**

The answer is a *policy* answer. It is produced off-chain, by the control
plane, and it changes nothing the chain has decided. A provider is never
eligible merely because it advertises attractive capabilities: eligibility
is the conjunction of the chain's executor authority, the worker's proven
identity, the provider's claims, the policy in force and the evidence that
backs those claims — and where evidence is required and absent, the answer
is no.

Nothing here is implemented as a control-plane product. A small **reference
evaluator** of the semantics in §10 exists in `crates/mbongo-compute/src/policy.rs`
with the test group `provider_policy` (§30); it is pure, deterministic and
not wired into the reference control plane, whose admission remains the
E §6 checks it implements today.

---

## 1. Purpose and the one fact everything rests on

RFC 0005 §2.5 considered three authorisation models and **chose a single
named executor** over "a general authorisation policy commitment", which it
deferred. The consequence for this document is total: **no policy is on the
chain, and no policy can change who may answer a task.** `task.executor` is
committed before the task exists (E §1), a different executor is a different
task (RFC 0005 §2.6), and consensus rejects a receipt from anyone else (rule
s).

So provider selection under policy happens where F §1 already places it —
**before submission**, when the client decides whom to name — and eligibility
under policy happens where E §6 already places it — **at admission**, when
the control plane decides whether the worker that proved the named executor's
key may proceed. This document gives those two decisions a shared vocabulary
and a shared, deterministic evaluation. It does not move either of them.

Provider selection *may* be constrained by privacy policy (parent P7,
REQUIRES_OFFCHAIN_ARCHITECTURE); jurisdiction and residency *are*
control-plane policy (P11); worker capability advertisements *are* claims
evaluated by control-plane policy (E §5.1). This document is that off-chain
architecture.

---

## 2. Authority boundary

| Authority | Decides | Never decides |
|---|---|---|
| **Chain** (plane 1) | task identity; `task.executor`; task validity, rules (k)–(s); receipt binding (RFC 0005, RFC 0002) | which provider, which worker instance, which policy, what any claim is worth |
| **Control plane** (plane 2) | eligibility policy; capability matching; operational lease decisions (E §3, §6) | executor identity; validity of a receipt; whether data is released (F) |
| **Private data plane** (plane 3) | access to private objects under capabilities; retention, deletion, key handling (F §5, §12, §14) | eligibility; who the executor is |
| **Execution environment** (plane 4) | the actual hardware and runtime properties; in future, attestation evidence about itself (parent §10–§11) | whether its own evidence is acceptable — that is policy, evaluated in plane 2 |

The whole of this document lives in plane 2, reads facts from planes 1 and 4,
and hands a decision to the lease path of E §7 and, in the future, to the
key-release path of F §9. It writes nothing to plane 1.

**Consensus does not choose providers, parse policy, or parse attestation**
(parent §11; E22). No validator ever sees a policy, a claim, a profile name
or a reason code defined here.

---

## 3. Exact authority map

Every dimension below is taken from repository authority; none is invented.
"Where enforced" names the plane that can make the property true; "concern"
is the parent's own classification (privacy architecture §17.1).

| Requirement | Authority | Current meaning | Where enforced | Concern |
|---|---|---|---|---|
| privacy level required | parent §17.1 row 1; §7; §10 | which profile the task needs: PUBLIC (provider sees plaintext), CONFIDENTIAL (attested, conditional key release) | control plane matches; data plane enforces key release | MIXED |
| provider identity | parent §8 ("an executor key is an identity in the protocol sense; a real-world identity is a control-plane attribute"); E §2 | who operates the executor; a control-plane attribute, not protocol | control plane | CONTROL_PLANE |
| provider reputation / trust | parent §8 ("history, derived off-chain, possibly from on-chain receipts"); §17.1 row 2 | a control-plane-derived record; **not** a provider self-claim (§5.4) | control plane | CONTROL_PLANE |
| hardware capability | parent §8 ("GPU class, memory, throughput"); §17.1 row 3; E §5.1 ("resource availability") | a worker claim; nothing is proven (E §5.1) | control plane (matching); execution environment (reality) | CONTROL_PLANE |
| runtime / execution capability | E §5 (contract version, representation tags per F §4.2); E §5.1 ("runtime and model family", "supported execution profile"); E §6 row 7 | which contract, representations and execution profiles the session can run; worker-local claim, control-plane policy | control plane | CONTROL_PLANE |
| attestation capability | parent §17.1 row 4; §10–§11; E §5.1 ("in future — attestation capability"); E §14; F §9 | whether an environment can produce evidence and whether that evidence is acceptable; verification in plane 2, generation in plane 4; FUTURE (P9) | control plane (verify); execution environment (generate); data plane (release) | MIXED |
| jurisdiction / residency | parent §17.1 row 5; §19 ("asserted or certified"; "an IP address is not proof of physical location"); P11 | where the provider, worker and data are; a claim unless certified off-chain; never a consensus property | control plane | CONTROL_PLANE |
| retention | parent §15; §17.1 row 6; F §12, §14 | what the provider and data plane promise to keep and delete; enforced only in the data plane and by the worker's destruction obligation | data plane (enforce); control plane (policy) | MIXED |
| egress and network policy | parent §8 ("not send onward"); §17.1 row 7 | what the execution environment may send out; a claim today | execution environment; control plane (policy) | MIXED |
| verification strength | parent §17.1 row 8 ("what independent checking, if any, a workload receives"); §18; RFC 0005 §9.2; [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) | independent checking of **results** — redundant execution, attestation-based, zero-knowledge; **none exists**; distinct from the evidence level of a claim (§6) | control plane today; a chain component only through a future verification RFC | CONTROL_PLANE |
| price | parent §8 ("commercial terms; none of this is consensus"); §17.1 row 9 | a commercial claim and a policy bound; no metering, oracle, auction or settlement exists (lock v0.4 §"Experimental and Deferred Surfaces") | control plane | CONTROL_PLANE |
| latency | parent §8; §17.1 row 10 | a commercial claim and a policy bound; no measurement system exists | control plane | CONTROL_PLANE |

Two rows that authority does **not** supply, and this document therefore does
not add as dimensions: a vendor identifier (E28, F18, P13 — no vendor is
protocol or architecture authority) and any on-chain registry of providers
(RFC 0005 §6; E §5 "registration is a session, not a transaction").

---

## 4. Terminology: provider, worker, executor, session

E §2 already separates these; this document uses E's definitions verbatim
and adds the provider-side vocabulary policy needs.

| Term | Meaning | Authority | Identity is… |
|---|---|---|---|
| **executor** | the `Address` named in `task.executor`; the public half of an Ed25519 key | chain (RFC 0005 §2.1) | protocol identity; proven by possession (F §5.3, E §5) |
| **provider** | the administrative entity — organisation or operator — offering compute; "may run many executors, or one executor across many machines" | E §2; parent §8 | a **control-plane attribute** (§5.2); never protocol |
| **worker class** | a provider-defined group of worker instances sharing hardware, runtime and location properties (a "pool") | this document | a control-plane label under the provider |
| **worker instance** | one live incarnation of a worker process; the unit a lease is granted to | E §2 | operational identifier |
| **worker session** | the authenticated relationship between one instance and the control plane, bound to one executor whose key the instance proved | E §5 | operational; the unit session-scoped claims bind to (§20) |
| **capability advertisement** | what a provider says about itself, its worker classes and a session (§5) | E §5.1 ("worker claims") | a document, not an identity and not a credential |

What one provider may operate — all YES, by E §2 and by the absence of any
authority to the contrary:

| May one provider… | Answer | Consequence for policy |
|---|---|---|
| operate several executors | yes ("may run many executors") | a requirement on the provider applies to every executor it operates; the executor gate (§10.1) is still per task |
| run several worker instances of one executor | yes ("one executor across many machines") | session-scoped claims differ per instance; provider-scoped claims are shared |
| operate several hardware pools | yes | worker classes carry hardware and runtime claims; a task is matched against the class of the *session*, never the provider's best pool |
| operate in several jurisdictions | yes | the provider's legal jurisdiction is one value; worker location, data residency and the other location dimensions are per class or per session (§16) |

**Provider identity is not executor identity (J18).** A provider may prove,
through a session, that it holds an executor key; that proves the executor,
not the provider's name, organisation or location. The mapping "this
executor belongs to this provider" is a control-plane record, kept because a
policy may allow or deny a provider; it is a claim with whatever evidence the
control plane has for it (§6), and it never substitutes for the executor
gate.

**A worker instance is not the provider (J19).** Claims that are true of an
instance — what it has loaded, what memory is free, what environment it
attests — are not true of the provider, and a provider-wide claim does not
describe any particular instance (§20).

---

## 5. Capability advertisement model

### 5.1 Shape

An advertisement is a structured, versioned document the control plane holds
about a provider. Conceptually:

```
advertisement {
  schema             "capability-advertisement-v1"
  provider_id        control-plane-local identifier (§5.2)
  executors          set of Address the provider operates
  issued_at, expires_at                          (§21)
  claims             map: dimension key -> claim (§5.3)
  worker_classes     map: class label -> { claims }        optional
  session_claims     map: session_id -> { claims }         per live session
}
claim {
  value              typed: token | number | set of tokens | boolean (§18)
  scope              PROVIDER | WORKER_CLASS | SESSION (§20)
  evidence           optional evidence record (§6.2); absent means CLAIM
}
```

The exact field names, serialisation and transport are **not fixed here**;
E §22 and F §19 leave every off-chain encoding to an implementation gate,
and this document does the same. What is fixed is the vocabulary (§5.3), the
value types (§18), the scope and evidence semantics (§6, §20) and how a
requirement is evaluated against a claim (§10).

### 5.2 Provider identity

`provider_id` is an opaque, control-plane-local identifier. It may be backed
by an organisation identity, a certificate or public key, or a registry
record the deployment trusts; any of those is *evidence for* the identity
claim (§6), not the identifier itself. It is:

- **not** `task_id`, `attempt_id`, `lease_id`, a session id or an executor
  `Address` — none of those identifies a provider, and `task_id` is an
  identifier of a task and nothing else (F §5.2, F3);
- **not a bearer capability** (§23): presenting a
  `provider_id` grants nothing, exactly as presenting an identifier or a
  locator grants nothing in F §3;
- **not on-chain** and not required to be (RFC 0005 §6; parent §8).

### 5.3 Dimension vocabulary, version 1

Keys are dotted, lower-case, ASCII. Classification is per this gate:
**REQUIRED_NOW** (every advertisement carries it; the reference control
plane already needs it), **OPTIONAL_NOW** (may be advertised and required by
policy today; a plain claim is the only evidence available),
**FUTURE** (the key is reserved; no implementation can satisfy a requirement
on it today), **NOT_ALLOWED** (not a dimension of this contract).

| Key | Type | Scope | Class | Authority | Evidence possible today | Hard-match semantics | Future |
|---|---|---|---|---|---|---|---|
| `identity.provider_id` | token | PROVIDER | REQUIRED_NOW | E §2, parent §8 | CLAIM; CERTIFIED where the deployment has an organisation credential | ONE_OF / NONE_OF (allow/deny lists) | — |
| `identity.executors` | set of `Address` | PROVIDER | REQUIRED_NOW | E §2 | CONTROL_PLANE_VERIFIED per session, by proof of possession (E §5) | CONTAINS `task.executor` — part of the executor gate (§10.1) | — |
| `execution.contract_version` | token | SESSION | REQUIRED_NOW | E §5 | CONTROL_PLANE_VERIFIED by the session handshake | EQUALS / ONE_OF | — |
| `execution.representation_tags` | set | SESSION | REQUIRED_NOW | F §4.2, E §5 | CLAIM | CONTAINS the task's representation | — |
| `execution.profile_tags` | set | SESSION | REQUIRED_NOW | E §6 row 7; reference `spec_tags` | CLAIM | CONTAINS the task's `execution_spec` tag | — |
| `privacy.confidential_execution` | boolean | SESSION | OPTIONAL_NOW | parent §10; E §5.1; conformance `Claims` | CLAIM only today; ATTESTED through K | IS_TRUE with `min_evidence ≥ ATTESTED` (§14); a plain `true` **never** satisfies | K supplies the evidence |
| `privacy.attestation_formats` | set | WORKER_CLASS | FUTURE | parent §10, E §14 | — | — | K |
| `jurisdiction.provider` | token | PROVIDER | OPTIONAL_NOW | parent §19, P11 | CLAIM; CERTIFIED by off-chain certification | ONE_OF / NONE_OF | attestation binding hardware to certified location (parent §19) |
| `jurisdiction.worker_location` | token | WORKER_CLASS or SESSION | OPTIONAL_NOW | parent §19 | CLAIM; CERTIFIED | ONE_OF / NONE_OF | as above |
| `residency.data` | token | WORKER_CLASS | OPTIONAL_NOW | parent §17.1, §19 | CLAIM; CERTIFIED | ONE_OF / NONE_OF | — |
| `residency.key` | token | WORKER_CLASS | OPTIONAL_NOW | F §14 (key holders) | CLAIM; CERTIFIED | ONE_OF / NONE_OF | — |
| `residency.result_storage` | token | WORKER_CLASS | OPTIONAL_NOW | F §10 | CLAIM; CERTIFIED | ONE_OF / NONE_OF | — |
| `retention.class` | token: `EPHEMERAL` \| `BOUNDED` \| `PERSISTENT` | PROVIDER | OPTIONAL_NOW | parent §15; F §12, §14 | CLAIM | ONE_OF; ordered EPHEMERAL < BOUNDED < PERSISTENT for AT_MOST | data-plane audit evidence |
| `retention.max_seconds` | number | PROVIDER | OPTIONAL_NOW | parent §15 | CLAIM | AT_MOST | — |
| `egress.mode` | token: `NONE` \| `RESTRICTED` \| `UNRESTRICTED` | WORKER_CLASS | OPTIONAL_NOW | parent §8, §17.1 | CLAIM | ONE_OF; ordered NONE < RESTRICTED < UNRESTRICTED for AT_MOST | runtime enforcement evidence |
| `egress.destinations` | set | WORKER_CLASS | OPTIONAL_NOW | parent §17.1 | CLAIM | subset-of a policy set | — |
| `hardware.accelerator_class` | token (abstract, deployment vocabulary) | WORKER_CLASS | OPTIONAL_NOW | parent §8 | CLAIM | ONE_OF | attestation-derived |
| `hardware.memory_bytes` | number | WORKER_CLASS | OPTIONAL_NOW | parent §8 | CLAIM | AT_LEAST | — |
| `hardware.available_memory_bytes` | number | SESSION | OPTIONAL_NOW | E §5.1 ("resource availability") | CLAIM | AT_LEAST | — |
| `hardware.throughput_units` | number (deployment-defined unit) | WORKER_CLASS | OPTIONAL_NOW | parent §8 ("throughput") | CLAIM | AT_LEAST | — |
| `hardware.runtime_features` | set | WORKER_CLASS | OPTIONAL_NOW | E §5.1 | CLAIM | CONTAINS_ALL | — |
| `runtime.family` / `runtime.version` | token | WORKER_CLASS | OPTIONAL_NOW | E §5.1 ("runtime and model family") | CLAIM | ONE_OF / EQUALS | — |
| `model.family`, `model.id`, `model.version`, `model.tokenizer_id`, `model.max_context` | token / number | WORKER_CLASS | FUTURE | E §5.1 ("model family"); parent §15 (model-agnostic) | — | reserved; an AI-worker gate defines the values | AI worker gate |
| `verification.result` | token: `NONE` \| a named mechanism class | PROVIDER | OPTIONAL_NOW | parent §17.1 row 8, §18; #52 | only `NONE` is satisfiable — no mechanism exists | ONE_OF | a verification RFC names classes |
| `commercial.price` + `commercial.price_unit` | number + token | PROVIDER | OPTIONAL_NOW | parent §8, §17.1 | CLAIM | AT_MOST, only when the units are equal (§18) | a pricing gate |
| `commercial.latency_ms` | number | WORKER_CLASS | OPTIONAL_NOW | parent §8, §17.1 | CLAIM (measured evidence is FUTURE) | AT_MOST | measurement |
| `availability.state` | token | SESSION | OPTIONAL_NOW | E §5.1 | CLAIM | EQUALS | — |
| `x.<namespace>.<field>` | any | any | extension | §25 | per extension | per extension; UNKNOWN to a control plane that does not know the namespace | — |
| `reputation.*` | — | — | **NOT_ALLOWED as a self-claim** | parent §8: reputation is derived by the control plane | control-plane-derived input (§5.4) | — | — |
| any vendor name as a key | — | — | **NOT_ALLOWED** | E28, F18, P13 | — | — | vendor tokens may appear as *values* under `x.*`, mapped by adapters; never as a key the core grammar defines |

Nothing in the FUTURE rows is mandatory and nothing in the OPTIONAL rows is
required by this document; a policy decides what it requires (§7). The
reference worker advertises exactly the REQUIRED_NOW rows (its session
carries `spec_tags`) and `privacy.confidential_execution = false`.

### 5.4 Control-plane-derived facts

Two inputs to eligibility are **not** part of the advertisement, because the
provider is not the authority for them:

| Fact | Authority | Evidence level |
|---|---|---|
| executor possession for this session | the control plane verified proof of possession (E §5) | CONTROL_PLANE_VERIFIED |
| reputation / history | the control plane's own records, "possibly from on-chain receipts" (parent §8) | CONTROL_PLANE_VERIFIED, by definition of being the control plane's own derivation; no scoring formula, stake or slashing is defined (§26) |

A policy may reference `reputation.score` as a dimension; its value comes
from the control plane, and an advertisement that carries it is malformed.

---

## 6. Claims are not proofs: the evidence model

### 6.1 The distinction

Every value in an advertisement is a **claim**. "GPU class X", "jurisdiction
CA", "confidential execution true", "retention zero", "latency under 100 ms"
— each is something the provider said. E §5.1 is exact: "Nothing about them
is cryptographically proven today, and the contract must not label them as
verified hardware facts. The only proven property in the ordinary profile is
possession of the executor key."

A policy therefore states, per requirement, **how much evidence it needs**.
A claim with less evidence than required does not satisfy the requirement,
however plausible the claim.

### 6.2 Evidence levels

An ordered ladder. Each level is defined by *who established the fact and
how*, never by a vendor format:

| Level | Meaning | Repository grounding | Achievable today |
|---|---|---|---|
| `CLAIM` | the provider asserted it; nothing checked | E §5.1 | yes — every advertised value |
| `CONTROL_PLANE_VERIFIED` | the control plane itself established it by a check it performed | E §5 (possession over a challenge); §5.4 | yes — executor possession, contract handshake, reputation records |
| `CERTIFIED` | a party the policy names as trusted attested to it off-chain (organisation credential, audit, residency certification) | parent §19 ("asserted or **certified**"); §17.2 SOVEREIGN ("certified off-chain") | only where a deployment has such a party; none is defined here |
| `ATTESTED` | the execution environment produced remote attestation evidence and a verifier checked it against the expected measured code, configuration and hardware class | parent §10–§11; E §14; F §9 | **no** — no verifier exists; Workstream K |
| `PROVEN` | a cryptographic proof of the property that needs no trusted verifier | parent §18 (zero-knowledge approaches, research #52) | **no** — reserved; no semantics defined here |

The mission vocabulary maps onto this ladder as CLAIM_ONLY → `CLAIM`,
CONTROL_PLANE_VERIFIED → `CONTROL_PLANE_VERIFIED`, THIRD_PARTY_ATTESTED →
`CERTIFIED`, HARDWARE_ATTESTED → `ATTESTED`, CRYPTOGRAPHICALLY_PROVEN →
`PROVEN`. This document defines **no cryptographic semantics** for
`ATTESTED` or `PROVEN`: what evidence looks like, which roots are acceptable
and how it is verified are K's to define (§22). What this document fixes is
only that a requirement can say *at least this level*, and that a claim
without evidence at that level fails.

An **evidence record** attached to a claim carries, at the abstraction
level:

```
evidence {
  level              one of the ladder above CLAIM
  issued_at          when established
  expires_at         optional; after it the level reverts to CLAIM (§19)
  binding            PROVIDER | SESSION(session_id) | CHALLENGE(nonce, session_id)
  verifier           who established it (control plane, named certifier, K verifier)
  reference          opaque identifier of the underlying material — never the material itself
}
```

The underlying material (a certificate, an attestation quote) is **not** part
of the advertisement, the decision or any log (§24). Only its level, binding,
window and an opaque reference are.

### 6.3 What this buys

Before K exists, the ladder lets a policy *express* a CONFIDENTIAL
requirement that no current provider can satisfy (§14). That is the intended
state: the requirement is representable, evaluation is deterministic, and the
answer is "ineligible: attestation required" rather than "eligible because
the provider said so" (J16, §23).

---

## 7. Policy requirement model

A **requirement** is one constraint on one dimension, with the evidence,
freshness and unknown-handling the policy author chose:

```
requirement {
  id                 unique within the effective policy; stable across evaluations
  source             DATA_OWNER | ORGANIZATION | CONTROL_PLANE | PROVIDER  (§8)
  kind               HARD | SOFT                                          (§9)
  dimension          a key from §5.3, §5.4 or an x.* extension
  constraint         EQUALS v | ONE_OF S | NONE_OF S | AT_LEAST n | AT_MOST n
                     | CONTAINS_ALL S | SUBSET_OF S | IS_TRUE
  min_evidence       CLAIM | CONTROL_PLANE_VERIFIED | CERTIFIED | ATTESTED | PROVEN
                     (default CLAIM)
  freshness          ANY | SESSION            (default ANY; §20)
  on_unknown         FAIL | ALLOW             (default FAIL; §11)
}
policy_fragment { schema "provider-policy-v1"; source; requirements[] }
```

Requirements are **off-chain and non-consensus** (J1, J2). None is a field
of `ComputeTask`: the six-field envelope is frozen (lock v0.4 B1), and
`execution_spec` is a public, bounded, opaque description that is not a
policy transport any more than it is a private-data transport (P5). A client
that wants its policy applied gives it to the control plane it uses, or
applies it itself before choosing whom to name.

The dimensions a requirement may name are exactly those of §5.3–§5.4. The
mission's list — minimum privacy profile, provider allow/deny list,
required and forbidden jurisdiction, residency, retention maximum, egress,
minimum verification strength, required execution profile, hardware and
runtime characteristics, maximum latency, maximum price — maps onto them
one for one; "minimum privacy profile" is not a dimension but a **template**
that expands into requirements (§12–§15).

---

## 8. Policy sources and composition

### 8.1 Sources

| Source | Who | Grounding | Typical content |
|---|---|---|---|
| `DATA_OWNER` | the client / submitter, "the root of authority over its own private objects" (F §2) | F §2, §5.1; parent §11 ("the client's policy") | privacy profile, residency, retention, egress, provider allow/deny |
| `ORGANIZATION` | the deployment or organisation the client acts within | this gate | jurisdiction rules, approved providers, minimum evidence |
| `CONTROL_PLANE` | the operator of the control plane | E §6 ("the control plane's own policy"), E §13 (quota, operator decision) | confirmation depth, quota, denylists, supported contract versions |
| `PROVIDER` | the provider itself, about what it will accept | parent §8 (negotiation) | "no CONFIDENTIAL tasks", served jurisdictions, price floors as a refusal |

### 8.2 Composition rule — a J architectural decision

Repository authority does not define precedence among these sources. This
document decides it, and states the decision as its own:

> **Effective policy is the union of every source's HARD requirements.**
> A source may **tighten** (add a requirement); no source may **weaken**
> (remove, relax or override another source's requirement). A worker is
> eligible only if it satisfies every HARD requirement of every source.

Justification: F §2 makes the data owner the root of authority over its
objects, E §6 says the control plane "declines or defers — it never 'fixes'
the task", and the safety property the parent's whole design serves is that
a weaker party cannot silently reduce a stronger party's protection. Union
of hard requirements is the only composition with that property that is also
deterministic and order-independent.

| Source | Can tighten? | Can weaken another source? | On conflict with another source |
|---|---|---|---|
| `DATA_OWNER` | yes | no | POLICY_CONFLICT (§8.3) |
| `ORGANIZATION` | yes | no | POLICY_CONFLICT |
| `CONTROL_PLANE` | yes | no | POLICY_CONFLICT |
| `PROVIDER` | yes — it may refuse work | no | POLICY_CONFLICT |

Answers to the questions this rule settles:

- **Can client policy weaken organisation policy?** No. The organisation's
  requirement stays in the effective set.
- **Can provider policy weaken data-owner policy?** No. A provider can only
  add its own refusals.
- **Can a lower-priority source set `on_unknown = ALLOW` for a dimension a
  stronger source requires with `FAIL`?** Both requirements are in the set;
  the `FAIL` one still fails on unknown. Relaxation by a second requirement
  is impossible by construction (J41).

### 8.3 Conflict

Two HARD requirements on the same dimension whose constraints no value can
satisfy jointly — `jurisdiction.provider ONE_OF {CA}` and
`jurisdiction.provider ONE_OF {US}`; `retention.max_seconds AT_MOST 0` and
`AT_LEAST 60` — make the effective policy **unsatisfiable**. The evaluator
does not resolve this by precedence: **every** worker is ineligible with
reason `POLICY_CONFLICT`, and the conflict is reported to the policy
authors. Choosing a winner silently would be exactly the weakening §8.2
forbids.

**When no provider satisfies all constraints** the task is simply not
admissible under this control plane (E §6: "declines or defers"). The task
stays open on-chain forever (RFC 0005 §2.9); the client may relax *its own*
requirements or submit a new task naming another executor. Nothing is
"fixed" for it.

### 8.4 Soft preferences across sources

SOFT requirements (§9) are collected, each labelled with its source, in the
fixed order `DATA_OWNER, ORGANIZATION, CONTROL_PLANE, PROVIDER`. This
document defines **no aggregation** of them into a score; that is selection
(§9.2), out of scope. The order exists only so that a future ranking
consumer sees a deterministic sequence.

---

## 9. Hard constraints versus soft preferences

### 9.1 The split

| Kind | Effect on eligibility | Unknown claim | Typical dimensions |
|---|---|---|---|
| **HARD** | any NOT_SATISFIED makes the worker ineligible; UNKNOWN does too unless `on_unknown = ALLOW` | fails closed (§11) | executor, compatibility, privacy profile, evidence, jurisdiction, residency, retention, egress, provider allow/deny, price cap, latency cap |
| **SOFT** | none — recorded in the decision, never changes `eligible` | recorded as UNKNOWN; never a failure | price, latency, availability, reputation as a ranking input |

Hard constraints are evaluated first and completely; soft preferences are
evaluated after and cannot override a hard failure (J24, J25). A dimension
may carry both: `commercial.price AT_MOST 100 (HARD)` and
`commercial.price AT_MOST 60 (SOFT)` — the first decides eligibility, the
second is a preference among the eligible.

### 9.2 Eligibility is not selection

This document defines **who is allowed**. It does not define who is
cheapest, fastest or best, and it defines no ranking, ordering, bidding or
market-clearing (J35). A future routing or marketplace gate may rank the
eligible set by price, latency, reputation or availability using the SOFT
findings of §10.4; it must start from the eligible set this contract
produces and may not admit a worker this contract rejected.

---

## 10. The eligibility function

### 10.1 Signature and order

```
eligible(task, session, advertisement, derived, effective_policy, now)
  -> Decision
```

| Input | Content | Authority |
|---|---|---|
| `task` | `task_id`, `task.executor`, `execution_spec` (as a profile tag), the representation tag the client registered | chain; F §4.2 |
| `session` | the executor this session **proved**, `session_id`, worker instance, worker class label, contract version, the session-scoped claims | E §5 |
| `advertisement` | §5 | control plane's record of the provider |
| `derived` | control-plane-derived facts (§5.4): possession verified, reputation | control plane |
| `effective_policy` | the composition of §8 | policy authors |
| `now` | the evaluation time, an explicit input | the caller's clock |

The evaluation proceeds in this fixed order, and **the first gate
short-circuits**:

```
0  policy well-formed and satisfiable            else INELIGIBLE(POLICY_INVALID | POLICY_CONFLICT)
1  EXECUTOR GATE (§10.2)                         else INELIGIBLE(EXECUTOR_MISMATCH) — nothing further evaluated
2  advertisement live: not expired, not revoked  else INELIGIBLE(ADVERTISEMENT_EXPIRED | ADVERTISEMENT_REVOKED)
3  compatibility: contract version, representation tag, profile tag (E §5, §6 row 7)
                                                 else hard failure(s) CONTRACT_VERSION_UNSUPPORTED |
                                                      REPRESENTATION_UNSUPPORTED | PROFILE_UNSUPPORTED
4  every HARD requirement, in ascending order of requirement id (§10.3)
5  every SOFT requirement, recorded only (§9)
6  Decision (§10.4)
```

Steps 3–5 are evaluated **completely** — every requirement, not just the
first failure — so that the decision explains every reason (§11, §23). Step
1 is the exception: a session that did not prove the task's executor learns
nothing about the policy that would have applied, because policy metadata
can itself be sensitive (§24) and an unauthorised party has no business
with it.

E §6's chain-derived rows — the task exists in a block, no receipt is
anchored — are **preconditions** of calling this function, established by
the control plane from the chain exactly as E §4 and §6 require. This
function does not re-decide them and cannot override them.

### 10.2 The executor gate comes first

```
session.proved_executor == task.executor
  and task.executor ∈ advertisement.identity.executors
```

Both must hold. The first is E §6 row 2 ("CHAIN-DERIVED identity,
WORKER-PROVEN possession"); the second ties the provider record to the
session, so that a provider cannot be evaluated under a different provider's
advertisement. A provider with every other capability and the wrong executor
is ineligible, with only `EXECUTOR_MISMATCH` reported (J4, J5). No policy,
preference, evidence or operator setting reaches past this gate; the control
plane has no field in which to "prefer" another executor, and E1/E17 and
rule (s) make the point moot on-chain.

### 10.3 Evaluating one requirement

For a requirement `r` on dimension `d`, given the claim `c` for `d` (from
the session's claims, then the session's worker class, then the provider —
the **most specific scope wins**, and a session-scoped value shadows a
provider-wide one), or from `derived` for §5.4 dimensions:

```
1  no claim for d                              -> UNKNOWN (CAPABILITY_UNKNOWN)
2  r.freshness = SESSION and c.scope ≠ SESSION,
   or c.evidence.binding names another session -> UNKNOWN (EVIDENCE_NOT_SESSION_BOUND)
3  effective level =
     c.evidence.level  if c.evidence present, not expired at `now`, not revoked,
                       and its binding is PROVIDER or this session
     CLAIM             otherwise
   if effective level < r.min_evidence         -> NOT_SATISFIED
        (ATTESTATION_REQUIRED if r.min_evidence ≥ ATTESTED,
         EVIDENCE_EXPIRED     if evidence existed but is expired/revoked,
         INSUFFICIENT_EVIDENCE otherwise)
4  value type ≠ constraint's expected type      -> UNKNOWN (CAPABILITY_UNKNOWN)
5  compare per §18                              -> SATISFIED | NOT_SATISFIED(dimension code, §11 table)
```

Then, for `r.kind = HARD`: NOT_SATISFIED is a hard failure; UNKNOWN is a
hard failure unless `r.on_unknown = ALLOW`, in which case it is recorded
under `unknown` with `allowed = true`. For `r.kind = SOFT`: the outcome is
recorded and nothing else happens.

**Missing evidence is never proof** (J10, J11). A claim that says exactly
what the policy wants, at level `CLAIM`, against a requirement whose
`min_evidence` is `CERTIFIED` or `ATTESTED`, is NOT_SATISFIED — not UNKNOWN,
not "probably fine".

### 10.4 The decision

```
Decision {
  eligible            bool
  policy_version      "provider-policy-v1"
  executor_gate       PASSED | EXECUTOR_MISMATCH
  hard_failures       [ { requirement_id, dimension, source, code } ]
  unknown             [ { requirement_id, dimension, source, allowed } ]
  satisfied           [ { requirement_id, dimension, evidence_level } ]
  soft                [ { requirement_id, dimension, source, outcome } ]
  lowest_evidence     the minimum evidence level among satisfied HARD requirements —
                      what the eligibility actually rests on; CLAIM for a PUBLIC decision
  evaluated_at        the `now` input
}
```

`eligible` is true iff the executor gate passed, the advertisement is live,
compatibility holds, and `hard_failures` is empty. The decision carries
requirement ids, dimension keys, sources, codes and evidence levels. It
carries **no claim values, no policy values, no evidence material and no
secret** (§24): "retention too long" is reported, "retention was 86400
against a maximum of 3600" is not, by default.

### 10.5 Determinism

For identical `task`, `session`, `advertisement`, `derived`,
`effective_policy` and `now`, the function returns an identical decision
(J23). The rules that make this so, and that an implementation must keep:

- the evaluation order of §10.1 is fixed; requirements are ordered by their
  ids, and sets are compared as sets;
- the claim lookup order (session, class, provider) is fixed;
- every comparison follows §18: byte-exact tokens, integer numbers in fixed
  base units, no locale, no case folding, no string-formatted numbers;
- `now` is an input, never read from a clock inside the evaluator;
- composition (§8.2) is a union, so fragment order does not matter, and a
  conflict is a fixed outcome, not a tie-break.

Two control planes with the same inputs reach the same decision. That is the
property that lets a client trust a decision it did not compute.

---

## 11. Unknown, missing and the reason taxonomy

### 11.1 Three-valued outcome

Every requirement evaluates to exactly one of:

| Outcome | Meaning |
|---|---|
| `SATISFIED` | a claim exists, with evidence at least `min_evidence`, fresh as required, and the value meets the constraint |
| `NOT_SATISFIED` | a claim or evidence exists and it fails the constraint or the evidence bar |
| `UNKNOWN` | no claim, wrong type, no session-bound claim where freshness demands one, or an extension namespace this control plane does not understand |

For a HARD requirement, **UNKNOWN fails closed** unless the requirement's
own author set `on_unknown = ALLOW` (J10–J12). `requires jurisdiction = CA;
provider jurisdiction = UNKNOWN → INELIGIBLE` is the default and the
example. Allowing unknown is a per-requirement, explicit, author-owned
relaxation; it cannot be granted by another source (§8.2).

### 11.2 Reason codes

Stable, implementation-neutral, **control-plane** codes. None is a consensus
error, none appears in a transaction or receipt, and none is an RPC error
code (J39). Each finding carries the code, the dimension and the
requirement id.

| Code | Raised when |
|---|---|
| `POLICY_INVALID` | the effective policy is malformed (unknown constraint for a type, a VERIFIED template with no strengthening, a `reputation.*` self-claim in an advertisement) |
| `POLICY_CONFLICT` | two HARD requirements cannot be jointly satisfied (§8.3) |
| `EXECUTOR_MISMATCH` | the executor gate failed (§10.2) — the only finding reported in that case |
| `ADVERTISEMENT_EXPIRED` / `ADVERTISEMENT_REVOKED` | §21 |
| `CONTRACT_VERSION_UNSUPPORTED` | the session's contract version is not one the policy accepts |
| `REPRESENTATION_UNSUPPORTED` | the session cannot read the task's representation (F §4.2) |
| `PROFILE_UNSUPPORTED` | the session cannot run the task's `execution_spec` tag (E §6 row 7) |
| `PROVIDER_DENIED` | `identity.provider_id` is in a NONE_OF set |
| `PROVIDER_NOT_ALLOWED` | `identity.provider_id` is not in a ONE_OF set |
| `INSUFFICIENT_EVIDENCE` | the claim's effective evidence level is below `min_evidence` |
| `ATTESTATION_REQUIRED` | as above, where `min_evidence ≥ ATTESTED` |
| `EVIDENCE_EXPIRED` | evidence existed and is past `expires_at` or revoked (§19) |
| `EVIDENCE_NOT_SESSION_BOUND` | freshness SESSION required; the claim or its evidence is provider-wide or bound to another session (§20) |
| `CAPABILITY_UNKNOWN` | no claim, wrong type, or unknown extension namespace, where the requirement fails on unknown |
| `JURISDICTION_NOT_ALLOWED` | a `jurisdiction.*` constraint failed |
| `RESIDENCY_NOT_SATISFIED` | a `residency.*` constraint failed |
| `RETENTION_TOO_LONG` | a `retention.*` constraint failed |
| `EGRESS_NOT_ALLOWED` | an `egress.*` constraint failed |
| `HARDWARE_CAPABILITY_MISMATCH` | a `hardware.*` constraint failed |
| `RUNTIME_CAPABILITY_MISMATCH` | a `runtime.*` or `model.*` constraint failed |
| `CONFIDENTIALITY_NOT_CLAIMED` | `privacy.confidential_execution` IS_TRUE failed on the value (the provider does not even claim it) |
| `VERIFICATION_STRENGTH_UNAVAILABLE` | a `verification.result` constraint other than `NONE` — unsatisfiable today (#52) |
| `PRICE_ABOVE_MAX` / `LATENCY_ABOVE_MAX` | a HARD `commercial.*` cap failed |
| `REPUTATION_BELOW_MIN` | a `reputation.score` constraint failed on the control-plane-derived value |
| `AVAILABILITY_MISMATCH` | an `availability.*` constraint failed |
| `EXTENSION_MISMATCH` | an `x.*` constraint failed in a namespace the control plane understands |

Codes are added by minor extension (§25); an existing code's meaning is
never changed within version 1.

---

## 12. PUBLIC profile

Authority: parent §17.2 — "ordinary provider; content off-chain; no
confidentiality from the provider is claimed"; §7; P8.

As a template, PUBLIC expands to **no requirement beyond the executor gate
and compatibility** (§10.1 steps 1–3). Its meaning is stated as what it does
*not* guarantee:

| Property | PUBLIC |
|---|---|
| provider may see plaintext | **yes** (P8, E20, F12) |
| confidential execution required | no |
| evidence required for any claim | none; CLAIM suffices (`lowest_evidence = CLAIM`) |
| encrypted input / output | recommended, not required (F §9) |
| confidentiality guarantee | **none** (J14) |
| satisfiable today | yes — the reference worker under the reference control plane is a PUBLIC provider |

A policy may add any §5.3 requirement on top of PUBLIC (a jurisdiction, a
price cap); doing so does not make the task any less PUBLIC in the sense
above.

---

## 13. VERIFIED profile

Authority: parent §17.2 — "PUBLIC plus stronger provider identity,
reputation or independent verification of results".

VERIFIED is PUBLIC plus **at least one configured strengthening** from
exactly the three the authority names:

| Strengthening | Requirement it expands to | Satisfiable today |
|---|---|---|
| stronger provider identity | `identity.provider_id` with `min_evidence ≥ CERTIFIED` | only where the deployment has a certifier it trusts |
| reputation | `reputation.score AT_LEAST n` on the control-plane-derived value (§5.4) | only where the control plane keeps such records; no scoring formula is defined here |
| independent verification of results | `verification.result ONE_OF {named class}` | **no** — no mechanism exists; #52 |

A VERIFIED template with **no** strengthening configured is `POLICY_INVALID`:
otherwise VERIFIED would silently collapse into PUBLIC, which is the kind of
weakening §8 exists to prevent.

**VERIFIED ≠ CONFIDENTIAL (J15).** Nothing in VERIFIED constrains what the
provider sees; a VERIFIED provider sees plaintext exactly as a PUBLIC one
does. Of the things "verification" could refer to — identity, reputation,
result verification, software measurement, hardware class, jurisdiction,
runtime integrity — the first three are the profile's scope by authority;
software measurement, hardware class and runtime integrity are attestation
matters (§14, K); jurisdiction is §16.

---

## 14. CONFIDENTIAL profile

Authority: parent §17.2 — "attested protected execution with conditional key
release; confidentiality from the provider within the limits of §10"; §10,
§11; E §14; F §9; P9.

CONFIDENTIAL is a **policy template** whose semantics this document defines
and whose satisfaction K will make possible:

| Requirement | Constraint | Evidence | Freshness |
|---|---|---|---|
| `privacy.confidential_execution` | IS_TRUE | `min_evidence = ATTESTED` | SESSION |
| acceptable attestation root / measured environment | expressed as the *policy* K's verifier evaluates against — the verifier's output is the evidence record on the claim above, carrying an opaque reference to the root it matched | — | SESSION |
| encrypted input, encrypted output to the client | required (F §9 table) | — | — |
| `egress.mode`, `retention.class` | policy-defined; typically `NONE` / `EPHEMERAL` (parent §10: "ephemeral key and data destruction at the end of the session") | CLAIM today | — |

The consequences, stated as invariants:

- `TEE_REQUIRED_FOR_REFERENCE_WORKER = NO` — the reference worker is a
  PUBLIC provider and advertises `privacy.confidential_execution = false`
  (F14, parent §22).
- `CONFIDENTIAL_PROFILE_SUPPORTED_AS_POLICY_MODEL = YES` — a policy can
  require it today, deterministically.
- `CONFIDENTIAL_EXECUTION_IMPLEMENTED = NO` — no verifier exists, so no
  provider can present evidence at level `ATTESTED`; every provider is
  ineligible for a CONFIDENTIAL task with `ATTESTATION_REQUIRED` (or
  `CONFIDENTIALITY_NOT_CLAIMED` when it does not even claim it). That is the
  correct answer before K, and it is acceptable (J16, §23).

A provider advertising `privacy.confidential_execution = true` with no
evidence, or with evidence at `CERTIFIED` (someone vouched for it) rather
than `ATTESTED` (the environment proved it), does **not** qualify. The
conformance suite already fails a subject that *claims* confidentiality
without attestation (C37); this profile is the policy-side statement of the
same rule.

---

## 15. SOVEREIGN profile

Authority: parent §17.2 — "CONFIDENTIAL or VERIFIED plus provider,
jurisdiction and residency constraints certified off-chain (§19)"; §19; P11.

SOVEREIGN is a **composition**, not a third kind of execution:

```
SOVEREIGN = base (CONFIDENTIAL | VERIFIED, as the policy configures)
          ∪ location requirements, each with min_evidence ≥ CERTIFIED
          ∪ optional provider constraints (ONE_OF / NONE_OF on identity.provider_id,
            min_evidence ≥ CERTIFIED)
          ∪ optional egress (typically AT_MOST RESTRICTED or NONE)
```

The location requirements are chosen from §16 — provider jurisdiction,
worker location, data residency, key residency, result-storage location —
**individually**. The authority's phrase is "certified off-chain", so a
plain claim never satisfies a SOVEREIGN location requirement: `min_evidence`
is at least `CERTIFIED`, and a `CLAIM` evaluates to `INSUFFICIENT_EVIDENCE`.
Mbongo "does not currently prove residency" (parent §19), and SOVEREIGN does
not pretend to: it requires the deployment to name whom it trusts to certify,
and fails closed until someone does.

**SOVEREIGN cannot be reduced to the company's incorporation jurisdiction**
(§16). `jurisdiction.provider` is one of five location dimensions;
a policy that requires "data remains in CA" writes `residency.data ONE_OF
{CA}` (and, if it means the execution too, `jurisdiction.worker_location`),
not `jurisdiction.provider`. A provider incorporated in CA whose workers run
elsewhere satisfies only the first.

**Composability.** SOVEREIGN composes with VERIFIED or CONFIDENTIAL as its
base by authority, and every profile composes with any §5.3 requirement.
Profiles are templates over one requirement set; they are not mutually
exclusive types (§15.1).

### 15.1 Profiles are not transaction types

`PUBLIC`, `VERIFIED`, `CONFIDENTIAL` and `SOVEREIGN` are names for
requirement sets. They are **not** `ComputeTask` variants, not consensus
enums, not RPC payloads and not SDK wire types (J13, J17). The chain has one
`ComputeTask` envelope (lock v0.4 B1), `execution_spec` selects an
application profile the chain never interprets, and no consensus parser
knows these four names. The privacy architecture said so (§17.2: "not
consensus transaction types"); this document keeps it so.

### 15.2 Profile matrix

`required` = the template adds the requirement; `policy-defined` = the
policy may add it, the template does not; `not implied` = the profile says
nothing about it and grants nothing.

| Dimension | PUBLIC | VERIFIED | CONFIDENTIAL | SOVEREIGN |
|---|---|---|---|---|
| executor gate (§10.2) | required | required | required | required |
| compatibility (contract, representation, profile tag) | required | required | required | required |
| provider identity evidence | not implied (CLAIM) | one of three: `≥ CERTIFIED` | not implied | required `≥ CERTIFIED` when provider constraints are configured |
| reputation threshold | not implied | one of three | not implied | policy-defined |
| result verification (`verification.result`) | not implied (`NONE`) | one of three — unsatisfiable today | not implied | policy-defined |
| `privacy.confidential_execution` | not implied — **provider sees plaintext** | not implied — **provider sees plaintext** | required, `≥ ATTESTED`, session-fresh | required iff base is CONFIDENTIAL |
| acceptable attestation root | not implied | not implied | required (K evaluates) | as base |
| encrypted input / output | recommended | recommended | required (F §9) | as base |
| jurisdiction / residency (§16) | not implied | not implied | not implied | required, each configured one `≥ CERTIFIED` |
| egress | not implied | not implied | policy-defined, typically `NONE` | policy-defined, typically `NONE` |
| retention | not implied | not implied | policy-defined, typically `EPHEMERAL` | policy-defined |
| hardware / runtime / model | policy-defined | policy-defined | policy-defined | policy-defined |
| price / latency | policy-defined | policy-defined | policy-defined | policy-defined |
| **satisfiable today** | **yes** (reference worker) | identity or reputation strengthening: where a certifier or records exist; result verification: no | **no** — K | **no** — K and/or a certifier |

---

## 16. Jurisdiction and residency

Authority: parent §19 — "An IP address is not proof of physical location. A
provider's assertion is an assertion"; P11.

Five location dimensions, which **must not be collapsed**:

| Dimension | Means | Scope | Who can certify it, in principle |
|---|---|---|---|
| `jurisdiction.provider` | the legal jurisdiction the provider entity is subject to | PROVIDER | an organisation credential |
| `jurisdiction.worker_location` | where the worker instances of a class physically run | WORKER_CLASS or SESSION | a data-centre certification; in future, attestation binding hardware to a certified location (parent §19) |
| `residency.data` | where the private data plane stores objects for this class | WORKER_CLASS | the data-plane operator's certification |
| `residency.key` | where content keys are held and released (F §14) | WORKER_CLASS | the key service's certification |
| `residency.result_storage` | where results are stored before retrieval (F §10) | WORKER_CLASS | as `residency.data` |

Not modelled as dimensions: the control plane's own location (a deployment
fact about the evaluator, not about the provider) and the client's location
(not a provider property).

**Semantics supported now.** Every location value is a token from a
vocabulary the deployment agrees (§18; the examples use ISO 3166-1 alpha-2
country codes such as `CA`, `US`, upper-case); constraints are ONE_OF /
NONE_OF; evidence is `CLAIM` unless a certifier the policy trusts supplies
`CERTIFIED`; an absent value is UNKNOWN and fails a HARD requirement (§11).
"Data remains in CA" is `residency.data ONE_OF {CA}`, and is **not**
equivalent to `jurisdiction.provider ONE_OF {CA}`.

**What is not proven.** No mechanism in this repository proves physical
location. A CERTIFIED location is as good as the certifier; an ATTESTED one
would be as good as the attestation's binding to certified hardware — a
future capability the parent names and this document does not implement.

---

## 17. Retention and egress

### 17.1 Retention

| Claim / requirement | Meaning |
|---|---|
| `retention.class = EPHEMERAL` | nothing outlives the attempt: input copy, plaintext, scratch, result copy at the worker are destroyed at completion (F §14) |
| `retention.class = BOUNDED` + `retention.max_seconds = n` | copies may persist at most `n` seconds after completion, then are deleted |
| `retention.class = PERSISTENT` | the provider may retain |
| requirement `retention.class AT_MOST BOUNDED`, `retention.max_seconds AT_MOST n` | the ordered semantics of §5.3 |

Retention is a **claim about behaviour the outside cannot observe** (E §17.2:
"the destruction obligation is stated (F §14) but not enforceable from
outside"). The reference implementation does not prove deletion, and this
document does not claim automatic deletion for anyone. A policy may require
more than any current provider can evidence; such providers are ineligible
until evidence exists — that is the intended fail-closed outcome, not a
defect.

### 17.2 Egress

| Value | Meaning |
|---|---|
| `egress.mode = NONE` | the execution environment makes no outbound connection other than to the data plane and the chain |
| `egress.mode = RESTRICTED` + `egress.destinations` | outbound only to the listed destinations |
| `egress.mode = UNRESTRICTED` | no restriction claimed |

A requirement is `egress.mode AT_MOST RESTRICTED` (ordered NONE < RESTRICTED
< UNRESTRICTED) and, optionally, `egress.destinations SUBSET_OF S`. Today
this is a claim; runtime or attestation-based enforcement evidence is a
future strengthening (parent §17.1: EXECUTION_ENVIRONMENT + CONTROL_PLANE).

---

## 18. Hardware, runtime and canonical comparison

### 18.1 Vendor-neutral abstraction

The core grammar names **no vendor, product, driver or toolkit** (E28, F18,
P13, J29, J30). Hardware is described by abstract dimensions —
`hardware.accelerator_class`, `hardware.memory_bytes`,
`hardware.throughput_units`, `hardware.runtime_features` — whose token
vocabularies a deployment defines and whose numbers are in base units. A
vendor-specific adapter maps a real device into those values; a vendor
identifier may appear as a *value* under an `x.*` extension namespace that
adapter owns, and a policy in that deployment may constrain it there. It
never appears as a key of this contract, and no policy written only in the
core vocabulary depends on one.

`model.*` is a reserved FUTURE namespace so that an AI-inference worker can
later advertise `model.family`, `model.id`, `model.version`,
`model.tokenizer_id`, `model.max_context` and a policy can require them —
by minor extension (§25), without redesign. This document defines none of
their values; that is the AI-worker gate's job, and AI inference is not
required by anything here (J38, J40).

### 18.2 Canonical comparison rules

| Type | Representation | Comparison |
|---|---|---|
| token | UTF-8 bytes, compared **byte for byte**; no case folding, no Unicode normalisation, no trimming | EQUALS / ONE_OF / NONE_OF |
| ordered token | a token from a set with a fixed order declared in §5.3 (`retention.class`, `egress.mode`) | AT_MOST / AT_LEAST by declared order |
| number | unsigned 64-bit integer in the dimension's base unit — bytes, seconds, milliseconds, the price's declared unit; never a string, never a float, never `80 GiB` | AT_LEAST / AT_MOST / EQUALS |
| set of tokens | a set (duplicates collapse; order is irrelevant) | CONTAINS_ALL / SUBSET_OF / ONE_OF (any member) |
| boolean | true / false | IS_TRUE |
| `Address` | 32 bytes | EQUALS / CONTAINS |
| price | number **and** `commercial.price_unit` token; a requirement names the unit it caps; units differ → UNKNOWN | AT_MOST |

A type mismatch between claim and constraint is UNKNOWN, never a coercion.
`min_vram >= 80 GiB` is written `hardware.memory_bytes AT_LEAST
85899345920`. This is deliberately **not a policy language**: no
expressions, no user code, no WASM, no SQL, no regular expressions (§26).
Data and comparison semantics only.

---

## 19. Evidence strength and freshness

Evidence (§6.2) is time-bounded and bound to something. Evaluation at `now`
applies:

| Condition | Effect |
|---|---|
| `now < issued_at` | evidence not yet valid → treated as absent (level CLAIM) |
| `expires_at` present and `now > expires_at` | expired → level CLAIM; code `EVIDENCE_EXPIRED` if the requirement needed more |
| revoked (§21) | as expired |
| `binding = SESSION(s)` and `s ≠` the evaluated session | not this instance's evidence → UNKNOWN (`EVIDENCE_NOT_SESSION_BOUND`) |
| `binding = CHALLENGE(nonce, s)` | as SESSION, and the challenge must be one the verifier issued for `s` — K's concern; this document only carries the binding |
| `binding = PROVIDER` against `freshness = SESSION` | UNKNOWN (`EVIDENCE_NOT_SESSION_BOUND`) |

The evaluator **does not parse** attestation material, certificates or
proofs (J31). It consumes an evidence record that a verifier produced, and
it applies the window and binding rules above. A future verifier that emits
evidence without `expires_at` or binding produces evidence that satisfies
only `freshness = ANY` requirements; CONFIDENTIAL requires SESSION (§14).

---

## 20. Session binding, scope and shadowing

| Scope | Claims that belong here | Why |
|---|---|---|
| PROVIDER | identity, legal jurisdiction, retention policy, egress policy as a commitment, price, `verification.result` | true of the operator as a whole |
| WORKER_CLASS | hardware installed, runtime family, physical location, data / key / result residency, egress capability | true of a pool, not of one instance |
| SESSION | contract version, representation and profile tags (E §5), available memory, availability, `privacy.confidential_execution`, attestation evidence | true of this instance, now |

Rules:

- a claim may be advertised at any scope, but a requirement with
  `freshness = SESSION` is satisfied only by a SESSION-scoped claim of *this*
  session, with evidence bound to it where evidence is required (J20);
- lookup is most-specific-first: session, then the session's worker class,
  then provider; a more specific claim **shadows** a less specific one
  (an instance that reports 8 GiB free is not credited with the pool's 80
  GiB installed for `available_memory_bytes`);
- a stale session-scoped claim from a previous instance never carries over:
  session claims live with the session (E §5, §15 — sessions re-authenticate
  after a restart) and are gone when it is;
- provider-wide claims may not substitute for session evidence when policy
  requires freshness (J20, J21). This is what stops "the provider attested
  once, last month, on some machine" from authorising a new instance today.

---

## 21. Advertisement lifetime, refresh, withdrawal, revocation

Control-plane semantics; no chain write is involved (RFC 0005 §6).

| Event | Effect on eligibility | Effect on existing leases, capabilities and attempts |
|---|---|---|
| advertisement issued with `expires_at` | evaluable until then | — |
| refresh before expiry | replaces the record; evaluations after the refresh use it | none by itself |
| expiry without refresh | `ADVERTISEMENT_EXPIRED`; no new lease is issued | E §13 governs: a live lease is revoked or allowed to finish **by explicit policy**, and the choice is recorded; unconsumed capabilities may be revoked (F §5.2); consumed ones stay consumed |
| withdrawal by the provider | as expiry | as expiry |
| revocation by the control plane (a claim found false, a certifier withdrew, an operator decision) | `ADVERTISEMENT_REVOKED`; no new lease | as expiry; the running attempt is not "un-run" (E §13: "irreversible work still happens") |
| revocation of one evidence record | that claim reverts to `CLAIM` (§19); requirements needing more fail | as above, for tasks whose eligibility rested on it |

An expired or revoked advertisement changes nothing on the chain and cannot:
the task stays open, the executor keeps its key, and an executor with the key
can still anchor a valid receipt for an open task whatever the control plane
now thinks of its provider (E §13). The control plane records what it can
and does not pretend otherwise.

### 21.1 Policy change during execution

Deterministic behaviour by phase of E §7, aligned with E §13 and §15:

| Policy or claim changes… | Behaviour |
|---|---|
| before a lease is issued | the new effective policy applies to every subsequent evaluation; nothing was granted |
| after the lease, before the capability | E §15: re-validate before any capability is obtained — re-evaluate eligibility; if ineligible, no capability is obtained, the lease is released, no execution starts |
| after the capability, before fetch | the control plane may ask the issuer to revoke the unconsumed capability (E §13, F §5.2) when the new policy says the attempt must not proceed; the choice is explicit and recorded |
| after fetch, during execution | E §13: allow to finish, or signal stop, **by explicit policy**; the worker may hold plaintext (F §14 obligation); the capability stays consumed |
| after result persistence, before the receipt | the executor may still anchor (nothing off-chain can prevent it); the control plane may decline to relay receipt material it now considers stale (E §13); the result exists in the data plane under F's rules |
| after the receipt is observed | nothing: the task is completed on-chain; policy has no retroactive effect (J22) |

A policy change never rewrites `task.executor`, `input_commitment`, a
receipt or any chain fact, and never makes a running attempt's past
eligibility "false"; it changes only what the control plane will do next.

---

## 22. Lease, data capability, and the handoff to K

### 22.1 Three grants, plus a decision

E §3 defines task authority, execution lease and data-plane capability and
forbids confusing them. Eligibility is a **fourth thing**, and it is not a
grant at all:

| | Eligibility decision | Task authority | Execution lease | Data-plane capability |
|---|---|---|---|---|
| issued by | control-plane policy evaluation | the chain | the control plane | the client or its delegated issuer (F §5) |
| answers | may this session be considered for this task under policy | who may answer this task | which instance is coordinating an attempt now | may this executor perform one operation on one object |
| grants | nothing (J6, J7, J8, J9) | the right to anchor | coordination | bytes |

Eligibility is a **prerequisite** for a lease (E §6 rows 5 and 7 are exactly
the two policy rows this document evaluates), and it is **not** a lease: an
eligible worker may still receive none — another instance holds the live
lease (E §10), quota, confirmation depth, or the control plane simply has
no capability to obtain (E §6 row 6). Nor is a lease a capability, nor
does an advertisement grant access to any private input: the capability
model of F is untouched, and no claim, however strong, is presented to the
data plane as authorisation (F §5.2).

### 22.2 Data capability relationship

Eligibility may be a policy **precondition** for issuing a data-plane
capability (the issuer is entitled to consult it), but the capability is
issued by F's issuer under F's binding, and the data plane enforces F's
checks, none of which reads a policy. `policy eligibility ≠ data access`.

### 22.3 The K handoff contract

K — confidential-compute integration — consumes this contract without
redesigning it. The exact interface:

```
J produces         effective_policy               (§7–§8; a CONFIDENTIAL template, §14)
                   capability advertisement       (§5) with privacy.confidential_execution claimed
worker presents    attestation evidence           (plane 4 → plane 2; E §14, parent §11)
K verifies         evidence -> evidence record    { level = ATTESTED, binding = CHALLENGE(nonce, session),
                                                    issued_at, expires_at, verifier, reference(root) }
                   against the acceptable-root / measurement policy that K defines
J evaluates        eligible(task, session, advertisement + evidence record, derived, effective_policy, now)
                   -> Decision; CONFIDENTIAL requirements SATISFIED iff the record meets §14
key release        release_decision(task_id, executor, session_id, decision) -> RELEASE | WITHHOLD(codes)
                   RELEASE iff decision.eligible and every requirement with min_evidence ≥ ATTESTED
                   is in decision.satisfied with evidence_level ≥ ATTESTED
data plane         releases the content key to that environment only on RELEASE (F §9, parent §11 step 4)
```

What K must supply: the evidence formats it accepts (`privacy.attestation_formats`
values), the acceptable-root policy object and its evaluation, the verifier,
the challenge issuance that produces `CHALLENGE` bindings, the key wrapping
to an attested key (F §9 table), and the conformance cases that prove C37's
counterpart (a confidentiality claim *with* acceptable attestation).

What K must **not** need: any change to `ComputeTask`, `task_id`, `Receipt`,
rules (k)–(s), the RPC surface or SDK wire types (E21, F13, lock v0.4). The
handoff is entirely between planes 2, 3 and 4; consensus parses none of it
(E22). `K_CAN_IMPLEMENT_WITHOUT_COMPUTETASK_CHANGE = YES`.

---

## 23. Security limitations, stated

- **A provider can lie in a claim.** Claim-based eligibility (`lowest_evidence
  = CLAIM`) is exactly as strong as the provider's honesty. This document
  does not solve malicious providers; it makes the assurance level of every
  decision explicit so that a policy can demand more (J3).
- **Attested eligibility is as strong as the attestation's limits** (parent
  §10: hardware defects, side channels, configuration, host integration,
  attestation infrastructure). `ATTESTED` is not "absolute".
- **No false confidentiality.** `privacy.confidential_execution = true` at
  level `CLAIM` never satisfies a CONFIDENTIAL requirement; before K, no
  provider is CONFIDENTIAL-eligible (§14).
- **No correctness claim.** Nothing about a provider's capabilities or a
  policy's satisfaction says the output is correct. The receipt remains a
  signed, bound claim (RFC 0005 §9.2, P16, E19); result verification is
  Workstream L / #52 and is represented here only as an unsatisfiable
  dimension (J34).
- **Eligibility is availability-neutral.** A malicious control plane can
  declare everyone ineligible (E §17.1: withhold, delay, deny — accepted).
  It still cannot make anyone else the executor.
- **Policy metadata is a privacy surface.** Which jurisdictions, which
  providers and which evidence a client demands reveals something about the
  client; §24 keeps values out of decisions and logs by default, and the
  executor gate short-circuits before any policy is evaluated for a
  stranger (§10.1).

---

## 24. Logging and privacy

F §13 and E §18 apply unchanged; conformance C30 (F19, E23) is the test.
For policy evaluation in addition:

| Item | In logs by default? |
|---|---|
| `task_id`, executor, `session_id`, worker instance, provider_id, requirement ids, dimension keys, reason codes, evidence levels, `eligible` | yes |
| claim **values** (a jurisdiction, a price, a memory size), policy **values** (thresholds, allowlists) | **no** by default — policy metadata may be sensitive; access-controlled diagnostic logs only |
| evidence material — certificates, attestation quotes, proofs, nonces | **never**; only the evidence record's level, window, binding kind and opaque reference |
| raw private payload, model prompt, private result, capability grant or proof, content key, executor key | **never** (F19) |

The `Decision` of §10.4 is constructed to be loggable as-is: it carries none
of the forbidden items (J26–J28). An implementation's `Debug` rendering of a
decision must redact as the reference worker's types do
([`reference-worker.md`](../development/reference-worker.md) "Logging").

---

## 25. Versioning and extensibility

`provider-policy-v1` names, together, the advertisement schema
(`capability-advertisement-v1`), the requirement schema, the evidence
abstraction, the reason codes and the evaluation semantics of §10, because
they must agree. It is an **architecture / control-plane version,
non-consensus**: not a protocol lock, not an RFC, not a wire freeze
(compare `compute-conformance-v1`, [`compute-conformance.md`](../development/compute-conformance.md)
"Versioning").

| Change | Class | What it needs |
|---|---|---|
| a new OPTIONAL_NOW or FUTURE dimension key, a new `x.*` namespace, a new reason code, a new ordered-token value at the end of an order | **minor extension** | edit this document; version unchanged; unknown keys are UNKNOWN to older evaluators (§11) and never silently satisfy |
| a change to evaluation order, to the unknown / evidence / freshness rules, to a constraint's semantics, removal of a code, a new value type | **new policy version** (`provider-policy-v2`) | a new document version; both may coexist; `execution.contract_version` distinguishes sessions |
| a change to the identity model (§4), the executor gate (§10.2) or the composition rule (§8.2) | **breaking architecture revision** | a revision of this document with an explicit compatibility statement; still non-consensus |
| anything that would put a policy, a profile name, a claim or an evidence commitment **on-chain** | **not this document** | an RFC under [`RFC_PROCESS.md`](../RFC_PROCESS.md); RFC 0005 §2.5 model C is the deferred door |

Extension namespaces are **typed** (their values use §18 types), **named**
(`x.<owner>.<field>`), and **fail safely**: a requirement on a namespace the
evaluator does not understand is UNKNOWN and, for a HARD requirement,
ineligible. There is no untyped blob and no consensus-level registry of
namespaces; a deployment's control plane knows the namespaces its adapters
define.

---

## 26. Non-goals

This document does not define, propose or implement:

- a marketplace, order book, bidding, auction, market clearing, dynamic
  pricing or ranking of eligible providers (J35)
- a pricing engine, metering, price oracle, MBO conversion, payment, escrow,
  staking, slashing, worker rewards or fee split (J36, J37)
- a reputation system, scoring formula or global reputation consensus (§5.4)
- GPU or AI runtimes, schedulers, CUDA/ROCm/any backend, GPU metering (J38)
- attestation generation, parsing or verification; key release; TEE
  integration (J31, J32 — K)
- proof of output correctness or any verification mechanism (J34 — L, #52)
- a policy language, DSL, virtual machine, user-supplied code, WASM or SQL
  predicates (§18.2)
- any consensus field, `ComputeTask` field, `Receipt` field, RPC version or
  SDK wire change (J39)
- legal or regulatory compliance certification (parent §24)

---

## 27. Invariants

Classified against the system as it exists on `dev`, in the manner of E §21.
"ALREADY_TRUE" cites the authority; "DEFINED_BY_THIS_GATE" is also
REQUIRES_IMPLEMENTATION for any production control plane, and is exercised
by the reference evaluator (§30) where marked ★.

| # | Invariant | Status |
|---|---|---|
| J1 | provider policy is off-chain | ALREADY_TRUE (RFC 0005 §2.5 model C deferred; parent §17.1) |
| J2 | policy is non-consensus | ALREADY_TRUE (parent §17.1: "none of them is a consensus field") |
| J3 | provider claims are not proofs | ALREADY_TRUE (E §5.1); DEFINED_BY_THIS_GATE as evidence levels (§6) ★ |
| J4 | `task.executor` remains authoritative | ALREADY_TRUE (RFC 0005 rule s; E1) |
| J5 | policy cannot substitute an executor | ALREADY_TRUE (E1, E17); DEFINED_BY_THIS_GATE (§10.2 gate first) ★ |
| J6 | eligibility is not task authority | DEFINED_BY_THIS_GATE (§22.1) |
| J7 | eligibility is not a lease | DEFINED_BY_THIS_GATE (§22.1) |
| J8 | a lease is not a data capability | ALREADY_TRUE (E6) |
| J9 | a capability advertisement grants no input access | DEFINED_BY_THIS_GATE (§22.2); ALREADY_TRUE that F's data plane reads no policy |
| J10 | a missing required security capability fails closed | DEFINED_BY_THIS_GATE (§11.1) ★ |
| J11 | missing required evidence fails closed | DEFINED_BY_THIS_GATE (§10.3 step 3) ★ |
| J12 | unknown jurisdiction cannot satisfy an explicit jurisdiction requirement | DEFINED_BY_THIS_GATE (§11.1, §16) ★ |
| J13 | profile names are not transaction types | ALREADY_TRUE (parent §17.2; lock v0.4 B1); §15.1 |
| J14 | PUBLIC does not imply confidentiality | ALREADY_TRUE (P8); §12 |
| J15 | VERIFIED does not imply confidentiality | DEFINED_BY_THIS_GATE (§13), on parent §17.2 |
| J16 | CONFIDENTIAL requires policy-defined evidence | DEFINED_BY_THIS_GATE (§14) ★ |
| J17 | SOVEREIGN semantics are policy composition, not consensus | ALREADY_TRUE (parent §17.2, §19); §15 |
| J18 | provider identity ≠ executor identity | ALREADY_TRUE (E §2; parent §8); §4 |
| J19 | worker instance ≠ provider identity | ALREADY_TRUE (E §2); §4 |
| J20 | a provider-wide claim may not substitute for session-specific evidence when policy requires freshness | DEFINED_BY_THIS_GATE (§20) ★ |
| J21 | stale or revoked evidence cannot satisfy a fresh requirement | DEFINED_BY_THIS_GATE (§19, §21) ★ |
| J22 | a policy change cannot rewrite chain authority | ALREADY_TRUE (RFC 0005 §8; E18); §21.1 |
| J23 | policy evaluation is deterministic for identical inputs | DEFINED_BY_THIS_GATE (§10.5) ★ |
| J24 | hard constraints are evaluated before soft ranking | DEFINED_BY_THIS_GATE (§9, §10.1) ★ |
| J25 | soft preferences cannot override hard failures | DEFINED_BY_THIS_GATE (§9.1) ★ |
| J26 | the decision exposes safe reasons | DEFINED_BY_THIS_GATE (§10.4, §11.2) ★ |
| J27 | policy logs contain no raw private payload | ALREADY_TRUE as F19/E23; §24 ★ |
| J28 | policy logs contain no reusable secrets or evidence material | DEFINED_BY_THIS_GATE (§24) ★ |
| J29 | the hardware model is vendor-neutral | DEFINED_BY_THIS_GATE (§18.1), on E28/F18/P13 |
| J30 | no TEE vendor is protocol or policy-grammar authority | ALREADY_TRUE (P13, E28); §18.1 |
| J31 | this gate does not implement attestation verification | ALREADY_TRUE (none exists); §19 |
| J32 | this gate does not release encryption keys | ALREADY_TRUE (F §9 unchanged); §22 |
| J33 | this gate defines the contract K consumes | DEFINED_BY_THIS_GATE (§22.3) |
| J34 | this gate does not prove output correctness | ALREADY_TRUE (RFC 0005 §9.2, P16) |
| J35 | no marketplace ranking is implemented | ALREADY_TRUE; §9.2, §26 |
| J36 | no pricing engine is implemented | ALREADY_TRUE; §26 |
| J37 | no MBO settlement is implemented | ALREADY_TRUE (lock v0.4 deferred surfaces); §26 |
| J38 | no GPU or AI runtime is added | ALREADY_TRUE; §18.1, §26 |
| J39 | no new RPC, wire or consensus field | ALREADY_TRUE — this document uses none |
| J40 | future CPU, GPU and AI workers can advertise through this model | DEFINED_BY_THIS_GATE (§5.3 namespaces, §25 extension) |
| J41 | policy composition can restrict and never silently weakens a stronger source's requirement | DEFINED_BY_THIS_GATE (§8.2) ★ |
| J42 | an unsatisfiable effective policy makes every worker ineligible, and is never resolved by precedence | DEFINED_BY_THIS_GATE (§8.3) ★ |
| J43 | reputation is a control-plane-derived input, never a provider self-claim | DEFINED_BY_THIS_GATE (§5.4), on parent §8 |
| J44 | the executor gate short-circuits: a wrong-executor session learns nothing of the policy | DEFINED_BY_THIS_GATE (§10.1) ★ |

Conflicts with existing authority: **none.** Every dimension, profile and
identity here is taken from the parent, E and F; the two decisions this
document makes on its own — composition (§8.2) and the reason taxonomy
(§11.2) — fill gaps those documents left open and contradict nothing.

---

## 28. Decision matrix (non-normative examples)

Generic hardware names throughout; `A`, `B` are executors; `P`, `Q` providers.

| # | Task requires | Session / provider presents | Evaluation | Decision |
|---|---|---|---|---|
| 1 | PUBLIC; executor `A`; profile `mbongo-ref:reverse-bytes:v1` | `P` proved `A`; profile tags contain the tag; ordinary compute, `confidential_execution = false` | gate passes; compatibility passes; no further HARD requirement | **eligible**, `lowest_evidence = CLAIM` |
| 2 | as 1 | `Q` proved `B`; every capability of `P` and more | gate fails | **ineligible**: `EXECUTOR_MISMATCH` only; nothing else evaluated |
| 3 | VERIFIED via identity: `identity.provider_id ONE_OF {P}`, `min_evidence = CERTIFIED` | `P` proved `A`; identity at level `CLAIM` | value matches; evidence CLAIM < CERTIFIED | **ineligible**: `INSUFFICIENT_EVIDENCE` |
| 4 | CONFIDENTIAL (§14) | `P` claims `confidential_execution = true`, no evidence | value true; evidence CLAIM < ATTESTED | **ineligible**: `ATTESTATION_REQUIRED` |
| 5 | CONFIDENTIAL | `P` claims true; evidence `{ATTESTED, binding CHALLENGE(n, this session), unexpired, root accepted by K's policy}` (future) | SATISFIED at ATTESTED | **eligible**, `lowest_evidence = ATTESTED`; key release may proceed (§22.3) |
| 6 | CONFIDENTIAL | as 5 but the evidence is bound to another session, or expired | UNKNOWN / EVIDENCE_EXPIRED | **ineligible**: `EVIDENCE_NOT_SESSION_BOUND` / `EVIDENCE_EXPIRED` |
| 7 | SOVEREIGN: `residency.data ONE_OF {CA}` ≥ CERTIFIED; `jurisdiction.worker_location ONE_OF {CA}` ≥ CERTIFIED; `egress.mode AT_MOST NONE`; base VERIFIED | `P` incorporated in CA (certified); workers in US (certified); egress NONE | provider jurisdiction irrelevant; worker location fails | **ineligible**: `JURISDICTION_NOT_ALLOWED` on `jurisdiction.worker_location` |
| 8 | `residency.data ONE_OF {CA}` (HARD, default FAIL on unknown) | `P` advertises no `residency.data` | UNKNOWN | **ineligible**: `CAPABILITY_UNKNOWN` |
| 9 | as 8 with `on_unknown = ALLOW` set by the same author | as 8 | UNKNOWN, allowed | **eligible**; decision lists the requirement under `unknown` with `allowed = true` |
| 10 | `commercial.price AT_MOST 100 (unit U)` HARD | `P` prices 120 U | 120 > 100 | **ineligible**: `PRICE_ABOVE_MAX` |
| 11 | `commercial.latency_ms AT_MOST 100` HARD | `P` claims 250 | fails | **ineligible**: `LATENCY_ABOVE_MAX` |
| 12 | `commercial.latency_ms AT_MOST 50` **SOFT**; PUBLIC | `P` claims 250 | soft outcome NOT_SATISFIED | **eligible**; the soft finding is recorded for a future ranker |
| 13 | `retention.max_seconds AT_MOST 3600` | `P` advertises `retention.class = BOUNDED`, `max_seconds = 86400` | fails | **ineligible**: `RETENTION_TOO_LONG` |
| 14 | `retention.max_seconds AT_MOST 3600` | `P` advertises nothing about retention | UNKNOWN | **ineligible**: `CAPABILITY_UNKNOWN` |
| 15 | `egress.mode AT_MOST RESTRICTED` | `P` claims `UNRESTRICTED` | fails | **ineligible**: `EGRESS_NOT_ALLOWED` |
| 16 | `verification.result ONE_OF {REDUNDANT}` | anyone | no mechanism exists | **ineligible**: `VERIFICATION_STRENGTH_UNAVAILABLE` (until a verification RFC) |
| 17 | DATA_OWNER: `jurisdiction.worker_location ONE_OF {CA}`; PROVIDER fragment: `jurisdiction.worker_location ONE_OF {CA, US}` | `P` workers in US | both requirements in the effective set; the data owner's fails | **ineligible**: `JURISDICTION_NOT_ALLOWED` — the provider's wider set did not weaken it |
| 18 | ORGANIZATION: `jurisdiction.provider ONE_OF {CA}`; CONTROL_PLANE: `jurisdiction.provider ONE_OF {US}` | anyone | unsatisfiable | **ineligible** for all: `POLICY_CONFLICT`; reported to the authors |

---

## 29. Provider capability schema, claims-versus-evidence and precedence — summary tables

**Claims versus evidence (what each level can vouch for today).**

| Dimension | CLAIM | CONTROL_PLANE_VERIFIED | CERTIFIED | ATTESTED | PROVEN |
|---|---|---|---|---|---|
| executor possession | — | **today** (E §5) | — | — | — |
| contract version | today | **today** (handshake) | — | — | — |
| representation / profile tags | **today** | — | — | future (measured software) | — |
| provider identity | **today** | — | where a certifier exists | — | — |
| reputation | not a claim | **today** (control-plane records) | — | — | — |
| hardware, runtime | **today** | — | possible | future (K) | — |
| confidential execution | today, **never sufficient** | — | not sufficient (§14) | **required** (K) | future |
| jurisdiction, residency | **today** | — | required for SOVEREIGN | future (hardware-bound location) | — |
| retention, egress | **today** | — | possible (audit) | future (runtime enforcement) | — |
| result verification | only `NONE` | — | — | — | future (#52) |
| price, latency | **today** | — | — | — | — |

**Policy source precedence** — §8.2: union of HARD requirements; every
source tightens; none weakens; conflict is `POLICY_CONFLICT`; SOFT
preferences ordered `DATA_OWNER, ORGANIZATION, CONTROL_PLANE, PROVIDER`
with no aggregation defined.

**Hard versus soft** — §9: jurisdiction, residency, confidentiality,
evidence, retention, egress, provider allow/deny, compatibility: HARD;
price and latency: HARD cap and/or SOFT preference, at the policy's choice;
reputation: HARD threshold (VERIFIED) or SOFT input; availability: SOFT.

---

## 30. Reference evaluator

`crates/mbongo-compute/src/policy.rs` implements §5–§11, §18–§21 and the
four templates of §12–§15 as a **pure, deterministic** function over the
inputs of §10.1, with the decision of §10.4. It is non-consensus, off-chain,
holds no registry, performs no network access, ranks nothing, and is **not
wired into** the reference control plane, whose admission is unchanged. The
named test group `provider_policy` (`cargo test -p mbongo-compute --test
provider_policy`) drives the decision matrix of §28 and the ★ invariants of
§27, and CI runs it as the step **Mbongo Compute Provider Policy**. The tests
use no clock and no sleep: `now` is an argument.

The evaluator is a proof of the semantics, not a product: field names,
serialisation and transport remain an implementation gate (§5.1).

---

## 31. Unresolved by this gate, deliberately

- **Location vocabularies and certifiers.** Which tokens name jurisdictions
  and who may certify them are deployment decisions; the examples use ISO
  3166-1 alpha-2 codes without making them authority.
- **The pricing unit.** `commercial.price_unit` is a token; no unit, oracle
  or MBO relation is defined (a future economic gate).
- **Soft-preference aggregation and ranking.** Selection is not eligibility
  (§9.2); a routing or marketplace gate defines it.
- **The acceptable-root / measurement policy object and evidence formats.**
  K's to define (§22.3).
- **Reputation derivation.** The control plane's records are an input; how
  they are computed is not defined here.
- **Serialisation and transport** of advertisements, policies and decisions
  — an implementation gate, as for every E §19 operation.

None touches consensus.

---

## 32. Relationship to other authority

- **RFC 0005** remains normative for the envelope, `task_id`, executor
  authorisation and binding; this document evaluates policy *around* its
  named executor and adds nothing to it. RFC 0005 §2.5 model C — an on-chain
  authorisation policy commitment — is deferred there and is **not**
  reopened here. If any sentence here conflicts with RFC 0005, RFC 0005
  wins.
- **RFC 0002, `RECEIPT_SPEC_v0.1`, `PROTOCOL_LOCK_v0.4`, `rpc_v0.3`** are
  untouched; no field, rule, method or wire form is added or read
  differently.
- **The parent architecture** remains authority for the planes, the
  dimensions (§17.1), the profiles (§17.2) and jurisdiction (§19); this
  document gives them evaluation semantics and yields to it.
- **E** remains authority for identities, sessions, admission, leases and
  the extension point; this document is the content of E §6's two policy
  rows and E §14's "policy" and changes no E rule.
- **F** remains authority for capabilities and key release; §22 composes
  with it and changes nothing.
- **`compute-conformance-v1`** is unchanged; the reference worker's
  behaviour is unchanged; the reference evaluator is a separate module.

---

## See also

- [`compute-privacy-data-plane.md`](compute-privacy-data-plane.md) — parent architecture (§8, §10–§11, §17, §19)
- [`compute-control-plane-worker-interface.md`](compute-control-plane-worker-interface.md) — E: identities, sessions, admission, leases, extension point
- [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md) — F: capabilities, key release, secret handling
- [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md) — normative, Released
- [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) — frozen surfaces
- [`compute-conformance.md`](../development/compute-conformance.md) — `compute-conformance-v1` (C37: no confidentiality claim without attestation)
- [`reference-worker.md`](../development/reference-worker.md) — the reference implementation this evaluator sits beside
- [#126](https://github.com/MbongoChain/mbongo-chain/issues/126) — the Compute vertical epic (closed; J was recorded there as future work)
- [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (future; `verification.result`)
