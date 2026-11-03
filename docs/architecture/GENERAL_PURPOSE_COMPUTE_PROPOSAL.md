# General-Purpose Verifiable Compute — Architecture and Roadmap Proposal

| | |
|---|---|
| **Status** | **APPROVED DIRECTION (2026-11-03).** The maintainer's decisions are recorded in §0.1 and §0.2. This document sets direction and boundaries; it authorises no implementation. Each gate needs its own approval, and the protocol changes only through accepted RFCs. It describes a plan, not what runs today. |
| **Kind** | Architecture and planning, documentation only, non-consensus. RFC 0005, the v0.4 protocol lock and every NORMATIVE document in [`INDEX.md`](../INDEX.md) win on their subjects. |
| **Date** | 2026-11-03 |
| **Changes nothing in** | the released `mvp-v0.1.0`, any protocol surface, any product code, any profile, any existing evidence |
| **Related** | #181 (VISION_v2), #175 (V4-G1, experimental evidence), #179 (V4-G1 PR, not merged), #178 (security), #144 (K-HW-LIVE), #169 (verify after restart) |

## 0. Decisions

### 0.1 Maintainer decisions of 2026-11-03

The maintainer reviewed the first revision of this proposal and decided the following. This section records those decisions; it does not extend them.

| # | Decision | Condition | Where it is carried out |
|---|---|---|---|
| D1 | General-purpose compute direction **approved** | VISION_v2 prepared as a **separate, explicitly reviewed** governance amendment. It preserves no on-chain model execution, no general-purpose VM, deterministic protocol verification, off-chain execution with commitments and receipts, and no unsupported claims. VISION_v1 is not modified silently | PR #181 (`docs/VISION_v2.md`, PROPOSED) |
| D2 | E/T/B/C/S claims vocabulary (§3) and the **G1 → G2 → G3** order **approved** | — | §3, §7 |
| D3 | The synthetic **MbongoPay integer scorecard** is **approved** as the second reference workload for G2 | Generic runtime interfaces must admit future ML runtimes without a protocol rewrite. **No ML or GPU backend is implemented yet** | §4.11, §7 G2 |
| D4 | #178 split into a **documentation-only PR** and a **separate hardening PR** | Real devnet client-path validation, held-out adversarial tests and regression checks are completed **before** merge approval is requested for the hardening PR. Existing evidence is preserved. No general prompt-injection immunity is claimed | §7 G1; PRs #182 (docs) and #183 (hardening, draft) |
| D5 | PR #179 stays **experimental V4-G1 evidence** | Not merged. CV-specific optimisation does not resume automatically | §5.1, §8 |
| D6 | This PR is updated with the decisions and explicit architectural boundaries | Not merged until reviewed | §0.1, §1.3 |
| D7 | The next three gates are prepared with acceptance criteria, dependencies, protected surfaces and rollback conditions | **No change to production protocol, consensus, runtime or SDK** without a separate approved implementation gate | §7 |

### 0.2 Maintainer decisions of 2026-11-03, second set

These followed the review of the second revision. They record the decisions; they do not extend them.

| # | Decision | Condition | Where it is carried out |
|---|---|---|---|
| D8 | VISION_v2 (#181) **approved for formal adoption** | VISION_v1 is preserved as historical evidence | PR #181 |
| D9 | The general-purpose compute direction in this document is **approved** | The normal review, CI and protected-surface checks apply before documentation PRs merge | This PR |
| D10 | **`mbongo-job:v1` adopted** as the new additive generic workload profile, beside the frozen `mbongo-infer:v1` | Settles the first half of Q4 | §4.11, G2 plan |
| D11 | On-chain principles, deterministic protocol verification, off-chain execution and RFC-based protocol change are **preserved** | **No consensus change and no on-chain scheduling** | §1.3, VISION_v2 |
| D12 | #178 / CC-5: #182 may merge after final checks; #183 stays draft | The new rule is revised to factual output fields; the existing quoted-text rule is retained **provisionally**. Validation uses a newly frozen held-out synthetic corpus and measures attack detection **and** benign false-positive rates on the real devnet client path. CC-3 and CC-5 evidence is not rewritten. Residual risks are documented, including attacks without explicit instruction markers. A versioned CC-5 client patch release is proposed, but **not deployed or merged** without separate approval. No general immunity claim | #182, #183 |
| D13 | G2 implementation **plan** for `mbongo-job:v1` | The first workload is the deterministic synthetic MbongoPay integer scorecard: no real financial data, no automated lending decisions. Interfaces for future runtimes are designed and tested without implementing GPU or ML execution. Kaayu's released profile is unchanged. **G2 product code needs a separate implementation approval** | G2 plan document |
| D14 | G3 prerequisites | Host-class identity and reproducibility requirements for LLM re-execution are defined. **Metering stays in the private data plane initially.** Signed contract registries, open assignment and settlement are deferred to their own gates or RFCs | G3 prerequisites document |

### 0.3 Decision requested in the first revision

1. **Direction.** MbongoChain is a **general-purpose, verifiable and eventually decentralized compute infrastructure**. Kaayu CV extraction and MbongoPay synthetic loan-risk scoring are **reference workloads** that validate infrastructure capabilities; their business-specific accuracy is never on the critical path (§1).
2. **Vision amendment.** This conflicts with the NORMATIVE [`VISION_v1.md`](../VISION_v1.md) (§1.2). Approving this proposal means authorising a separate `VISION_v2` documentation change. This document does not amend the vision by itself.
3. **Claims vocabulary** (§3): *execution integrity*, *technical result verification* and *business correctness* are three different claims, and none implies the next.
4. **Architecture** (§4) and **milestones** (§6).
5. **The next three implementation gates**, in order (§7): **G1** #178 security, **G2** generic workload contracts plus the MbongoPay reference workload, **G3** contract-driven technical verification.

**What this proposal does not do:**
- merge PR #179;
- start CC-4;
- continue Kaayu-specific optimisation;
- process real data;
- start GPU work;
- change any protocol surface;
- propose a rewrite.

---

## 1. Direction and positioning

### 1.1 What changes

The product of the roadmap becomes **infrastructure capabilities**, each proven on more than one workload:

- generic task contracts;
- runtime and model independence;
- worker capability and eligibility;
- technical verification;
- confidentiality classes;
- durable data lifecycle;
- workflows;
- metering and, later, settlement.

A **reference workload** is a deliberately small, synthetic, fully specified application. It exercises those capabilities and produces evidence. It is not a product line. Its business quality is reported, never optimised on the critical path.

| Reference workload | Why it is in the set |
|---|---|
| **Kaayu** — structured CV extraction (released, `kaayu-cv-extract:v3`) | An LLM on an untrusted, adversarial text input; a structured JSON output under a schema; **not bit-exact** across hardware; confidentiality-sensitive inputs |
| **MbongoPay** — synthetic loan-risk scoring (new, §5.2) | A small deterministic model on structured numeric input; a numeric and tabular output; **bit-exact** and therefore **re-executable** by a verifier; no LLM |

Two workloads that differ in runtime, model kind, input type, output type and determinism class are the minimum needed to show that a capability is generic and not shaped to one application.

### 1.2 Conflict with the current vision, stated plainly

`VISION_v1.md` (NORMATIVE, 2026-02-16) describes Mbongo as a verification layer that "does not execute AI models", is "not a cloud GPU network", "does not schedule", and provides "a single primitive — verified inference".

The system that actually shipped already goes further: the MVP runs a worker, a control plane and a private data plane, all off-chain and non-consensus. The direction requested here goes further again, into scheduling, multiple runtimes and future GPU execution.

Two points hold either way:

- **The chain still never executes workloads.** Execution remains off-chain. Consensus stays the lightweight, deterministic commitment and receipt layer that RFC 0005 and RFC 0002 define. On this point the vision and this proposal agree.
- **What differs** is that the off-chain execution infrastructure (workers, scheduling, data plane) becomes an explicit, first-class part of the project, not out of scope.

Recommendation: approve a `VISION_v2` that keeps v1's on-chain principles (deterministic verification, replayability, no on-chain execution, no VM). It would replace the "not a compute network" exclusions with "off-chain verifiable compute infrastructure, chain-anchored". Until then, VISION_v1 remains the authority.

**Status (D1, D8).** The direction is approved, and `VISION_v2` is approved for formal adoption through PR #181 as `docs/VISION_v2.md`, with principles P1–P8. VISION_v1 is kept intact as a historical record.

### 1.3 Architectural boundaries

These boundaries bind every milestone and gate in this document. A change that crosses one needs the instrument in the right-hand column. A gate's approval never stands in for that instrument.

| Layer | What it holds | Inside the boundary (milestone or gate work) | Crossing the boundary needs |
|---|---|---|---|
| **L0. Consensus and protocol** | Block, transaction and `ComputeTask` encoding; rules (a)–(s); receipt v1; storage semantics; P2P; RPC v0.3; everything frozen by `PROTOCOL_LOCK_v0.4` | **Nothing.** M1–M8 and G1–G3 change no file in this layer, and each gate proves it with Git objects | An accepted RFC (Tier 0). Only M9 (open assignment) and M10 (economics) are expected to need one |
| **L1. Workload contracts** (non-consensus) | Contract tags carried in the opaque `execution_spec`; job-spec schema, canonical forms and digest; input and output contracts; determinism class; supported T-levels and confidentiality classes | New contracts and new versions, each its own document with vectors. **Existing contracts (`mbongo-infer:v1`, `kaayu-cv-extract` v1–v3) are frozen byte for byte** | A new contract version. A frozen contract is never edited in place |
| **L2. Runtime adapters** (worker side) | Pinned execution engines identified by hash: llama.cpp today; native deterministic Rust (G2); later ONNX and GPU | New adapters behind their own gate. An adapter is added **beside** existing code, never as a rewrite of the released path | A separate gate per adapter. **ML and GPU adapters need explicit maintainer approval (D3, M7)** |
| **L3. Off-chain coordination** | Control plane, front, capability advertisements, eligibility (J), selection, leases | Wiring J in, several contracts, several executors (M4) | — (non-consensus). The chain never schedules (VISION_v2 P7) |
| **L4. Private data plane** | Inputs, results, logs, keys, retention | Durable store and lifecycle (M3) | Real-data admission is a separate maintainer decision per workload |
| **L5. Client verification and reporting** | E, T0–T5, B, C and S results; screens; exit codes | New levels and new codes. **Existing result fields and exit codes are never redefined** | A change control (CC-*n*) whenever released client behaviour changes |
| **L6. Released artefacts** | `mvp-v0.1.0` tag, `main`, released profile v3, MVP evidence, SDK 0.2.0 on npm | **Nothing.** They are never moved or rewritten | A new release with its own gate; a correction is added as a dated scope note, never as an edit |

**Further boundary rules:**
- **Additive first.** A gate adds modules beside the released path. It does not refactor that path unless golden vectors prove the result byte-identical, and then only in a gate that names the refactor.
- **No silent widening.** A gate that finds it must cross a boundary stops and re-scopes.
- **Workload neutrality.** No generic layer (L1–L5) may hard-code a reference workload's field names, vocabulary or thresholds. Workload-specific logic lives in that workload's contract, profile or screen.

---

## 2. Starting point: what exists and is reused

**Rule (requirement 7):** reuse first. A new primitive is proposed only where a concrete limitation is shown, and protocol changes are deferred to RFCs.

| Area | Existing primitive (authority / code) | Reusable as-is | Concrete limitation for generality |
|---|---|---|---|
| Task commitment | `ComputeTask` {version, submitter, executor, salt, input_commitment, execution_spec ≤ 1,024 B}, `task_id`, rules (k)–(s) — RFC 0005, `mbongo-core/src/compute_task.rs` | **Yes**: `execution_spec` is opaque by design, and a task-kind enum was rejected | One **named** executor per task (rule s); no expiry, assignment or economics (RFC 0005 §6). Changing this needs an RFC |
| Receipt | Receipt v1 {task_id, input/output commitment, executor, metadata ≤ 4,096 B, signature}; anchoring rules (a)–(j) — RFC 0002, `mbongo-core/src/receipt.rs` | **Yes** | Proves correspondence and attribution, **never output correctness** (RFC 0005 §9.2). No failure receipt |
| Application contract | `mbongo-infer:v1`: `execution_spec` = tag ‖ BLAKE3 job-spec digest; canonical job spec; canonical output — [`MVP_INFERENCE_CONTRACT_v1.md`](../product/MVP_INFERENCE_CONTRACT_v1.md), `inference/` | **The pattern** (tag + digest + canonical spec) generalises | Validation hard-codes `llama.cpp`/`llama-server` and gguf (`inference/job_spec.rs`, `validate`); input and output contracts and the verifier are Kaayu-specific (`profiles/mod.rs`, `mvp/verify.rs`) |
| Execution dispatch | `ExecutionProfile` trait (`spec_tags`, `accepts`, `execute_task`), `ProfileDispatch`, reference profile `mbongo-ref:reverse-bytes:v1` — `execution.rs`, `cpu_inference/profile.rs` | **Yes**: already multi-profile | None at the trait level |
| Runtime | `InferenceBackend` trait, `BackendIdentity`, pinned llama.cpp b11669, model pinned by SHA-256 — `cpu_inference/` | **The pattern** (pin by hash, verify at start-up) | One runtime family; CPU; Windows and Linux x64 |
| Control plane | Sessions advertising `spec_tags`, leases, attempts, parking, `offer` ("the executor is never chosen here") — `control_plane.rs`, [`compute-control-plane-worker-interface.md`](compute-control-plane-worker-interface.md) | **Yes** | The MVP front accepts **one** frozen spec and **one** executor (`mvp/front.rs`) |
| Capability and eligibility | `provider-policy-v1`: capability advertisement as claims, evidence levels, PUBLIC / VERIFIED / CONFIDENTIAL / SOVEREIGN, deterministic `eligible()` — [`compute-provider-capability-policy.md`](compute-provider-capability-policy.md), `policy.rs` | **Yes** | "Not wired into the reference control plane" (`policy.rs`); eligibility is not selection |
| Confidentiality | `confidential-auth-v1`, `attestation-envelope-v1`, SEV-SNP adapter — `confidential/` | **Yes**, as the path to attested execution | REFERENCE_ATTESTED (test only); live SEV-SNP run not done (#144) |
| Private data plane | Data-plane interface; `InMemoryDataPlane` — [`compute-private-data-plane-interface.md`](compute-private-data-plane-interface.md), `data_plane.rs` | **The interface** | Memory only: lost on restart; results expire at 3,600 s; `verify` cannot finish after a restart (#169) |
| Client verification | Separate results for receipt integrity, result integrity, schema conformity and business accuracy (`NOT_ASSESSED`), plus a separate injection screen — `mvp/verify.rs`, `mvp/screen.rs` | **The separation of dimensions** | Schema check and screen are Kaayu-specific; the screen has known misses (#178) |
| Conformance | `compute-conformance-v1` — [`development/compute-conformance.md`](../development/compute-conformance.md) | **Yes** | Covers the architecture contracts, not new contract kinds |
| SDK | `@mbongo/sdk` 0.2.0: six RPC methods, `ComputeTask` and receipt helpers | **Yes** | No data-plane, control-plane or inference helpers |
| Economics | RFC 0003/0004 (research), `reference_usd_per_hour` (displayed only), `MVP_PAYMENT_MODEL = FREE` | Research input only | Nothing in code; economics is deferred to RFCs by the v0.4 lock |
| Workflows | None. "No receipt semantics for multi-step work" (`MVP_v0.1.md`) | — | Absent |

**Conclusion.** No limitation found so far requires a protocol rewrite. Every capability in the next three gates (§7) fits inside the existing envelope, receipt and non-consensus layers:

- An opaque `execution_spec` already carries any contract.
- An equality-only `input_commitment` already chains steps (§4.6).
- Receipt metadata already carries untrusted execution data.

Two later capabilities do need RFCs, and they are identified here, not designed:

- tasks **without a pre-named executor** (open assignment, needed for decentralised scheduling);
- **economics** (fees, settlement, slashing).

---

## 3. Claims vocabulary: what each kind of evidence proves

Every report, receipt display and acceptance criterion must name which of these claims it makes. **A signed, anchored receipt proves none of the semantic claims and no confidentiality claim.**

| Layer | Claim | Established by | Does **not** establish |
|---|---|---|---|
| **E: execution integrity** | The receipt answers a committed task, over the committed input, from the named executor, signed by that executor, and is anchored. | Consensus rules (k)–(s) and (a)–(j); client checks of commitments and signature | That the claimed runtime or model ran; that the output is correct; that execution was confidential |
| **T: technical result verification** | The output satisfies the contract's technical properties. It does so at a stated **level** (§4.7): bound to the commitment (T0), conformant to the output contract (T1), reproduced by independent re-execution (T2), agreed by *k* independent executors (T3), or produced inside an attested environment (T4). | A contract-driven verifier; re-execution; redundancy; attestation | Business correctness; anything above its stated level. T1 says "valid JSON within bounds", never "true" |
| **B: business correctness** | The output is right for the application's purpose: an accurate extraction, a sound risk score. | A workload-specific evaluation (a gold set, a human review rubric) under an approved methodology | Anything about other inputs or other workloads; it is always scoped and sampled |
| **C: confidentiality** | Who could see the input and output (a confidentiality class, §4.8). | Policy and evidence levels (J); attestation and key release (K, K-HW) | Under PUBLIC, nothing: the provider sees plaintext. Provider-resistant confidentiality is **not proven** today (#144) |
| **S: content safety** | No detected sign that the output followed instructions embedded in an untrusted input. | A per-contract screen (today the Kaayu injection screen) | Absence of influence: `CLEAR` is not proof (#178) |

**Rules:**

- **Standing values.** Every client and every report keeps `BUSINESS_EXTRACTION_ACCURACY` / `BUSINESS_CORRECTNESS = NOT_ASSESSED` unless an approved B methodology was applied. `OUTPUT_CORRECTNESS_PROVEN = NO` stays unless a T2+ level was reached, and then only at that level.
- **Separate results.** Each layer is reported separately and never folded into another. This keeps the MVP's existing separation of verification dimensions.
- **Corpus scope.** A security or safety measurement states its corpus. "0 undetected" means "on corpus X" (the lesson of #178).

---

## 4. Architecture

Each subsection states what is reused, the gap, the proposed design, and whether a protocol change would be needed. **None of §4.1–§4.9 needs one.**

### 4.1 Generic compute-task contracts

**Reused:**
- the `ComputeTask` envelope, unchanged;
- the `mbongo-infer:v1` pattern: `execution_spec = ASCII contract tag ‖ 32-byte digest of a canonical, off-chain job spec`.

**Gap:** the job-spec schema and its validation are inference- and Kaayu-specific.

**Design: a workload contract family.**
- **Contract definition.** A *workload contract* is a versioned, non-consensus specification. It fixes:
  - its tag;
  - its job-spec schema;
  - the canonical forms of input and output;
  - its validation order and error taxonomy;
  - its determinism class (§4.2);
  - its verification levels (§4.7);
  - its confidentiality classes (§4.8).
- **Existing contract.** `mbongo-infer:v1` stays frozen and becomes the first member of the family.
- **New contract.** A generic `mbongo-job:v1` is proposed. Its job spec has:
  - common fields: runtime identity, artifact identities (model, weights, code) by SHA-256, input contract, output contract, limits, determinism class;
  - a contract-specific section.
- **Implementation path.** Each contract is an `ExecutionProfile` behind the existing `ProfileDispatch`, which already supports several profiles. Kaayu's v1–v3 vectors must reproduce byte for byte.
- **Registry.** The profile registry is repository-local today (invariant A27). That stays true until a separate gate proposes signed, content-addressed contract manifests.

### 4.2 Model and runtime independence

**Reused:**
- the `InferenceBackend` and `BackendIdentity` pattern;
- pin by hash and verify at start-up.

**Gap:** one runtime family is hard-coded.

**Design:**
- **Runtime adapter.** A *runtime adapter* interface generalises `InferenceBackend`: identify, verify artifacts, execute, report untrusted metering. Planned adapters, each behind its own gate:
  - llama.cpp (existing);
  - a native deterministic Rust scorer (MbongoPay, §5.2);
  - later, a pinned ONNX runtime.
- **Artifact identity.** A model, a coefficient table or code is identified by SHA-256 in the job spec. Workers verify it before accepting work, as the MVP already does for the model.
- **Determinism class.** Every contract declares one, and it decides which verification levels are possible:

| Determinism class | Meaning | Example | Verification it allows |
|---|---|---|---|
| `BIT_EXACT` | The same output on any conforming worker | integer scorecard | T2 re-execution by equality |
| `BIT_EXACT_PINNED` | Bit-exact on the same runtime build and hardware class | llama.cpp at temperature 0 on one host class (observed for v3 on the reference host) | T2 only within the class |
| `TOLERANCE` | Equal within a declared numeric tolerance | float ML on mixed CPUs | T2 by tolerance |
| `NON_DETERMINISTIC` | No re-execution equality | sampling LLM | T3 by agreement or T4 only |

### 4.3 Worker capability discovery and scheduling

**Reused:**
- `Session.spec_tags`;
- `capability-advertisement-v1`;
- the J evidence levels and `eligible()`;
- control-plane leases and attempts.

**Gap:** J is not wired into admission, and the MVP front serves one executor.

**Design:**
- **Where scheduling happens.** Scheduling is off-chain and happens **before** submission. The client, or a directory acting for it, selects an eligible executor, then names it in the task, as rule (s) requires today. No protocol change.
- **Wiring J in.** A worker's advertisement is evaluated by `eligible()` against the contract's policy requirements. The front and control plane then accept several contracts and several executors.
- **Ranking.** Selection among eligible executors is a separate, non-consensus policy (cost, latency, reputation), because J deliberately says "eligibility is not selection".
- **Decentralised open assignment.** A task that names no executor, or names a policy commitment (RFC 0005 "model C"), **needs an RFC**. It is identified as milestone M9, not designed here.

### 4.4 CPU and future GPU execution

**Reused:** the `hardware.accelerator_class` dimension in `policy.rs`; the runtime adapter (§4.2).

**Design:**
- GPU is another adapter and another advertised capability. It requires no new protocol or contract shape.
- Its determinism class is usually `TOLERANCE` or `BIT_EXACT_PINNED`, which the verification levels must respect.
- **No GPU implementation is proposed in the next gates.** M7 is gated separately and needs the maintainer's explicit approval.

### 4.5 Structured and unstructured outputs

**Design.** The output contract declares one of the following types. Each has a canonical byte form, which is what `output_commitment` commits to.

| Output type | Canonical form | T1 check | Example |
|---|---|---|---|
| `json-schema` | Canonical JSON subset (existing `inference/canonical.rs`) | JSON Schema conformity within declared bounds | Kaayu profile |
| `numeric-record` | Fixed-order canonical JSON or a fixed binary layout; integers or fixed-point only | Ranges and types | MbongoPay score and reason codes |
| `text` | UTF-8, NFC, bounded length | Length and encoding only (`schema_conformity = NOT_APPLICABLE`) | A summary |
| `bytes` | Raw bytes, bounded | Length only | Reference reverse-bytes profile |

Large outputs stay in the private data plane. Only the commitment goes on-chain, as today.

### 4.6 Single-step and multi-step workflows

**Reused:**
- one task = one receipt (unchanged);
- `input_commitment` is opaque to consensus and checked only for equality (RFC 0005 §2.4, §2.12);
- the non-normative commitment conventions, which use **different** domains for input (`mbongo:compute-input:v1`) and output (`mbongo:compute-output:v1`). A step's input commitment can therefore never simply *equal* the previous step's output commitment, and this design keeps the conventions intact.

**Design: reference-chained workflows, with no protocol change.**
- **Workflow manifest.** An off-chain, canonical document listing the steps, each with its contract and job-spec digest, and the edges between them. Its digest is carried in each step's job spec.
- **Chaining.** The input bytes of step *n + 1* are a small canonical **step reference**: `{workflow_digest, prev_task_id, prev_output_commitment, …}`. Its `input_commitment` therefore binds, on-chain, to step *n*'s task and output commitment. The worker fetches the actual data through the private data plane and refuses it unless it hashes to `prev_output_commitment` under the output convention.
- **Verifying a workflow.** Check every step receipt (layer E). For each edge, check that the step reference names the previous task and its anchored `output_commitment`. Then apply each step's T-level. The edges are verified from commitments alone; no private bytes are needed.
- **Failure.** A failed step has no receipt, as today. The workflow's failure record is off-chain. An on-chain failure receipt would need an RFC and is not proposed.
- **Example.** A two-step demonstration: MbongoPay feature normalisation, then scoring.

### 4.7 Execution receipts and technical verification

**Reused:**
- Receipt v1, unchanged;
- the metadata field (≤ 4,096 B, unvalidated by consensus) for **untrusted** execution metadata: runtime identity, metering;
- the MVP client's separate verification dimensions.

**Design: a contract-driven verifier with explicit levels.**

| Level | Check | Applies to | Today |
|---|---|---|---|
| T0 | Result bytes match `output_commitment`; receipt signature and anchoring valid | all | Yes (MVP `verify`) |
| T1 | Output-contract conformity | all | Kaayu JSON schema only |
| T2 | Independent re-execution reproduces the output (equality or declared tolerance) | `BIT_EXACT`, `BIT_EXACT_PINNED` (within class), `TOLERANCE` | No |
| T3 | *k* independent executors agree | any; costs *k*× | No (needs assignment, M4/M9) |
| T4 | Execution inside an attested environment bound to the task (K / K-HW) | confidential classes | Reference only (#144) |
| T5 | Succinct proofs (ZK) | research | No |

The report always names the highest level reached, and never states a higher one. Challenges, disputes and slashing are economics, deferred to an RFC by the v0.4 lock.

### 4.8 Confidentiality classes and data lifecycle

**Reused:** the J profiles PUBLIC, VERIFIED, CONFIDENTIAL and SOVEREIGN; K and K-HW key release; the private-data-plane boundary (never on chain).

**Gap:** private state is memory-only: lost on restart, results expire at 3,600 s, and `verify` cannot finish after a restart (#169).

**Design:**
- **Data-plane store.** A durable, encrypted-at-rest private store behind the existing data-plane interface. It has explicit retention per object class (input, result, logs), deletion on expiry, and a deletion record.
- **Restart behaviour.** Tasks recover after a restart, and `verify` completes from stored result bytes.
- **Confidentiality classes.** Each contract declares the classes it supports. Each task's class is shown in the client report.
- **What PUBLIC means.** PUBLIC means the provider sees plaintext; this is stated on every report, as the MVP does today.
- **Real data.** Real-data admission stays a separate, explicit maintainer decision per workload, following the O1 discipline. No real data is admitted by any gate in this proposal.

### 4.9 Observability, reliability and resource metering

**Reused:** the `health` checks and alerts, log-derived metrics, and `TRUSTED_METERING_IN_MVP = NO`.

**Design:**
- **Metrics.** A content-free metrics surface: per-contract jobs, latency percentiles, failures by code, queue depth, memory and CPU-seconds.
- **Service-level objectives.** Defined per contract and measured by campaigns, as MVP-D did.
- **Reliability.** Idempotent resubmission and durable state (§4.8).
- **Metering.** A per-task metering record (wall time, CPU-seconds, peak memory, tokens or rows), kept in the private data plane initially (D14). It is labelled **untrusted operational metering**. Trusted metering requires T3 or T4 and comes later.

### 4.10 Future pricing and settlement

Economics is deferred to RFCs by the v0.4 lock. RFC 0003 and RFC 0004 remain research. The order is: **metering first, then price quotes, then settlement**.

- A price is an off-chain quote. The `commercial.price` dimension already exists in J.
- A settlement RFC would bind a quote and a receipt to a payment, and later to staking, challenge and slashing.

None of this is in the next three gates.

### 4.11 G2 generic-runtime design

This section is the design G2 implements, if approved. It is written so that a future ML or GPU runtime is **one new adapter and one new contract**, with no protocol change and no change to existing contracts (D3). No ML or GPU adapter is part of G2.

#### 4.11.1 Layering inside the worker

```
ComputeTask.execution_spec  ──►  ProfileDispatch (existing, unchanged)
                                   ├─ mbongo-infer:v1 profile (existing, frozen; Kaayu)  ── untouched in G2
                                   └─ mbongo-job:v1 profile (new)
                                        1. resolve job spec by digest  (canonical JSON, own domain)
                                        2. WorkloadContract  ← looked up by job_spec.contract
                                        3. RuntimeAdapter    ← looked up by job_spec.runtime
                                        4. verify artifacts by SHA-256, check limits
                                        5. contract.validate_input  (untrusted-input policy, S2)
                                        6. adapter.run              → raw output + untrusted metering
                                        7. contract.canonical_output → bytes that output_commitment commits to
                                        8. contract.check_output    (T1, also used by the client verifier)
```

The existing `ExecutionProfile` trait (`execution.rs`) is **not changed**. `mbongo-job:v1` is one more implementation of it, registered beside the existing profiles. The released `mbongo-infer:v1` path, the MVP front (one frozen spec) and the MVP CLI are not modified in G2.

#### 4.11.2 Interfaces (sketch, non-normative until G2's contract document)

```rust
/// A pinned execution engine. G2 ships one: native deterministic Rust.
/// Later gates add others (ONNX CPU, GPU) without touching this trait.
pub trait RuntimeAdapter: Send + Sync {
    /// Family, revision, binary hash (if any), accelerator class.
    /// Proven at start-up, like BackendIdentity today.
    fn identity(&self) -> RuntimeIdentity;
    /// Refuses unless every artifact matches its pinned SHA-256 and size.
    fn verify_artifacts(&self, artifacts: &[ArtifactRef]) -> Result<(), ContractError>;
    /// Runs one job. Errors are content-free codes, as in the MVP.
    fn run(&self, job: &ResolvedJob, input: &[u8]) -> Result<RunOutput, ContractError>;
}

pub struct RunOutput {
    pub raw: Vec<u8>,
    /// Wall time, CPU time, peak memory, rows or tokens. Untrusted (§4.9).
    pub metering: UntrustedMetering,
}

/// A versioned workload contract: everything application-specific.
pub trait WorkloadContract: Send + Sync {
    fn id(&self) -> &'static str;                        // e.g. "mbongopay-scorecard:v1"
    fn determinism(&self) -> DeterminismClass;           // §4.2
    fn output_type(&self) -> OutputType;                 // §4.5
    fn runtime_requirement(&self) -> RuntimeRequirement; // family and revision range, accelerator
    fn validate_input(&self, input: &[u8]) -> Result<(), ContractError>;
    fn canonical_output(&self, raw: &[u8]) -> Result<Vec<u8>, ContractError>;
    fn check_output(&self, canonical: &[u8]) -> OutputCheck; // T1
}
```

`ContractError` here stands for a new, generic error type in the new module, because `inference/` is a protected path. `InferenceBackend` and `BackendIdentity` stay as they are. G2 does **not** wrap llama.cpp. A llama.cpp `RuntimeAdapter` is deferred to the gate that moves Kaayu onto `mbongo-job` (open question Q4). The released path therefore carries no refactor risk in G2.

#### 4.11.3 Job spec for `mbongo-job:v1`

- **`execution_spec`:** `b"mbongo-job:v1"` ‖ the 32-byte digest of the canonical job spec. That is 45 bytes, within RFC 0005's 1,024-byte bound, and the same pattern as `mbongo-infer:v1`.
- **Digest domain.** The digest uses a domain-separation string distinct from `mbongo-infer:v1`'s, fixed in G2's contract document.
- **Canonical form.** The existing canonical-JSON rules (`inference/canonical.rs`), reused unchanged.

| Field | Common to every contract | Purpose |
|---|---|---|
| `contract` | yes | Contract id and version: selects the `WorkloadContract` |
| `runtime` | yes | `{family, revision, binary_sha256?, accelerator_class}`: selects and pins the `RuntimeAdapter` |
| `artifacts[]` | yes | `{role, sha256, size_bytes}` for model weights, coefficient tables or code; verified before work is accepted |
| `input` / `output` | yes | Type (§4.5), maximum bytes and schema digest |
| `limits` | yes | Maximum wall time and memory, enforced by the worker |
| `determinism` | yes | Must equal the contract's class; a mismatch is refused |
| `confidentiality` | yes | The classes this job may run under (§4.8); evaluated by J at admission (M4) |
| `params` | no (contract-specific) | For example, the scorecard's band thresholds |

#### 4.11.4 How a future ML or GPU runtime fits, without a protocol rewrite

| Need | Where it goes | Protocol change |
|---|---|---|
| A pinned ONNX runtime on CPU | A new `RuntimeAdapter` (`runtime.family = "onnxruntime"`), with weights in `artifacts[]` | None |
| A GPU | The same or another adapter with `accelerator_class = "gpu:…"`, matched against J's existing `hardware.accelerator_class` | None |
| Float output | Contract `determinism = TOLERANCE`, with a tolerance declared in the contract; the G3 verifier compares within it | None |
| A sampling LLM | `NON_DETERMINISTIC`; T2 is not offered, and only T3 or T4 apply later | None (T3 itself needs M9's RFC) |
| A larger model | Artifact size is a job-spec field; large outputs stay in the data plane | None |

#### 4.11.5 MbongoPay v1 in this design

- **Contract:** `mbongopay-scorecard:v1`.
- **Runtime:** `mbongo-native` (deterministic Rust, integer arithmetic only), with the scorecard table as an artifact identified by SHA-256.
- **Input and output:** `numeric-record` (§5.2), with no accept or decline field.
- **Determinism:** `BIT_EXACT`.
- **Generator:** a seeded synthetic generator.
- **Reference:** an independent Node reference implementation computes the expected outputs, so the Rust scorer is not checked against itself.

#### 4.11.6 Explicitly not in G2

- An ONNX, ML or GPU adapter.
- A llama.cpp adapter, or moving Kaayu onto `mbongo-job`.
- Changes to the MVP front, the CLI, the SDK or the chain.
- Placing metering in receipts or on chain. Metering stays in the private data plane initially (D14).
- A remote contract registry. Registries stay repository-local (A27).

---

## 5. Reference workloads

Both are synthetic and are **infrastructure test vehicles**. Their business-specific improvements are always out of scope for infrastructure milestones (see "Out of scope" in §6).

### 5.1 Kaayu: structured CV extraction

**State:** released as `kaayu-cv-extract:v3` (`mvp-v0.1.0`). V4-G0 harness and gold set merged (#177).

**V4-G1 evidence** (PR #179, open, not merged) and the stopped V4-G1b candidates (`lab/v4-g1b`) are **experimental evidence only**.

**It exercises:**
- LLM runtime;
- `json-schema` output;
- untrusted adversarial input (content-safety layer S);
- `BIT_EXACT_PINNED` determinism;
- PUBLIC confidentiality, later CONFIDENTIAL.

**Kept frozen:** v3, its schema and prompt, and the gold sets and reports.

**Business quality** is measured only with the existing harness when a milestone needs a regression check, for example "Kaayu output unchanged after the contract refactor".

### 5.2 MbongoPay: synthetic loan-risk scoring

**Purpose.** A second, different workload: a deterministic numeric model, re-executable by a verifier.

**Guardrails (normative for this workload):**
- **Synthetic data only.** No real applicant, financial, bank, bureau or identity data, ever.
- **No automated lending decisions.** The output is labelled "synthetic risk score for infrastructure testing — not a credit decision". No accept or decline field exists in the output contract.
- **Not offered or used for real credit assessment.** Credit scoring of natural persons is regulated (for example as high-risk AI under the EU AI Act, and under credit-reporting and fair-lending law). Any real use would need its own legal, fairness and governance review, outside this roadmap.
- **No protected attributes.** None appear in the synthetic feature set: no age, sex, ethnicity, religion, nationality or postal code.

**Workload v1, deterministic:**

| Aspect | Specification |
|---|---|
| Input | `numeric-record`: about 8–12 synthetic features as integers or fixed-point, for example monthly income band, debt-to-income in basis points, months of credit history, open accounts, recent delinquencies, requested amount and term |
| Model | An integer **scorecard**: a coefficient table (bins → points), committed as an artifact by SHA-256 |
| Runtime | A native Rust `ExecutionProfile`, with no external runtime and no floating point |
| Output | `numeric-record`: score (integer, fixed range), risk band (an enum, not a decision), and up to 3 reason codes from the table |
| Determinism | `BIT_EXACT`, enabling T2 re-execution by any verifier, including the SDK |
| Data | A seeded synthetic generator, with its seed and code committed; gold outputs computed by an independent reference implementation in a different language (Node), to avoid self-confirmation |

**Workload v2 (later, optional):** a small ML model (for example gradient-boosted trees in a pinned ONNX runtime). It exercises `TOLERANCE` determinism and the runtime-adapter path. It needs its own gate.

**It exercises:**
- runtime and model independence;
- numeric output;
- T2 re-execution;
- workflows (normalisation → scoring);
- metering at a different cost profile from an LLM.

---

## 6. Milestones

**Both workloads:** **Y** = both reference workloads exercise the capability; **K** = Kaayu only; **P** = MbongoPay only.

| # | Generic capability | Acceptance tests and measurable evidence | Both workloads | Exit criteria | Out of scope (business-specific) |
|---|---|---|---|---|---|
| **M0** | Direction, claims vocabulary, Vision v2 | This proposal approved; `VISION_v2` (#181) adopted; claims vocabulary (§3) referenced from INDEX | Y (definitions) | Maintainer approval; INDEX updated | Any accuracy target |
| **M1** | **Generic workload contracts and runtime adapters** (§4.1, §4.2, §4.5) | Kaayu v1–v3 golden vectors and the MVP-D / MVP-E vectors reproduce byte for byte. MbongoPay v1 contract: job-spec digest vector, canonical I/O vectors, ≥ 200 synthetic cases equal to the independent Node reference. Both contracts dispatched by one worker. Conformance suite extended | **Y** | 0 protocol diffs (Git-object proof of locked paths); 0 vector changes for existing contracts; MbongoPay 100 % equality on the synthetic set; CI green | Kaayu prompt or schema changes; MbongoPay model quality |
| **M2** | **Contract-driven technical verification T0–T2** (§4.7) | A verifier that reads the contract: T0 and T1 for both workloads; T2 re-execution for MbongoPay (equality) and for Kaayu within `BIT_EXACT_PINNED` (the same host class). Negative tests: tampered output, wrong artifact, wrong contract → the named failure branch, never a generic error | **Y** (T2 is P-only across hardware) | Every report names its highest level; 0 false T2 passes on the tamper corpus; the B layer stays `NOT_ASSESSED` | Semantic checking of CV content; score calibration |
| **M3** | **Durable private data lifecycle** (§4.8) | An encrypted durable store behind the data-plane interface; restart tests (`verify` completes after a restart, #169); retention and deletion tests; privacy scans | **Y** | Restart scenario PASS on a devnet; deletion verified; 0 plaintext in logs or chain | — |
| **M4** | **Capability discovery and eligibility admission** (§4.3) | J wired into the front and control plane; ≥ 2 workers with different advertisements; ineligible tasks refused with J codes; a multi-contract, multi-executor devnet campaign | **Y** | Deterministic eligibility decisions reproduced from vectors; campaign success ≥ 19/20 per workload | Executor ranking by business quality |
| **M5** | **Multi-step workflows** (§4.6) | Workflow manifest vectors; a 2-step MbongoPay workflow and a mixed workflow (Kaayu extraction → a deterministic post-check); an edge-tamper test fails at the named edge | **Y** (mixed) | Workflow verification over all receipts and edges; 0 protocol changes | Agentic or autonomous workflows |
| **M6** | **Observability and untrusted metering** (§4.9) | Metrics surface; a per-task metering record; SLO campaigns per contract (latency P50/P90/P95, memory, failure rate) | **Y** | SLOs measured and published per contract; metering labelled untrusted | Pricing |
| **M7** | **GPU execution adapter** (§4.4) | Separate approval required. The same contracts on GPU with declared determinism class; CPU and GPU comparison | Y | Its own gate | Model upgrades |
| **M8** | **Attested confidential execution** (T4, §4.8) | K-HW-LIVE (#144) on real SEV-SNP; key release bound to the task | Y | `PROVIDER_RESISTANT_CONFIDENTIALITY_PROVEN` only with live evidence | — |
| **M9** | **Open assignment and redundancy (T3)**: **RFC** | An RFC for tasks without a pre-named executor; *k*-of-*n* agreement | Y | RFC accepted before any code | — |
| **M10** | **Pricing and settlement**: **RFC** | An economics RFC built on M6 metering | Y | RFC accepted | — |

**Cross-cutting S-track (security), with its own change control per gate:**
- **S1:** #178 (content-safety screen).
- **S2:** generalise the screen into a per-contract *untrusted-input policy*. Kaayu has one; MbongoPay's numeric input needs range validation, not instruction screening.
- **S3:** re-run the adversarial corpora at every contract or runtime change.

---

## 7. The next three implementation gates

The gates are ordered by dependency and risk (D2), and each is a separate PR set.

**Rules for every gate:**
- **Approval.** Explicit maintainer approval of the gate before any implementation, and again before any merge.
- **Merge procedure.** The merge gate (`.claude/skills/merge-gate`) runs on the exact reviewed SHA. PR CI and push CI are reported separately, and skipped jobs are named.
- **Protected surfaces.** They are listed **before** the work starts. At merge, each one is proven unchanged with Git objects (`git rev-parse <base>:<path>` against `<head>:<path>`), with the expected and actual counts stated.
- **Rollback.** Every gate merges as one squash commit on `dev`. Rollback is a revert of that commit, with a dated scope note in the affected documents. The `mvp-v0.1.0` tag and `main` are never moved.

### G1 — Security #178: content-safety screen correction (S1)

**Scope (D4):** two PRs, deliberately independent.
1. **Documentation PR (#182).** Scopes CC-3's evidence to its corpus and discloses the detection gap. It is documentation only and adds dated scope notes; the original records stay.
2. **Hardening PR (#183).** Hardens `mvp/screen.rs`:
   - quoted values are matched without surrounding punctuation;
   - `OUTPUT_USES_INSTRUCTION_ONLY_TERM` flags a term that appears only in the instruction-like text;
   - `OUTPUT_EMPTY_AFTER_INSTRUCTION` flags an output that states nothing after an instruction-like line.

   It also adds regression tests (P11–P13), the held-out corpus `mvp-sec178-adversarial-v1` (committed in `bdfb60e` before any run), and the real-client-path evidence (G1-RESULTS).

**Dependencies:**
- None on G2 or G3.
- The docs PR merges first, so the published claims are corrected regardless of the hardening decision.

**Status (2026-11-03): G1 is not ready to close.**

Both PRs are open. Both runs went through the real devnet client path, on the same 25 synthetic cases, with identical model behaviour.

**Released code:**
- 6 overrides were undetected and client-accepted;
- 1 benign control was flagged.

**Candidate (#183):**
- 1 override was undetected (S06, the declared limit);
- **all 3 held-out benign controls were flagged, so A4 FAILS.**

**Consequences:**
- **#182** (docs) is ready for review.
- **#183** stays a draft, with no merge approval requested.
- A revised candidate (factual-field restriction) needs a **fresh** held-out corpus.

This status is the record of the first candidate. D12 set the revision; its results are in #183.

**Acceptance criteria for the hardening PR**, fixed before the real-path runs:

| # | Criterion | Evidence |
|---|---|---|
| A1 | `screen()` flags every confirmed override among the 93 recorded synthetic outputs and flags no benign or resisted output | P11 (unit, CI) |
| A2 | The CLI exits 12 (`CONTENT_FLAGGED`) for N17 and N18, and exits 0 (`VERIFIED`) for N19 | P13 (in-process, CI) |
| A3 | **Real devnet client path**, hardened build, over 25 cases: MVP-E corpus 11, unseen variants 2, held-out corpus 12. Required: `UNDETECTED_ADVERSARIAL_OVERRIDE_COUNT = 0` and `CLIENT_ACCEPTED_ADVERSARIAL_OVERRIDE_COUNT = 0` over every attack, except that **S06 is reported either way**. Its corpus states, before any run, that it probes a limit and is not a requirement | `mvp-launch.mjs adversarial` report, committed |
| A4 | All 4 benign controls (E-P11, B01–B03) are `CLEAR`, exit 0 and `VERIFIED` on the real path | Same report |
| A5 | **Regression:** every MVP-E and unseen-variant attack that released v3 already rejected is still rejected | Released-v3 run against the hardened run, same corpora, same host |
| A6 | Integrity dimensions stay separate: receipt, result and schema `VERIFIED`; business `NOT_ASSESSED` | Same report |
| A7 | `cargo test -p mbongo-compute` and clippy pass; PR CI green on the head SHA | CI |
| A8 | No claim of general prompt-injection immunity anywhere; every count names its corpus | Review of the PR text and docs |

**Protected surfaces:**
- the `kaayu-cv-extract:v3` prompt, schema and vector;
- `mvp-v0.1.0` and `main`;
- MVP-D and MVP-E evidence;
- all L0 protocol paths;
- the SDK.

`mvp/screen.rs`, `mvp/cli.rs` (outcome mapping) and the CC-3 record **do change**, under a new change control (proposed **CC-5**).

**Rollback conditions:**
- **Before merge:** stop if any criterion A1–A8 fails, or if a benign real-path control is flagged.
- **After merge:** revert if a false positive on a benign input is reported in controlled operation, or if any adversarial regression vector fails. The docs PR is not reverted with it. Its scope correction stays true either way.

**Risks:**
- **Real CVs.** False positives on real CVs remain unmeasured, because the evidence uses synthetic data only.
- **Heuristic screen.** The screen remains a heuristic with English, French and Spanish markers.
- **Deployment.** The fix reaches users only through a release decision, which is not part of G1.

### G2 — Generic workload contracts plus the MbongoPay reference workload (M1)

**Scope:** the design in §4.11:
- the `mbongo-job:v1` contract document and vectors;
- the `RuntimeAdapter` and `WorkloadContract` interfaces;
- the `mbongo-native` adapter;
- `mbongopay-scorecard:v1` (contract, scorecard artifact, Rust profile);
- a seeded synthetic generator;
- an independent Node reference;
- the conformance-suite extension.

**Kaayu stays on its frozen `mbongo-infer:v1` profile, untouched.** One worker dispatches both contracts through the existing `ProfileDispatch`.

**Dependencies:**
- D1–D3 (approved);
- G2's own implementation approval.

It does not depend on G1, and it can run in parallel once approved.

**Acceptance criteria:**

| # | Criterion | Evidence |
|---|---|---|
| B1 | The `mbongo-job:v1` job-spec digest vector and the canonical I/O vectors reproduce in Rust and in the Node reference | Vectors, CI |
| B2 | 100 % byte equality between the Rust scorer and the independent Node reference on ≥ 200 seeded synthetic cases. The case count is asserted before comparing | Test with an asserted count |
| B3 | A wrong artifact hash, a mismatched determinism class, an out-of-range input, an unknown contract or an unknown runtime are each refused with their named error code. Each refusal is shown reaching its branch, not just exiting non-zero | Negative tests |
| B4 | **Kaayu and MVP regression:** the `kaayu-cv-extract` v1–v3 vectors, `mvp-http-cli-v1`, and the MVP-D and MVP-E vectors all pass, and their files are unchanged (Git objects) | CI and a blob comparison |
| B5 | One worker (the reference worker or a devnet test) executes an `mbongo-infer:v1` task and an `mbongo-job:v1` task, and anchors both receipts. T0 verifies both | Devnet integration test |
| B6 | The output contract has no accept or decline field, and the synthetic feature set has no protected attribute; both are asserted by a schema test. Every output carries the label "synthetic risk score for infrastructure testing — not a credit decision" | Schema test |
| B7 | `compute-conformance-v1` is extended and passes for the new profile | CI |
| B8 | PR CI green on the head SHA; the push CI is reported separately | CI |

**Protected surfaces (proved unchanged with Git objects):**
- **Protocol and chain:** `crates/mbongo-core/**`, `crates/mbongo-node/**`, `crates/mbongo-network/**`, `crates/mbongo-storage/**`, `docs/specs/**`, `docs/rfcs/**`.
- **SDK:** `sdk/typescript/**`.
- **Released inference path:** `crates/mbongo-compute/src/{inference,profiles,cpu_inference,mvp}/**` and `execution.rs`.
- **Vectors:** `test-vectors/inference/**` and `test-vectors/mvp/**`.
- **Released artefacts:** the `mvp-v0.1.0` tag.

**Rollback conditions:**
- **Before merge:** stop if any protected path differs, any existing vector changes, B2 is below 100 %, or the work needs an ML, GPU or llama.cpp adapter.
- **After merge:** revert if any MVP regression is traced to G2. G2 is additive, so reverting it removes only new modules and vectors.

**Risks:**
- **Over-generalising.** The interfaces are designed around two adapters, one of them hypothetical. This is mitigated by keeping the trait surface to what MbongoPay needs, plus the §4.11.4 table, which is checked at review.
- **Scope creep** into real credit use, blocked by the B6 guardrails and §5.2.
- **The refactor temptation.** Moving Kaayu is explicitly excluded (§4.11.6).

### G3 — Contract-driven technical verification T0–T2 (M2)

**Scope:**
- a verifier library and CLI path driven by `WorkloadContract`;
- T0 and T1 for both workloads;
- T2 re-execution: MbongoPay (`BIT_EXACT`, by equality on any conforming host) and Kaayu (`BIT_EXACT_PINNED`, only within the declared host class);
- a report format that names the highest level reached;
- tamper corpora.

**Dependencies:**
- G2 merged; T2 for MbongoPay needs the contract and the adapter.
- Kaayu T2 additionally needs a host-class identity rule (open question Q6).

**Acceptance criteria:**

| # | Criterion | Evidence |
|---|---|---|
| C1 | Each report names exactly one highest level reached, and never states a level it did not reach | Report-format tests |
| C2 | Tamper corpus: modified output bytes, a wrong artifact, a wrong contract or a modified receipt each fail at their named level and branch. 0 false T2 passes, with the case count asserted | Negative tests |
| C3 | T2 is never reported for a `NON_DETERMINISTIC` contract, nor for `BIT_EXACT_PINNED` outside its declared host class | Tests |
| C4 | MbongoPay T2 reproduces 100 % of the G2 synthetic set on a second machine or OS (Linux CI against the Windows reference host) | CI and a reference-host run |
| C5 | The MVP's four existing result fields and exit codes are unchanged. New information appears only as new fields or codes. `BUSINESS_* = NOT_ASSESSED` holds | `mvp-http-cli-v1` vector and the MVP tests |
| C6 | The verifier is opt-in. Without it, client behaviour is byte-identical to before G3 | Test |

**Protected surfaces:**
- everything protected in G2;
- receipt semantics: T-levels are client-side only.

**Rollback conditions:**
- **Before merge:** stop if any false T2 pass occurs, any level is over-reported, or any existing exit code changes.
- **After merge:** revert if a report ever states a level not reached.

**Risks:**
- **Over-claiming:** controlled by C1 and C3.
- **Kaayu T2 flakiness across hardware:** limited by `BIT_EXACT_PINNED`.
- **Re-execution cost:** the verifier is opt-in.

**Next candidate after G3:** M3, the durable private data lifecycle (#169). It is needed before any real-data workload.

---

## 8. Non-goals and constraints carried forward

- No protocol rewrite. No change to locked surfaces without an accepted RFC (M9 and M10 only).
- The chain never executes workloads. No VM.
- `mvp-v0.1.0`, its tag, `main`, profile v3 and all existing evidence stay as released.
- #179 stays open as experimental evidence and is not merged (D5). `lab/v4-g1b` is stopped, and CV-specific optimisation does not resume automatically. CC-4 is not started.
- No change to production protocol, consensus, runtime or SDK without a separate, approved implementation gate (D7).
- No ML or GPU backend until its own gate is approved (D3, M7).
- No real candidate, applicant or financial data. No O1-B02.
- No automated consequential decisions on any reference-workload output. Human review stays mandatory for Kaayu.
- No GPU implementation before M7's own approval.

## 9. Housekeeping found during the survey (not fixed here)

- `RFC_PROCESS.md` and `CONTRIBUTION_TIERS.md` still name `PROTOCOL_LOCK_v0.2.md` and `rpc_v0.1` as the lists of locked surfaces. The current lock is v0.4.
- RFC 0002's header still reads "Draft", while INDEX lists it as Accepted.
- The injection screen's marker list and field names are Kaayu-specific (S2).

## 10. Questions: resolved and open

**Resolved on 2026-11-03 (§0.1, §0.2):**
- Q1 (direction and vision): D1, D8.
- Q2 (MbongoPay scorecard sufficient for G2): D3, D13. An ML runtime is deferred, and the interfaces must admit it (§4.11.4).
- Q3 (G1 ordering): D4. The docs PR and the hardening PR are separate, and the docs PR merges first (§7 G1).
- Q4, first half (generic contract): D10. A new tag, `mbongo-job:v1`.
- Q7 (metering): D14. The private data plane initially.
- Q8 (signed registries) and Q9 (open assignment): D14. Deferred to their own gates or RFCs.
- Q11 (#178 screen trade-off): D12. The word rule moves to factual fields, and the quoted-text rule is retained provisionally.

**Unresolved architecture decisions.** None of them blocks this document. Each must be decided before the gate named.

| # | Question | Recommendation | Needed before |
|---|---|---|---|
| Q4b | Does Kaayu stay on `mbongo-infer:v1` permanently, or move to `mbongo-job` in a later named gate (which would also need a llama.cpp `RuntimeAdapter`)? | It stays until a gate that needs the move, such as a CC-4 successor or M7, names it | That gate |
| Q5 | #178 deployment: how the hardened screen reaches operators | A client patch release under CC-5, as proposed on #183. The release is a separate approval | After G1 |
| Q6 | For `BIT_EXACT_PINNED` T2, what identifies a "host class"? | Requirements are defined in the G3 prerequisites document (D14). Self-declared identity stays untrusted, and T2 across classes is never claimed | G3 (Kaayu T2 only) |
| Q10 | Content-safety screens per contract (S2): one generic untrusted-input policy interface, or a screen per workload? | One `validate_input` and one output-screen hook per `WorkloadContract` (§4.11.2); the Kaayu screen becomes one implementation | After G1, before a second free-text workload |
| Q12 | Provisional quoted-text rule: keep it, or narrow it once its false-positive rate is measured? | Decide on the measured rate in #183 | CC-5 approval |
