# General-Purpose Verifiable Compute — Architecture and Roadmap Proposal

| | |
|---|---|
| **Status** | **PROPOSED**: for maintainer approval. Until it is approved, this document is ASPIRATIONAL and decides nothing. |
| **Kind** | Architecture and planning, documentation only, non-consensus. RFC 0005, the v0.4 protocol lock and every NORMATIVE document in [`INDEX.md`](../INDEX.md) win on their subjects. |
| **Date** | 2026-11-03 |
| **Changes nothing in** | the released `mvp-v0.1.0`, any protocol surface, any product code, any profile, any existing evidence |
| **Related** | #175 (V4-G1, experimental evidence), #178 (security), #144 (K-HW-LIVE), #169 (verify after restart) |

## 0. Decision requested

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
- **Metering.** A per-task metering record (wall time, CPU-seconds, peak memory, tokens or rows) in receipt metadata or the data plane. It is labelled **untrusted operational metering**. Trusted metering requires T3 or T4 and comes later.

### 4.10 Future pricing and settlement

Economics is deferred to RFCs by the v0.4 lock. RFC 0003 and RFC 0004 remain research. The order is: **metering first, then price quotes, then settlement**.

- A price is an off-chain quote. The `commercial.price` dimension already exists in J.
- A settlement RFC would bind a quote and a receipt to a payment, and later to staking, challenge and slashing.

None of this is in the next three gates.

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
| **M0** | Direction, claims vocabulary, Vision v2 | This proposal approved; `VISION_v2` merged; claims vocabulary (§3) referenced from INDEX | Y (definitions) | Maintainer approval; INDEX updated | Any accuracy target |
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

They are ordered by dependency and risk. Each is a separate PR set, approved before merge, with PR CI and push CI reported separately.

### G1 — Security #178: content-safety screen correction (S1)

**Scope:**
1. **Documentation correction (prepared).** Branch `docs/178-cc3-scope` (`a33b669`): scopes CC-3's evidence to its corpus and discloses the detection gap. Documentation only.
2. **Screen-hardening candidate (prepared, not merged).** Branch `security/178-screen-hardening` (`17459e5`):
   - quoted values are matched without punctuation;
   - `OUTPUT_USES_INSTRUCTION_ONLY_TERM` (unquoted values);
   - `OUTPUT_EMPTY_AFTER_INSTRUCTION` (suppression).

   **Measured with `screen()` on 93 real synthetic outputs:**
   - 9/9 confirmed overrides flagged, 0/83 benign or resisted outputs flagged (test P11);
   - the in-process CLI rejects N17 and N18 with exit 12 (test P13);
   - all `mbongo-compute` tests pass, and clippy is clean.
3. **Still owed:** a real devnet client-path validation (`mvp-launch.mjs adversarial`) on three corpora: the MVP-E corpus, the unseen variants, and the held-out corpus `mvp-sec178-adversarial-v1`. That corpus has 6 attacks and 3 benign controls with innocent instruction-like lines, and was committed (`bdfb60e`) before any run. The validation is run once on released v3 and once with the candidate.

**Dependencies:** none.

**Protected surfaces:**
- `mvp/screen.rs` and the CLI outcome mapping (released client behaviour);
- the CC-3 record (`MVP_v0.1.md` §20.3);
- v3's profile, which is unchanged.

**Change control:** a new change record (proposed CC-5) naming the decision, the evidence and the replacement. The released `mvp-v0.1.0` tag is never moved.

**Risks:**
- **False positives on real CVs.** A benign instruction-like line combined with a word used only there; measured only on synthetic data.
- **The screen stays heuristic.** The marker list is English, French and Spanish, and S06 (no marker) is a known probe of that limit.

**Exit criteria:**
- 0 undetected and 0 client-accepted overrides on all three corpora through the real client path;
- benign controls `VERIFIED`;
- the corpus scope stated in every claim;
- the docs PR merged first.

### G2 — Generic workload contracts plus the MbongoPay reference workload (M1)

**Scope:**
- the `mbongo-job:v1` contract family definition;
- the runtime-adapter interface;
- the MbongoPay v1 deterministic scorer as a new `ExecutionProfile`;
- a seeded synthetic generator and independent Node reference;
- MbongoPay vectors;
- the conformance extension.

Kaayu is moved behind the generic interfaces **without any byte change**.

**Dependencies:** M0 approval (this proposal and the claims vocabulary). It is independent of G1.

**Protected surfaces (proved unchanged with Git objects):**
- the `ComputeTask` and Receipt code and vectors;
- rpc_v0.3;
- the `mbongo-infer:v1` contract and its vectors;
- the `kaayu-cv-extract` v1–v3 assets and vectors;
- the released MVP front behaviour (the frozen spec stays accepted);
- the MVP-D / MVP-E vectors;
- gold v1.

**Risks:**
- **Refactor regression** in the released path. Mitigated by byte-identical golden vectors and the existing suites.
- **Scope creep** into ML models or real credit use. Blocked by the §5.2 guardrails.
- **Over-generalising before a second runtime exists.** Kept to two concrete adapters.

**Exit criteria:** the M1 row of §6; 0 protocol diffs; MbongoPay 100 % equality with the independent reference on ≥ 200 synthetic cases.

### G3 — Contract-driven technical verification T0–T2 (M2)

**Scope:**
- a verifier library and CLI path driven by the contract;
- T2 re-execution for `BIT_EXACT` (MbongoPay) and for `BIT_EXACT_PINNED` within one host class (Kaayu);
- a report format naming the level reached;
- tamper corpora.

**Dependencies:** G2.

**Protected surfaces:**
- receipt semantics, which are unchanged; T-levels are client-side;
- the MVP's four existing result fields and exit codes, which can be extended only with new codes;
- `BUSINESS_* = NOT_ASSESSED`.

**Risks:**
- **Over-claiming.** Mitigated by naming the level and testing that T2 is never reported for a `NON_DETERMINISTIC` contract.
- **Hardware non-determinism making Kaayu T2 flaky.** It is declared `BIT_EXACT_PINNED`, so T2 is limited to the host class.
- **Re-execution cost.** The verifier is opt-in.

**Exit criteria:** the M2 row of §6.

**Next candidate after G3:** M3 (durable private data lifecycle, #169). It is needed before any real-data workload.

---

## 8. Non-goals and constraints carried forward

- No protocol rewrite. No change to locked surfaces without an accepted RFC (M9 and M10 only).
- The chain never executes workloads. No VM.
- `mvp-v0.1.0`, its tag, `main`, profile v3 and all existing evidence stay as released.
- #179 stays open as experimental evidence and is not merged. `lab/v4-g1b` is stopped. CC-4 is not started.
- No real candidate, applicant or financial data. No O1-B02.
- No automated consequential decisions on any reference-workload output. Human review stays mandatory for Kaayu.
- No GPU implementation before M7's own approval.

## 9. Housekeeping found during the survey (not fixed here)

- `RFC_PROCESS.md` and `CONTRIBUTION_TIERS.md` still name `PROTOCOL_LOCK_v0.2.md` and `rpc_v0.1` as the lists of locked surfaces. The current lock is v0.4.
- RFC 0002's header still reads "Draft", while INDEX lists it as Accepted.
- The injection screen's marker list and field names are Kaayu-specific (S2).

## 10. Open questions for the maintainer

1. Approve the `VISION_v2` direction (§1.2), or keep VISION_v1 and treat this as an extension?
2. Is MbongoPay v1's integer scorecard sufficient as the second workload, or is an ML runtime (v2) required earlier?
3. Should G1's documentation correction merge before its screen change (recommended), or together?
4. Should the generic contract be a new tag (`mbongo-job:v1`, recommended), or should `mbongo-infer` be versioned to v2?
