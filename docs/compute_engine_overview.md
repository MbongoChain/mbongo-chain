<!-- Verified against tokenomics.md -->
# Mbongo Chain — Compute Engine Overview

> **Document Version:** 1.0  
> **Last Updated:** November 2025  
> **Status:** Living Document

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Compute-First Vision](#2-compute-first-vision)
3. [Proof-of-Useful-Work (PoUW) Model](#3-proof-of-useful-work-pouw-model)
4. [Compute Task Lifecycle](#4-compute-task-lifecycle)
5. [GPU Provider Architecture](#5-gpu-provider-architecture)
6. [Deterministic Compute Validation](#6-deterministic-compute-validation)
7. [Integration with Runtime Execution](#7-integration-with-runtime-execution)
8. [Global Compute Marketplace](#8-global-compute-marketplace)
9. [Security Model](#9-security-model)
10. [Future Roadmap](#10-future-roadmap)

---

## 1. Purpose of This Document

This document serves as the **master technical overview** for Mbongo Chain's Compute Engine—the subsystem that transforms blockchain consensus into a global compute marketplace.

### Scope

The Compute Engine encompasses:

| Component | Description |
|-----------|-------------|
| **PoUW Protocol** | Proof-of-Useful-Work consensus integration |
| **GPU Coordination** | Global GPU provider network orchestration |
| **Task Management** | Job submission, assignment, and tracking |
| **Validation Layer** | Deterministic result verification |
| **Reward System** | Incentive distribution for compute providers |

### Core Thesis

Traditional blockchains waste computational resources on arbitrary proof-of-work or limit computation to simple VM operations. Mbongo Chain redirects this computational capacity toward **useful work**—ML inference, training, ZK proof generation, and general-purpose GPU compute—while maintaining consensus security guarantees.

### Audience

- **Compute Provider Operators** setting up GPU nodes
- **Protocol Engineers** implementing PoUW mechanisms
- **Application Developers** integrating compute capabilities
- **Economists** analyzing incentive structures

---

## 2. Compute-First Vision

### 2.1 The Compute Bottleneck

Modern AI and blockchain workloads face a critical resource constraint:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    THE COMPUTE CRISIS                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   AI Training Demand          GPU Supply                            │
│        ▲                                                            │
│        │ ████████████████████████████████                          │
│        │ ████████████████████████████████                          │
│        │ ████████████████████████████████   ← Exponential Growth   │
│        │ ███████████████████                                        │
│        │ ██████████████                                             │
│        │ █████████         ┌─────────────────────────┐             │
│        │ ██████            │ Available GPU capacity  │             │
│        │ ████              │ grows linearly          │             │
│        │ ██                └─────────────────────────┘             │
│        │ █                                                          │
│        └──────────────────────────────────────────▶ Time           │
│                                                                     │
│   Result: Compute is scarce, expensive, and centralized            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Observations:**

- AI model sizes double every 3-6 months
- GPU production cannot match demand growth
- Cloud providers control access and pricing
- Idle consumer GPUs remain underutilized globally

### 2.2 Mbongo's Compute-Centric Design

Mbongo Chain addresses this by making compute a **first-class protocol primitive**:

```
┌─────────────────────────────────────────────────────────────────────┐
│              MBONGO COMPUTE-CENTRIC ARCHITECTURE                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Traditional Blockchain          Mbongo Chain                      │
│   ────────────────────           ─────────────                      │
│                                                                     │
│   ┌─────────────────┐            ┌─────────────────┐               │
│   │   Consensus     │            │   Consensus     │               │
│   │   (PoW/PoS)     │            │   (PoS + PoUW)  │               │
│   └────────┬────────┘            └────────┬────────┘               │
│            │                              │                         │
│            ▼                              ▼                         │
│   ┌─────────────────┐            ┌─────────────────┐               │
│   │   Execution     │            │   Execution     │               │
│   │   (Simple VM)   │            │ (Native + GPU)  │               │
│   └────────┬────────┘            └────────┬────────┘               │
│            │                              │                         │
│            ▼                              ▼                         │
│   ┌─────────────────┐            ┌─────────────────┐               │
│   │    Storage      │            │ Compute Engine  │◀── NEW        │
│   │   (State Only)  │            │  (Global GPUs)  │               │
│   └─────────────────┘            └────────┬────────┘               │
│                                           │                         │
│                                           ▼                         │
│                                  ┌─────────────────┐               │
│                                  │    Storage      │               │
│                                  └─────────────────┘               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 2.3 Competitive Landscape Comparison

| Feature | Ethereum | Solana | FuelVM | Render/Akash | **Mbongo** |
|---------|----------|--------|--------|--------------|------------|
| Consensus | PoS | PoH+PoS | PoS | N/A | **PoS + PoUW** |
| Native GPU Compute | ✗ | ✗ | ✗ | ✓ | **✓** |
| Consensus-Integrated Compute | ✗ | ✗ | ✗ | ✗ | **✓** |
| Deterministic Validation | ✓ | ✓ | ✓ | Limited | **✓** |
| On-Chain Task Assignment | ✗ | ✗ | ✗ | Off-chain | **✓** |
| Compute Rewards in Consensus | ✗ | ✗ | ✗ | ✗ | **✓** |
| Smart Contract Integration | ✓ | ✓ | ✓ | Limited | **✓ [FUTURE]** |

**Key Differentiators:**

1. **Consensus Integration:** PoUW scores directly influence block production rights
2. **On-Chain Coordination:** Task assignment and verification are protocol-native
3. **Unified Incentives:** Validators and compute providers share aligned economics

### 2.4 Stakeholder Benefits

| Stakeholder | Benefits |
|-------------|----------|
| **Developers** | Access global GPU pool via simple API; pay-per-compute pricing |
| **Validators** | Earn additional rewards by providing compute; higher block production probability |
| **Compute Providers** | Monetize GPU hardware; permissionless participation |
| **Users** | Access AI/ML capabilities through on-chain applications |

---

## 3. Proof-of-Useful-Work (PoUW) Model

### 3.1 Supported Workload Types

The PoUW system supports verified compute across multiple categories:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SUPPORTED WORKLOAD TYPES                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐    │
│   │  ML INFERENCE   │  │  ML TRAINING    │  │  ZK GENERATION  │    │
│   │                 │  │                 │  │                 │    │
│   │ • LLM inference │  │ • Gradient comp │  │ • SNARK proofs  │    │
│   │ • Image models  │  │ • Fine-tuning   │  │ • STARK proofs  │    │
│   │ • Embeddings    │  │ • LoRA updates  │  │ • Rollup proofs │    │
│   └─────────────────┘  └─────────────────┘  └─────────────────┘    │
│                                                                     │
│   ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐    │
│   │ BATCH COMPUTE   │  │    RENDERING    │  │    ENCODING     │    │
│   │                 │  │                 │  │                 │    │
│   │ • Matrix ops    │  │ • 3D rendering  │  │ • Video transc. │    │
│   │ • Simulations   │  │ • Ray tracing   │  │ • Audio process │    │
│   │ • Data process  │  │ • Image synth   │  │ • Compression   │    │
│   └─────────────────┘  └─────────────────┘  └─────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Work Assignment Process

Tasks are assigned through deterministic protocol rules:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    WORK ASSIGNMENT FLOW                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Task Queue                    Provider Pool                       │
│   ──────────                    ─────────────                       │
│   ┌─────────┐                   ┌─────────┐                        │
│   │ Task A  │──┐                │Provider1│ Capacity: 100 TFLOPS   │
│   │ Task B  │  │                │Provider2│ Capacity: 50 TFLOPS    │
│   │ Task C  │  │                │Provider3│ Capacity: 200 TFLOPS   │
│   │ Task D  │  │                └────┬────┘                        │
│   └─────────┘  │                     │                              │
│                │                     │                              │
│                ▼                     ▼                              │
│        ┌─────────────────────────────────────┐                     │
│        │         ASSIGNMENT ENGINE           │                     │
│        │                                     │                     │
│        │  Factors:                           │                     │
│        │  • Provider capacity & availability │                     │
│        │  • Task requirements (VRAM, time)   │                     │
│        │  • Provider reputation score        │                     │
│        │  • Geographic distribution          │                     │
│        │  • VRF-based randomization          │                     │
│        │                                     │                     │
│        └───────────────┬─────────────────────┘                     │
│                        │                                            │
│                        ▼                                            │
│        ┌─────────────────────────────────────┐                     │
│        │  Task A → Provider3                 │                     │
│        │  Task B → Provider1                 │                     │
│        │  Task C → Provider1                 │                     │
│        │  Task D → Provider2                 │                     │
│        └─────────────────────────────────────┘                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

**Assignment Algorithm:**

```
AssignTask(task, providers):
    eligible = filter(providers, p => p.capacity >= task.requirements)
    eligible = filter(eligible, p => p.reputation >= MIN_REPUTATION)
    weights  = map(eligible, p => p.stake * p.capacity * p.uptime)
    selected = VRF_weighted_select(weights, task.id)
    return selected
```

### 3.3 Result Verification

Results undergo multi-layer verification:

| Layer | Method | Coverage |
|-------|--------|----------|
| **Syntactic** | Format validation, bounds checking | 100% of results |
| **Replicated** | Re-execution by verifier nodes | Configurable % |
| **Probabilistic** | Random sampling verification | Statistical guarantee |
| **Cryptographic** | ZK proofs [FUTURE] | Selected workloads |

### 3.4 PoUW Score Calculation

Compute providers accumulate PoUW scores:

```
PoUW_Score = Σ (Task_Weight × Verification_Multiplier × Time_Bonus)

Where:
  Task_Weight           = FLOPS_completed × Difficulty_factor
  Verification_Multiplier = 1.0 (verified) | 0.0 (failed) | 0.5 (pending)
  Time_Bonus            = max(0, 1 - (actual_time / deadline))
```

### 3.5 Hybrid PoS + PoUW Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│              PoS + PoUW HYBRID CONSENSUS                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────────┐         ┌─────────────────┐                  │
│   │   PoS Weight    │         │   PoUW Score    │                  │
│   │                 │         │                 │                  │
│   │  Staked Tokens  │         │ Verified Compute│                  │
│   │  × Uptime       │         │ × Task Quality  │                  │
│   └────────┬────────┘         └────────┬────────┘                  │
│            │                           │                            │
│            │      ┌───────────┐        │                            │
│            └─────▶│  COMBINE  │◀───────┘                            │
│                   │           │                                     │
│                   │ α=0.5 PoS │                                     │
│                   │ β=0.5 PoUW│                                     │
│                   └─────┬─────┘                                     │
│                         │                                           │
│                         ▼                                           │
│              ┌─────────────────────┐                               │
│              │  LEADER SELECTION   │                               │
│              │                     │                               │
│              │  Combined_Score =   │                               │
│              │  (α × PoS) +        │                               │
│              │  (β × PoUW) +       │                               │
│              │  VRF(slot)          │                               │
│              └─────────────────────┘                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.6 End-to-End PoUW Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PoUW END-TO-END FLOW                             │
└─────────────────────────────────────────────────────────────────────┘

  ┌────────┐    ┌────────────┐    ┌─────────────┐    ┌─────────────┐
  │ Client │───▶│ Assignment │───▶│GPU Provider │───▶│  Compute    │
  │        │    │   Engine   │    │             │    │  Receipt    │
  └────────┘    └────────────┘    └─────────────┘    └──────┬──────┘
       │                                                     │
       │  1. Submit Task                                     │
       │                                                     │
       │                         2. Assign to Provider       │
       │                                                     │
       │                                      3. Execute     │
       │                                                     │
       │                                            4. Generate Proof
       │                                                     │
       │                                                     ▼
       │                                            ┌─────────────┐
       │                                            │ Verification│
       │                                            │    Layer    │
       │                                            └──────┬──────┘
       │                                                   │
       │                              5. Validate Result   │
       │                                                   ▼
       │                                            ┌─────────────┐
       │◀───────────────────────────────────────────│PoUW Reward │
       │              6. Distribute Rewards         │  & Score    │
       │                                            └─────────────┘
```

---

## 4. Compute Task Lifecycle

### 4.1 Complete 10-Step Lifecycle

```
┌─────────────────────────────────────────────────────────────────────┐
│              COMPUTE TASK LIFECYCLE (10 STEPS)                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 1: TASK REGISTRATION                                    │  │
│  │ Client submits compute task with specifications:             │  │
│  │ • Workload type (inference, training, ZK, etc.)              │  │
│  │ • Input data hash / pointer                                  │  │
│  │ • Resource requirements (VRAM, compute time)                 │  │
│  │ • Fee deposit                                                │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 2: TASK VALIDATION                                      │  │
│  │ Protocol validates:                                          │  │
│  │ • Fee sufficient for requested resources                     │  │
│  │ • Input data accessible                                      │  │
│  │ • Task format compliant                                      │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 3: QUEUE INSERTION                                      │  │
│  │ Task enters priority queue:                                  │  │
│  │ • Priority = fee_rate × urgency_flag                         │  │
│  │ • Task assigned unique task_id                               │  │
│  │ • Deadline timer started                                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 4: PROVIDER ASSIGNMENT                                  │  │
│  │ Assignment engine selects provider:                          │  │
│  │ • Filter by capacity, reputation, availability               │  │
│  │ • VRF-weighted selection among eligible providers            │  │
│  │ • Assignment recorded on-chain                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 5: INPUT RETRIEVAL                                      │  │
│  │ Provider fetches task inputs:                                │  │
│  │ • Download from decentralized storage (IPFS/Arweave)         │  │
│  │ • Verify input hash matches task specification               │  │
│  │ • Load model weights if required                             │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 6: COMPUTATION EXECUTION                                │  │
│  │ GPU provider executes workload:                              │  │
│  │ • Sandboxed execution environment                            │  │
│  │ • Deterministic compute settings                             │  │
│  │ • Resource metering active                                   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 7: PROOF GENERATION                                     │  │
│  │ Provider generates compute receipt:                          │  │
│  │ • Output data hash                                           │  │
│  │ • Execution metadata (time, resources used)                  │  │
│  │ • Provider signature                                         │  │
│  │ • Optional: intermediate checkpoints                         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 8: RESULT SUBMISSION                                    │  │
│  │ Provider submits to network:                                 │  │
│  │ • Compute receipt broadcast                                  │  │
│  │ • Output data uploaded to storage                            │  │
│  │ • Verification request initiated                             │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 9: VALIDATION                                           │  │
│  │ Verifier nodes validate result:                              │  │
│  │ • Replicated execution (if selected)                         │  │
│  │ • Output hash comparison                                     │  │
│  │ • Fraud proof window                                         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │ STEP 10: REWARD DISTRIBUTION                                 │  │
│  │ Upon successful validation:                                  │  │
│  │ • Provider receives compute fee                              │  │
│  │ • PoUW score updated                                         │  │
│  │ • Client notified of completion                              │  │
│  │ • Task marked finalized                                      │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 State Transitions

| State | Description | Next States |
|-------|-------------|-------------|
| `REGISTERED` | Task submitted and validated | `QUEUED`, `REJECTED` |
| `QUEUED` | Waiting for provider assignment | `ASSIGNED`, `EXPIRED` |
| `ASSIGNED` | Provider selected | `EXECUTING`, `REASSIGNED` |
| `EXECUTING` | Computation in progress | `COMPLETED`, `FAILED` |
| `COMPLETED` | Result submitted | `VALIDATING` |
| `VALIDATING` | Verification in progress | `FINALIZED`, `DISPUTED` |
| `FINALIZED` | Successfully completed | Terminal |
| `DISPUTED` | Fraud proof submitted | `FINALIZED`, `SLASHED` |

---

## 5. GPU Provider Architecture

### 5.1 Provider Node Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    GPU PROVIDER NODE                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │                     NODE DAEMON                              │  │
│   │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌──────────┐  │  │
│   │  │  Network  │  │   Task    │  │  Result   │  │  Health  │  │  │
│   │  │  Manager  │  │ Scheduler │  │ Submitter │  │  Monitor │  │  │
│   │  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └────┬─────┘  │  │
│   │        │              │              │             │         │  │
│   └────────┼──────────────┼──────────────┼─────────────┼─────────┘  │
│            │              │              │             │             │
│   ┌────────┼──────────────┼──────────────┼─────────────┼─────────┐  │
│   │        ▼              ▼              ▼             ▼         │  │
│   │   ┌─────────────────────────────────────────────────────┐   │  │
│   │   │              EXECUTION RUNTIME                       │   │  │
│   │   │                                                      │   │  │
│   │   │  ┌────────────┐  ┌────────────┐  ┌────────────┐     │   │  │
│   │   │  │  Sandbox   │  │ Determin.  │  │  Resource  │     │   │  │
│   │   │  │  Manager   │  │  Enforcer  │  │   Meter    │     │   │  │
│   │   │  └────────────┘  └────────────┘  └────────────┘     │   │  │
│   │   │                                                      │   │  │
│   │   └──────────────────────────┬───────────────────────────┘   │  │
│   │                              │                               │  │
│   │   ┌──────────────────────────▼───────────────────────────┐   │  │
│   │   │                   GPU LAYER                          │   │  │
│   │   │                                                      │   │  │
│   │   │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────┐  │   │  │
│   │   │  │  GPU 0  │  │  GPU 1  │  │  GPU 2  │  │  GPU N │  │   │  │
│   │   │  │ (24GB)  │  │ (24GB)  │  │ (48GB)  │  │  ...   │  │   │  │
│   │   │  └─────────┘  └─────────┘  └─────────┘  └────────┘  │   │  │
│   │   │                                                      │   │  │
│   │   └──────────────────────────────────────────────────────┘   │  │
│   │                         HARDWARE LAYER                       │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.2 Hardware Requirements

| Tier | GPU | VRAM | CPU | RAM | Storage | Network |
|------|-----|------|-----|-----|---------|---------|
| **Entry** | RTX 3080 | 10 GB | 8 cores | 32 GB | 500 GB NVMe | 500 Mbps |
| **Standard** | RTX 3090/4090 | 24 GB | 16 cores | 64 GB | 1 TB NVMe | 1 Gbps |
| **Professional** | A100 | 40-80 GB | 32 cores | 128 GB | 2 TB NVMe | 10 Gbps |
| **Enterprise** | H100 | 80 GB | 64 cores | 256 GB | 4 TB NVMe | 25 Gbps |

### 5.3 Deterministic Execution Requirements

To ensure verifiable computation:

| Requirement | Implementation |
|-------------|----------------|
| **Fixed Precision** | FP32/FP16 with deterministic rounding |
| **Ordered Operations** | Sequential kernel execution |
| **Seed Control** | Fixed RNG seeds for stochastic operations |
| **Version Pinning** | Locked CUDA/ROCm and library versions |
| **Memory Layout** | Deterministic tensor memory allocation |

### 5.4 Multi-GPU Farm Setup

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MULTI-GPU FARM TOPOLOGY                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────────────────────────────────────────────────┐    │
│   │                    FARM CONTROLLER                         │    │
│   │  • Load balancing across nodes                            │    │
│   │  • Unified capacity reporting                             │    │
│   │  • Centralized task distribution                          │    │
│   └───────────────────────────┬───────────────────────────────┘    │
│                               │                                     │
│         ┌─────────────────────┼─────────────────────┐              │
│         │                     │                     │              │
│         ▼                     ▼                     ▼              │
│   ┌───────────┐         ┌───────────┐         ┌───────────┐       │
│   │  Node 1   │         │  Node 2   │         │  Node 3   │       │
│   │ 4× A100   │         │ 8× RTX4090│         │ 2× H100   │       │
│   │ 320 GB    │         │ 192 GB    │         │ 160 GB    │       │
│   └───────────┘         └───────────┘         └───────────┘       │
│                                                                     │
│   Total Farm Capacity: 672 GB VRAM, ~500 TFLOPS FP16               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.5 Security & Isolation

| Layer | Protection |
|-------|------------|
| **Process Isolation** | Container-based execution (Docker/Podman) |
| **Memory Isolation** | GPU memory cleared between tasks |
| **Network Isolation** | No external network during execution |
| **Filesystem** | Read-only root, ephemeral scratch space |
| **Time Limits** | Hard timeout enforcement |

### 5.6 Slashing Conditions

| Violation | Penalty | Recovery |
|-----------|---------|----------|
| **Invalid Result** | 10% stake slash | Automatic |
| **Repeated Failures** | 25% stake + temporary ban | Manual review |
| **Collusion Detected** | 100% stake slash | Permanent ban |
| **Timeout Abuse** | 5% stake slash | Automatic |
| **Data Exfiltration** | 100% stake slash | Permanent ban |

---

## 6. Deterministic Compute Validation

### 6.1 Determinism Enforcement

```
┌─────────────────────────────────────────────────────────────────────┐
│              DETERMINISM ENFORCEMENT STACK                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Input Layer                                                       │
│   ──────────                                                        │
│   • Canonical input serialization                                   │
│   • Fixed-precision data types                                      │
│   • Deterministic data loading order                                │
│                              │                                      │
│                              ▼                                      │
│   Execution Layer                                                   │
│   ───────────────                                                   │
│   • Ordered GPU kernel dispatch                                     │
│   • Controlled floating-point rounding                              │
│   • Disabled non-deterministic optimizations                        │
│   • Pinned library versions                                         │
│                              │                                      │
│                              ▼                                      │
│   Output Layer                                                      │
│   ────────────                                                      │
│   • Canonical output serialization                                  │
│   • Hash computation over outputs                                   │
│   • Metadata recording                                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 Validation Methods

#### Method 1: Replicated Compute

```
┌─────────────────────────────────────────────────────────────────────┐
│                    REPLICATED COMPUTE                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Task T                                                            │
│      │                                                              │
│      ├────────────▶ Provider A ────▶ Result_A ──┐                  │
│      │                                          │                   │
│      ├────────────▶ Provider B ────▶ Result_B ──┼──▶ Compare       │
│      │                                          │                   │
│      └────────────▶ Provider C ────▶ Result_C ──┘                  │
│                                                                     │
│   Validation: Result_A == Result_B == Result_C                     │
│   Consensus:  2-of-3 agreement required                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Method 2: Probabilistic Sampling

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PROBABILISTIC SAMPLING                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   All Tasks (1000)                                                  │
│        │                                                            │
│        ▼                                                            │
│   ┌─────────────────────────────────────────────────────────┐      │
│   │  VRF Selection: 5% sample rate                          │      │
│   └─────────────────────────────────────────────────────────┘      │
│        │                                                            │
│        ▼                                                            │
│   Selected Tasks (50) ────▶ Full Replicated Verification           │
│                                                                     │
│   Statistical Guarantee:                                            │
│   • If >1% tasks invalid, detection probability >99.9%             │
│   • Cost: 5% overhead vs 200% for full replication                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Method 3: ZK Proofs [FUTURE]

```
┌─────────────────────────────────────────────────────────────────────┐
│                    ZK-PROVED COMPUTE [FUTURE]                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Provider executes:                                                │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│   │   Input X   │───▶│  Compute    │───▶│  Output Y   │            │
│   │             │    │  f(X) = Y   │    │  + Proof π  │            │
│   └─────────────┘    └─────────────┘    └─────────────┘            │
│                                                                     │
│   Verifier checks:                                                  │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │  Verify(commitment(X), Y, π) == true                        │  │
│   │  Cost: O(1) vs O(n) for re-execution                        │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.3 Invalid Work Detection

| Detection Method | Trigger | Response |
|------------------|---------|----------|
| **Hash Mismatch** | Replicated result differs | Slash original provider |
| **Timeout** | Execution exceeds limit | Reassign task |
| **Format Error** | Output fails schema validation | Reject + minor penalty |
| **Fraud Proof** | Third-party proves invalidity | Slash + reward reporter |

### 6.4 Compute Receipts in Block Metadata

```
BlockMetadata {
    ...
    compute_receipts: [
        ComputeReceipt {
            task_id: 0x...,
            provider: 0x...,
            input_hash: 0x...,
            output_hash: 0x...,
            execution_time_ms: 1234,
            resources_used: { gpu_time: 1.2, vram_peak: 22GB },
            verification_status: Verified,
            pouw_score_delta: 150,
        },
        ...
    ],
    total_pouw_score_this_block: 4500,
}
```

---

## 7. Integration with Runtime Execution

### 7.1 Runtime ↔ Compute Engine Interface

```
┌─────────────────────────────────────────────────────────────────────┐
│              RUNTIME ↔ COMPUTE ENGINE INTEGRATION                   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Runtime Execution                        Compute Engine           │
│   ─────────────────                        ──────────────           │
│                                                                     │
│   ┌─────────────────┐                     ┌─────────────────┐      │
│   │  Transaction    │                     │  Task Queue     │      │
│   │  Processor      │                     │                 │      │
│   └────────┬────────┘                     └────────┬────────┘      │
│            │                                       │                │
│            │  1. Request Compute                   │                │
│            │─────────────────────────────────────▶│                │
│            │                                       │                │
│            │  2. Return Task ID                    │                │
│            │◀─────────────────────────────────────│                │
│            │                                       │                │
│            │        (async execution)              │                │
│            │                                       │                │
│            │  3. Query Result                      │                │
│            │─────────────────────────────────────▶│                │
│            │                                       │                │
│            │  4. Return Compute Receipt            │                │
│            │◀─────────────────────────────────────│                │
│            │                                       │                │
│            ▼                                       │                │
│   ┌─────────────────┐                             │                │
│   │  State Update   │                             │                │
│   │  (with receipt) │                             │                │
│   └─────────────────┘                             │                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 7.2 Smart Contract Compute Requests [FUTURE]

```rust
// Pseudocode: Smart contract requesting compute
contract AIModel {
    fn request_inference(input: Bytes) -> TaskId {
        // Submit compute task to engine
        let task_id = compute_engine::submit_task(
            TaskType::Inference,
            model_id: self.model,
            input: input,
            max_fee: msg.value,
        );
        
        // Store pending request
        self.pending_tasks.insert(task_id, msg.sender);
        
        return task_id;
    }
    
    fn receive_result(task_id: TaskId, receipt: ComputeReceipt) {
        // Called by runtime when compute completes
        require(receipt.verified == true);
        
        let requester = self.pending_tasks.remove(task_id);
        emit InferenceComplete(requester, receipt.output_hash);
    }
}
```

### 7.3 Gas Model for Compute Tasks

| Resource | Unit | Base Cost | Notes |
|----------|------|-----------|-------|
| **GPU Time** | GPU-second | 1000 gas | Per GPU-second used |
| **VRAM** | GB-second | 100 gas | Peak VRAM × time |
| **Data Transfer** | MB | 10 gas | Input/output data |
| **Verification** | Fixed | 500 gas | Per task verified |
| **Priority** | Multiplier | 1-10x | Urgency premium |

```
Total_Compute_Gas = (GPU_time × 1000) + (VRAM × 100) + (Data × 10) 
                    + 500 + (Base × Priority_multiplier)
```

---

## 8. Global Compute Marketplace

### 8.1 Job Submission Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    JOB SUBMISSION FLOW                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   User/Application                                                  │
│        │                                                            │
│        │  1. Prepare Job                                           │
│        │     • Upload data to IPFS                                 │
│        │     • Define task specification                           │
│        │     • Set max fee and deadline                            │
│        │                                                            │
│        ▼                                                            │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │                    RPC / API LAYER                           │  │
│   │  POST /compute/submit                                        │  │
│   │  {                                                           │  │
│   │    "type": "ml_inference",                                   │  │
│   │    "model": "llama-7b",                                      │  │
│   │    "input_cid": "Qm...",                                     │  │
│   │    "max_fee": "1000000000",                                  │  │
│   │    "deadline_blocks": 100,                                   │  │
│   │    "priority": "normal"                                      │  │
│   │  }                                                           │  │
│   └─────────────────────────────────────────────────────────────┘  │
│        │                                                            │
│        ▼                                                            │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │                  ON-CHAIN REGISTRATION                       │  │
│   │  • Task recorded in compute registry                        │  │
│   │  • Fee locked in escrow                                     │  │
│   │  • Event emitted for providers                              │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.2 Price Discovery [FUTURE]

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PRICE DISCOVERY MODEL                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Market-Based Pricing:                                             │
│   ─────────────────────                                             │
│                                                                     │
│   Price = Base_Rate × Demand_Multiplier × Urgency_Factor           │
│                                                                     │
│   Where:                                                            │
│   • Base_Rate     = Protocol minimum (covers provider costs)       │
│   • Demand_Mult   = f(queue_depth, available_capacity)             │
│   • Urgency_Factor = 1 + (MAX_PRIORITY - deadline) / MAX_PRIORITY  │
│                                                                     │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │  Price                                                      │  │
│   │   ▲                                                         │  │
│   │   │            ╱                                            │  │
│   │   │          ╱   High Demand                                │  │
│   │   │        ╱                                                │  │
│   │   │      ╱                                                  │  │
│   │   │────────────── Base Rate                                 │  │
│   │   │                                                         │  │
│   │   └──────────────────────────────────────▶ Queue Depth     │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.3 Job Batching

| Batch Type | Description | Use Case |
|------------|-------------|----------|
| **Sequential** | Tasks depend on previous outputs | Multi-step pipelines |
| **Parallel** | Independent tasks, same deadline | Bulk inference |
| **DAG** | Directed acyclic graph of dependencies | Complex workflows |

### 8.4 Priority Queues

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PRIORITY QUEUE STRUCTURE                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │  CRITICAL (10x fee)  │ Task_A │ Task_B │                    │  │
│   ├──────────────────────┴────────┴────────┴────────────────────┤  │
│   │  HIGH (5x fee)       │ Task_C │ Task_D │ Task_E │           │  │
│   ├──────────────────────┴────────┴────────┴────────┴───────────┤  │
│   │  NORMAL (1x fee)     │ Task_F │ Task_G │ Task_H │ Task_I │  │  │
│   ├──────────────────────┴────────┴────────┴────────┴────────┴──┤  │
│   │  LOW (0.5x fee)      │ Task_J │ Task_K │ ...                │  │
│   └─────────────────────────────────────────────────────────────┘  │
│                                                                     │
│   Processing Order: CRITICAL → HIGH → NORMAL → LOW                 │
│   Within tier: FIFO with deadline consideration                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 8.5 Reputation System

| Metric | Weight | Calculation |
|--------|--------|-------------|
| **Success Rate** | 40% | Successful tasks / Total tasks |
| **Timeliness** | 25% | On-time completions / Total |
| **Verification Pass** | 25% | Verified correct / Sampled |
| **Longevity** | 10% | Days active (capped) |

```
Reputation = (0.4 × Success) + (0.25 × Timeliness) + 
             (0.25 × Verification) + (0.1 × Longevity)
```

---

## 9. Security Model

### 9.1 Attack Vectors

```
┌─────────────────────────────────────────────────────────────────────┐
│                    ATTACK SURFACE ANALYSIS                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────────┐                                              │
│   │ COMPUTE LAYER   │                                              │
│   │                 │                                              │
│   │ • Invalid results                                              │
│   │ • Resource exhaustion                                          │
│   │ • Timing attacks                                               │
│   │ • Side-channel leaks                                           │
│   └────────┬────────┘                                              │
│            │                                                        │
│   ┌────────▼────────┐                                              │
│   │ CONSENSUS LAYER │                                              │
│   │                 │                                              │
│   │ • PoUW score manipulation                                      │
│   │ • Collusion attacks                                            │
│   │ • Sybil providers                                              │
│   └────────┬────────┘                                              │
│            │                                                        │
│   ┌────────▼────────┐                                              │
│   │ ECONOMIC LAYER  │                                              │
│   │                 │                                              │
│   │ • Fee manipulation                                             │
│   │ • Denial of service                                            │
│   │ • Front-running                                                │
│   └─────────────────┘                                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 9.2 Invalid Compute Prevention

| Defense | Mechanism |
|---------|-----------|
| **Stake Requirement** | Providers must stake tokens (slashable) |
| **Replicated Verification** | Random subset re-executed |
| **Fraud Proofs** | Anyone can challenge with proof |
| **Timeout Bounds** | Maximum execution time enforced |
| **Output Validation** | Format and bounds checking |

### 9.3 Collusion Detection

```
┌─────────────────────────────────────────────────────────────────────┐
│                    COLLUSION DETECTION                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Monitoring Signals:                                               │
│   ───────────────────                                               │
│   • Statistical anomalies in assignment acceptance                 │
│   • Correlated verification patterns                               │
│   • Unusual stake distribution clustering                          │
│   • Geographic/network proximity patterns                          │
│   • Timing correlation in submissions                              │
│                                                                     │
│   Detection Response:                                               │
│   ──────────────────                                                │
│   1. Flag suspicious provider cluster                              │
│   2. Increase verification sampling rate                           │
│   3. Exclude from same-task verification                           │
│   4. Governance review if confirmed                                │
│   5. Coordinated slashing if proven                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 9.4 Rate Limiting

| Limit Type | Threshold | Window |
|------------|-----------|--------|
| **Task Submission** | 100 tasks | Per block |
| **Provider Capacity** | Declared max | Continuous |
| **Verification Requests** | 10 per task | Per block |
| **Fraud Proof Submissions** | 5 | Per block |

### 9.5 Economic Incentives

```
┌─────────────────────────────────────────────────────────────────────┐
│                    INCENTIVE ALIGNMENT                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   Honest Behavior Rewards:                                          │
│   ────────────────────────                                          │
│   • Compute fees for valid work                                    │
│   • PoUW score → increased block production                        │
│   • Reputation → priority task assignment                          │
│   • Longevity bonuses                                              │
│                                                                     │
│   Dishonest Behavior Costs:                                         │
│   ──────────────────────────                                        │
│   • Stake slashing (10-100%)                                       │
│   • Reputation damage                                              │
│   • Temporary/permanent bans                                       │
│   • Lost future earnings                                           │
│                                                                     │
│   Nash Equilibrium:                                                │
│   ─────────────────                                                │
│   Expected_Honest  = Fee + PoUW_Bonus + Reputation_Value           │
│   Expected_Cheat   = (1-p) × Fee - p × (Slash + Ban_Cost)          │
│                                                                     │
│   Where p = detection probability (designed to be high)            │
│   System tuned so: Expected_Honest >> Expected_Cheat               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 10. Future Roadmap

### 10.1 Development Phases

```
┌─────────────────────────────────────────────────────────────────────┐
│                    COMPUTE ENGINE ROADMAP                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   PHASE 1: Foundation (Current)                                     │
│   ─────────────────────────────                                     │
│   ☑ Core PoUW protocol                                             │
│   ☑ Basic GPU provider support                                     │
│   ☑ Replicated verification                                        │
│   ☐ Testnet deployment                                             │
│                                                                     │
│   PHASE 2: Maturity (6-12 months)                                  │
│   ────────────────────────────────                                  │
│   ☐ Production mainnet launch                                      │
│   ☐ Multi-GPU farm support                                         │
│   ☐ Advanced reputation system                                     │
│   ☐ Price discovery mechanism                                      │
│                                                                     │
│   PHASE 3: Scale (12-24 months)                                    │
│   ──────────────────────────────                                    │
│   ☐ ZK-proved compute                                              │
│   ☐ Compute sharding                                               │
│   ☐ Cross-chain compute                                            │
│   ☐ Marketplace APIs                                               │
│                                                                     │
│   PHASE 4: Ecosystem (24+ months)                                  │
│   ───────────────────────────────                                   │
│   ☐ Decentralized model training                                   │
│   ☐ Peer-to-peer GPU scheduling                                    │
│   ☐ Developer SDKs                                                 │
│   ☐ Enterprise integrations                                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 10.2 ZK-Proved Compute

**Vision:** Replace probabilistic verification with cryptographic proofs.

| Approach | Latency | Cost | Applicability |
|----------|---------|------|---------------|
| **zkML** | High | High | ML inference |
| **RISC Zero** | Medium | Medium | General compute |
| **Custom Circuits** | Low | High dev cost | Specific tasks |

### 10.3 Decentralized Model Training

```
┌─────────────────────────────────────────────────────────────────────┐
│              DECENTRALIZED TRAINING ARCHITECTURE                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐      │
│   │ Provider 1│  │ Provider 2│  │ Provider 3│  │ Provider N│      │
│   │  Shard 1  │  │  Shard 2  │  │  Shard 3  │  │  Shard N  │      │
│   └─────┬─────┘  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘      │
│         │              │              │              │              │
│         │     Local Gradients         │              │              │
│         └──────────────┼──────────────┴──────────────┘              │
│                        │                                            │
│                        ▼                                            │
│              ┌─────────────────────┐                               │
│              │  Secure Aggregator  │                               │
│              │  (On-chain/MPC)     │                               │
│              └──────────┬──────────┘                               │
│                         │                                           │
│                         ▼                                           │
│              ┌─────────────────────┐                               │
│              │   Global Model      │                               │
│              │   Update            │                               │
│              └─────────────────────┘                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 10.4 Compute Sharding

**Goal:** Horizontal scaling of compute capacity.

- **Task Shards:** Partition task queue across validator sets
- **Provider Shards:** Regional provider clusters
- **State Shards:** Isolated compute state per shard
- **Cross-Shard:** Atomic task routing between shards

### 10.5 Peer-to-Peer GPU Scheduling

**Vision:** Direct provider-to-provider task delegation.

```
User ──▶ Network ──▶ Provider A (overloaded) ──▶ Provider B (available)
                              │
                              └── P2P delegation with fee sharing
```

### 10.6 Marketplace APIs for Developers

| API | Description |
|-----|-------------|
| **Submit API** | POST compute tasks |
| **Status API** | Query task progress |
| **Result API** | Retrieve outputs |
| **Stream API** | WebSocket task updates |
| **Batch API** | Submit task DAGs |
| **Estimate API** | Get cost/time estimates |

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | November 2025 | Compute Team | Initial document |

---

## References

- [Mbongo Chain Architecture Overview](./architecture_master_overview.md)
- [PoUW Consensus Specification](./consensus_pouw_spec.md) [FUTURE]
- [GPU Provider Setup Guide](./gpu_provider_guide.md) [FUTURE]

---

*This document is maintained by the Mbongo Chain Compute Team. For questions or contributions, please open an issue or pull request in the main repository.*

