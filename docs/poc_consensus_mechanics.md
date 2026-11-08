# Mbongo Chain — PoC Consensus Mechanics

> **HISTORICAL, NON-NORMATIVE.** Banner added 2026-11-08; the text below it is unchanged.
>
> - **Nothing here is implemented.** The runtime has no consensus protocol. One node,
>   started with `--producer`, builds every block, and blocks carry no producer
>   signature. `crates/mbongo-consensus` is an empty stub. There is no PoX, PoS, PoUW,
>   PoC or AIDA, and no VRF leader election, validator set, staking, slashing, finality
>   mechanism or guardian node.
> - **Status lines and checkmarks below are historical.** "Canonical", "Final
>   Specification" or a feature marked as done does not describe the running system.
> - **No parameter here is adopted:** not the block or slot time, epoch length, quorum,
>   finality depth, validator count, stake minimum, slashing rate or unbonding period.
> - **PoUW is obsolete.** Validators executing workloads, and PoUW weights or rewards, are
>   withdrawn by [`VISION_v2.md`](VISION_v2.md) P1, and consensus outcomes must be
>   deterministic (P3).
> - **What is current:** the authority map, [`INDEX.md`](INDEX.md), and the protocol
>   lock, [`specs/PROTOCOL_LOCK_v0.4.md`](specs/PROTOCOL_LOCK_v0.4.md). Any change to
>   block production or validation needs an accepted RFC under [`RFC_PROCESS.md`](RFC_PROCESS.md).

> **Document Version:** 1.0.0  
> **Last Updated:** November 2025  
> **Status:** Final Specification

---

## Table of Contents

1. [Overview](#1-overview)
2. [Goals of PoC](#2-goals-of-poc)
3. [Compute Units (CU)](#3-compute-units-cu)
4. [Simple Compute Score](#4-simple-compute-score)
5. [Reliability Score](#5-reliability-score)
6. [Proof Validity Score](#6-proof-validity-score)
7. [Decay Function](#7-decay-function)
8. [Full PoC Score](#8-full-poc-score)
9. [Influence on Consensus](#9-influence-on-consensus)
10. [Probability of Leader Selection](#10-probability-of-leader-selection)
11. [Anti-Fraud & Anti-Sybil](#11-anti-fraud--anti-sybil)
12. [Verifiable Compute (Advanced)](#12-verifiable-compute-advanced)
13. [Trusted vs Non-Trusted Compute](#13-trusted-vs-non-trusted-compute)
14. [Security Attacks & Mitigations](#14-security-attacks--mitigations)
15. [Economics & Rewards](#15-economics--rewards)
16. [Simple vs Advanced Summary Table](#16-simple-vs-advanced-summary-table)
17. [Conclusion](#17-conclusion)

---

## 1. Overview

### The PoX Hybrid Consensus Model

Mbongo Chain implements **PoX** (Proof-of-Everything), a revolutionary hybrid consensus mechanism combining three complementary proof systems:

| Component | Role | Primary Function |
|-----------|------|------------------|
| **PoS** (Proof of Stake) | Economic Security | Validator selection, finality |
| **PoUW** (Proof of Useful Work) | Productive Computation | Real-world task execution |
| **PoC** (Proof of Compute) | Merit Accumulation | Compute contribution scoring |

```
┌─────────────────────────────────────────────────────────────────────┐
│                      PoX CONSENSUS ARCHITECTURE                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    CONSENSUS LAYER                           │   │
│  │                                                               │   │
│  │    ┌──────────┐    ┌──────────┐    ┌──────────┐             │   │
│  │    │   PoS    │    │  PoUW    │    │   PoC    │             │   │
│  │    │  Stake   │ +  │  Tasks   │ +  │  Score   │             │   │
│  │    │  Weight  │    │Execution │    │  Merit   │             │   │
│  │    └────┬─────┘    └────┬─────┘    └────┬─────┘             │   │
│  │         │               │               │                    │   │
│  │         └───────────────┼───────────────┘                    │   │
│  │                         │                                    │   │
│  │                         ▼                                    │   │
│  │              ┌─────────────────────┐                        │   │
│  │              │   TOTAL WEIGHT      │                        │   │
│  │              │  (Leader Selection) │                        │   │
│  │              └─────────────────────┘                        │   │
│  │                                                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### What is Proof-of-Compute (PoC)?

**Proof-of-Compute (PoC)** is a merit-based scoring system that tracks and validates computational contributions to the network. Unlike simple economic incentives that only reward task completion, PoC creates a **persistent reputation layer** that:

1. **Accumulates Merit** — Builds a verifiable history of compute contributions
2. **Influences Consensus** — Provides additional weight in leader selection
3. **Rewards Reliability** — Values consistent, honest participation
4. **Prevents Gaming** — Uses cryptographic proofs to verify actual work

### Why PoC is Crucial

| Challenge | How PoC Addresses It |
|-----------|---------------------|
| Pure PoS plutocracy | Compute merit supplements stake weight |
| Compute fraud | Cryptographic proof verification |
| Sybil attacks | Accumulated reputation is costly to fake |
| GPU hoarding | Square-root dampening prevents domination |
| Short-term exploitation | Decay function rewards sustained participation |

### PoC vs Simple Economic Incentives

| Aspect | Simple Incentives | PoC System |
|--------|-------------------|------------|
| **Persistence** | Per-task only | Cumulative score |
| **Consensus Role** | None | Influences leader selection |
| **Fraud Detection** | Post-hoc | Real-time verification |
| **Reputation** | Not tracked | Central to scoring |
| **Sybil Resistance** | Low | High (proof accumulation) |

---

## 2. Goals of PoC

### Primary Objectives

```
┌─────────────────────────────────────────────────────────────────────┐
│                      PoC DESIGN GOALS                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. REWARD VERIFIED COMPUTE                                        │
│     └─ Only cryptographically proven work contributes to score     │
│                                                                     │
│  2. INFLUENCE LEADER SELECTION                                     │
│     └─ Compute contributors gain consensus participation rights    │
│                                                                     │
│  3. PREVENT GPU DOMINATION                                         │
│     └─ Square-root dampening limits large provider advantage       │
│                                                                     │
│  4. CRYPTOECONOMIC SECURITY                                        │
│     └─ Economic incentives aligned with honest behavior            │
│                                                                     │
│  5. SYBIL RESISTANCE                                               │
│     └─ Accumulated proof history is expensive to replicate         │
│                                                                     │
│  6. SUSTAINABLE PARTICIPATION                                      │
│     └─ Decay function rewards continuous contribution              │
│                                                                     │
│  7. FRAUD PREVENTION                                               │
│     └─ Multi-layer verification detects and penalizes cheating     │
│                                                                     │
│  8. FAIR DISTRIBUTION                                              │
│     └─ Balances stake-holders and compute-providers                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Detailed Goal Specifications

| Goal | Description | Mechanism |
|------|-------------|-----------|
| **Verified Compute Rewards** | Only proven work earns PoC score | TEE attestation, ZK proofs, multi-replication |
| **Consensus Influence** | Compute providers participate in block production | `total_weight = stake_weight + sqrt(poc_score)` |
| **Anti-Domination** | Large GPU farms cannot monopolize consensus | Square-root function limits marginal returns |
| **Cryptoeconomic Security** | Attack cost exceeds potential gain | Slashing, stake requirements, proof bonds |
| **Sybil Resistance** | Multiple fake identities provide no advantage | Cumulative proof verification, hardware attestation |
| **Sustainable Participation** | Long-term honest behavior rewarded | Decay function, reliability scoring |
| **Fraud Prevention** | Fake/invalid compute detected and penalized | Multi-verifier consensus, slashing |
| **Fair Distribution** | Both capital and compute contributors benefit | 50/50 PoS/PoUW reward split |

---

## 3. Compute Units (CU)

### Definition

**Compute Units (CU)** are the standardized measurement of computational work performed on the Mbongo network. CUs provide a hardware-agnostic metric for comparing diverse workloads.

### Measurement Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                    COMPUTE UNIT CALCULATION                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  CU = f(FLOPS, VRAM, BatchSize, JobDifficulty, Category)           │
│                                                                     │
│  Where:                                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  FLOPS_normalized = actual_FLOPS / reference_FLOPS          │   │
│  │  VRAM_factor = log2(vram_gb) / log2(reference_vram)         │   │
│  │  Batch_factor = batch_size / reference_batch                │   │
│  │  Difficulty_multiplier = job_difficulty_coefficient         │   │
│  │  Category_weight = category_specific_multiplier             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Base Formula:                                                      │
│  CU = (FLOPS_normalized × VRAM_factor × Batch_factor)              │
│       × Difficulty_multiplier × Category_weight × time_seconds     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Reference Values

| Parameter | Reference Value | Unit |
|-----------|-----------------|------|
| **Reference FLOPS** | 10 TFLOPS | FP32 |
| **Reference VRAM** | 8 GB | Bytes |
| **Reference Batch** | 32 | Items |
| **Base Time Unit** | 1 second | Time |

### Job Categories

| Category | Multiplier | Typical CU/Hour | Description |
|----------|------------|-----------------|-------------|
| **AI Inference** | 1.0× | 100-500 | LLM, image classification |
| **ML Training** | 1.5× | 500-5,000 | Neural network training |
| **3D Rendering** | 1.2× | 200-1,000 | Ray tracing, animation |
| **Scientific HPC** | 1.3× | 300-2,000 | Simulations, modeling |
| **Cryptography** | 1.4× | 400-3,000 | ZK proofs, encryption |

### CU Calculation Examples

```
Example 1: AI Inference on RTX 4090
─────────────────────────────────────
FLOPS: 82.58 TFLOPS → normalized = 82.58/10 = 8.258
VRAM: 24 GB → factor = log2(24)/log2(8) = 1.53
Batch: 64 → factor = 64/32 = 2.0
Difficulty: Standard → multiplier = 1.0
Category: AI Inference → weight = 1.0
Time: 60 seconds

CU = (8.258 × 1.53 × 2.0) × 1.0 × 1.0 × 60 = 1,515 CU

Example 2: ML Training on H100
─────────────────────────────────────
FLOPS: 1,979 TFLOPS (Tensor) → normalized ≈ 50 (adjusted)
VRAM: 80 GB → factor = log2(80)/log2(8) = 2.10
Batch: 512 → factor = 512/32 = 16.0
Difficulty: Complex → multiplier = 1.5
Category: ML Training → weight = 1.5
Time: 3600 seconds (1 hour)

CU = (50 × 2.10 × 16.0) × 1.5 × 1.5 × 3600 = 13,608,000 CU
```

---

## 4. Simple Compute Score

### Formula

The **Simple Compute Score** is the basic calculation of compute contribution value:

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│              compute_score = compute_units × job_value              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Description | Range |
|-----------|-------------|-------|
| `compute_units` | CUs from completed job | 0 → ∞ |
| `job_value` | Economic value coefficient | 0.1 → 10.0 |

### Job Value Determination

| Factor | Impact on job_value |
|--------|---------------------|
| Task priority | Higher priority = higher value |
| Market demand | Scarce compute = higher value |
| Task complexity | More complex = higher value |
| Verification cost | Harder to verify = higher value |

### Example Calculations

```
Job A: Standard AI Inference
  compute_units = 1,000 CU
  job_value = 1.0
  compute_score = 1,000 × 1.0 = 1,000

Job B: Priority ML Training
  compute_units = 50,000 CU
  job_value = 2.5
  compute_score = 50,000 × 2.5 = 125,000

Job C: Complex ZK Proof Generation
  compute_units = 10,000 CU
  job_value = 3.0
  compute_score = 10,000 × 3.0 = 30,000
```

---

## 5. Reliability Score

### Formula

The **Reliability Score** measures a provider's consistency and success rate:

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│         reliability_score = successful_jobs / total_jobs            │
│                                                                     │
│         Clamped to range: [0.0, 1.0]                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Success Criteria

| Outcome | Classification | Impact |
|---------|----------------|--------|
| Task completed + verified | ✅ Successful | +1 to numerator |
| Task completed + failed verification | ❌ Failed | +0 to numerator |
| Task timeout | ❌ Failed | +0 to numerator |
| Task abandoned | ❌ Failed | +0 to numerator |
| Task rejected (invalid proof) | ❌ Failed | +0 to numerator |

### Minimum Job Threshold

To prevent gaming with few high-success jobs:

```
effective_reliability = reliability_score × min(1.0, total_jobs / MIN_JOBS)

Where: MIN_JOBS = 100
```

### Reliability Tiers

| Score Range | Tier | Multiplier Effect |
|-------------|------|-------------------|
| 0.95 - 1.00 | Excellent | 1.0× (full credit) |
| 0.85 - 0.94 | Good | 0.9× |
| 0.70 - 0.84 | Fair | 0.7× |
| 0.50 - 0.69 | Poor | 0.5× |
| < 0.50 | Unreliable | 0.0× (suspended) |

---

## 6. Proof Validity Score

### Formula

The **Proof Validity Score** measures the cryptographic correctness of submitted proofs:

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│      proof_validity_score = verified_proofs / submitted_proofs      │
│                                                                     │
│      Clamped to range: [0.0, 1.0]                                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Proof Verification Outcomes

| Outcome | verified_proofs | submitted_proofs |
|---------|-----------------|------------------|
| Proof valid | +1 | +1 |
| Proof invalid | +0 | +1 |
| Proof missing | +0 | +1 |
| Proof malformed | +0 | +1 |

### Validity Implications

| Score | Interpretation | Consequence |
|-------|----------------|-------------|
| 1.00 | Perfect validity | Maximum PoC contribution |
| 0.95+ | Excellent | Full participation |
| 0.80-0.94 | Good | Minor score reduction |
| 0.50-0.79 | Suspicious | Investigation triggered |
| < 0.50 | Fraud likely | Provider suspended, slashing |

### Fraud Detection Threshold

```
IF proof_validity_score < 0.80 AND submitted_proofs > 50:
    TRIGGER fraud_investigation()
    
IF proof_validity_score < 0.50:
    EXECUTE provider_suspension()
    INITIATE slashing_procedure()
```

---

## 7. Decay Function

### Purpose

The **Decay Function** ensures that compute reputation reflects recent activity, preventing:
- Accumulation of stale reputation
- "Stake and forget" behavior
- Dormant account exploitation

### Formula

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│              decay = exp(-k × inactivity_hours)                     │
│                                                                     │
│              Where: k = 0.001 (decay constant)                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Decay Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| **k** | 0.001 | Base decay rate |
| **Half-life** | ~693 hours (~29 days) | Time for score to halve |
| **Min decay** | 0.01 | Floor to prevent total loss |

### Decay Over Time

| Inactivity Period | Decay Factor | Remaining Score |
|-------------------|--------------|-----------------|
| 0 hours (active) | 1.000 | 100% |
| 24 hours (1 day) | 0.976 | 97.6% |
| 168 hours (1 week) | 0.846 | 84.6% |
| 720 hours (30 days) | 0.487 | 48.7% |
| 1440 hours (60 days) | 0.237 | 23.7% |
| 2160 hours (90 days) | 0.115 | 11.5% |

### Visual Decay Curve

```
Score Retention
    │
100%┤████████─────────────────────────────────────
    │        ████████
 75%┤                ████████
    │                        ████████
 50%┤                                ████████
    │                                        ████
 25%┤                                            ████
    │                                                ████
  0%┼────────┬────────┬────────┬────────┬────────┬────▶ Days
    0        7       14       21       28       35
```

---

## 8. Full PoC Score

### Complete Formula

The **Full PoC Score** combines all components into a comprehensive compute reputation metric:

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│  poc_score = (compute_units × job_value × reliability_score         │
│               × proof_validity_score) × decay                       │
│                                                                     │
│  Expanded:                                                          │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                                                               │   │
│  │  poc_score = Σ(CU_i × JV_i) × (S_jobs/T_jobs)                │   │
│  │              × (V_proofs/S_proofs) × exp(-k × inactivity)    │   │
│  │                                                               │   │
│  │  Where:                                                       │   │
│  │    CU_i = Compute units for job i                            │   │
│  │    JV_i = Job value for job i                                │   │
│  │    S_jobs = Successful jobs                                  │   │
│  │    T_jobs = Total jobs                                       │   │
│  │    V_proofs = Verified proofs                                │   │
│  │    S_proofs = Submitted proofs                               │   │
│  │    k = 0.001 (decay constant)                                │   │
│  │    inactivity = hours since last verified job                │   │
│  │                                                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Component Ranges

| Component | Range | Impact |
|-----------|-------|--------|
| compute_units × job_value | 0 → ∞ | Base contribution |
| reliability_score | 0.0 → 1.0 | Quality multiplier |
| proof_validity_score | 0.0 → 1.0 | Integrity multiplier |
| decay | 0.01 → 1.0 | Recency multiplier |

### Example Calculation

```
Provider: GPU-Cluster-Alpha

Accumulated compute_score: 1,000,000 (from jobs)
Successful jobs: 950
Total jobs: 1,000
Verified proofs: 980
Submitted proofs: 1,000
Last job: 48 hours ago

Calculations:
  reliability_score = 950/1000 = 0.95
  proof_validity_score = 980/1000 = 0.98
  decay = exp(-0.001 × 48) = 0.953

Final PoC Score:
  poc_score = 1,000,000 × 0.95 × 0.98 × 0.953
  poc_score = 886,367
```

---

## 9. Influence on Consensus

### Total Weight Formula

PoC score influences consensus through the **Total Weight** calculation:

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│          total_weight = stake_weight + sqrt(poc_score)              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Why Square Root?

The square root function serves critical purposes:

| Purpose | Explanation |
|---------|-------------|
| **Diminishing Returns** | Large providers gain less marginal influence |
| **PoS Dominance Guarantee** | Stake remains primary consensus factor |
| **Anti-Monopoly** | Prevents GPU farms from dominating consensus |
| **Fair Participation** | Smaller providers remain competitive |

### Mathematical Properties

```
Effect of sqrt() on PoC influence:
─────────────────────────────────────────────────────────
poc_score    | sqrt(poc_score) | Marginal gain per 100K
─────────────────────────────────────────────────────────
100,000      | 316             | —
200,000      | 447             | 131 (+41%)
500,000      | 707             | 260 (+37% per 300K)
1,000,000    | 1,000           | 293 (+29% per 500K)
10,000,000   | 3,162           | 2,162 (+216% per 9M)
100,000,000  | 10,000          | 6,838 (+68% per 90M)
─────────────────────────────────────────────────────────

Observation: 100× more compute only yields 10× more influence
```

### PoS Dominance Guarantee

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CONSENSUS WEIGHT BALANCE                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  For stake_weight = 32,000 MBO (minimum validator stake):          │
│                                                                     │
│  To match stake weight with PoC alone:                             │
│    sqrt(poc_score) = 32,000                                        │
│    poc_score = 32,000² = 1,024,000,000                             │
│                                                                     │
│  This requires approximately:                                       │
│    • 1 billion compute units at job_value=1.0                      │
│    • Perfect reliability (1.0)                                      │
│    • Perfect proof validity (1.0)                                   │
│    • Zero inactivity decay                                          │
│                                                                     │
│  Conclusion: Stake remains dominant; PoC provides meaningful        │
│              but bounded supplemental influence                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### PoC Empowerment Effect

| Scenario | stake_weight | poc_score | sqrt(poc) | total_weight | PoC % |
|----------|--------------|-----------|-----------|--------------|-------|
| Stake only | 32,000 | 0 | 0 | 32,000 | 0% |
| Small provider | 32,000 | 100,000 | 316 | 32,316 | 1.0% |
| Medium provider | 32,000 | 1,000,000 | 1,000 | 33,000 | 3.0% |
| Large provider | 32,000 | 10,000,000 | 3,162 | 35,162 | 9.0% |
| Mega provider | 32,000 | 100,000,000 | 10,000 | 42,000 | 23.8% |

---

## 10. Probability of Leader Selection

### Block Production Probability

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│    block_probability = total_weight_i / Σ(total_weight_all)         │
│                                                                     │
│    Where:                                                           │
│      total_weight_i = stake_weight_i + sqrt(poc_score_i)           │
│      Σ(total_weight_all) = sum of all validator weights            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Selection Process

```
┌─────────────────────────────────────────────────────────────────────┐
│                    LEADER SELECTION FLOW                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. WEIGHT CALCULATION (per epoch)                                 │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ For each validator v:                                    │    │
│     │   poc_score_v = calculate_poc_score(v)                  │    │
│     │   total_weight_v = stake_v + sqrt(poc_score_v)          │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  2. PROBABILITY ASSIGNMENT                                         │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ total_network_weight = Σ(total_weight_all_validators)   │    │
│     │ For each validator v:                                    │    │
│     │   prob_v = total_weight_v / total_network_weight        │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
│  3. VERIFIABLE RANDOM SELECTION                                    │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │ random_seed = VRF(prev_block_hash, slot_number)         │    │
│     │ leader = weighted_random_select(validators, prob)       │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Example Network Scenario

```
Network State:
  Total validators: 100
  Total stake: 5,000,000 MBO
  Total PoC score: 500,000,000

Validator A:
  stake = 100,000 MBO
  poc_score = 10,000,000
  total_weight = 100,000 + sqrt(10,000,000) = 100,000 + 3,162 = 103,162

Network total_weight (simplified):
  Σ = 5,000,000 + sqrt(500,000,000) ≈ 5,000,000 + 22,361 = 5,022,361

Validator A probability:
  P(A) = 103,162 / 5,022,361 = 2.05%

Without PoC contribution:
  P(A) = 100,000 / 5,000,000 = 2.00%

PoC boost: +0.05% (2.5% relative increase)
```

---

## 11. Anti-Fraud & Anti-Sybil

### Protection Mechanisms

```
┌─────────────────────────────────────────────────────────────────────┐
│                    ANTI-FRAUD PROTECTIONS                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. FAKE JOBS PREVENTION                                           │
│     • Task submission requires stake deposit                       │
│     • Jobs must originate from verified contracts/users            │
│     • Economic cost for task creation prevents spam                │
│                                                                     │
│  2. GPU SPOOFING DETECTION                                         │
│     • Hardware attestation via TEE                                 │
│     • Benchmark verification against claimed specs                 │
│     • Performance anomaly detection                                │
│                                                                     │
│  3. REPLAY ATTACK PREVENTION                                       │
│     • Unique job IDs with timestamps                               │
│     • Nonce requirements in proofs                                 │
│     • Chain of custody verification                                │
│                                                                     │
│  4. DOUBLE-COMPUTE PREVENTION                                      │
│     • Job assignment is exclusive                                  │
│     • Receipt submission is atomic                                 │
│     • Duplicate detection in verification layer                    │
│                                                                     │
│  5. COMPUTE INFLATION PREVENTION                                   │
│     • CU calculations are deterministic                            │
│     • Third-party verification of compute metrics                  │
│     • Statistical anomaly detection for outliers                   │
│                                                                     │
│  6. ORACLE FRAUD PREVENTION                                        │
│     • Multi-oracle consensus for external data                     │
│     • Stake-weighted oracle voting                                 │
│     • Economic penalties for false oracle reports                  │
│                                                                     │
│  7. TEE COLLUSION PREVENTION                                       │
│     • Rotating TEE attestation keys                                │
│     • Multi-vendor TEE diversity requirement                       │
│     • Remote attestation verification                              │
│                                                                     │
│  8. ZK VERIFICATION                                                │
│     • Zero-knowledge proofs for compute correctness                │
│     • SNARK/STARK proof verification on-chain                      │
│     • Succinct verification with minimal overhead                  │
│                                                                     │
│  9. COMPUTE SPAM LIMITS                                            │
│     • Rate limiting per provider                                   │
│     • Minimum task value thresholds                                │
│     • Reputation-gated task access                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Sybil Attack Resistance

| Attack Vector | Defense Mechanism |
|---------------|-------------------|
| Multiple fake identities | Hardware attestation required |
| Distributed stake | Minimum stake per validator |
| Shared compute | TEE binds proof to specific hardware |
| Identity splitting | Accumulated reputation is non-transferable |

---

## 12. Verifiable Compute (Advanced)

### Verification Methods

| Method | Security Level | Performance | Use Case |
|--------|---------------|-------------|----------|
| **TEE** | High | Fast | Real-time inference |
| **ZK-ML** | Highest | Slow | Critical computations |
| **Multi-Replication** | Medium | Medium | General tasks |

### TEE Verification

**Supported Trusted Execution Environments:**

| Platform | TEE Technology | Support Level |
|----------|---------------|---------------|
| Intel | SGX, TDX | Full |
| AMD | SEV-SNP | Full |
| ARM | TrustZone | Partial |
| Apple | Secure Enclave | Partial |
| NVIDIA | H100 Confidential Computing | Full |

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TEE VERIFICATION FLOW                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Provider initializes TEE enclave                               │
│  2. TEE generates attestation report                               │
│  3. Task code loaded into enclave                                  │
│  4. Computation executes in isolated environment                   │
│  5. TEE signs result with enclave key                              │
│  6. Attestation + result submitted to network                      │
│  7. Verifiers check attestation against known-good measurements    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### ZK-ML Verification

```
┌─────────────────────────────────────────────────────────────────────┐
│                    ZK-ML PROOF GENERATION                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  For ML inference f(x) = y:                                        │
│                                                                     │
│  1. Convert model to arithmetic circuit                            │
│  2. Execute inference with ZK witness generation                   │
│  3. Generate SNARK/STARK proof π                                   │
│  4. Submit (y, π) to verifiers                                     │
│  5. Verifiers check: Verify(π, public_inputs) = true               │
│                                                                     │
│  Proof guarantees:                                                  │
│    • Correct model was used                                        │
│    • Correct input was processed                                   │
│    • Output is mathematically correct                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Multi-Replication Verification

For non-TEE, non-ZK workloads:

1. Task assigned to N providers (typically N=3)
2. Each provider executes independently
3. Results compared for consensus
4. Majority result accepted
5. Outliers flagged for investigation

### Trust Tiers & PoC Score Scaling

| Trust Tier | Verification Method | PoC Multiplier |
|------------|---------------------|----------------|
| **Tier 1** | ZK-ML proof | 1.5× |
| **Tier 2** | TEE attestation | 1.25× |
| **Tier 3** | Multi-replication (3+) | 1.0× |
| **Tier 4** | Single execution | 0.5× |
| **Tier 5** | Unverified | 0.0× |

---

## 13. Trusted vs Non-Trusted Compute

### Comparison Table

| Aspect | Trusted (TEE/ZK) | Non-Trusted (Replication) |
|--------|------------------|---------------------------|
| **Security Level** | Cryptographic | Statistical |
| **Verification Cost** | Low (single proof) | High (N× execution) |
| **Latency** | Lower | Higher |
| **PoC Multiplier** | 1.25-1.5× | 0.5-1.0× |
| **Hardware Requirement** | Specialized | Any |
| **Fraud Detection** | Immediate | Probabilistic |
| **Collusion Resistance** | High | Medium |
| **Scalability** | Excellent | Limited |

### Trust Mode Selection

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TRUST MODE DECISION TREE                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                    ┌─────────────────┐                              │
│                    │ Task Submitted  │                              │
│                    └────────┬────────┘                              │
│                             │                                       │
│              ┌──────────────▼──────────────┐                       │
│              │ ZK-ML circuit available?    │                       │
│              └──────────────┬──────────────┘                       │
│                      Yes    │    No                                │
│              ┌──────────────┴──────────────┐                       │
│              │                             │                        │
│              ▼                             ▼                        │
│     ┌─────────────┐           ┌─────────────────────┐              │
│     │ ZK-ML Mode  │           │ TEE available?      │              │
│     │ (Tier 1)    │           └──────────┬──────────┘              │
│     └─────────────┘                Yes   │   No                    │
│                               ┌──────────┴──────────┐              │
│                               │                     │               │
│                               ▼                     ▼               │
│                      ┌─────────────┐       ┌─────────────┐         │
│                      │ TEE Mode    │       │ Replication │         │
│                      │ (Tier 2)    │       │ (Tier 3)    │         │
│                      └─────────────┘       └─────────────┘         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 14. Security Attacks & Mitigations

### Attack Catalog

| Attack | Description | Mitigation |
|--------|-------------|------------|
| **Fake Job Injection** | Submit non-existent tasks for rewards | Stake deposit, verified origins |
| **GPU Spoofing** | Claim false hardware specs | TEE attestation, benchmarks |
| **Replay Attack** | Resubmit old proofs | Unique nonces, timestamps |
| **Double-Compute Claim** | Claim same job twice | Atomic receipt submission |
| **Compute Inflation** | Artificially inflate CUs | Deterministic calculation, audits |
| **Oracle Manipulation** | Corrupt external data | Multi-oracle consensus |
| **TEE Key Extraction** | Compromise enclave keys | Key rotation, remote attestation |
| **Sybil Attack** | Multiple fake identities | Hardware binding, stake requirements |
| **Grinding Attack** | Manipulate random selection | VRF-based randomness |
| **Long-Range Attack** | Rewrite history with old keys | Checkpoints, finality |
| **Collusion** | Providers conspire on results | Multi-vendor diversity, random assignment |
| **Resource Exhaustion** | Spam network with tasks | Rate limits, minimum fees |

### Slashing Conditions

| Violation | Slashing Amount | Detection Method |
|-----------|-----------------|------------------|
| Invalid proof submission | 1,000 MBO | Proof verification failure |
| Repeated timeouts (>10) | 500 MBO | Performance monitoring |
| GPU spoofing | 5,000 MBO | Hardware attestation mismatch |
| Double-signing | 10,000 MBO | Duplicate proof detection |
| Collusion (proven) | Full stake | Multi-party investigation |

---

## 15. Economics & Rewards

### Reward Distribution Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                    BLOCK REWARD DISTRIBUTION                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Total Block Reward: 100 MBO (example)                             │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                                                               │   │
│  │  PoS Validators: 50 MBO (50%)                                │   │
│  │  ├── Proposer bonus: 5 MBO                                   │   │
│  │  └── Attesters: 45 MBO (proportional to attestations)        │   │
│  │                                                               │   │
│  │  PoUW Compute Providers: 50 MBO (50%)                        │   │
│  │  ├── Direct task rewards: 40 MBO                             │   │
│  │  └── PoC score bonus: 10 MBO                                 │   │
│  │                                                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### PoC-Adjusted Rewards

Provider rewards are scaled by PoC score:

```
provider_reward = base_reward × (1 + poc_bonus_factor)

Where:
  poc_bonus_factor = min(0.25, poc_score / BONUS_THRESHOLD)
  BONUS_THRESHOLD = 1,000,000
```

### Direct Compute Rewards

| Metric | Reward Basis |
|--------|--------------|
| Compute Units completed | CU × base_rate |
| Job priority | Priority multiplier |
| Verification tier | Trust multiplier |
| PoC score rank | Bonus allocation |

### Slashing for Compute Fraud

| Offense | Penalty |
|---------|---------|
| Invalid proof (1st) | Warning + 100 MBO |
| Invalid proof (2nd) | 500 MBO + suspension |
| Invalid proof (3rd) | 1,000 MBO + ban |
| Intentional fraud | Full stake |

### Reward Adjustments Based on PoC Score

| PoC Score Percentile | Reward Multiplier |
|---------------------|-------------------|
| Top 1% | 1.25× |
| Top 10% | 1.15× |
| Top 25% | 1.10× |
| Top 50% | 1.05× |
| Bottom 50% | 1.00× |

---

## 16. Simple vs Advanced Summary Table

| Feature | Simple Mode | Advanced Mode |
|---------|-------------|---------------|
| **PoC Formula** | `CU × job_value` | Full formula with all factors |
| **Verification** | Multi-replication | TEE + ZK-ML |
| **Trust Tier** | Tier 3-4 | Tier 1-2 |
| **PoC Multiplier** | 0.5-1.0× | 1.25-1.5× |
| **Fraud Detection** | Statistical | Cryptographic |
| **Hardware Requirement** | Standard GPU | TEE-enabled hardware |
| **Setup Complexity** | Low | High |
| **Latency** | Higher | Lower |
| **Reward Potential** | Standard | Enhanced |
| **Sybil Resistance** | Medium | High |
| **Collusion Resistance** | Medium | High |
| **Recommended For** | New providers | Enterprise providers |

---

## 17. Conclusion

### Why PoC Makes Mbongo Chain Unique

**Proof-of-Compute (PoC)** represents a fundamental innovation in blockchain consensus design. By creating a merit-based scoring system that accumulates verified computational contributions, Mbongo Chain achieves:

1. **True Hybrid Consensus**: PoC bridges the gap between stake-based security (PoS) and compute-based utility (PoUW), creating a unified system where both capital and computational contributions matter.

2. **Sustainable Compute Economics**: Unlike proof-of-work systems that waste energy on meaningless puzzles, PoC rewards real-world useful computation while maintaining blockchain security guarantees.

3. **Anti-Plutocracy Mechanisms**: The square-root dampening function ensures that large GPU farms cannot dominate consensus, preserving decentralization while rewarding legitimate contribution.

4. **Cryptographic Integrity**: Multi-tier verification (TEE, ZK-ML, replication) provides strong guarantees that compute claims are legitimate, making fraud economically irrational.

5. **Dynamic Reputation**: The decay function ensures that reputation reflects recent behavior, preventing stale reputation exploitation and encouraging continuous honest participation.

6. **Fair Value Distribution**: The 50/50 PoS/PoUW reward split, combined with PoC score bonuses, ensures that both validators and compute providers are appropriately compensated.

### The PoX Vision

```
┌─────────────────────────────────────────────────────────────────────┐
│                                                                     │
│           PoS (Security) + PoUW (Utility) + PoC (Merit)            │
│                              =                                      │
│               PoX: The Complete Consensus Solution                  │
│                                                                     │
│  • Economically secure through stake                               │
│  • Productively useful through real computation                    │
│  • Fairly distributed through merit-based scoring                  │
│  • Resistant to centralization through mathematical constraints    │
│  • Verifiably honest through cryptographic proofs                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

Mbongo Chain's PoC consensus mechanics establish a new paradigm where blockchain security, computational utility, and fair reward distribution coexist harmoniously—creating a platform that is not only secure and decentralized but also genuinely useful to the world.

---

*Document maintained by the Mbongo Chain Protocol Team*  
*Last reviewed: November 2025*

