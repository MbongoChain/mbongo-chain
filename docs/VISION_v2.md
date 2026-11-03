# Mbongo Vision v2 — Verifiable Off-Chain Compute, Anchored On-Chain

| | |
|---|---|
| **Status** | **PROPOSED governance amendment.** It decides nothing until it is adopted (§9). Until then [`VISION_v1.md`](VISION_v1.md) is the NORMATIVE vision and this document is not an authority on any subject. |
| **Amends** | [`VISION_v1.md`](VISION_v1.md) (NORMATIVE, 2026-02-16). VISION_v1 is not edited by this amendment; on adoption it is marked SUPERSEDED and kept intact as a historical record. |
| **Date** | 2026-11-03 |
| **Origin** | Maintainer direction of 2026-11-03: approve the general-purpose compute direction and prepare `VISION_v2` as a separate, explicitly reviewed amendment. Architecture and roadmap: [`architecture/GENERAL_PURPOSE_COMPUTE_PROPOSAL.md`](architecture/GENERAL_PURPOSE_COMPUTE_PROPOSAL.md) (PR #180, under review). |
| **Kind** | Vision and scope. Non-consensus. It authorises no implementation: protocol changes still need an accepted RFC ([`RFC_PROCESS.md`](RFC_PROCESS.md)), and every implementation step needs its own approved gate. |

This document states **what Mbongo is for and what it will never be**. It does
not restate protocol facts; those live in
[`specs/PROTOCOL_LOCK_v0.4.md`](specs/PROTOCOL_LOCK_v0.4.md), RFC 0005 and the
other authorities listed in [`INDEX.md`](INDEX.md).

---

## 0. Why the vision is amended

VISION_v1 describes Mbongo as a verification layer providing "a single
primitive — verified inference", which "does not schedule" and is "not a cloud
GPU network".

Two things have changed since February 2026:

1. **What shipped already goes further.** `mvp-v0.1.0` runs a worker, a
   control plane and a private data plane. They are all off-chain and
   non-consensus, and they are first-class parts of the project, not out of
   scope.
2. **The direction has changed.** On 2026-11-03 the maintainer set the
   direction: Mbongo is a general-purpose, verifiable and eventually
   decentralised compute infrastructure. Application workloads such as Kaayu
   and MbongoPay are reference workloads, not the product.

What has **not** changed is the on-chain design. The chain still executes no
workload, has no VM, and decides deterministically. This amendment keeps
those principles (§2) and widens only what happens off-chain (§3).

---

## 1. What Mbongo is

**Mbongo is infrastructure for verifiable compute.** Work runs off-chain, on
workers operated by compute providers. The chain anchors what was asked and
what was answered, as commitments and signed receipts. It decides
deterministically whether a receipt is valid, and it never decides whether an
answer is *true*.

Mbongo serves any compute workload that can be described by a versioned
contract:

- inference by language and other ML models;
- deterministic computation, such as scoring;
- multi-step workflows, including agent workflows built from such steps.

Workers may run on CPU or, later, GPU, under several pinned runtimes.
Coordination between clients and providers is by **capability**: what a worker
can run, under which confidentiality class and with which evidence. Over time,
assigning providers and settling payment become decentralised, through
accepted RFCs only.

---

## 2. Foundational principles (preserved from v1)

These carry VISION_v1's principles forward. Changing any of them needs a
further, separately reviewed vision amendment.

| # | Principle | Meaning |
|---|---|---|
| **P1** | **No on-chain model execution.** | Models and workloads never run inside the chain runtime or inside block validation. Validators do no compute work for clients. |
| **P2** | **No general-purpose blockchain VM.** | No smart-contract VM and no plan to add one. Mbongo is not a general-purpose L1. |
| **P3** | **Deterministic protocol verification.** | Given the same block, receipt and chain state, every node reaches the same accept or reject decision. No probabilistic consensus outcome, and no consensus rule that depends on an off-chain judgement. |
| **P4** | **Off-chain execution, on-chain commitments and receipts.** | Execution is off-chain. What the chain holds is commitments, attributions and signatures, never private inputs or outputs. |
| **P5** | **No unsupported claims.** | A signed, anchored receipt proves *execution integrity* only: this executor answered this committed task over this committed input. It proves neither semantic correctness nor confidential execution. Every stronger claim names its evidence and its level (§4). |
| **P6** | **Replayability and auditability.** | Every anchored commitment and verification decision can be replayed from genesis. |
| **P7** | **The chain does not schedule.** | Discovery, matching, scheduling, negotiation and data transfer are off-chain. Off-chain infrastructure may coordinate them; consensus does not. The same rule appears in VISION_v1 §2 and RFC 0005 §6. |
| **P8** | **Protocol change only by RFC.** | Locked surfaces change only through an accepted RFC under [`RFC_PROCESS.md`](RFC_PROCESS.md). A vision statement never authorises a protocol change by itself. |

---

## 3. What the vision expands

| VISION_v1 statement | VISION_v2 statement | Bounded by |
|---|---|---|
| "A single primitive — verified inference" | **Multiple compute runtimes and workload kinds**, each behind a versioned, non-consensus workload contract: ML inference, deterministic computation, and multi-step and agent workflows | P1, P4 |
| "Not a cloud GPU network. Mbongo does not schedule, route, or manage GPU hardware" | **CPU and GPU workers** run by providers. Off-chain infrastructure (control plane, directories) **coordinates by capability**; the chain still does not schedule | P1, P7 |
| "Not an AI marketplace. Mbongo does not match buyers with sellers" | **Capability-based coordination now; decentralised provider assignment and settlement eventually.** Each step that touches the protocol (open assignment, fees, settlement, staking for executors, challenges) needs an accepted RFC | P7, P8 |
| A receipt as "cryptographic proof that the inference was performed correctly" (§5) | **A claims vocabulary** (§4): execution integrity, technical verification levels, business correctness, confidentiality and content safety are separate claims, and none implies the next | P5 |
| "Optional on-chain execution — PoUW as an opt-in extension" (§4, v2+) | **Withdrawn.** Validators do not execute workloads, even optionally | P1 |
| "Credit — AI-based credit scoring with auditable decisions" (§7) | **No automated consequential decisions.** Mbongo verifies how an output was produced, never whether acting on it is appropriate. Workloads touching credit, employment or other regulated decisions are synthetic reference workloads only, unless a separate legal, fairness and governance review approves otherwise | §5 |
| "zkML" as a v3 milestone | Succinct proofs remain **research** (technical level T5), not a commitment | P5 |

---

## 4. Claims Mbongo makes, and does not make

The proposal (§3) defines a claims vocabulary. Its summary is:

- **E, execution integrity:** what a valid, anchored receipt establishes.
- **T0–T5, technical result verification:** levels from commitment match, through output-contract conformity, independent re-execution, *k*-of-*n* agreement and attested execution, up to succinct proofs. A report names the highest level it actually reached.
- **B, business correctness:** whether an output is right for its purpose. It is always scoped to an evaluation methodology and a sample. It is `NOT_ASSESSED` unless such a methodology was applied.
- **C, confidentiality:** who could see the data. Under PUBLIC, the provider sees plaintext. Provider-resistant confidentiality is claimed only with live attestation evidence.
- **S, content safety:** no *detected* sign that an untrusted input steered the output. A clear screen is never proof that the output was uninfluenced.

The rule that follows is part of this vision: **Mbongo never presents E or T
as B, C or S.**

---

## 5. Reference workloads

A reference workload is a small, synthetic, fully specified application. It
proves an infrastructure capability on more than one workload kind. It is a
test vehicle, not a product line, and its business accuracy is never on the
infrastructure critical path.

- **Kaayu** (structured CV extraction): LLM runtime, adversarial untrusted text, JSON output. Released as `kaayu-cv-extract:v3` in `mvp-v0.1.0`.
- **MbongoPay** (synthetic loan-risk scoring): deterministic integer scorecard, numeric output, exactly re-executable.

Its guardrails are part of this vision:

- synthetic data only;
- no accept or decline output;
- no protected attributes;
- not offered or used for real credit assessment.

Real-data admission for any workload is a separate, explicit maintainer
decision.

---

## 6. What Mbongo is not

- **Not a general-purpose L1 and not a VM** (P2).
- **Not an on-chain executor.** No workload runs in consensus (P1).
- **Not a source of semantic truth.** It does not certify that an output is correct, fair or safe to act on (P5).
- **Not a decision-maker.** It does not make or automate consequential decisions about people from workload outputs (§3).
- **Not a training platform.** Model training is out of scope of this vision.
- **Not a DeFi protocol.** Other systems may consume Mbongo receipts; Mbongo is not itself a financial product.

---

## 7. Direction of travel

The order is capability first, then decentralisation, then economics. Each
step has its own gate, and steps marked *RFC* change the protocol and need an
accepted RFC first. Milestone detail is in the proposal, §6.

1. Released MVP: one workload, one CPU worker, PUBLIC.
2. Generic workload contracts and runtime adapters; a second reference workload.
3. Contract-driven technical verification (T0–T2).
4. Durable private data lifecycle.
5. Capability discovery and eligibility in the control plane.
6. Multi-step workflows.
7. Observability and untrusted metering.
8. GPU execution adapters.
9. Attested confidential execution (T4), with live hardware evidence.
10. Open provider assignment and redundancy (T3). *RFC.*
11. Pricing and settlement. *RFC.*

No date is attached to any step. Versions and dates live in the documents that
decide them.

---

## 8. What this amendment does not do

- It does not edit VISION_v1, the protocol lock, any RFC, any spec or any code.
- It does not authorise any implementation gate, GPU work, an ML backend, real-data processing or a protocol change.
- It does not alter the released `mvp-v0.1.0`, its evidence or its documented limitations.

---

## 9. Adoption

Adoption happens only through explicit maintainer review of the pull request
that carries this document. The steps are:

1. **Review.** The maintainer reviews this text. It stays PROPOSED while review is open.
2. **Adoption commit.** After approval, and before the merge, one visible commit:
   - sets this document's status to NORMATIVE, with the approval date;
   - changes the "Project vision and scope" row of [`INDEX.md`](INDEX.md) from VISION_v1 to VISION_v2;
   - adds a SUPERSEDED banner at the top of VISION_v1, leaving its text unchanged;
   - repoints the "chain is not a scheduler or marketplace" citations in `architecture/compute-privacy-data-plane.md` (P14) and `architecture/compute-control-plane-worker-interface.md` (E25) to P7 here. Their meaning does not change.

   The merge gate runs on that exact commit.
3. **If not approved.** VISION_v1 remains NORMATIVE and this document is not merged.
