<!-- Verified against tokenomics.md -->
# Mbongo Chain — Consensus Master Overview

> **Document Type:** Technical Specification  
> **Last Updated:** November 2025  
> **Status:** Canonical Reference

---

## Table of Contents

1. [Consensus Purpose](#1-consensus-purpose)
2. [Full Consensus Pipeline](#2-full-consensus-pipeline)
3. [Hybrid PoS + PoUW Model](#3-hybrid-pos--pouw-model)
4. [Safety & Liveness Guarantees](#4-safety--liveness-guarantees)
5. [Message Types](#5-message-types)
6. [Validation Rules Summary](#6-validation-rules-summary)
7. [Failure Modes](#7-failure-modes)
8. [Cross-Layer Integration](#8-cross-layer-integration)
9. [Future Roadmap](#9-future-roadmap)

---

## 1. Consensus Purpose

### 1.1 High-Level Guarantees

The consensus layer provides the following guarantees for Mbongo Chain:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CONSENSUS LAYER GUARANTEES                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SAFETY                              LIVENESS                                          │
│   ──────                              ────────                                          │
│   • No two honest nodes finalize      • New blocks are produced                        │
│     conflicting blocks                  in bounded time                                │
│   • Finalized blocks are never        • Transactions eventually                        │
│     reverted                            get included                                   │
│   • State transitions are             • Network recovers from                          │
│     deterministic                       temporary partitions                           │
│                                                                                         │
│   CONSISTENCY                         AVAILABILITY                                      │
│   ───────────                         ────────────                                      │
│   • All nodes agree on canonical      • System operates with                           │
│     chain                               partial validator set                          │
│   • Fork choice is deterministic      • Graceful degradation                           │
│   • Checkpoints are consistent          under attack                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Role in Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                      CONSENSUS IN MBONGO ARCHITECTURE                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                              ┌─────────────────────┐                                   │
│                              │    CONSENSUS        │                                   │
│                              │      LAYER          │                                   │
│                              └──────────┬──────────┘                                   │
│                                         │                                              │
│            ┌────────────────────────────┼────────────────────────────┐                │
│            │                            │                            │                │
│            ▼                            ▼                            ▼                │
│   ┌─────────────────┐          ┌─────────────────┐          ┌─────────────────┐       │
│   │   NETWORKING    │          │   EXECUTION     │          │    COMPUTE      │       │
│   │                 │◀────────▶│                 │◀────────▶│    (PoUW)       │       │
│   │ • Peer gossip   │          │ • State trans.  │          │ • GPU receipts  │       │
│   │ • Block prop.   │          │ • Validation    │          │ • Score calc    │       │
│   │ • Sync protocol │          │ • Commit        │          │ • Verification  │       │
│   └─────────────────┘          └─────────────────┘          └─────────────────┘       │
│                                                                                        │
│   Consensus orchestrates:                                                              │
│   • Block production timing and leader selection                                      │
│   • Vote collection and weight aggregation                                            │
│   • Finality determination                                                            │
│   • Fork choice resolution                                                            │
│                                                                                        │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Component Interactions

| Component | Consensus Reads | Consensus Writes |
|-----------|-----------------|------------------|
| **PoS Engine** | Validator set, stake weights | Slashing events |
| **PoUW Engine** | Compute receipts, scores | Task assignments |
| **Execution** | State roots, receipts | Block execution requests |
| **Networking** | Peer messages | Block broadcasts, votes |
| **Mempool** | Pending transactions | Inclusion confirmations |
| **Storage** | Historical blocks | Finalized blocks |

---

## 2. Full Consensus Pipeline

### 2.1 Master Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           CONSENSUS MASTER PIPELINE                                     │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  ════════════════════════════════════════════════════════════════════════════════════════
  PHASE 1: PEER DISCOVERY & SYNC
  ════════════════════════════════════════════════════════════════════════════════════════

  Bootstrap Nodes                        DHT Discovery
       │                                      │
       ▼                                      ▼
  ┌─────────────┐                      ┌─────────────┐
  │  Connect    │─────────────────────▶│   Gossip    │
  │  to Seeds   │                      │  Subscribe  │
  └─────────────┘                      └──────┬──────┘
                                              │
                                              ▼
                                       ┌─────────────┐
                                       │  Sync Chain │
                                       │  (Headers)  │
                                       └──────┬──────┘
                                              │
  ════════════════════════════════════════════▼═══════════════════════════════════════════
  PHASE 2: SLOT PROCESSING
  ════════════════════════════════════════════════════════════════════════════════════════

  Slot Timer                           Validator Set
       │                                    │
       ▼                                    ▼
  ┌─────────────┐                    ┌─────────────────┐
  │ New Slot    │───────────────────▶│ Leader Election │
  │ t = n       │                    │                 │
  └─────────────┘                    │ score = α×stake │
                                     │       + β×pouw  │
                                     │       + VRF(n)  │
                                     └────────┬────────┘
                                              │
                              ┌───────────────┴───────────────┐
                              │                               │
                              ▼                               ▼
                       I am Leader                     Not Leader
                              │                               │
  ════════════════════════════▼═══════════════════════════════▼═══════════════════════════
  PHASE 3: BLOCK PROPOSAL (Leader Path)
  ════════════════════════════════════════════════════════════════════════════════════════

                       ┌─────────────────┐
                       │ Collect from    │
                       │ Mempool         │
                       └────────┬────────┘
                                │
                       ┌────────▼────────┐
                       │ Include PoUW    │
                       │ Receipts        │
                       └────────┬────────┘
                                │
                       ┌────────▼────────┐
                       │ Build Block     │
                       │ Header          │
                       └────────┬────────┘
                                │
                       ┌────────▼────────┐
                       │ Sign & Broadcast│
                       │ PROPOSE         │
                       └────────┬────────┘
                                │
  ════════════════════════════════▼═══════════════════════════════════════════════════════
  PHASE 4: VOTING (All Validators)
  ════════════════════════════════════════════════════════════════════════════════════════

                       ┌─────────────────┐
                       │ Receive PROPOSE │
                       └────────┬────────┘
                                │
                       ┌────────▼────────┐
                       │ Validate Block  │
                       │ • Header        │
                       │ • Transactions  │
                       │ • PoUW Receipts │
                       └────────┬────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
               Valid                    Invalid
                    │                       │
                    ▼                       ▼
           ┌───────────────┐        ┌───────────────┐
           │ Broadcast     │        │ Reject Block  │
           │ PRE-VOTE      │        │ (No Vote)     │
           └───────┬───────┘        └───────────────┘
                   │
  ═════════════════▼══════════════════════════════════════════════════════════════════════
  PHASE 5: WEIGHT AGGREGATION
  ═════════════════════════════════════════════════════════════════════════════════════════

           ┌───────────────────────────────────────────────────────────────┐
           │                    WEIGHT CALCULATION                         │
           │                                                               │
           │   vote_weight = stake_weight × 0.50 + pouw_score × 0.50      │
           │                                                               │
           │   ┌─────────────────────────────────────────────────────────┐ │
           │   │  Validator   │  Stake  │  PoUW  │  Weight │  Vote     │ │
           │   ├──────────────┼─────────┼────────┼─────────┼───────────┤ │
           │   │  V1          │  1000   │   500  │   850   │  PRE-VOTE │ │
           │   │  V2          │   800   │   300  │   650   │  PRE-VOTE │ │
           │   │  V3          │   500   │  1000  │   650   │  PRE-VOTE │ │
           │   │  V4          │   300   │   200  │   270   │    —      │ │
           │   └─────────────────────────────────────────────────────────┘ │
           │                                                               │
           │   Total Voting Weight: 2150 / 2420 = 88.8%                   │
           │   Threshold: 66.7% (⅔ majority)                              │
           │   Result: QUORUM REACHED                                     │
           │                                                               │
           └───────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
                              ┌─────────────────┐
                              │ Broadcast       │
                              │ PRE-COMMIT      │
                              └────────┬────────┘
                                       │
  ═════════════════════════════════════▼══════════════════════════════════════════════════
  PHASE 6: RECEIPT VERIFICATION
  ═════════════════════════════════════════════════════════════════════════════════════════

                              ┌─────────────────┐
                              │ Verify PoUW     │
                              │ Receipts        │
                              └────────┬────────┘
                                       │
                    ┌──────────────────┼──────────────────┐
                    │                  │                  │
                    ▼                  ▼                  ▼
           ┌───────────────┐  ┌───────────────┐  ┌───────────────┐
           │ Task Exists?  │  │ Signature OK? │  │ Output Valid? │
           └───────┬───────┘  └───────┬───────┘  └───────┬───────┘
                   │                  │                  │
                   └──────────────────┴──────────────────┘
                                       │
                                       ▼
                              ┌─────────────────┐
                              │ Update PoUW     │
                              │ Scores          │
                              └────────┬────────┘
                                       │
  ═════════════════════════════════════▼══════════════════════════════════════════════════
  PHASE 7: FINALITY
  ═════════════════════════════════════════════════════════════════════════════════════════

                              ┌─────────────────┐
                              │ Collect         │
                              │ PRE-COMMITs     │
                              └────────┬────────┘
                                       │
                    ┌──────────────────┴──────────────────┐
                    │                                     │
                    ▼                                     ▼
           ⅔ Weight Reached                      Timeout
                    │                                     │
                    ▼                                     ▼
           ┌───────────────┐                     ┌───────────────┐
           │ FINALIZE      │                     │ Next Round    │
           │ Block         │                     │ (Fallback)    │
           └───────┬───────┘                     └───────────────┘
                   │
  ═════════════════▼══════════════════════════════════════════════════════════════════════
  PHASE 8: COMMIT & BROADCAST
  ═════════════════════════════════════════════════════════════════════════════════════════

                   │
                   ▼
           ┌───────────────┐
           │ Execute Block │
           │ (State Trans.)│
           └───────┬───────┘
                   │
                   ▼
           ┌───────────────┐
           │ Verify State  │
           │ Root Match    │
           └───────┬───────┘
                   │
                   ▼
           ┌───────────────┐
           │ Commit to     │
           │ Storage       │
           └───────┬───────┘
                   │
                   ▼
           ┌───────────────┐
           │ Broadcast     │
           │ Final Block   │
           └───────┬───────┘
                   │
                   ▼
              ┌─────────┐
              │  DONE   │
              └─────────┘
```

### 2.2 Phase Timing

| Phase | Duration | Trigger |
|-------|----------|---------|
| Slot Start | 0ms | Timer |
| Proposal Window | 0-500ms | Leader broadcasts |
| PRE-VOTE Window | 500-1500ms | Validators respond |
| PRE-COMMIT Window | 1500-2000ms | Aggregate + commit |
| Finality | 2000ms | Block finalized |

---

## 3. Hybrid PoS + PoUW Model

### 3.1 Stake Weight Computation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           STAKE WEIGHT CALCULATION                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Raw Stake:                                                                            │
│   ──────────                                                                            │
│   raw_stake = validator.bonded_tokens                                                  │
│                                                                                         │
│   Normalized Stake:                                                                     │
│   ─────────────────                                                                     │
│   normalized_stake = raw_stake / Σ(all_validator_stakes)                               │
│                                                                                         │
│   Effective Stake Weight:                                                               │
│   ───────────────────────                                                               │
│   stake_weight = normalized_stake × STAKE_COEFFICIENT                                  │
│                                                                                         │
│   Where:                                                                                │
│   • STAKE_COEFFICIENT = 0.50 (50% weight from stake)                                   │
│   • Minimum stake required: 10,000 MBO                                                 │
│   • Maximum stake cap: 10% of total supply per validator                               │
│                                                                                         │
│   Example:                                                                              │
│   ────────                                                                              │
│   Validator A: 50,000 MBO staked                                                       │
│   Total staked: 500,000 MBO                                                            │
│   normalized_stake = 50,000 / 500,000 = 0.10 (10%)                                     │
│   stake_weight = 0.10 × 0.50 = 0.05 (5% of total weight)                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Compute Receipt Contribution (PoUW)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           PoUW SCORE CONTRIBUTION                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Raw PoUW Score:                                                                       │
│   ───────────────                                                                       │
│   raw_pouw = Σ(completed_tasks × task_difficulty × verification_multiplier)            │
│                                                                                         │
│   Normalized PoUW:                                                                      │
│   ────────────────                                                                      │
│   normalized_pouw = raw_pouw / Σ(all_validator_pouw_scores)                            │
│                                                                                         │
│   Effective PoUW Weight:                                                                │
│   ──────────────────────                                                                │
│   pouw_weight = normalized_pouw × POUW_COEFFICIENT                                     │
│                                                                                         │
│   Where:                                                                                │
│   • POUW_COEFFICIENT = 0.50 (50% weight from compute)                                  │
│   • Task difficulty: 1-100 based on compute requirements                               │
│   • Verification multiplier: 1.0 (verified), 0.0 (failed)                              │
│                                                                                         │
│   Combined Weight:                                                                      │
│   ────────────────                                                                      │
│   total_weight = stake_weight + pouw_weight                                            │
│                = (norm_stake × 0.50) + (norm_pouw × 0.50)                              │
│                                                                                         │
│   Example:                                                                              │
│   ────────                                                                              │
│   Validator B:                                                                          │
│   • normalized_stake = 0.08 → stake_weight = 0.056 (5.6%)                             │
│   • normalized_pouw = 0.15 → pouw_weight = 0.045 (4.5%)                               │
│   • total_weight = 0.056 + 0.045 = 0.101 (10.1%)                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Slashing Conditions

| Condition | Severity | Penalty | Evidence Required |
|-----------|----------|---------|-------------------|
| **Double Signing** | Critical | 100% stake | Two signed blocks at same height |
| **Invalid Block Proposal** | High | 10% stake | Block fails validation |
| **PoUW Fraud** | High | 50% stake | Verified incorrect compute result |
| **Prolonged Downtime** | Medium | 1% stake/day | Missed attestations |
| **Invalid Attestation** | Medium | 5% stake | Attestation for invalid block |
| **Equivocation** | Critical | 100% stake | Conflicting votes in same round |

### 3.4 Threat Model & Mitigations

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              THREAT MODEL                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   THREAT                     │  MITIGATION                                             │
│   ──────                     │  ──────────                                             │
│                              │                                                          │
│   51% Stake Attack           │  • High capital requirement                             │
│   (Adversary controls        │  • PoUW dilutes pure stake power                        │
│   majority stake)            │  • Slashing makes attack costly                         │
│                              │                                                          │
│   PoUW Manipulation          │  • Replicated verification                              │
│   (Fake compute results)     │  • Random verifier assignment                           │
│                              │  • Economic penalties for fraud                         │
│                              │                                                          │
│   Long-Range Attack          │  • Checkpoint finality                                  │
│   (Rewrite old history)      │  • Weak subjectivity windows                            │
│                              │  • Social consensus on checkpoints                      │
│                              │                                                          │
│   Validator Collusion        │  • VRF-based leader selection                           │
│   (Coordinated misbehavior)  │  • Distributed validator set                            │
│                              │  • Public slashing evidence                             │
│                              │                                                          │
│   Network Partition          │  • Timeout-based round advancement                      │
│   (Split brain scenario)     │  • View change protocol                                 │
│                              │  • Minority chain detection                             │
│                              │                                                          │
│   Nothing-at-Stake           │  • Slashing for equivocation                            │
│   (Vote on all forks)        │  • Deposit lockup period                                │
│                              │  • Attribution of conflicting votes                     │
│                              │                                                          │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Safety & Liveness Guarantees

### 4.1 Deterministic Finality Rules

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           FINALITY RULES                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   RULE F1: Quorum Requirement                                                          │
│   ───────────────────────────                                                           │
│   A block is finalized IFF:                                                            │
│     Σ(PRE-COMMIT weights) ≥ ⅔ × total_weight                                          │
│                                                                                         │
│   RULE F2: Single Finality per Height                                                  │
│   ────────────────────────────────────                                                  │
│   At most ONE block can be finalized at each height:                                   │
│     ∀ height h: |{b : finalized(b) ∧ b.height = h}| ≤ 1                               │
│                                                                                         │
│   RULE F3: Finality Irreversibility                                                    │
│   ─────────────────────────────────                                                     │
│   Once finalized, a block cannot be reverted:                                          │
│     finalized(b) ⟹ ∀ future_state: b ∈ canonical_chain                               │
│                                                                                         │
│   RULE F4: Ancestry Requirement                                                        │
│   ────────────────────────────                                                          │
│   A block can only be finalized if its parent is finalized:                            │
│     finalized(b) ⟹ finalized(b.parent) ∨ b.height = 0                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Timeout and Fallback Voting

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           TIMEOUT PROTOCOL                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Round r, Slot s                                                                       │
│        │                                                                                │
│        ▼                                                                                │
│   ┌─────────────┐                                                                      │
│   │ Wait for    │─── Proposal received ───▶ Normal voting flow                         │
│   │ PROPOSE     │                                                                      │
│   │ (500ms)     │─── Timeout ───┐                                                      │
│   └─────────────┘               │                                                      │
│                                 ▼                                                      │
│                          ┌─────────────┐                                               │
│                          │ Broadcast   │                                               │
│                          │ NIL-VOTE    │                                               │
│                          └──────┬──────┘                                               │
│                                 │                                                      │
│   ┌─────────────┐               │                                                      │
│   │ Wait for    │◀──────────────┘                                                      │
│   │ ⅔ NIL-VOTES │                                                                      │
│   │ (1000ms)    │─── ⅔ NIL received ───▶ View Change (round r+1)                       │
│   └─────────────┘                                                                      │
│        │                                                                                │
│        │─── Timeout ───▶ Force View Change                                             │
│                                                                                         │
│   View Change Protocol:                                                                 │
│   ─────────────────────                                                                 │
│   1. Increment round counter: r' = r + 1                                               │
│   2. Select new leader for round r'                                                    │
│   3. Reset timeout timers                                                              │
│   4. Carry over locked block (if any)                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Canonical Chain Rule

```
FORK CHOICE RULE:

canonical_chain = argmax(chain, weight(chain))

where:
  weight(chain) = Σ(block_weight for block in chain)
  
  block_weight = base_difficulty 
               + attestation_weight 
               + pouw_bonus
               
  attestation_weight = Σ(attester.weight for attester in attestations)
  pouw_bonus = Σ(receipt.score for receipt in block.pouw_receipts) × POUW_BLOCK_MULTIPLIER

TIE-BREAKING:
  If weight(chain_A) == weight(chain_B):
    canonical = chain with lower block_hash at fork point
```

### 4.4 Reorg Protection

| Protection | Mechanism | Guarantee |
|------------|-----------|-----------|
| **Finality Depth** | Blocks with ⅔ commits are final | No reorg of finalized blocks |
| **Checkpoint Anchors** | Periodic finality checkpoints | Social consensus recovery point |
| **Lock Mechanism** | Validators lock on PRE-COMMIT | Prevents voting on conflicting chains |
| **Slashing** | Punish conflicting votes | Economic disincentive for reorgs |
| **Weak Subjectivity** | Trust recent finalized state | Prevents long-range attacks |

---

## 5. Message Types

### 5.1 Core Consensus Messages

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           CONSENSUS MESSAGE TYPES                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PROPOSE                                                                               │
│   ───────                                                                               │
│   {                                                                                     │
│     type: "PROPOSE",                                                                   │
│     slot: u64,                                                                         │
│     round: u32,                                                                        │
│     block: Block,                                                                      │
│     pouw_receipts: Vec<ComputeReceipt>,                                               │
│     proposer_signature: Signature,                                                     │
│   }                                                                                     │
│   Purpose: Leader proposes new block for slot                                          │
│                                                                                         │
│   PRE-VOTE                                                                              │
│   ────────                                                                              │
│   {                                                                                     │
│     type: "PRE_VOTE",                                                                  │
│     slot: u64,                                                                         │
│     round: u32,                                                                        │
│     block_hash: Hash,          // Hash of proposed block (or NIL)                      │
│     validator: ValidatorId,                                                            │
│     signature: Signature,                                                              │
│   }                                                                                     │
│   Purpose: Validator signals acceptance of proposal                                    │
│                                                                                         │
│   PRE-COMMIT                                                                            │
│   ──────────                                                                            │
│   {                                                                                     │
│     type: "PRE_COMMIT",                                                                │
│     slot: u64,                                                                         │
│     round: u32,                                                                        │
│     block_hash: Hash,                                                                  │
│     validator: ValidatorId,                                                            │
│     stake_weight: u64,                                                                 │
│     pouw_score: u64,                                                                   │
│     signature: Signature,                                                              │
│   }                                                                                     │
│   Purpose: Validator commits to block after seeing ⅔ PRE-VOTEs                        │
│                                                                                         │
│   RECEIPT                                                                               │
│   ───────                                                                               │
│   {                                                                                     │
│     type: "RECEIPT",                                                                   │
│     task_id: TaskId,                                                                   │
│     provider: Address,                                                                 │
│     input_hash: Hash,                                                                  │
│     output_hash: Hash,                                                                 │
│     execution_time: u64,                                                               │
│     pouw_score: u64,                                                                   │
│     verification_status: VerificationStatus,                                           │
│     signature: Signature,                                                              │
│   }                                                                                     │
│   Purpose: GPU provider submits compute proof                                          │
│                                                                                         │
│   FINALIZE                                                                              │
│   ────────                                                                              │
│   {                                                                                     │
│     type: "FINALIZE",                                                                  │
│     slot: u64,                                                                         │
│     block_hash: Hash,                                                                  │
│     commit_signatures: Vec<(ValidatorId, Signature)>,                                 │
│     aggregate_weight: u64,                                                             │
│   }                                                                                     │
│   Purpose: Announce block finalization                                                 │
│                                                                                         │
│   CHECKPOINT [Future]                                                                   │
│   ────────────────────                                                                  │
│   {                                                                                     │
│     type: "CHECKPOINT",                                                                │
│     epoch: u64,                                                                        │
│     finalized_block: Hash,                                                             │
│     state_root: Hash,                                                                  │
│     validator_set_hash: Hash,                                                          │
│     signatures: Vec<(ValidatorId, Signature)>,                                        │
│   }                                                                                     │
│   Purpose: Periodic finality anchor for fast sync                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Message Flow Diagram

```
  Leader                    Validators (V1, V2, V3)              Network
    │                              │                                │
    │────── PROPOSE ──────────────▶│                                │
    │                              │                                │
    │                              │──── Validate Block ────        │
    │                              │                                │
    │◀───── PRE-VOTE ──────────────│                                │
    │◀───── PRE-VOTE ──────────────│                                │
    │◀───── PRE-VOTE ──────────────│                                │
    │                              │                                │
    │────── Aggregate ─────────────│                                │
    │                              │                                │
    │◀───── PRE-COMMIT ────────────│                                │
    │◀───── PRE-COMMIT ────────────│                                │
    │◀───── PRE-COMMIT ────────────│                                │
    │                              │                                │
    │────── FINALIZE ─────────────▶│───────────────────────────────▶│
    │                              │                                │
```

---

## 6. Validation Rules Summary

### 6.1 Proposal Validity

| Rule | Condition | Error |
|------|-----------|-------|
| **PRO-001** | Proposer matches slot leader | `InvalidProposer` |
| **PRO-002** | Slot number is current or next | `InvalidSlot` |
| **PRO-003** | Round number is current | `InvalidRound` |
| **PRO-004** | Parent block exists | `UnknownParent` |
| **PRO-005** | Block height = parent + 1 | `InvalidHeight` |
| **PRO-006** | Timestamp > parent.timestamp | `InvalidTimestamp` |
| **PRO-007** | Proposer signature valid | `InvalidSignature` |
| **PRO-008** | Transactions root matches | `TxRootMismatch` |
| **PRO-009** | Block size within limits | `OversizedBlock` |

### 6.2 Weight Aggregation

| Rule | Condition | Error |
|------|-----------|-------|
| **WGT-001** | Validator in active set | `UnknownValidator` |
| **WGT-002** | Stake > 0 | `ZeroStake` |
| **WGT-003** | Weight = stake×0.7 + pouw×0.3 | `WeightMismatch` |
| **WGT-004** | Total ≤ 100% | `WeightOverflow` |
| **WGT-005** | No double-counting | `DuplicateWeight` |

### 6.3 Receipt Validity

| Rule | Condition | Error |
|------|-----------|-------|
| **RCP-001** | Task ID exists | `TaskNotFound` |
| **RCP-002** | Provider registered | `UnknownProvider` |
| **RCP-003** | Output hash matches verified | `OutputMismatch` |
| **RCP-004** | Provider signature valid | `InvalidReceiptSig` |
| **RCP-005** | Not already processed | `DuplicateReceipt` |
| **RCP-006** | Score within bounds | `ScoreOutOfBounds` |

### 6.4 Peer Legitimacy

| Rule | Condition | Error |
|------|-----------|-------|
| **PER-001** | Peer ID known | `UnknownPeer` |
| **PER-002** | Connection authorized | `Unauthorized` |
| **PER-003** | Rate limit not exceeded | `RateLimited` |
| **PER-004** | Not banned | `PeerBanned` |

### 6.5 Conflict Detection

| Rule | Condition | Error |
|------|-----------|-------|
| **CNF-001** | No double PRE-VOTE same round | `DoubleVote` |
| **CNF-002** | No double PRE-COMMIT same round | `DoubleCommit` |
| **CNF-003** | No conflicting block proposals | `DoublePropose` |
| **CNF-004** | Locked block respected | `LockViolation` |

### 6.6 Signature Requirements

| Message | Required Signature | Key Type |
|---------|-------------------|----------|
| PROPOSE | Proposer | Validator key |
| PRE-VOTE | Voter | Validator key |
| PRE-COMMIT | Committer | Validator key |
| RECEIPT | Provider | Provider key |
| FINALIZE | Aggregated | BLS aggregate |
| CHECKPOINT | Quorum | BLS aggregate |

### 6.7 Round Transition Rules

| Condition | Action |
|-----------|--------|
| ⅔ PRE-VOTEs for block B | Progress to PRE-COMMIT |
| ⅔ PRE-VOTEs for NIL | View change to round r+1 |
| ⅔ PRE-COMMITs for block B | Finalize B |
| Timeout (no ⅔) | View change to round r+1 |
| Conflicting votes detected | Slash offender |

---

## 7. Failure Modes

### 7.1 Failure Categories

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           CONSENSUS FAILURE MODES                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CATEGORY              │  FAILURES                    │  RECOVERY                     │
│   ────────              │  ────────                    │  ────────                     │
│                         │                              │                               │
│   PROPOSAL              │  • InvalidProposal           │  Wait for next slot           │
│                         │  • MissingProposal           │  Timeout → view change        │
│                         │  • MalformedBlock            │  Reject, log                  │
│                         │                              │                               │
│   TIMING                │  • ProposalTimeout           │  NIL vote, advance round      │
│                         │  • VoteTimeout               │  Force view change            │
│                         │  • CommitTimeout             │  Retry round                  │
│                         │                              │                               │
│   WEIGHT                │  • WeightMismatch            │  Recalculate, reject if off   │
│                         │  • InsufficientQuorum        │  Wait for more votes          │
│                         │  • WeightOverflow            │  Cap at maximum               │
│                         │                              │                               │
│   RECEIPT               │  • MissingReceipts           │  Process without, log         │
│                         │  • InvalidReceipt            │  Skip receipt, slash          │
│                         │  • DuplicateReceipt          │  Ignore duplicate             │
│                         │                              │                               │
│   VOTE                  │  • ConflictingVotes          │  Detect, slash offender       │
│                         │  • InvalidVoteSignature      │  Reject vote                  │
│                         │  • VoteForUnknownBlock       │  Request block                │
│                         │                              │                               │
│   HEADER                │  • HeaderMismatch            │  Reject block                 │
│                         │  • InvalidParent             │  Request parent               │
│                         │  • FutureTimestamp           │  Hold until valid             │
│                         │                              │                               │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Error Codes

| Error | Code | Description |
|-------|------|-------------|
| `InvalidProposal` | `C1001` | Proposal fails validation |
| `ProposalTimeout` | `C1002` | No proposal in time window |
| `WeightMismatch` | `C2001` | Calculated weight differs |
| `InsufficientQuorum` | `C2002` | <⅔ weight reached |
| `MissingReceipts` | `C3001` | PoUW receipts not included |
| `InvalidReceipt` | `C3002` | Receipt verification failed |
| `ConflictingVotes` | `C4001` | Double voting detected |
| `InvalidVoteSig` | `C4002` | Vote signature invalid |
| `HeaderMismatch` | `C5001` | Header fields inconsistent |

---

## 8. Cross-Layer Integration

### 8.1 Integration Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CONSENSUS CROSS-LAYER INTEGRATION                               │
└─────────────────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────────┐
                              │     CONSENSUS       │
                              │       LAYER         │
                              └──────────┬──────────┘
                                         │
       ┌─────────────┬───────────────────┼───────────────────┬─────────────┐
       │             │                   │                   │             │
       ▼             ▼                   ▼                   ▼             ▼
┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  EXECUTION  │ │   STATE     │ │   MEMPOOL   │ │  NETWORKING │ │   COMPUTE   │
│   ENGINE    │ │   MACHINE   │ │             │ │   /GOSSIP   │ │  MARKETPLACE│
└──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘ └──────┬──────┘
       │               │               │               │               │
       │               │               │               │               │
  ┌────▼────┐     ┌────▼────┐     ┌────▼────┐     ┌────▼────┐     ┌────▼────┐
  │ Execute │     │ Verify  │     │ Select  │     │ Receive │     │  Score  │
  │ Block   │     │ State   │     │ Txs for │     │ Msgs &  │     │ Compute │
  │         │     │ Root    │     │ Block   │     │ Bcast   │     │ Results │
  └─────────┘     └─────────┘     └─────────┘     └─────────┘     └─────────┘
```

### 8.2 Integration Details

#### Consensus → Execution Engine

| Operation | Data Flow | Trigger |
|-----------|-----------|---------|
| **Execute Block** | Block → Execution | Block finalized |
| **Verify State** | State root comparison | Post-execution |
| **Rollback** | Revert command | Fork detected |

```rust
// Interface: Consensus calls Execution
trait ExecutionInterface {
    fn execute_block(&mut self, block: &Block) -> ExecutionResult;
    fn verify_state_root(&self, expected: Hash) -> bool;
    fn rollback_to(&mut self, block_hash: Hash) -> Result<()>;
}
```

#### Consensus → State Machine

| Operation | Data Flow | Trigger |
|-----------|-----------|---------|
| **Get State** | Query current state | Block building |
| **Apply Diff** | State changes | Block commit |
| **Checkpoint** | State snapshot | Epoch boundary |

#### Consensus → Mempool

| Operation | Data Flow | Trigger |
|-----------|-----------|---------|
| **Get Transactions** | Pending txs → Block | Proposal creation |
| **Remove Included** | Tx hashes | Block finalized |
| **Reinsert** | Txs from reverted block | Fork reorg |

```rust
// Interface: Consensus calls Mempool
trait MempoolInterface {
    fn get_transactions(&self, max_gas: u64, max_count: usize) -> Vec<Transaction>;
    fn remove_transactions(&mut self, tx_hashes: &[Hash]);
    fn reinsert_transactions(&mut self, txs: Vec<Transaction>);
}
```

#### Consensus → Networking/Gossip

| Operation | Data Flow | Trigger |
|-----------|-----------|---------|
| **Broadcast Block** | Block → Peers | Block proposed |
| **Broadcast Vote** | Vote → Peers | Vote cast |
| **Request Block** | Hash → Peer | Unknown parent |
| **Sync Request** | Range → Peers | Behind chain |

#### Consensus → GPU Compute Marketplace

| Operation | Data Flow | Trigger |
|-----------|-----------|---------|
| **Get Receipts** | Verified receipts | Proposal creation |
| **Submit Receipts** | Receipts → Block | Block building |
| **Update Scores** | Score deltas | Block finalized |
| **Slash Provider** | Slash command | Fraud detected |

```rust
// Interface: Consensus calls Compute
trait ComputeInterface {
    fn get_verified_receipts(&self, max_count: usize) -> Vec<ComputeReceipt>;
    fn update_pouw_scores(&mut self, block: &Block);
    fn slash_provider(&mut self, provider: Address, reason: SlashReason);
    fn get_provider_score(&self, provider: Address) -> u64;
}
```

---

## 9. Future Roadmap

### 9.1 Development Timeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CONSENSUS ROADMAP                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PHASE 1: Foundation (Current)                                                         │
│   ─────────────────────────────                                                         │
│   ☑ Basic PoS consensus                                                                │
│   ☑ PoUW score integration                                                             │
│   ☐ Slashing implementation                                                            │
│   ☐ Testnet deployment                                                                 │
│                                                                                         │
│   PHASE 2: Optimization (6-12 months)                                                  │
│   ────────────────────────────────────                                                  │
│   ☐ BLS signature aggregation                                                          │
│   ☐ Parallel vote processing                                                           │
│   ☐ Optimized fork choice                                                              │
│   ☐ Enhanced timeout handling                                                          │
│                                                                                         │
│   PHASE 3: Advanced Features (12-18 months)                                            │
│   ──────────────────────────────────────────                                            │
│   ☐ ZK-enabled receipts                                                                │
│   ☐ Multi-shard coordination                                                           │
│   ☐ Cross-chain finality proofs                                                        │
│                                                                                         │
│   PHASE 4: Ecosystem (18-24 months)                                                    │
│   ─────────────────────────────────                                                     │
│   ☐ Lightweight consensus for sidechains                                               │
│   ☐ Pluggable consensus modules                                                        │
│   ☐ Formal verification                                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 ZK-Enabled Receipts

```
Current: Replicated Verification
──────────────────────────────────
Provider executes → Multiple verifiers re-execute → Compare results

Future: ZK-Proved Computation
─────────────────────────────
Provider executes → Generate ZK proof → Single verification O(1)

Benefits:
• Verification cost: O(n) → O(1)
• No need for replicated execution
• Instant result finality
• Reduced network overhead

Implementation Path:
1. zkML integration for ML workloads
2. RISC Zero for general compute
3. Custom circuits for high-frequency tasks
```

### 9.3 Parallel Voting

```
Current: Sequential Vote Processing
───────────────────────────────────
Vote1 → Verify → Aggregate → Vote2 → Verify → Aggregate → ...

Future: Parallel Vote Processing
────────────────────────────────
┌── Vote1 → Verify ──┐
├── Vote2 → Verify ──┼──▶ Batch Aggregate
├── Vote3 → Verify ──┤
└── Vote4 → Verify ──┘

Benefits:
• Faster finality (100ms → 20ms per round)
• Better validator scalability
• Reduced latency under load
```

### 9.4 Multi-Shard Finality

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MULTI-SHARD CONSENSUS [FUTURE]                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Shard 0              Shard 1              Shard 2                                    │
│   ───────              ───────              ───────                                    │
│   ┌───────┐            ┌───────┐            ┌───────┐                                  │
│   │ Block │            │ Block │            │ Block │                                  │
│   │  0.N  │            │  1.N  │            │  2.N  │                                  │
│   └───┬───┘            └───┬───┘            └───┬───┘                                  │
│       │                    │                    │                                      │
│       └────────────────────┼────────────────────┘                                      │
│                            │                                                            │
│                            ▼                                                            │
│                    ┌───────────────┐                                                   │
│                    │ Beacon Chain  │                                                   │
│                    │ (Coordinator) │                                                   │
│                    └───────────────┘                                                   │
│                            │                                                            │
│                            ▼                                                            │
│                    ┌───────────────┐                                                   │
│                    │ Cross-Shard   │                                                   │
│                    │ Finality      │                                                   │
│                    └───────────────┘                                                   │
│                                                                                         │
│   Properties:                                                                          │
│   • Each shard has independent consensus                                              │
│   • Beacon chain coordinates cross-shard finality                                     │
│   • Atomic cross-shard transactions                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.5 Lightweight Consensus for Sidechains

| Feature | Main Chain | Sidechain |
|---------|------------|-----------|
| **Validator Set** | Full set | Subset or federated |
| **Finality** | ⅔ quorum | Configurable threshold |
| **PoUW** | Full integration | Optional |
| **Security** | Economic (stake) | Anchored to main chain |
| **Throughput** | Standard | Higher (specialized) |

```
Main Chain                              Sidechain
──────────                              ─────────
Block N ◀───────── Anchor ─────────────▶ Block M
   │                                        │
   │    Periodic state commitment           │
   │◀───────────────────────────────────────│
   │                                        │
   │    Fraud proofs (if needed)            │
   │◀───────────────────────────────────────│
```

---

## Appendix: Consensus Configuration Parameters

```toml
[consensus]
# Timing
slot_duration_ms = 2000
proposal_timeout_ms = 500
prevote_timeout_ms = 1000
precommit_timeout_ms = 500

# Weight coefficients
stake_coefficient = 0.50
pouw_coefficient = 0.50

# Thresholds
quorum_threshold = 0.667          # ⅔ majority
proposal_threshold = 0.50         # Simple majority for proposal acceptance

# Limits
max_validators = 1000
min_stake = 10000                 # MBO tokens
max_stake_ratio = 0.10            # 10% of total supply

# Slashing
double_sign_slash = 1.00          # 100% of stake
invalid_block_slash = 0.10        # 10% of stake
downtime_slash_per_day = 0.01     # 1% per day

# Checkpoints
checkpoint_interval = 1000        # blocks
```

---

*This document is the canonical consensus specification for Mbongo Chain. All consensus implementations must conform to these rules.*

