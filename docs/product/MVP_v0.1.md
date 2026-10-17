# Mbongo MVP v0.1 — Frozen product scope

**Status:** FROZEN PRODUCT SCOPE (product and architecture decision; non-consensus).
**Decided:** 2026-10-17 at `89cadb670c3d0470fcda70fb2e7f53404157cb9e` (protocol v0.4, frozen).
**Authority this document holds:** what Mbongo launches first, for whom, on which workload, runtime, model, worker, privacy level, provider model, payment model, client surface and exit criteria — and what is deferred until that launch has happened.
**Authority this document does not hold:** nothing here changes a protocol surface. [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md), [PROTOCOL_LOCK_v0.4](../specs/PROTOCOL_LOCK_v0.4.md), [rpc_v0.3](../specs/rpc_v0.3.md) and the architecture documents named in [`docs/INDEX.md`](../INDEX.md) win on every subject they decide. Where this document chooses something they leave open, it is a product choice and says so.

> **Product statement.** Mbongo MVP lets **Kaayu's recruiting backend** submit **one single-turn structured-extraction inference (a candidate CV, as text, to a fixed JSON profile schema)** to **one Mbongo-operated worker running llama.cpp with a pinned open-weight model**, and receive **the JSON result off-chain within a measured latency and cost**, with **the Mbongo guarantee that the task was committed on-chain before execution, the private input and result never touched the chain, the executor that answered is the one the client named, and a signed receipt binding task, input commitment and output commitment is anchored and verifiable by the client**.

The purpose of this freeze is to make Mbongo launchable before it is complete. Every direction the project could take next is classified below as **MVP BLOCKING**, **POST-MVP** or **PARALLEL RESEARCH**. Before launch, each feature request is answered with one question: *does the first real user need this to complete the selected workflow?* If not, it is deferred unless it fixes a security or reliability blocker.

---

## 1. Where the repository stands

Everything in this table is implemented and tested at the decision commit; the MVP reuses it rather than redesigning it.

| Layer | State | Authority |
|---|---|---|
| `ComputeTask` commitment, `task_id`, consensus rules (k)–(s), task-bound receipts | released, frozen (v0.4) | [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md), [PROTOCOL_LOCK_v0.4](../specs/PROTOCOL_LOCK_v0.4.md) |
| JSON-RPC (six methods, `ComputeTask` payload variant) | frozen | [rpc_v0.3](../specs/rpc_v0.3.md) |
| TypeScript SDK `@mbongo/sdk` 0.2.0: build, sign, submit tasks; decode blocks; bound receipts; executor anchoring | published | [`sdk/typescript/README.md`](../../sdk/typescript/README.md) |
| Four-plane privacy boundary (chain / control plane / private data plane / execution environment) | normative | [compute-privacy-data-plane.md](../architecture/compute-privacy-data-plane.md) |
| Control plane ↔ worker contract (sessions, leases, attempts, retry) and private data plane contract (objects, capabilities, commitments) | normative, transport-free | [E](../architecture/compute-control-plane-worker-interface.md), [F](../architecture/compute-private-data-plane-interface.md) |
| Reference worker, in-process control plane and data plane; conformance suite `compute-conformance-v1` (38 cases); v0.4 vertical harness on a three-node devnet | current | [reference-worker.md](../development/reference-worker.md), [compute-conformance.md](../development/compute-conformance.md), [DEVNET_V0.4_OPERATIONS.md](../runbooks/DEVNET_V0.4_OPERATIONS.md) |
| Provider capability / policy model `provider-policy-v1` (PUBLIC / VERIFIED / CONFIDENTIAL / SOVEREIGN profiles, deterministic eligibility) | normative, reference evaluator | [J](../architecture/compute-provider-capability-policy.md) |
| Confidential attestation and conditional key release `confidential-auth-v1` (reference path, test only) | normative | [K](../architecture/compute-confidential-attestation-key-release.md) |
| AMD SEV-SNP hardware attestation adapter `snp-adapter-v1` | merged; live guest run **not performed** | [K-HW](../architecture/compute-hardware-attestation-adapter.md), [#144](https://github.com/MbongoChain/mbongo-chain/issues/144) |

What does **not** exist: any real execution profile beyond the reference `reverse-bytes` transform, any client-facing off-chain transport for the control plane and data plane (E §19 and F §5.3 are deliberately transport-free), any inference runtime, any metering, pricing, payment or settlement, any GPU path, any verification of output correctness ([#52](https://github.com/MbongoChain/mbongo-chain/issues/52)).

### 1.1 Relationship to the vision

[`VISION_v1.md`](../VISION_v1.md) names capital-moving integrations (DeFi oracles, risk engines, insurance, governance) as the first market for *verified* inference. The chain does not verify correctness today (RFC 0005 §6; J §13 — `verification.result` has no mechanism), so no capital-moving integration can honestly be sold on it yet. The MVP therefore exercises the primitive that **is** real — committed task, off-chain private execution, named executor, anchored receipt — on a workload whose value does not depend on correctness verification. This is a sequencing decision inside the vision, not a change of vision: the primitive, the planes and the receipt are the same ones the vision's integrations will consume.

---

## 2. Candidate comparison

Evaluated before any runtime, model, hardware or privacy choice was made.

| Candidate | User value | Engineering effort | Time-to-launch | Uses existing Mbongo | Requires new complexity | MVP suitability |
|---|---|---|---|---|---|---|
| **A. Generic compute job** (arbitrary binary or container, reverse-bytes generalized) | low: no first user has a "generic job" they cannot run on a VM; the receipt attests nothing a user wants to show | medium (sandboxing, packaging) | medium | high (lifecycle unchanged) | sandbox, resource limits, image distribution | **weak** — technically closest, no concrete user |
| **B. Private AI inference** (one open model, one runtime, input and result off-chain, PUBLIC profile) | real: a real backend gets an inference result plus an on-chain commitment and a signed receipt it can keep as an audit record; the data never touches a chain | medium (one execution profile, a thin off-chain transport, a CLI) | **short** | high (task, planes, worker lifecycle, receipt, SDK, J PUBLIC) | inference contract, runtime packaging, model pinning | **selected** |
| **C. Confidential AI inference** (B plus CONFIDENTIAL profile, SEV-SNP guest) | higher in principle (provider-resistant confidentiality) but **unproven**: the live SEV-SNP run is blocked on paid provisioning (#144), and the first user does not need confidentiality from a worker its own organization operates | B + guest provisioning, key release in production, persistent authority state | long and externally blocked | high | hardware guest, durability gap, GPU in TEE | **deferred** to a parallel security track; B must not wait for it |
| **D. Agent execution** (multi-step, tool-using agent runtime) | speculative: no receipt semantics for multi-step work; no first user asking for it | high (runtime, tools, state, new commitment model) | long | low (one task = one commitment; agents need many) | agent runtime, harness adapters, step commitments | **deferred** (research) |
| **E. GPU marketplace** (open providers, pricing, settlement) | none until there is demand; contradicts VISION_v1 §2 ("not a cloud GPU network", "not an AI marketplace") | very high | very long | low (no economics in protocol by design, RFC 0005 §6) | marketplace, MBO settlement, metering, reputation | **rejected** for MVP |

**Why B and not A.** A and B share every Mbongo layer; B differs only in the execution profile and in having a user. The reference transform already proves the lifecycle; what is missing is a workload someone wants.

**Why B and not C.** The first user's data is already processed in plaintext by its own backend today. A worker operated by the same organization sees exactly what that backend sees; the protection the MVP adds is *from the chain and the public*, which the PUBLIC profile delivers now ([privacy §7](../architecture/compute-privacy-data-plane.md), [J §12](../architecture/compute-provider-capability-policy.md)). Hardware confidentiality becomes a requirement the day a worker is operated by someone the client does not trust; that is a later provider model, not the first one.

---

## 3. First user

**MVP_PRIMARY_USER = Kaayu internal AI workload** (the recruiting product operated by the maintainer's organization), **KAAYU_FIRST_CLIENT = YES**, one workload only.

### 3.1 Who, and what they are trying to do

Kaayu's backend already turns candidate documents into structured data with an LLM. It needs, for each CV it processes: a structured profile (schema-fixed JSON) returned within a bounded time, the candidate's document kept out of any public system, and a durable record that *this* extraction was requested, *this* input was used (by commitment, not content) and *this* executor produced *this* output (by commitment).

### 3.2 Why Mbongo instead of the alternatives

| Alternative | What it gives | What it does not give, which the MVP does |
|---|---|---|
| OpenAI / Anthropic / Together / Fireworks | best-quality models, lowest effort | the document leaves the organization; no third-party-verifiable record of what was requested and answered; model and provider can change silently under the same name |
| local inference (Ollama on a developer machine) | privacy, zero cost | no commitment, no receipt, no separation between the party that requests and the party that executes, nothing a third party can later check |
| ordinary cloud VM running the same runtime | privacy within the organization, control | the same: the VM's logs are the only record, and they are the executor's own word |
| direct GPU rental | capacity | nothing above, plus a provider that sees plaintext |

**What Mbongo adds in the MVP, concretely:** (1) the request is committed on a chain *before* execution — submitter, named executor, input commitment and execution spec are permanent and public, while the input is not; (2) the executor that answered is provably the one the client named (RFC 0005 §2.5, rules (q)–(s)); (3) a receipt signed by that executor binds `task_id`, `input_commitment` and `output_commitment`, and the client verifies the signature and the binding itself with the published SDK; (4) the private input and the result travel only through the private data plane under single-use capabilities, and the harness scans prove they appear in no block. That is an **audit record of AI processing** a recruiting product can keep and show — not a proof the answer was right (see §13).

What is explicitly **not** claimed as MVP value: decentralization, Web3, "AI infrastructure", a future marketplace, token economics, cheaper GPUs.

---

## 4. Workload

**MVP_USE_CASE = private AI inference** (PUBLIC profile, off-chain data).
**MVP_WORKLOAD = single-turn structured extraction: CV text → JSON candidate profile**, schema-constrained.

- **Input:** the CV as UTF-8 text (Kaayu already extracts text from PDFs; PDF parsing is not Mbongo's job), bounded at 2,500 model tokens, plus the fixed extraction instruction. The whole prompt is one private-data-plane object; its BLAKE3 commitment is the task's `input_commitment` (RFC 0005 §2.4 convention).
- **Output:** one JSON document conforming to a fixed schema (identity-free fields such as skills, roles with date ranges, education, languages, seniority estimate — the exact schema is fixed in the inference contract gate), bounded at 400 generated tokens. Its commitment is the receipt's `output_commitment`.
- **One turn, no tools, no memory, no retrieval.** The job is a pure function of the prompt bytes and the pinned model/runtime/parameters.

Rejected for the first workload: candidate–job matching explanation (needs two documents and a judgement the product would have to defend), interview summarization (audio pipeline), any autonomous recruiting agent.

---

## 5. Runtime and model

**MVP_RUNTIME = llama.cpp `llama-server`** (MIT; ggml-org), one pinned release, CPU build, driven through its OpenAI-compatible `/v1/chat/completions` with `response_format` set to the JSON schema (grammar-constrained decoding).

| Criterion | llama.cpp | vLLM | Ollama | ODS or other |
|---|---|---|---|---|
| runs on the development laptop and on a small CPU VM | yes | no (GPU-first) | yes | unknown |
| hardware flexibility, future GPU path without contract change | CPU, CUDA, Vulkan, Metal builds of the same server | GPU | wraps llama.cpp | — |
| exact model identity | a GGUF file, pinned by digest | HF revision | registry tag (mutable) | — |
| packaging and operation | single binary + one file | Python stack | daemon + registry | — |
| schema-constrained JSON | native (JSON schema → grammar) | yes | partial | — |
| license | MIT | Apache-2.0 | MIT | — |

Ollama is already in use on the development machine and is a legitimate developer convenience; it is **not** the MVP runtime because a mutable registry tag is not a model identity. vLLM is the natural **post-MVP** GPU runtime if throughput ever matters.

**MVP_MODEL = Qwen3-4B-Instruct-2507**, GGUF `Q4_K_M` quantization, pinned by the SHA-256 of the exact GGUF file. **MVP_MODEL_LICENSE = Apache-2.0** (open weights; commercial use permitted). Chosen because it is a non-thinking instruct model (no `<think>` blocks to strip), strong at structured extraction at 4B parameters, fits CPU inference with ~3 GB of weights, and its license carries no acceptable-use registry. Alternatives considered: Llama 3.x (community license, not open), Gemma (Gemma terms), Mistral 7B (Apache-2.0, larger, slower on CPU), Phi-4-mini (MIT, weaker at long structured extraction in the maintainer's experience — to be re-checked only if Qwen fails the latency gate).

**Model identity for the receipt record** (defined precisely in the inference contract gate; not a global registry): runtime id and release, GGUF file digest, tokenizer as embedded in the GGUF, quantization, generation parameters (temperature 0, fixed seed, max tokens), schema digest. The model is **not** protocol authority; it is bytes named by the execution spec.

---

## 6. Execution backend

**MVP_EXECUTION_BACKEND = CPU** (CPU-first; GPU is post-MVP).

| Option | Latency for the workload (4B, Q4, ≤2,500 in / ≤400 out) | Cost | Time-to-launch | Verdict |
|---|---|---|---|---|
| CPU-first (8 vCPU AMD Milan VM or the i7-8850H laptop) | estimated 60–120 s end to end; to be measured | ≈ US$0.35/h for an 8-vCPU VM; $0 on the laptop | shortest | **selected** |
| GPU-first | seconds | ≥ US$0.5–1/h plus driver/container work | longer | rejected for MVP |
| CPU-dev + GPU-production | two configurations to validate | both | longer | rejected: one path only |

A recruiting backend tolerates a minute per CV for a batch audit path; it does not tolerate an unmeasured one. The latency gate (§12) is measured on the CPU reference host; if it fails, the first lever is the input bound or a smaller quantization, not a GPU. GPU is not chosen "because Mbongo is a compute network".

---

## 7. Provider model, privacy level, and K-HW-LIVE

**MVP_PROVIDER_MODEL = Mbongo-controlled worker**, **MVP_PROVIDER_COUNT = 1**. The worker holds the executor key the client names in every task; there is no discovery, matching or marketplace (RFC 0005 §6 — assignment is off-protocol by design).

**MVP_PRIVACY_LEVEL = PRIVATE_OFFCHAIN** — the data is never on-chain, and the provider may see plaintext. This is exactly J's **PUBLIC profile** ([J §12](../architecture/compute-provider-capability-policy.md): "provider may see plaintext — yes; confidentiality guarantee — none") and the privacy architecture's baseline ("OFF-CHAIN ≠ PRIVATE FROM THE PROVIDER", [§7](../architecture/compute-privacy-data-plane.md)). The MVP says so in its own client documentation and never uses the word "confidential" for this path.

**HARDWARE_ATTESTATION_REQUIRED_FOR_MVP = NO. K_HW_LIVE_BLOCKS_MVP = NO.** The live SEV-SNP proof ([#144](https://github.com/MbongoChain/mbongo-chain/issues/144)) is a **PARALLEL** security track. It becomes launch-blocking only when a provider the client does not trust operates the worker; that is not the MVP provider model. K and K-HW stay merged, tested and unchanged; nothing in the MVP weakens them.

---

## 8. Payment, pricing, metering

**MVP_PAYMENT_MODEL = FREE** (internal first client; no invoice, no MBO, no on-chain fee rule — RFC 0005 §6 has none). The simpler-than-FIXED_OFFCHAIN model is chosen because the first client and the worker operator are the same organization; a fixed off-chain price becomes the model the day a second client exists.

**MVP_PRICING_MODEL = fixed reference price per task, displayed, not charged**: the CLI prints, for each job, the worker's wall-clock seconds multiplied by the configured hourly infrastructure rate. It exists so that cost is *observable* and the cost target (§12) can be enforced, not to bill anyone.

**MVP_METERING = observed, untrusted:** `task_id`, worker instance, model and runtime identity, `submitted_at` / `started_at` / `completed_at`, wall-clock execution seconds, prompt and completion token counts as reported by llama-server, success or failure class. No trusted economic metering: nothing pays on these numbers (`TRUSTED_METERING_IN_MVP = NO`).

---

## 9. Client surface

**MVP_CLIENT_API = `@mbongo/sdk` 0.2.0 for everything on-chain (unchanged) + one thin HTTP front over the existing control-plane and data-plane operations (new, non-consensus) + one CLI.** `SDK_WIRE_CHANGE_REQUIRED = NO`; `NEW_RPC_VERSION_REQUIRED = NO`.

The HTTP front is an *implementation* of operations E §19 and F §4–§10 leave transport-free; it adds no authority. The minimum:

| Call | Existing operation it transports |
|---|---|
| `PUT /v1/inputs` → `{object_ref, input_commitment}` | F §4.1 private-input creation; commitment per RFC 0005 §2.4 |
| `POST /v1/tasks/{task_id}/register` `{object_ref, executor}` | E "register input reference with the control plane" (reference-worker lifecycle) |
| `GET /v1/tasks/{task_id}` → `{state, timings, failure_class, receipt_tx?}` | E observe / report operations; states are the control plane's own |
| `GET /v1/tasks/{task_id}/result` (under a get-result grant) → result bytes | F §10 private result retrieval; client verifies against the receipt's `output_commitment` |

Task submission and receipt reading stay on the chain through the SDK (`submitComputeTask`, `computeTasksInBlock`, `receiptsInBlock`, `assertReceiptBoundToTask`). The SDK package itself keeps its stated boundary: no control-plane, data-plane or worker API inside `@mbongo/sdk`; the CLI composes the two.

**MVP_DEMO_CLIENT = a CLI** (`mbongo-mvp submit <cv.txt>` → prints `task_id`, waits, prints the JSON result path, the receipt verification result, latency and the reference cost). It is the fastest route to a real Kaayu integration (a subprocess or a small TypeScript import), and it is what the exit run (§12) drives.

---

## 10. End-to-end flow

Every step is either implemented today or named in exactly one MVP gate (§15).

| # | Step | Exists | Gate |
|---|---|---|---|
| 1 | Client builds the prompt (CV text + fixed instruction), computes `input_commitment`, stores the object in the private data plane | commitment: yes (SDK, crate); store: in-process only | MVP-C (HTTP front) |
| 2 | Client builds `execution_spec = "mbongo-infer:v1" ‖ BLAKE3(job spec)` and commits the `ComputeTask` naming the Mbongo worker's executor address | task construction, signing, submission: yes | MVP-A (spec convention) |
| 3 | Control plane observes the task in a block at confirmation depth 1 | yes | — |
| 4 | Worker session (proof of possession), lease, single-use fetch capability, fetch, commitment verification | yes | — |
| 5 | Worker resolves the job spec (model digest, runtime, parameters, schema), runs llama-server, obtains schema-valid JSON | no | MVP-B |
| 6 | Worker persists the result under a put capability; data plane confirms durability | yes | — |
| 7 | Worker builds the bound receipt, signs it with the executor key, anchors `AnchorReceipt` | yes | — |
| 8 | Chain applies rules (a)–(j), (q)–(s) | yes (frozen) | — |
| 9 | Client reads the receipt from the block, verifies signature and binding with the SDK | yes | — |
| 10 | Client retrieves the result under a get-result grant, verifies `output_commitment`, prints latency and reference cost | retrieval in-process only; cost display: no | MVP-C, MVP-D |

---

## 11. Worker scope and the inference contract

**Worker.** The reference worker gains **one** execution profile, `mbongo-infer:v1`, beside the reference `reverse-bytes` profile (which stays, as the conformance subject). Everything else is preserved as is: task binding, input-commitment verification before execution, result persisted before the receipt, receipt signing and anchoring as the named executor, E's failure classes and retry semantics, and the conformance suite (`compute-conformance-v1` must still pass against the inference worker — that is the point of having it).

**Inference contract (MVP-A), stated as constraints on the gate rather than as its design:**

- `execution_spec` is opaque, bounded at 1,024 bytes and *application-versioned by convention* (RFC 0005 §2.10, §2.12). The MVP convention is a leading ASCII tag `mbongo-infer:v1` followed by the 32-byte BLAKE3 of an off-chain **job spec** document; the job spec (runtime id and release, GGUF digest, quantization, generation parameters, output schema digest, input and output bounds) is itself a private-data-plane object or a public static document — the gate decides, and either is legal because the chain never interprets the field. Nothing is added to `ComputeTask`. **No contract gap exists** against current authority.
- The prompt is the input object; the JSON is the output object; both commitments follow the RFC 0005 §2.4 interoperability convention already used by the reference worker.
- Receipt `metadata` stays empty in the MVP (RFC 0002 bound; nothing private may go there — privacy §13).

---

## 12. Exit criteria, targets, security threshold

**MVP_LAUNCH_ENVIRONMENT = private devnet** (the three-node v0.4 devnet from the runbook, plus one worker host) **→ controlled demo**. No public devnet, no mainnet.

**Exit criteria** (all objective; the run is scripted and its log is the evidence):

| Criterion | Threshold |
|---|---|
| real client submits the workload | the Kaayu CLI path, on real (consented, internal) CV texts or synthetic CVs of the same shape — never a candidate's document without consent |
| real worker executes it | the Mbongo worker, `mbongo-infer:v1`, pinned runtime and model |
| private input stays off-chain | the vertical harness scans find no input or result bytes in any block (already enforced) |
| real model/runtime produces the result | schema-valid JSON for every success |
| result retrieved and verified | `output_commitment` matches for every success |
| receipt anchored and verified by the client | signature + binding verified by the SDK for every success |
| reliability (**MVP_RELIABILITY_TARGET**) | **20 consecutive jobs, ≥ 19 succeed (≥ 95 %)**, 0 private-data leaks, 0 invalid receipts, 0 duplicate receipts |
| latency (**MVP_LATENCY_TARGET**) | **P95 ≤ 120 s** submission → result retrievable, measured over the 20-job run on the CPU reference host at ≤ 2,500 input / ≤ 400 output tokens |
| cost (**MVP_COST_TARGET**) | **≤ US$0.05 per job** at the displayed reference price, and **≤ US$0.50 per hour** of total MVP infrastructure (worker host + devnet) |
| failure recovery | one job with the worker killed mid-execution: the lease expires, the retry attempt completes, exactly one receipt is anchored |
| observability | for every job: `task_id`, worker, three timestamps, duration, model/runtime ids, failure class, receipt transaction — and **no prompt or result bytes in any log** |

**Security threshold at launch:** no secrets on-chain; no raw prompt or result in logs; worker identity proven by possession of the executor key; input commitment verified before execution; result persisted before the receipt; receipt signature and binding verified by the client; executor key never leaves the worker process. These are the existing E/F invariants, kept.

---

## 13. What the receipt proves, and the overclaim rule

LLM inference is not bit-reproducible across hardware or runtime builds even at temperature 0. The receipt proves **that the named executor signed a receipt bound to this task, this input commitment and this output commitment**. It does not prove the model produced the correct, best or even the same answer another run would give. `OUTPUT_CORRECTNESS_PROVEN = NO` is the MVP's standing position, and client-facing text may not say "verified inference" for this path. Fixing temperature and seed reduces variance; it is not a correctness claim.

---

## 14. Frozen defer list

| Feature | Class |
|---|---|
| live SEV-SNP proof, CONFIDENTIAL profile in production, persistent confidential authority state | PARALLEL (security track; #144) |
| open provider marketplace, permissionless providers, reputation | POST-MVP |
| MBO settlement, staking economics, fee market, dynamic pricing, auctions | POST-MVP |
| trusted compute metering | POST-MVP |
| GPU worker, vLLM, multi-GPU, distributed inference | POST-MVP |
| multi-model routing, second model, second runtime | POST-MVP |
| agent execution, agent runtime, Claude/DeepSeek harness adapters, ODS adapter | PARALLEL RESEARCH (`AGENT_RUNTIME_STUDY = POST_MVP_OR_PARALLEL`) |
| ODS architecture audit, Together/Fireworks audit | PARALLEL RESEARCH (inform runtime and routing strategy; not launch blockers) |
| multi-vendor TEE (TDX, NVIDIA CC-GPU, Nitro) | POST-MVP |
| result verification, fraud proofs, ZK-ML, correctness proofs (#52, L) | PARALLEL RESEARCH |
| advanced PoUW, on-chain execution | POST-MVP (VISION_v1 §4: v2+) |
| training, fine-tuning | out of scope (VISION_v1 §2) |
| PDF/document parsing, OCR | client-side; not Mbongo |
| multi-turn, retrieval, tools | POST-MVP |

Consensus scope is frozen for the MVP: `NEW_CONSENSUS_FIELD = NO`, `NEW_TRANSACTION_TYPE = NO`, `NEW_RECEIPT_FIELD = NO`, `NEW_PROTOCOL_VERSION = NO`.

---

## 15. MVP gates and dependency graph

Milestones, not calendar promises. One implementation gate opens at a time.

| Gate | Name | Delivers | Class |
|---|---|---|---|
| 1 | **MVP scope freeze** | this document, the MVP issue ([#148](https://github.com/MbongoChain/mbongo-chain/issues/148), milestone *MVP v0.1*), roadmap reconciliation | done at this commit |
| 2 | **MVP-A — AI inference execution contract** | the `mbongo-infer:v1` `execution_spec` convention, the job-spec document, model identity, prompt and output formats, commitments, bounds; fixtures; no worker code | MVP BLOCKING — **next gate** |
| 3 | **MVP-B — inference worker** | the `mbongo-infer:v1` execution profile on the reference worker, llama-server integration, pinned model fetch and digest check, schema validation, conformance suite green, latency measured on the CPU reference host | MVP BLOCKING |
| 4 | **MVP-C — client transport and CLI** | the HTTP front over E/F, the CLI, SDK-based receipt verification, cost display | MVP BLOCKING |
| 5 | **MVP-D — end-to-end on the private devnet** | the scripted 20-job Kaayu run, failure-recovery case, privacy scans, the evidence log | MVP BLOCKING |
| 6 | **MVP-E — launch hardening** | observability fields, failure runbook, controlled demo, client documentation stating the exact guarantee | MVP BLOCKING |

```
v0.4 ✅  ─  J ✅  ─  K ✅  ─  K-HW adapter ✅
        │
        ▼
Gate 1  MVP scope freeze (this document)                       ✅
        ▼
Gate 2  MVP-A  inference execution contract                    ← NEXT
        ▼
Gate 3  MVP-B  inference worker (llama.cpp + Qwen3-4B, CPU)
        ▼
Gate 4  MVP-C  HTTP front over E/F + CLI (SDK unchanged)
        ▼
Gate 5  MVP-D  20-job Kaayu run on the private devnet
        ▼
Gate 6  MVP-E  launch hardening  ──►  MVP LAUNCH (controlled demo)

PARALLEL (never on the path above):
  K-HW-LIVE live SEV-SNP proof (#144) · agent runtime study · ODS / Together / Fireworks audits · verification research (#52)
POST-MVP:
  GPU / vLLM · second model or routing · open providers · MBO settlement · metering · multi-vendor TEE · PoUW
```

**MVP_BLOCKING_GATE_COUNT = 5** (MVP-A through MVP-E).

---

## 16. Build / adapt / reuse / defer

| Layer | Decision | Note |
|---|---|---|
| task commitment, receipt, chain rules, RPC | REUSE | frozen v0.4 |
| SDK | REUSE | 0.2.0 unchanged; CLI imports it |
| control plane, data plane, worker lifecycle, conformance suite | REUSE | in-process reference implementations, as they are |
| execution profile | BUILD | `mbongo-infer:v1`, one |
| inference runtime | REUSE (llama.cpp) | pinned release; no fork |
| model serving | ADAPT | llama-server configuration + model digest check in the worker |
| hardware detection | DEFER | CPU only; no detection needed |
| client transport (HTTP front over E/F) | BUILD | minimal, four calls |
| demo client | BUILD | one CLI |
| observability | ADAPT | the fields in §12 on top of existing worker logging rules (F §13) |
| payment, pricing, metering | DEFER | display-only reference cost |
| hardware attestation | REUSE, not required | K/K-HW stay as they are; PARALLEL |
| GPU, agents, marketplace, MBO, ZK | DEFER | §14 |

---

## 17. MVP architecture

```
Kaayu backend ──► CLI ──┬──► @mbongo/sdk ──► JSON-RPC ──► MbongoChain (3-node private devnet)
                        │        ComputeTask commit · receipt read · verification
                        └──► HTTP front (new, non-consensus) ──► control plane + private data plane (existing)
                                                                        │ lease · single-use capabilities
                                                                        ▼
                                                         Mbongo-operated worker (existing lifecycle)
                                                                        │ execution profile mbongo-infer:v1 (new)
                                                                        ▼
                                                         llama-server (pinned) + Qwen3-4B-Instruct-2507 GGUF (pinned digest), CPU
                                                                        │ schema-valid JSON
                                                                        ▼
                                                         private result ──► bound receipt ──► AnchorReceipt ──► MbongoChain
```

No marketplace, settlement, routing, GPU pool or attestation appears in this diagram because none is on the MVP path.

---

## 18. Non-goals

The MVP is **not**: a full decentralized AI cloud; an open GPU marketplace; an agent marketplace; a general-purpose AI platform; a token-economics launch; a correctness-proof system; a confidential-computing product (that claim waits for #144 and a provider model that needs it); a multi-model or multi-provider service.

---

## 19. Product decision matrix

| Decision | Value |
|---|---|
| MVP_PRIMARY_USER | Kaayu internal AI workload (recruiting backend) |
| MVP_USE_CASE | private AI inference (PUBLIC profile, data off-chain) |
| MVP_WORKLOAD | single-turn structured extraction: CV text → JSON candidate profile |
| MVP_RUNTIME | llama.cpp `llama-server`, one pinned release, CPU build |
| MVP_MODEL | Qwen3-4B-Instruct-2507, GGUF Q4_K_M, pinned by file digest (Apache-2.0) |
| MVP_EXECUTION_BACKEND | CPU |
| MVP_PROVIDER_MODEL | Mbongo-controlled worker |
| MVP_PROVIDER_COUNT | 1 |
| MVP_PRIVACY_LEVEL | PRIVATE_OFFCHAIN (= J PUBLIC profile) |
| HARDWARE_ATTESTATION_REQUIRED_FOR_MVP | NO |
| MVP_PAYMENT_MODEL | FREE (internal client) |
| MVP_PRICING_MODEL | fixed reference price per task, displayed, not charged |
| MVP_METERING | observed, untrusted (fields in §8) |
| MVP_DEMO_CLIENT | CLI |
| KAAYU_FIRST_CLIENT | YES |
| MVP_LAUNCH_ENVIRONMENT | private devnet → controlled demo |

---

## 20. Change control

This document is **frozen** until the MVP exit criteria (§12) are met or a gate proves one of its decisions wrong. A change needs a dedicated pull request that names the decision, the evidence that invalidated it, and the single replacement — not a widening. Adding a feature to the MVP path requires showing which exit criterion cannot be met without it.

## See also

- [`docs/INDEX.md`](../INDEX.md) — authority map (this document is the product-scope row)
- [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md) §2.10, §2.12, §6 — bounds, opacity and non-goals the inference contract must respect
- [compute-privacy-data-plane.md](../architecture/compute-privacy-data-plane.md) §7, §17.2, §22 — ordinary provider privacy, profiles, roadmap position
- [compute-provider-capability-policy.md](../architecture/compute-provider-capability-policy.md) §12–§14 — PUBLIC, VERIFIED, CONFIDENTIAL
- [reference-worker.md](../development/reference-worker.md) — the lifecycle the inference worker keeps
- [#148](https://github.com/MbongoChain/mbongo-chain/issues/148) — the MVP issue: launch blockers only (milestone *MVP v0.1*)
- [#144](https://github.com/MbongoChain/mbongo-chain/issues/144) — K-HW live run (parallel track); [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) — verification research (parallel track)
