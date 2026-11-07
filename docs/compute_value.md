<!-- Verified against tokenomics.md -->
# Mbongo Chain — Compute Value

> **HISTORICAL, NON-NORMATIVE.** Banner added 2026-11-07; the text below it is unchanged.
>
> - **Nothing here is implemented.** The runtime has no issuance, supply cap, burn, fees,
>   treasury, staking, rewards or vesting.
> - **No parameter here is adopted:** not the 31,536,000 MBO figure, the emission schedule,
>   the block time, the allocation percentages or the reward splits.
> - **PoUW is obsolete.** Validators executing workloads, and PoUW rewards or allocations,
>   are withdrawn by [`VISION_v2.md`](VISION_v2.md) P1. Provider assignment and settlement
>   need their own RFCs (VISION_v2 §7).
> - **Where every monetary question stands:** RFC 0004 §12, where no decision is made. The
>   inconsistencies between these documents are listed in RFC 0004 §13:
>   [`rfcs/0004-mbo-monetary-decision-framework.md`](rfcs/0004-mbo-monetary-decision-framework.md).

> **Document Type:** Compute Economics Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Why Compute Matters (Architecture-Grade)](#2-why-compute-matters-architecture-grade)
3. [Compute as an Economic Asset (PoUW)](#3-compute-as-an-economic-asset-pouw)
4. [Compute Supply & Demand Model](#4-compute-supply--demand-model)
5. [Compute Utility for Users](#5-compute-utility-for-users)
6. [Incentives for GPU Providers](#6-incentives-for-gpu-providers)
7. [Strategic Value (Why This Model Wins)](#7-strategic-value-why-this-model-wins)
8. [Summary](#8-summary)

---

## 1. Purpose of This Document

This document defines the **economic, functional, and strategic value of compute** within Mbongo Chain's Proof-of-Useful-Work (PoUW) model.

**PoUW is a heterogeneous compute layer** that accepts proofs from GPUs, TPUs, CPUs, FPGAs, ASICs, and future accelerator hardware, as long as the computation is deterministic and verifiable on-chain. The architecture is compute-first with initial focus on GPUs, but designed for heterogeneous accelerators.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE AS A NATIVE RESOURCE                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TRADITIONAL BLOCKCHAINS                                                               │
│   ═══════════════════════                                                               │
│                                                                                         │
│   • Computation is a side-effect of transaction validation                             │
│   • No economic value derived from useful work                                         │
│   • GPU power is "wasted" on arbitrary hash puzzles (PoW)                              │
│   • Or entirely absent from consensus (PoS)                                            │
│                                                                                         │
│                                                                                         │
│   MBONGO CHAIN                                                                          │
│   ════════════                                                                          │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   COMPUTE IS A CONSENSUS-LEVEL RESOURCE                                         │  │
│   │   ═════════════════════════════════════                                         │  │
│   │                                                                                 │  │
│   │   • Computation is a FIRST-CLASS ECONOMIC ACTIVITY                             │  │
│   │   • Compute work is USEFUL (AI/ML, rendering, ZK proofs, video, scientific)    │  │
│   │   • Execution is VERIFIABLE (deterministic receipts)                           │  │
│   │   • Results are EMBEDDED IN CONSENSUS (block metadata)                         │  │
│   │   • Providers are INCENTIVIZED (50% of block rewards)                          │  │
│   │                                                                                 │  │
│   │   Compute is not an add-on service—it is native to the protocol.               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.1 What This Document Covers

| Topic | Description |
|-------|-------------|
| **Architecture** | How compute integrates with consensus |
| **Economics** | How providers earn and users pay |
| **Supply/Demand** | Market dynamics for compute resources |
| **User Utility** | What users can do with on-chain compute |
| **Provider Incentives** | Why GPUs join the network |
| **Strategic Value** | Long-term positioning and competitive advantage |

---

## 2. Why Compute Matters (Architecture-Grade)

### 2.1 Deterministic PoUW Verification Model

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DETERMINISTIC PoUW VERIFICATION                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CORE PRINCIPLE                                                                        │
│   ══════════════                                                                        │
│                                                                                         │
│   Every compute operation in Mbongo Chain produces a DETERMINISTIC result.             │
│   Any node can re-execute the same computation and verify the output.                  │
│                                                                                         │
│                                                                                         │
│   VERIFICATION PIPELINE                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│   │   TASK      │    │   GPU       │    │  RECEIPT    │    │  VALIDATOR  │            │
│   │   SUBMITTED │───▶│   EXECUTES  │───▶│  GENERATED  │───▶│  VERIFIES   │            │
│   │             │    │             │    │             │    │             │            │
│   │   • Input   │    │   • Work    │    │   • Hash    │    │   • Match   │            │
│   │   • Params  │    │   • Output  │    │   • Sig     │    │   • Accept  │            │
│   │   • Gas     │    │   • Proof   │    │   • Meta    │    │   • Reward  │            │
│   └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘            │
│                                                                                         │
│                                                                                         │
│   DETERMINISM GUARANTEES                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   1. REPRODUCIBILITY                                                                    │
│      • Same input → same output (always)                                               │
│      • No floating-point non-determinism                                               │
│      • Canonical execution order                                                       │
│                                                                                         │
│   2. VERIFIABILITY                                                                      │
│      • Any node can verify results                                                     │
│      • No trusted third party required                                                 │
│      • Fraud is mathematically detectable                                              │
│                                                                                         │
│   3. FINALITY                                                                           │
│      • Valid receipts become permanent                                                 │
│      • Invalid receipts trigger slashing                                               │
│      • No ambiguity in outcome                                                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Compute Receipts Validated On-Chain

Compute receipts are hardware-agnostic and support proof validation for GPUs, TPUs, CPUs, FPGAs, ASICs, and compatible accelerators.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE RECEIPT STRUCTURE                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   RECEIPT FIELDS                                                                        │
│   ══════════════                                                                        │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   struct ComputeReceipt {                                                       │  │
│   │       task_id:        Hash,           // Unique task identifier                │  │
│   │       provider_id:    PublicKey,      // Compute provider identity             │  │
│   │       input_hash:     Hash,           // Hash of task inputs                   │  │
│   │       output_hash:    Hash,           // Hash of computation result            │  │
│   │       work_units:     u64,            // Measured compute cycles               │  │
│   │       timestamp:      u64,            // Unix timestamp                        │  │
│   │       execution_time: u64,            // Milliseconds to complete              │  │
│   │       attester_sigs:  Vec<Signature>, // Attester endorsements                 │  │
│   │       provider_sig:   Signature,      // Provider's signature                  │  │
│   │   }                                                                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ON-CHAIN VALIDATION                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   Every receipt undergoes:                                                             │
│                                                                                         │
│   ✓ SIGNATURE VERIFICATION                                                             │
│     • Provider signature must match registered key                                     │
│     • Attester signatures must meet threshold                                          │
│                                                                                         │
│   ✓ HASH VERIFICATION                                                                  │
│     • output_hash must match re-execution (sampling)                                   │
│     • input_hash must match submitted task                                             │
│                                                                                         │
│   ✓ IDENTITY VERIFICATION                                                              │
│     • Provider must be registered                                                      │
│     • Provider must not be slashed/jailed                                              │
│                                                                                         │
│   ✓ TIMING VERIFICATION                                                                │
│     • Timestamp within acceptable window                                               │
│     • Execution time reasonable for task                                               │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Integration with Core Systems

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SYSTEM INTEGRATION                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │                           EXECUTION ENGINE                                      │  │
│   │                                  │                                              │  │
│   │                                  ▼                                              │  │
│   │   ┌──────────────────────────────────────────────────────────────────────────┐ │  │
│   │   │                          RUNTIME                                         │ │  │
│   │   │                                                                          │ │  │
│   │   │   • Receives compute requests                                           │ │  │
│   │   │   • Routes to GPU assignment                                            │ │  │
│   │   │   • Processes compute receipts                                          │ │  │
│   │   │   • Updates state with results                                          │ │  │
│   │   │                                                                          │ │  │
│   │   └──────────────────────────────────────────────────────────────────────────┘ │  │
│   │                                  │                                              │  │
│   │                    ┌─────────────┼─────────────┐                               │  │
│   │                    │             │             │                               │  │
│   │                    ▼             ▼             ▼                               │  │
│   │   ┌────────────────────┐ ┌────────────┐ ┌─────────────────┐                   │  │
│   │   │     MEMPOOL        │ │  CONSENSUS │ │    STORAGE      │                   │  │
│   │   │                    │ │            │ │                 │                   │  │
│   │   │  • Task queue      │ │  • Receipt │ │  • Receipt      │                   │  │
│   │   │  • Priority order  │ │    in block│ │    archive      │                   │  │
│   │   │  • Fee validation  │ │  • PoUW    │ │  • Result       │                   │  │
│   │   │                    │ │    scoring │ │    cache        │                   │  │
│   │   └────────────────────┘ └────────────┘ └─────────────────┘                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   INTEGRATION POINTS                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   1. EXECUTION ENGINE                                                                   │
│      • Processes compute transactions                                                  │
│      • Validates receipt format and signatures                                         │
│      • Triggers reward distribution                                                    │
│                                                                                         │
│   2. RUNTIME                                                                            │
│      • Manages compute task state                                                      │
│      • Handles compute provider registration                                           │
│      • Enforces compute gas limits                                                     │
│                                                                                         │
│   3. MEMPOOL                                                                            │
│      • Queues compute tasks by priority                                                │
│      • Validates compute fees                                                          │
│      • Manages task assignment                                                         │
│                                                                                         │
│   4. CONSENSUS                                                                          │
│      • Includes receipts in block proposals                                            │
│      • Calculates PoUW score for blocks                                                │
│      • Validates receipt integrity                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Computation Hashes in Block Metadata

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         BLOCK METADATA STRUCTURE                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   BLOCK HEADER INCLUDES                                                                 │
│   ════════════════════                                                                  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   BlockHeader {                                                                 │  │
│   │       // Standard fields                                                        │  │
│   │       parent_hash:      Hash,                                                  │  │
│   │       height:           u64,                                                   │  │
│   │       timestamp:        u64,                                                   │  │
│   │       transactions_root: Hash,                                                 │  │
│   │       state_root:       Hash,                                                  │  │
│   │                                                                                 │  │
│   │       // PoUW-specific fields                                                  │  │
│   │       compute_receipts_root: Hash,    // Merkle root of all receipts          │  │
│   │       total_work_units:      u64,     // Aggregate compute cycles in block    │  │
│   │       pouw_score:            u64,     // Computed PoUW contribution           │  │
│   │       provider_count:        u32,     // Number of providers rewarded         │  │
│   │   }                                                                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   WHY THIS MATTERS                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   • Compute is ANCHORED to consensus (not external)                                    │
│   • Receipts are IMMUTABLE once in block                                               │
│   • Work proof is VERIFIABLE by any node                                               │
│   • PoUW score AFFECTS block weight in fork choice                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.5 Supported Workloads

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SUPPORTED COMPUTE WORKLOADS                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   AI/ML WORKLOADS                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   • Image classification and generation                                                │
│   • Natural language processing                                                        │
│   • Speech recognition and synthesis                                                   │
│   • Model inference (pre-trained models)                                               │
│   • Training job chunks (federated learning)                                           │
│                                                                                         │
│                                                                                         │
│   HEAVY PARALLEL COMPUTE                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   • Scientific simulations                                                             │
│   • Financial modeling                                                                 │
│   • Video rendering and encoding                                                       │
│   • 3D asset generation                                                                │
│   • Physics simulations                                                                │
│                                                                                         │
│                                                                                         │
│   CRYPTOGRAPHIC OPERATIONS                                                              │
│   ════════════════════════                                                              │
│                                                                                         │
│   • ZK proof generation                                                                │
│   • Hash computations at scale                                                         │
│   • Signature batch verification                                                       │
│   • Encryption/decryption at scale                                                     │
│                                                                                         │
│                                                                                         │
│   DATA PROCESSING                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   • Large dataset transformations                                                      │
│   • Analytics pipelines                                                                │
│   • Search indexing                                                                    │
│   • Graph computations                                                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Compute as an Economic Asset (PoUW)

### 3.1 Provider Submission Flow

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE PROOF SUBMISSION                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PROVIDER WORKFLOW                                                                     │
│   ════════════════                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   STEP 1: REGISTRATION                                                          │  │
│   │   ────────────────────                                                          │  │
│   │   • Provider registers with network                                            │  │
│   │   • Stakes collateral (optional, improves priority)                            │  │
│   │   • Submits hardware specifications                                            │  │
│   │   • Receives provider_id                                                       │  │
│   │                                                                                 │  │
│   │   STEP 2: TASK ASSIGNMENT                                                       │  │
│   │   ───────────────────────                                                       │  │
│   │   • Provider polls for available tasks                                         │  │
│   │   • Network assigns task based on:                                             │  │
│   │     - Provider capacity                                                        │  │
│   │     - Provider reputation                                                      │  │
│   │     - Task requirements                                                        │  │
│   │                                                                                 │  │
│   │   STEP 3: EXECUTION                                                             │  │
│   │   ─────────────────                                                             │  │
│   │   • Provider downloads task input                                              │  │
│   │   • Executes computation on GPU                                                │  │
│   │   • Generates output                                                           │  │
│   │   • Measures work units                                                        │  │
│   │                                                                                 │  │
│   │   STEP 4: RECEIPT GENERATION                                                    │  │
│   │   ──────────────────────────                                                    │  │
│   │   • Creates ComputeReceipt with all fields                                     │  │
│   │   • Signs receipt with provider key                                            │  │
│   │   • Submits to attesters for co-signing                                        │  │
│   │                                                                                 │  │
│   │   STEP 5: SUBMISSION                                                            │  │
│   │   ──────────────────                                                            │  │
│   │   • Submits signed receipt to network                                          │  │
│   │   • Receipt enters mempool                                                     │  │
│   │   • Included in next block                                                     │  │
│   │                                                                                 │  │
│   │   STEP 6: REWARD                                                                │  │
│   │   ──────────────                                                                │  │
│   │   • Receipt validated by consensus                                             │  │
│   │   • MBO reward distributed to provider                                         │  │
│   │   • Reputation score updated                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Rewards Tied to Valid Receipts

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         RECEIPT-BASED REWARDS                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   REWARD MECHANISM                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   Providers earn MBO ONLY when:                                                        │
│   ✓ Receipt is cryptographically valid                                                 │
│   ✓ Output hash matches verification (sampling)                                        │
│   ✓ Attesters have co-signed                                                           │
│   ✓ Receipt is included in a finalized block                                           │
│                                                                                         │
│                                                                                         │
│   NO VALID RECEIPT = NO REWARD                                                          │
│   ════════════════════════════                                                          │
│                                                                                         │
│   This creates strong incentive for:                                                   │
│   • Honest computation                                                                 │
│   • Accurate work measurement                                                          │
│   • Timely submission                                                                  │
│   • Correct execution                                                                  │
│                                                                                         │
│                                                                                         │
│   REWARD CALCULATION                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   provider_reward = (work_units[provider] / total_work_units) × pouw_pool              │
│                                                                                         │
│   Where:                                                                                │
│   • work_units[provider]: Compute cycles from valid receipts                           │
│   • total_work_units: All valid work in the block                                      │
│   • pouw_pool: 50% of block reward (0.05 MBO in Year 1-5)                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Block Reward Allocation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         50% PoUW ALLOCATION                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   BLOCK REWARD: 0.1 MBO (Year 1-5)                                              ║  │
│   ║                                                                                 ║  │
│   ║   ┌───────────────────────────────────────────────────────────────────────────┐║  │
│   ║   │                                                                           │║  │
│   ║   │   ┌─────────────────────────┐      ┌─────────────────────────┐           │║  │
│   ║   │   │                         │      │                         │           │║  │
│   ║   │   │     PoS POOL (50%)      │      │    PoUW POOL (50%)      │           │║  │
│   ║   │   │       0.05 MBO          │      │       0.05 MBO          │           │║  │
│   ║   │   │                         │      │                         │           │║  │
│   ║   │   │   ▼ Validators (80%)    │      │   ▼ GPU Providers       │           │║  │
│   ║   │   │   ▼ Delegators (20%)    │      │   (proportional to work)│           │║  │
│   ║   │   │                         │      │                         │           │║  │
│   ║   │   └─────────────────────────┘      └─────────────────────────┘           │║  │
│   ║   │                                                                           │║  │
│   ║   │           Security Layer                    Compute Layer                 │║  │
│   ║   │                                                                           │║  │
│   ║   └───────────────────────────────────────────────────────────────────────────┘║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ANNUAL PoUW REWARDS (Year 1)                                                          │
│   ════════════════════════════                                                          │
│                                                                                         │
│   • Block reward to PoUW: 0.05 MBO                                                     │
│   • Blocks per year: 31,536,000                                                        │
│   • Total annual PoUW: 1,576,800 MBO                                                   │
│                                                                                         │
│   This is distributed to all compute providers proportionally.                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Deterministic Scoring

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PERFORMANCE-BASED SCORING                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SCORING FACTORS                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   1. WORK UNITS COMPLETED                                                               │
│      • Measured compute cycles                                                         │
│      • Verified via receipts                                                           │
│      • Primary determinant of reward                                                   │
│                                                                                         │
│   2. EXECUTION QUALITY                                                                  │
│      • Accuracy of results                                                             │
│      • Verification success rate                                                       │
│      • No invalid receipts                                                             │
│                                                                                         │
│   3. AVAILABILITY                                                                       │
│      • Uptime percentage                                                               │
│      • Task acceptance rate                                                            │
│      • Response latency                                                                │
│                                                                                         │
│                                                                                         │
│   SCORING FORMULA                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   provider_score = work_units × quality_multiplier × availability_bonus                │
│                                                                                         │
│   Where:                                                                                │
│   • work_units: Sum of compute cycles from valid receipts                              │
│   • quality_multiplier: 1.0 base, reduced for any verification failures               │
│   • availability_bonus: 1.0 - 1.1 based on uptime                                      │
│                                                                                         │
│                                                                                         │
│   DETERMINISM GUARANTEE                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   • All factors are measurable on-chain                                                │
│   • No subjective judgments                                                            │
│   • Same inputs → same score on all nodes                                              │
│   • Integer arithmetic only                                                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.5 No Trusted Oracle: Full On-Chain Verification

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TRUSTLESS VERIFICATION                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   NO EXTERNAL ORACLE REQUIRED                                                   ║  │
│   ║                                                                                 ║  │
│   ║   All verification happens on-chain through:                                    ║  │
│   ║                                                                                 ║  │
│   ║   1. Deterministic re-execution (sampling)                                      ║  │
│   ║   2. Cryptographic proofs                                                       ║  │
│   ║   3. Attester consensus                                                         ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   VERIFICATION METHODS                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   METHOD 1: REPLICATED COMPUTE                                                  │  │
│   │   ────────────────────────────                                                  │  │
│   │   • Multiple providers execute same task                                       │  │
│   │   • Results must match                                                         │  │
│   │   • Discrepancies trigger investigation                                        │  │
│   │                                                                                 │  │
│   │   METHOD 2: PROBABILISTIC SAMPLING                                              │  │
│   │   ────────────────────────────────                                              │  │
│   │   • Random subset of receipts re-verified                                      │  │
│   │   • Validators re-execute and compare                                          │  │
│   │   • Statistical guarantee of correctness                                       │  │
│   │                                                                                 │  │
│   │   METHOD 3: ZK PROOFS (FUTURE)                                                  │  │
│   │   ────────────────────────────                                                  │  │
│   │   • Provider generates ZK proof of correct execution                           │  │
│   │   • Verification is O(1) regardless of computation size                        │  │
│   │   • Ultimate scalability for verification                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   TRUST MODEL                                                                           │
│   ═══════════                                                                           │
│                                                                                         │
│   • No single party can fake results                                                   │
│   • Economic incentives prevent collusion                                              │
│   • Mathematical verification replaces trust                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Compute Supply & Demand Model

### 4.1 Task Submission Sources

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE DEMAND SOURCES                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   INDIVIDUAL USERS                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • Direct task submission via CLI/SDK                                         │  │
│   │   • Personal AI inference requests                                             │  │
│   │   • One-off computation jobs                                                   │  │
│   │   • Development and testing                                                    │  │
│   │                                                                                 │  │
│   │   DECENTRALIZED APPLICATIONS (DApps)                                            │  │
│   │   ──────────────────────────────────                                            │  │
│   │   • AI-powered smart contracts                                                 │  │
│   │   • On-chain gaming with GPU compute                                           │  │
│   │   • DeFi risk models                                                           │  │
│   │   • NFT generation pipelines                                                   │  │
│   │                                                                                 │  │
│   │   EXTERNAL CHAINS                                                               │  │
│   │   ───────────────                                                               │  │
│   │   • Cross-chain compute requests via bridges                                   │  │
│   │   • ZK proof generation for other protocols                                    │  │
│   │   • Shared compute marketplace access                                          │  │
│   │   • Interoperability compute services                                          │  │
│   │                                                                                 │  │
│   │   ENTERPRISE CLIENTS                                                            │  │
│   │   ──────────────────                                                            │  │
│   │   • Batch processing pipelines                                                 │  │
│   │   • ML model inference at scale                                                │  │
│   │   • Rendering and media processing                                             │  │
│   │   • Scientific computation                                                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Priority Compute Gas

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE PRIORITIZATION                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PRIORITY FEE MECHANISM                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   Users can pay higher fees for:                                                       │
│   • Faster task assignment                                                             │
│   • Priority queue placement                                                           │
│   • Higher-quality provider matching                                                   │
│                                                                                         │
│                                                                                         │
│   COMPUTE GAS STRUCTURE                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   compute_fee = base_compute_gas + (priority_multiplier × work_units)                  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Priority Level │ Multiplier │ Queue Position │ Est. Wait Time                │  │
│   │   ───────────────┼────────────┼────────────────┼─────────────────────────────── │  │
│   │   Standard       │ 1.0x       │ Normal         │ Network average               │  │
│   │   Fast           │ 1.5x       │ Priority       │ 50% faster                    │  │
│   │   Instant        │ 2.5x       │ Top priority   │ Next available                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   FEE DISTRIBUTION                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   • Base fee: BURNED (deflationary)                                                    │
│   • Priority fee: To compute provider                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Supply and Demand Dynamics

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MARKET EQUILIBRIUM                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEMAND SIDE                                                                           │
│   ═══════════                                                                           │
│                                                                                         │
│   Demand grows with:                                                                   │
│   • AI adoption globally                                                               │
│   • DApp ecosystem growth                                                              │
│   • Cross-chain compute requests                                                       │
│   • Enterprise onboarding                                                              │
│                                                                                         │
│   As AI usage increases:                                                               │
│   → More compute tasks submitted                                                       │
│   → More MBO needed for fees                                                           │
│   → More base fees burned                                                              │
│   → Increased scarcity                                                                 │
│                                                                                         │
│                                                                                         │
│   SUPPLY SIDE                                                                           │
│   ═══════════                                                                           │
│                                                                                         │
│   Supply grows with:                                                                   │
│   • New compute providers joining                                                      │
│   • Existing providers upgrading hardware                                              │
│   • Geographic expansion                                                               │
│   • Improved software efficiency                                                       │
│                                                                                         │
│   As GPU supply increases:                                                             │
│   → More compute capacity available                                                    │
│   → Lower wait times                                                                   │
│   → Competitive pricing                                                                │
│   → Attracts more demand                                                               │
│                                                                                         │
│                                                                                         │
│   EQUILIBRIUM MECHANISM                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   High Demand + Low Supply:                                                     │  │
│   │   → Priority fees increase                                                     │  │
│   │   → More GPUs attracted to network                                             │  │
│   │   → Supply increases to meet demand                                            │  │
│   │                                                                                 │  │
│   │   Low Demand + High Supply:                                                     │  │
│   │   → Priority fees decrease                                                     │  │
│   │   → Some GPUs exit (opportunity cost)                                          │  │
│   │   → Supply adjusts to match demand                                             │  │
│   │                                                                                 │  │
│   │   Result: Self-regulating market                                               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   FIXED SUPPLY TOKEN IMPACT                                                             │
│   ═════════════════════════                                                             │
│                                                                                         │
│   • MBO supply: Fixed at 31,536,000                                                    │
│   • Base fees: Burned continuously                                                     │
│   • Net effect: Growing compute demand + declining MBO supply                          │
│   • Long-term: Increased MBO value per compute unit                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Compute Utility for Users

### 5.1 User-Facing Compute Services

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE UTILITY FOR USERS                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ON-CHAIN INFERENCE TASKS                                                      │  │
│   │   ════════════════════════                                                      │  │
│   │                                                                                 │  │
│   │   Users can run AI inference directly on-chain:                                │  │
│   │                                                                                 │  │
│   │   • Image classification                                                       │  │
│   │     submit_inference("classify", image_data) → category                        │  │
│   │                                                                                 │  │
│   │   • Text generation                                                            │  │
│   │     submit_inference("generate", prompt) → response                            │  │
│   │                                                                                 │  │
│   │   • Sentiment analysis                                                         │  │
│   │     submit_inference("sentiment", text) → score                                │  │
│   │                                                                                 │  │
│   │   Results are verifiable, deterministic, and immutable.                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   BATCH COMPUTE JOBS                                                            │  │
│   │   ══════════════════                                                            │  │
│   │                                                                                 │  │
│   │   Submit large-scale computation in batches:                                   │  │
│   │                                                                                 │  │
│   │   • Process 10,000 images at once                                              │  │
│   │   • Run Monte Carlo simulations                                                │  │
│   │   • Generate training data                                                     │  │
│   │   • Perform large matrix operations                                            │  │
│   │                                                                                 │  │
│   │   Batch jobs are:                                                               │  │
│   │   • Parallelized across multiple GPUs                                          │  │
│   │   • Priced per work unit                                                       │  │
│   │   • Delivered with aggregated receipts                                         │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   AI PIPELINE EXECUTION                                                         │  │
│   │   ═════════════════════                                                         │  │
│   │                                                                                 │  │
│   │   Chain multiple AI operations:                                                │  │
│   │                                                                                 │  │
│   │   Pipeline Example:                                                             │  │
│   │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                        │  │
│   │   │ Input Image │───▶│ Object      │───▶│ Description │                        │  │
│   │   │             │    │ Detection   │    │ Generation  │                        │  │
│   │   └─────────────┘    └─────────────┘    └─────────────┘                        │  │
│   │                                                                                 │  │
│   │   Each step produces verified receipt.                                         │  │
│   │   Pipeline completion triggers final callback.                                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   GPU-ACCELERATED CONTRACT OPERATIONS                                           │  │
│   │   ═══════════════════════════════════                                           │  │
│   │                                                                                 │  │
│   │   Smart contracts (future) can call GPU compute:                               │  │
│   │                                                                                 │  │
│   │   contract.requestCompute(task_type, input, callback)                          │  │
│   │                                                                                 │  │
│   │   Use cases:                                                                    │  │
│   │   • DeFi: Complex risk calculations                                            │  │
│   │   • Gaming: Physics simulations                                                │  │
│   │   • NFTs: On-demand generation                                                 │  │
│   │   • DAOs: AI-assisted governance                                               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   OFF-CHAIN JOBS VERIFIED ON-CHAIN                                              │  │
│   │   ════════════════════════════════                                              │  │
│   │                                                                                 │  │
│   │   Heavy computations can run off-chain with on-chain verification:             │  │
│   │                                                                                 │  │
│   │   1. User submits job request on-chain                                         │  │
│   │   2. Provider executes off-chain                                               │  │
│   │   3. Provider submits receipt on-chain                                         │  │
│   │   4. Receipt verified deterministically                                        │  │
│   │   5. Result hash stored permanently                                            │  │
│   │                                                                                 │  │
│   │   Benefits:                                                                     │  │
│   │   • Unlimited computation complexity                                           │  │
│   │   • On-chain audit trail                                                       │  │
│   │   • Verifiable results                                                         │  │
│   │   • Cost efficiency                                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Incentives for GPU Providers

### 6.1 Provider Economics

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GPU PROVIDER INCENTIVES                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   REVENUE STREAMS                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   1. BLOCK REWARDS (PoUW Pool)                                                  │  │
│   │   ────────────────────────────                                                  │  │
│   │   • 50% of all block rewards                                                   │  │
│   │   • Distributed proportionally to work units                                   │  │
│   │   • Predictable income based on computation                                    │  │
│   │                                                                                 │  │
│   │   2. COMPUTE FEES                                                               │  │
│   │   ───────────────                                                               │  │
│   │   • Direct payment from task submitters                                        │  │
│   │   • Priority fees for faster execution                                         │  │
│   │   • Scales with demand                                                         │  │
│   │                                                                                 │  │
│   │   3. LONG-TERM MBO APPRECIATION                                                 │  │
│   │   ─────────────────────────────                                                 │  │
│   │   • Fixed supply + growing demand                                              │  │
│   │   • Earnings held in MBO may appreciate                                        │  │
│   │   • Aligns provider interests with network success                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   EXAMPLE PROVIDER ECONOMICS (Year 1)                                                   │
│   ═══════════════════════════════════                                                   │
│                                                                                         │
│   Assumptions:                                                                          │
│   • Provider contributes 1% of total network compute                                   │
│   • 100% uptime                                                                        │
│   • No slashing events                                                                 │
│                                                                                         │
│   Annual PoUW pool: 1,576,800 MBO                                                      │
│   Provider share: 1% × 1,576,800 = 15,768 MBO                                          │
│   Plus compute fees (variable based on demand)                                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Performance-Based Scoring

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PERFORMANCE SCORING                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SCORING FACTORS                                                                       │
│   ═══════════════                                                                       │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Factor              │ Weight │ Measurement                                    │  │
│   │   ────────────────────┼────────┼────────────────────────────────────────────────│  │
│   │   Work Completed      │ 60%    │ Total valid compute cycles                    │  │
│   │   Verification Rate   │ 25%    │ % of receipts passing verification            │  │
│   │   Availability        │ 15%    │ Uptime and task acceptance rate               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   HIGH PERFORMERS EARN MORE                                                             │
│   ═════════════════════════                                                             │
│                                                                                         │
│   • More work units = larger share of PoUW pool                                        │
│   • Higher verification rate = quality multiplier                                      │
│   • Better availability = priority task assignment                                     │
│                                                                                         │
│   Result: Meritocratic system where quality providers thrive.                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Deterministic Verification (Cannot Cheat)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CHEAT-PROOF DESIGN                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   WHY PROVIDERS CANNOT CHEAT                                                            │
│   ══════════════════════════                                                            │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   CHEAT ATTEMPT 1: Fake Work Units                                              │  │
│   │   ─────────────────────────────────                                             │  │
│   │   Attack: Claim more compute cycles than actually used                         │  │
│   │   Prevention: Work units verified via result reproduction                      │  │
│   │   Detection: Mismatch triggers investigation                                   │  │
│   │   Consequence: Slashing + reputation damage                                    │  │
│   │                                                                                 │  │
│   │   CHEAT ATTEMPT 2: Invalid Results                                              │  │
│   │   ────────────────────────────────                                              │  │
│   │   Attack: Submit wrong computation results                                     │  │
│   │   Prevention: Result hash verified via sampling                                │  │
│   │   Detection: Hash mismatch is conclusive fraud proof                           │  │
│   │   Consequence: 1,000 MBO slashing per invalid receipt                          │  │
│   │                                                                                 │  │
│   │   CHEAT ATTEMPT 3: Forged Receipts                                              │  │
│   │   ────────────────────────────────                                              │  │
│   │   Attack: Create receipts for work not done                                    │  │
│   │   Prevention: Multi-attester signatures required                               │  │
│   │   Detection: Signature verification fails                                      │  │
│   │   Consequence: Receipt rejected, no reward                                     │  │
│   │                                                                                 │  │
│   │   CHEAT ATTEMPT 4: Collusion                                                    │  │
│   │   ──────────────────────────────                                                │  │
│   │   Attack: Multiple providers collude to validate fake work                     │  │
│   │   Prevention: Random attester selection                                        │  │
│   │   Detection: Statistical anomaly detection                                     │  │
│   │   Consequence: All colluders slashed                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ECONOMIC DETERRENCE                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   Cost of cheating > Benefit of cheating                                               │
│   Therefore: Rational providers choose honesty                                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.4 Slashing on Invalid Receipts

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING PENALTIES                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PENALTY SCHEDULE                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Offense                         │ Penalty         │ Additional                │  │
│   │   ────────────────────────────────┼─────────────────┼───────────────────────────│  │
│   │   Invalid compute receipt         │ 1,000 MBO       │ Receipt rejected          │  │
│   │   Repeated invalid receipts (3+)  │ 5,000 MBO       │ 24h suspension            │  │
│   │   Systematic fraud                │ Full collateral │ Permanent ban             │  │
│   │   Result manipulation             │ 2,000 MBO       │ 7d jail                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   All slashed MBO is BURNED (not redistributed).                                       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.5 Stake Benefits

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         OPTIONAL STAKING                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   STAKE IS OPTIONAL BUT BENEFICIAL                                                      │
│   ════════════════════════════════                                                      │
│                                                                                         │
│   Providers can operate without stake, but staking provides:                           │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   REPUTATION BOOST                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • Staked providers trusted more by network                                   │  │
│   │   • Faster reputation building                                                 │  │
│   │   • Priority in task assignment                                                │  │
│   │                                                                                 │  │
│   │   PRIORITY ACCESS                                                               │  │
│   │   ───────────────                                                               │  │
│   │   • First access to high-value tasks                                           │  │
│   │   • Better matching algorithm priority                                         │  │
│   │   • Longer task reservation windows                                            │  │
│   │                                                                                 │  │
│   │   SKIN IN THE GAME                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • Demonstrates commitment                                                    │  │
│   │   • Users prefer staked providers                                              │  │
│   │   • Network-wide trust signal                                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   RECOMMENDED STAKE                                                                     │
│   ═════════════════                                                                     │
│                                                                                         │
│   • Minimum: None (can operate without)                                                │
│   • Recommended: 10,000 MBO for priority benefits                                      │
│   • Premium: 50,000+ MBO for maximum priority                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Strategic Value (Why This Model Wins)

### 7.1 Global AI Compute Demand

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GROWING DEMAND FOR AI COMPUTE                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MARKET TRENDS                                                                         │
│   ═════════════                                                                         │
│                                                                                         │
│   • AI compute demand growing 10x every 2 years                                        │
│   • Centralized providers (AWS, Google, Azure) cannot scale fast enough                │
│   • GPU shortages creating bottlenecks                                                 │
│   • Enterprises seeking alternatives                                                   │
│                                                                                         │
│                                                                                         │
│   MBONGO'S POSITION                                                                     │
│   ═════════════════                                                                     │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Mbongo captures this demand by:                                               │  │
│   │                                                                                 │  │
│   │   • Aggregating global GPU capacity                                            │  │
│   │   • Providing verifiable compute                                               │  │
│   │   • Enabling trustless execution                                               │  │
│   │   • Creating economic incentives for providers                                 │  │
│   │                                                                                 │  │
│   │   Result: Decentralized alternative to cloud compute monopolies                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Fixed Supply Token with Deflationary Pressure

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TOKENOMICS ADVANTAGE                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FIXED SUPPLY + GROWING DEMAND                                                         │
│   ═════════════════════════════                                                         │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Supply Side:                                                                  │  │
│   │   • Total: 31,536,000 MBO (fixed forever)                                      │  │
│   │   • Base fees: 100% burned                                                     │  │
│   │   • Slashing: 100% burned                                                      │  │
│   │   • Net: Declining circulating supply                                          │  │
│   │                                                                                 │  │
│   │   Demand Side:                                                                  │  │
│   │   • Compute jobs require MBO                                                   │  │
│   │   • Staking locks MBO                                                          │  │
│   │   • Fees consume MBO                                                           │  │
│   │   • Growing network = growing demand                                           │  │
│   │                                                                                 │  │
│   │   Intersection:                                                                 │  │
│   │   • Fixed/declining supply + growing demand                                    │  │
│   │   • = Long-term value appreciation                                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 AI + Blockchain Hybrid Unlocks New Applications

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         NEW APPLICATION CATEGORIES                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   APPLICATIONS ONLY POSSIBLE WITH VERIFIABLE AI COMPUTE                                 │
│   ═════════════════════════════════════════════════════                                 │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   AI-GOVERNED DAOs                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • AI agents make governance decisions                                        │  │
│   │   • Decisions verifiable on-chain                                              │  │
│   │   • No trusted operator required                                               │  │
│   │                                                                                 │  │
│   │   TRUSTLESS AI ORACLES                                                          │  │
│   │   ────────────────────                                                          │  │
│   │   • AI models provide data to smart contracts                                  │  │
│   │   • Model execution is verified                                                │  │
│   │   • Results are deterministic                                                  │  │
│   │                                                                                 │  │
│   │   ON-CHAIN AI MARKETS                                                           │  │
│   │   ───────────────────                                                           │  │
│   │   • Buy/sell AI compute directly                                               │  │
│   │   • No intermediaries                                                          │  │
│   │   • Automatic settlement                                                       │  │
│   │                                                                                 │  │
│   │   VERIFIABLE ML MODELS                                                          │  │
│   │   ────────────────────                                                          │  │
│   │   • Model training verified                                                    │  │
│   │   • Inference results auditable                                                │  │
│   │   • Data provenance tracked                                                    │  │
│   │                                                                                 │  │
│   │   DECENTRALIZED AI SERVICES                                                     │  │
│   │   ─────────────────────────                                                     │  │
│   │   • Chatbots with verified outputs                                             │  │
│   │   • Image generation with provenance                                           │  │
│   │   • Recommendation systems without bias                                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.4 PoUW Model Advantages

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoUW COMPETITIVE ADVANTAGES                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VS. PURE PoS                                                                          │
│   ════════════                                                                          │
│                                                                                         │
│   • PoS alone: Security only from stake                                                │
│   • Mbongo: Security from stake + compute                                              │
│   • Benefit: Dual-layer security model                                                 │
│                                                                                         │
│   VS. PURE PoW                                                                          │
│   ════════════                                                                          │
│                                                                                         │
│   • PoW: Computation is "wasted" (hash puzzles)                                        │
│   • Mbongo: Computation is "useful" (AI, rendering, etc.)                              │
│   • Benefit: Work has real-world value                                                 │
│                                                                                         │
│   VS. ADD-ON COMPUTE NETWORKS                                                           │
│   ═══════════════════════════                                                           │
│                                                                                         │
│   • Add-ons: Compute separate from consensus                                           │
│   • Mbongo: Compute is part of consensus                                               │
│   • Benefit: Native integration, stronger security                                     │
│                                                                                         │
│                                                                                         │
│   GPU PROVIDERS AS CONSENSUS PARTICIPANTS                                               │
│   ═══════════════════════════════════════                                               │
│                                                                                         │
│   In Mbongo, compute providers (GPU/TPU/CPU/FPGA/ASIC) are NOT add-on actors. They are:│
│                                                                                         │
│   • Core consensus participants                                                        │
│   • Rewarded from block rewards (50%)                                                  │
│   • Subject to slashing                                                                │
│   • Contributing to chain security                                                     │
│                                                                                         │
│   This creates stronger alignment than any "compute layer" built on top of PoS.       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Summary

### 8.1 Key Points

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE VALUE SUMMARY                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ COMPUTE IS A CORE CONSENSUS COMPONENT                                       │  │
│   │                                                                                 │  │
│   │     • Not an add-on service—native to protocol                                 │  │
│   │     • 50% of block rewards allocated to compute                                │  │
│   │     • Compute providers are consensus participants                             │  │
│   │     • Compute receipts embedded in block metadata                              │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ VERIFIED COMPUTE RECEIPTS SECURE THE CHAIN                                  │  │
│   │                                                                                 │  │
│   │     • Every receipt is cryptographically signed                                │  │
│   │     • Results are deterministically verifiable                                 │  │
│   │     • Invalid receipts trigger slashing                                        │  │
│   │     • No trusted oracle required                                               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ PROVIDERS EARN MBO THROUGH PERFORMANCE                                      │  │
│   │                                                                                 │  │
│   │     • Rewards proportional to valid work units                                 │  │
│   │     • Quality multipliers for high verification rates                          │  │
│   │     • Priority access for staked providers                                     │  │
│   │     • Cannot cheat—deterministic verification                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ USERS ACCESS VERIFIABLE AI COMPUTE                                          │  │
│   │                                                                                 │  │
│   │     • On-chain inference tasks                                                 │  │
│   │     • Batch compute jobs                                                       │  │
│   │     • AI pipelines                                                             │  │
│   │     • GPU-accelerated contracts (future)                                       │  │
│   │     • Off-chain jobs with on-chain verification                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ DEMAND FOR COMPUTE INCREASES MBO VALUE                                      │  │
│   │                                                                                 │  │
│   │     • AI compute demand growing exponentially                                  │  │
│   │     • More compute = more MBO needed                                           │  │
│   │     • More fees = more MBO burned                                              │  │
│   │     • Fixed supply + growing demand = value appreciation                       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE VALUE QUICK REFERENCE                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ARCHITECTURE                                                                          │
│   ────────────                                                                          │
│   Integration:     Native to consensus (not add-on)                                    │
│   Verification:    Deterministic, on-chain                                             │
│   Receipts:        Embedded in block metadata                                          │
│   Workloads:       AI/ML, rendering, ZK proofs, scientific compute                     │
│                                                                                         │
│   ECONOMICS                                                                             │
│   ─────────                                                                             │
│   Reward Pool:     50% of block rewards (0.05 MBO/block Year 1)                        │
│   Distribution:    Proportional to valid work units                                    │
│   Fees:            Base (burned) + Priority (to provider)                              │
│   Slashing:        1,000 MBO per invalid receipt                                       │
│                                                                                         │
│   PROVIDERS                                                                             │
│   ─────────                                                                             │
│   Revenue:         Block rewards + compute fees                                        │
│   Scoring:         Work (60%) + Quality (25%) + Availability (15%)                     │
│   Stake:           Optional but improves priority                                      │
│   Security:        Cannot cheat (deterministic verification)                           │
│                                                                                         │
│   USERS                                                                                 │
│   ─────                                                                                 │
│   Services:        Inference, batch jobs, pipelines, GPU contracts                     │
│   Verification:    All results on-chain and auditable                                  │
│   Priority:        Higher fees = faster execution                                      │
│                                                                                         │
│   STRATEGIC                                                                             │
│   ─────────                                                                             │
│   Market:          Growing AI compute demand globally                                  │
│   Token:           Fixed supply with deflationary pressure                             │
│   Position:        AI + blockchain hybrid (unique)                                     │
│   Advantage:       Compute in consensus (not add-on)                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [compute_engine_overview.md](./compute_engine_overview.md) | Technical compute architecture |
| [incentive_design.md](./incentive_design.md) | Incentive mechanisms |
| [utility_value.md](./utility_value.md) | MBO token utility |
| [consensus_master_overview.md](./consensus_master_overview.md) | Consensus integration |

---

*This document defines the economic, functional, and strategic value of compute within Mbongo Chain. Compute is a native consensus-level resource with verifiable execution.*

