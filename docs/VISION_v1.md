# Mbongo Vision v1 — Deterministic AI Inference Verification Layer

**Status:** Strategic direction document
**Last updated:** 2026-02-16

---

## 1. What Mbongo IS

Mbongo is a blockchain specialized in verifying deterministic AI inference results on-chain. It does not execute AI models. It verifies cryptographic receipts submitted after off-chain inference, then settles the economic outcome on-chain.

Core properties:

- **Deterministic verification.** Given the same receipt and the same chain state, every node reaches the same accept/reject decision. No probabilistic outcomes.
- **On-chain settlement.** Payment for compute, reward distribution, and slashing are executed as atomic state transitions within block application.
- **Economic security.** Proof of Stake (planned for v0.3+) binds executor honesty to capital at risk. Fraud is economically irrational.
- **Replayability and auditability.** Every receipt, every verification decision, and every settlement is recorded in the block history and can be replayed from genesis.

---

## 2. What Mbongo IS NOT

- **Not a cloud GPU network.** Mbongo does not schedule, route, or manage GPU hardware. Execution happens off-chain on infrastructure the executor controls.
- **Not a training platform.** Model training is out of scope. Mbongo verifies inference results, not training runs.
- **Not a generic L1.** Mbongo does not compete with Ethereum, Solana, or general-purpose chains. It has no smart contract VM and no plans to add one.
- **Not an AI marketplace.** Mbongo does not match buyers with sellers. It verifies that work was done correctly and settles payment.
- **Not a DeFi protocol.** Mbongo is infrastructure that DeFi protocols consume, not a DeFi product itself.

Mbongo is infrastructure. It provides a single primitive — verified inference — and does it well.

---

## 3. v1 Scope (Verification Layer)

**Execution model:**

1. AI inference runs off-chain on the executor's hardware.
2. The executor generates a `ComputeReceipt` containing the output hash, execution metadata, and a cryptographic proof blob.
3. The receipt is submitted on-chain as a transaction.
4. The chain verifies the receipt deterministically using the rules defined in the protocol.
5. On success, settlement occurs: the submitter pays, the executor is rewarded, and the receipt is finalized.

**What v1 does not include:**

- No Proof of Useful Work. Validators do not execute AI workloads.
- No on-chain model execution. Models never run inside the chain runtime.
- No heavy compute inside validators. Verification is lightweight and deterministic.

**v1 deliverables:**

- Receipt structure and SCALE encoding (see [COMPUTE_INTERFACE_v0.1.md](specs/COMPUTE_INTERFACE_v0.1.md)).
- Deterministic verification rules for receipt acceptance and rejection.
- Economic fee model for task submission and executor compensation.
- SDK for submitting tasks and retrieving receipts programmatically.
- Explorer support for visualizing compute task lifecycle.

---

## 4. v2+ Evolution

| Version | Milestone | Scope |
|---------|-----------|-------|
| **v0.3** | Minimal PoS security | Stake-weighted validator set. Slashing for equivocation. No compute integration yet. |
| **v0.4** | Receipt standardization | Canonical receipt format finalized. Challenge mechanism for disputing fraudulent receipts. |
| **v1.0** | Verified inference primitive | Receipt verification live on mainnet. SDK stable. Adopted by initial DeFi and oracle integrators. |
| **v2+** | Optional on-chain execution | PoUW as an opt-in extension. Validators may execute lightweight inference if staked and hardware-attested. |

On-chain execution is an optional future expansion. It is not a v1 goal. The verification layer must be proven, adopted, and economically stable before execution is considered.

---

## 5. Target Initial Market

First integrations where verified inference creates immediate value:

- **AI-powered DeFi oracles.** Price feeds, risk scores, and sentiment analysis where the consuming protocol needs cryptographic proof that the inference was performed correctly.
- **On-chain risk engines.** Credit scoring, collateral valuation, and liquidation triggers that must be auditable and reproducible.
- **Parametric insurance.** Claim adjudication backed by verifiable AI assessment of real-world data.
- **Governance systems.** DAOs that use AI analysis for proposal evaluation and need proof that the analysis was not tampered with.

**Positioning:** Mbongo is the verified inference layer for decisions that move capital. If the output of an AI model affects money, governance, or automation, that output should be verified on Mbongo.

---

## 6. Long-Term Ambition

A neutral, global verification layer for AI decisions that affect capital, governance, and automation. Any AI model, any executor, any consumer — one chain that settles whether the inference was honest.

The end state is not a compute marketplace. It is a trust primitive: a single, credible answer to the question "was this AI output computed correctly?" that any protocol, enterprise, or institution can rely on without trusting the executor.

---

## 7. Long-Term Vision (3-10 Years)

### The Settlement Layer for Verifiable AI-Driven Capital Allocation

Mbongo's ultimate ambition is not just to verify AI computations.
It is to become the *global settlement layer* for any AI-driven decision that affects capital.

### Target Domains

| Domain | Use Case |
|--------|----------|
| *Finance* | Verified trading signals, portfolio rebalancing, market predictions |
| *Insurance* | Automated claims assessment, risk pricing, fraud detection |
| *Credit* | AI-based credit scoring with auditable decisions |
| *Risk Engines* | Real-time risk assessment for DeFi and TradFi |
| *DAO Governance* | Verifiable AI-assisted proposal analysis and voting recommendations |

### What This Means

Every AI decision that moves capital will need:

- *Auditability* — Who made the decision? What inputs were used?
- *Verifiability* — Can the computation be independently verified?
- *Accountability* — Is there economic stake behind the decision?

Mbongo provides all three.

### The Path

| Phase | Focus |
|-------|-------|
| v1 (Now) | Verification layer foundation |
| v2 | PoUW + zkML research partnerships |
| v3 | Native zkML proofs for inference |
| v4+ | Settlement primitive for global AI-capital infrastructure |

This is not a 12-month goal.
This is a decade-long mission.
