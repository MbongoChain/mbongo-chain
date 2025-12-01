<!-- Verified against tokenomics.md -->
# Mbongo Chain — Consensus Validation Summary

> **Document Type:** Technical Specification  
> **Last Updated:** November 2025  
> **Status:** Canonical Reference

---

## Table of Contents

1. [Overview](#1-overview)
2. [Validation Rules — Full Table](#2-validation-rules--full-table)
3. [Validation Pipeline Diagram](#3-validation-pipeline-diagram)
4. [Failure Modes & Detection](#4-failure-modes--detection)
5. [Developer Notes](#5-developer-notes)

---

## 1. Overview

### 1.1 Purpose of Consensus Validation

Consensus validation ensures that all participants in the Mbongo Chain network agree on the canonical chain state. The validation layer enforces protocol rules at the consensus level, preventing invalid blocks, fraudulent votes, and malicious compute receipts from affecting chain integrity.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                      CONSENSUS VALIDATION OBJECTIVES                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PRIMARY OBJECTIVES                                                                    │
│   ──────────────────                                                                    │
│   1. PROPOSAL INTEGRITY     Ensure only valid blocks from legitimate proposers         │
│   2. VOTE AUTHENTICITY      Verify all votes are signed by active validators           │
│   3. WEIGHT CORRECTNESS     Validate stake + PoUW weight calculations                  │
│   4. RECEIPT SOUNDNESS      Confirm PoUW compute receipts are legitimate               │
│   5. FINALITY SAFETY        Prevent conflicting finalizations                          │
│                                                                                         │
│   SECONDARY OBJECTIVES                                                                  │
│   ────────────────────                                                                  │
│   • Detect and slash misbehaving validators                                            │
│   • Maintain consensus liveness under network partitions                               │
│   • Support fast synchronization with verifiable checkpoints                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Integration Layer

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                      CONSENSUS VALIDATION IN PROTOCOL STACK                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           NETWORKING LAYER                                       │  │
│   │   • Message deserialization                                                      │  │
│   │   • Peer authorization                                                           │  │
│   │   • Rate limiting                                                                │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                         │                                              │
│                                         ▼                                              │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                      ══► CONSENSUS VALIDATION LAYER ◄══                          │  │
│   │                                                                                  │  │
│   │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐        │  │
│   │   │  Proposal   │   │    Vote     │   │   Receipt   │   │  Integrity  │        │  │
│   │   │ Validation  │   │ Validation  │   │ Validation  │   │   Checks    │        │  │
│   │   └─────────────┘   └─────────────┘   └─────────────┘   └─────────────┘        │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                         │                                              │
│                    ┌────────────────────┼────────────────────┐                        │
│                    │                    │                    │                        │
│                    ▼                    ▼                    ▼                        │
│   ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐          │
│   │      MEMPOOL        │  │     EXECUTION       │  │      STORAGE        │          │
│   │                     │  │                     │  │                     │          │
│   │ • Tx selection      │  │ • Block execution   │  │ • Block persistence │          │
│   │ • Priority updates  │  │ • State transition  │  │ • State commits     │          │
│   └─────────────────────┘  └─────────────────────┘  └─────────────────────┘          │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Deterministic Guarantees

#### Safety Guarantees

| Guarantee | Description | Enforcement |
|-----------|-------------|-------------|
| **No Conflicting Finality** | At most one block finalized per height | Quorum requirement (⅔) |
| **No Invalid State** | Only valid state transitions committed | Execution verification |
| **No Forged Votes** | All votes from legitimate validators | Signature verification |
| **No Fake Receipts** | PoUW receipts verified before inclusion | Receipt validation |

#### Liveness Guarantees

| Guarantee | Description | Enforcement |
|-----------|-------------|-------------|
| **Progress** | New blocks produced in bounded time | Timeout + view change |
| **Inclusion** | Valid transactions eventually included | Mempool + proposer duty |
| **Recovery** | Network recovers from partitions | View change protocol |
| **Availability** | System operates with partial validators | Quorum threshold |

```
SAFETY INVARIANT:
  ∀ height h, ∀ nodes N1, N2:
    finalized(N1, h) = B1 ∧ finalized(N2, h) = B2 ⟹ B1 = B2

LIVENESS INVARIANT:
  ∀ valid tx T, ∃ bounded time t:
    submitted(T, time=0) ⟹ included(T, time≤t) ∨ rejected(T, time≤t)
```

---

## 2. Validation Rules — Full Table

### 2.A Proposal Validation

#### A.1 Slot/Round Validity

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PRO-S01` | Slot Current | `proposal.slot ∈ {current_slot, current_slot + 1}` | `E_INVALID_SLOT` |
| `PRO-S02` | Slot Not Past | `proposal.slot ≥ last_finalized_slot` | `E_SLOT_TOO_OLD` |
| `PRO-S03` | Round Valid | `proposal.round ≥ 0 ∧ proposal.round ≤ MAX_ROUNDS` | `E_INVALID_ROUND` |
| `PRO-S04` | Round Sequence | `proposal.round ≥ last_seen_round[slot]` | `E_ROUND_REGRESSION` |
| `PRO-S05` | Slot Timing | `slot_start ≤ now ≤ slot_end` | `E_SLOT_TIMING` |

#### A.2 Proposer Legitimacy

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PRO-P01` | In Validator Set | `proposer ∈ active_validators` | `E_UNKNOWN_PROPOSER` |
| `PRO-P02` | Is Slot Leader | `proposer == compute_leader(slot, round)` | `E_NOT_LEADER` |
| `PRO-P03` | Not Slashed | `slashed[proposer] == false` | `E_PROPOSER_SLASHED` |
| `PRO-P04` | Stake Sufficient | `stake[proposer] ≥ MIN_STAKE` | `E_INSUFFICIENT_STAKE` |
| `PRO-P05` | Not Jailed | `jailed_until[proposer] < current_slot` | `E_PROPOSER_JAILED` |

#### A.3 Block Header Checks

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PRO-H01` | Parent Exists | `parent_hash ∈ known_blocks` | `E_UNKNOWN_PARENT` |
| `PRO-H02` | Height Sequential | `height == parent.height + 1` | `E_INVALID_HEIGHT` |
| `PRO-H03` | Timestamp Valid | `timestamp > parent.timestamp` | `E_INVALID_TIMESTAMP` |
| `PRO-H04` | Timestamp Bound | `timestamp ≤ now + MAX_FUTURE_TIME` | `E_FUTURE_TIMESTAMP` |
| `PRO-H05` | State Root Format | `len(state_root) == 32` | `E_INVALID_STATE_ROOT` |
| `PRO-H06` | Tx Root Matches | `merkle_root(txs) == tx_root` | `E_TX_ROOT_MISMATCH` |
| `PRO-H07` | Receipts Root Format | `len(receipts_root) == 32` | `E_INVALID_RECEIPTS_ROOT` |
| `PRO-H08` | Block Size | `serialized_size(block) ≤ MAX_BLOCK_SIZE` | `E_OVERSIZED_BLOCK` |
| `PRO-H09` | Gas Limit | `block.gas_limit ≤ MAX_GAS_LIMIT` | `E_GAS_LIMIT_EXCEEDED` |

#### A.4 Commit/Receipt Linkage

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PRO-L01` | PoUW Root Matches | `merkle_root(pouw_receipts) == pouw_root` | `E_POUW_ROOT_MISMATCH` |
| `PRO-L02` | Receipt Count | `len(pouw_receipts) ≤ MAX_RECEIPTS_PER_BLOCK` | `E_TOO_MANY_RECEIPTS` |
| `PRO-L03` | Receipt Unique | `∀ r1, r2 ∈ receipts: r1.task_id ≠ r2.task_id` | `E_DUPLICATE_RECEIPT` |
| `PRO-L04` | Commit Reference | `parent_commit.block_hash == parent_hash` | `E_COMMIT_MISMATCH` |

#### A.5 Signature Requirements

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PRO-SIG01` | Signature Present | `proposal.signature ≠ null` | `E_MISSING_SIGNATURE` |
| `PRO-SIG02` | Signature Valid | `verify(proposer_pubkey, header_hash, signature)` | `E_INVALID_SIGNATURE` |
| `PRO-SIG03` | Signature Scheme | `signature.scheme ∈ SUPPORTED_SCHEMES` | `E_UNSUPPORTED_SCHEME` |
| `PRO-SIG04` | No Double Propose | `¬∃ other: other.proposer == proposer ∧ other.slot == slot` | `E_DOUBLE_PROPOSAL` |

---

### 2.B Vote Validation

#### B.1 PREVOTE Rules

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PV-01` | Voter Valid | `voter ∈ active_validators` | `E_UNKNOWN_VOTER` |
| `PV-02` | Slot Match | `vote.slot == current_slot` | `E_VOTE_WRONG_SLOT` |
| `PV-03` | Round Match | `vote.round == current_round` | `E_VOTE_WRONG_ROUND` |
| `PV-04` | Block Known | `vote.block_hash ∈ {known_blocks, NIL}` | `E_VOTE_UNKNOWN_BLOCK` |
| `PV-05` | Signature Valid | `verify(voter_pubkey, vote_hash, signature)` | `E_INVALID_VOTE_SIG` |
| `PV-06` | Not Duplicate | `¬seen_prevote[voter][slot][round]` | `E_DUPLICATE_PREVOTE` |
| `PV-07` | Not Conflicting | `¬∃ v: v.voter == voter ∧ v.block_hash ≠ vote.block_hash` | `E_CONFLICTING_PREVOTE` |

#### B.2 PRECOMMIT Rules

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `PC-01` | Voter Valid | `voter ∈ active_validators` | `E_UNKNOWN_VOTER` |
| `PC-02` | Slot Match | `vote.slot == current_slot` | `E_VOTE_WRONG_SLOT` |
| `PC-03` | Round Match | `vote.round == current_round` | `E_VOTE_WRONG_ROUND` |
| `PC-04` | Block Known | `vote.block_hash ∈ known_blocks` (no NIL) | `E_VOTE_UNKNOWN_BLOCK` |
| `PC-05` | Signature Valid | `verify(voter_pubkey, vote_hash, signature)` | `E_INVALID_VOTE_SIG` |
| `PC-06` | Prevote Quorum | `prevote_weight[block_hash] ≥ ⅔ × total_weight` | `E_NO_PREVOTE_QUORUM` |
| `PC-07` | Not Duplicate | `¬seen_precommit[voter][slot][round]` | `E_DUPLICATE_PRECOMMIT` |
| `PC-08` | Not Conflicting | `¬∃ v: v.voter == voter ∧ v.block_hash ≠ vote.block_hash` | `E_CONFLICTING_PRECOMMIT` |
| `PC-09` | Lock Respected | `locked_block == null ∨ vote.block_hash == locked_block` | `E_LOCK_VIOLATION` |

#### B.3 Weight Accumulation (50% Stake + 50% GPU Compute)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         WEIGHT ACCUMULATION RULES                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CALCULATION:                                                                          │
│   ────────────                                                                          │
│   vote_weight(v) = stake_component(v) + pouw_component(v)                              │
│                                                                                         │
│   stake_component(v) = (stake[v.voter] / total_stake) × STAKE_COEFFICIENT              │
│   pouw_component(v)  = (pouw_score[v.voter] / total_pouw) × POUW_COEFFICIENT           │
│                                                                                         │
│   Where:                                                                                │
│     STAKE_COEFFICIENT = 0.50                                                           │
│     POUW_COEFFICIENT  = 0.50                                                           │
│                                                                                         │
│   AGGREGATION:                                                                          │
│   ────────────                                                                          │
│   total_vote_weight(block) = Σ vote_weight(v) for v in votes[block]                    │
│                                                                                         │
│   QUORUM:                                                                               │
│   ───────                                                                               │
│   quorum_reached(block) ⟺ total_vote_weight(block) ≥ QUORUM_THRESHOLD                 │
│                                                                                         │
│   Where:                                                                                │
│     QUORUM_THRESHOLD = 0.667 (⅔ majority)                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `WGT-01` | Stake Positive | `stake[voter] > 0` | `E_ZERO_STAKE` |
| `WGT-02` | Weight Bounded | `vote_weight(v) ≤ MAX_SINGLE_WEIGHT` | `E_WEIGHT_OVERFLOW` |
| `WGT-03` | Total Valid | `total_vote_weight ≤ 1.0` | `E_TOTAL_OVERFLOW` |
| `WGT-04` | No Double Count | `count(voter, votes) == 1` | `E_DOUBLE_COUNTED` |
| `WGT-05` | PoUW Fresh | `pouw_score_epoch == current_epoch` | `E_STALE_POUW` |

#### B.4 Conflict Detection

| Rule ID | Rule | Detection | Action |
|---------|------|-----------|--------|
| `CNF-01` | Double PREVOTE | Same voter, slot, round, different blocks | Slash 100% stake |
| `CNF-02` | Double PRECOMMIT | Same voter, slot, round, different blocks | Slash 100% stake |
| `CNF-03` | Lock Violation | PRECOMMIT for non-locked block after locking | Slash 50% stake |
| `CNF-04` | Equivocation | Any conflicting signed messages | Slash 100% stake |

```
CONFLICT DETECTION ALGORITHM:

function detect_conflicts(vote):
    key = (vote.voter, vote.slot, vote.round, vote.type)
    
    if key in seen_votes:
        existing = seen_votes[key]
        if existing.block_hash != vote.block_hash:
            emit SlashEvent(vote.voter, EQUIVOCATION, [existing, vote])
            return CONFLICT_DETECTED
    
    seen_votes[key] = vote
    return NO_CONFLICT
```

#### B.5 Round Transition Logic

| Condition | Current State | Next State | Action |
|-----------|---------------|------------|--------|
| ⅔ PREVOTEs for B | PREVOTE phase | PRECOMMIT phase | Broadcast PRECOMMIT(B) |
| ⅔ PREVOTEs for NIL | PREVOTE phase | New round | Increment round, new leader |
| ⅔ PRECOMMITs for B | PRECOMMIT phase | FINALIZE | Finalize block B |
| Timeout (no ⅔) | Any phase | New round | Broadcast NIL, increment round |
| Conflict detected | Any phase | Slash | Emit slash event, continue |

---

### 2.C Receipt Validation (PoUW)

#### C.1 Compute Receipt Format & Fields

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE RECEIPT SCHEMA                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ComputeReceipt {                                                                      │
│     task_id:            TaskId,        // 32 bytes, unique task identifier             │
│     provider:           Address,       // 20 bytes, GPU provider address               │
│     input_hash:         Hash,          // 32 bytes, hash of input data                 │
│     output_hash:        Hash,          // 32 bytes, hash of output data                │
│     execution_time_ms:  u64,           // Execution duration in milliseconds           │
│     resources_used: {                                                                  │
│       gpu_time_ms:      u64,           // GPU compute time                             │
│       vram_peak_mb:     u32,           // Peak VRAM usage                              │
│       flops:            u64,           // Floating point operations                    │
│     },                                                                                 │
│     verification: {                                                                    │
│       status:           VerifyStatus,  // Pending | Verified | Failed                  │
│       verifier:         Option<Addr>,  // Verifier address (if replicated)             │
│       proof:            Option<Bytes>, // ZK proof (future)                            │
│     },                                                                                 │
│     pouw_score:         u64,           // Computed PoUW contribution                   │
│     timestamp:          u64,           // Submission timestamp                         │
│     signature:          Signature,     // Provider signature over receipt              │
│   }                                                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `RCP-F01` | Task ID Valid | `len(task_id) == 32 ∧ task_id ≠ 0` | `E_INVALID_TASK_ID` |
| `RCP-F02` | Provider Valid | `len(provider) == 20` | `E_INVALID_PROVIDER` |
| `RCP-F03` | Hashes Valid | `len(input_hash) == 32 ∧ len(output_hash) == 32` | `E_INVALID_HASH` |
| `RCP-F04` | Time Positive | `execution_time_ms > 0` | `E_INVALID_EXEC_TIME` |
| `RCP-F05` | Score Bounded | `pouw_score ≤ MAX_POUW_SCORE` | `E_SCORE_OVERFLOW` |
| `RCP-F06` | Timestamp Recent | `timestamp ≥ current_time - MAX_RECEIPT_AGE` | `E_STALE_RECEIPT` |

#### C.2 Execution Output Hash Comparison

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `RCP-O01` | Task Exists | `task_id ∈ task_registry` | `E_TASK_NOT_FOUND` |
| `RCP-O02` | Input Matches | `receipt.input_hash == task.input_hash` | `E_INPUT_MISMATCH` |
| `RCP-O03` | Output Verified | `verify_output(receipt.output_hash, task)` | `E_OUTPUT_INVALID` |
| `RCP-O04` | Deterministic | `receipt.output_hash == expected_output_hash` | `E_NONDETERMINISTIC` |

```
OUTPUT VERIFICATION METHODS:

1. REPLICATED VERIFICATION
   verify_output(hash, task):
     verifier_result = execute_task(task.input)
     return hash(verifier_result) == hash

2. PROBABILISTIC SAMPLING (selected tasks)
   verify_output(hash, task):
     if selected_for_verification(task.id):
       return replicated_verify(hash, task)
     return true  // Trust receipt

3. ZK PROOF VERIFICATION [FUTURE]
   verify_output(hash, task):
     return verify_zk_proof(receipt.proof, task.circuit, hash)
```

#### C.3 GPU Identity and Proof-of-Execution Requirements

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `RCP-G01` | Provider Registered | `provider ∈ registered_providers` | `E_UNREGISTERED_PROVIDER` |
| `RCP-G02` | Provider Staked | `provider_stake[provider] ≥ MIN_PROVIDER_STAKE` | `E_UNSTAKED_PROVIDER` |
| `RCP-G03` | Provider Not Slashed | `slashed_providers[provider] == false` | `E_SLASHED_PROVIDER` |
| `RCP-G04` | Signature Valid | `verify(provider_pubkey, receipt_hash, signature)` | `E_INVALID_RECEIPT_SIG` |
| `RCP-G05` | Execution Proof | `verify_execution_proof(receipt)` | `E_INVALID_EXEC_PROOF` |

#### C.4 Invalid Receipt Conditions

| Condition | Detection | Action | Slash Amount |
|-----------|-----------|--------|--------------|
| **Forged Signature** | Signature verification fails | Reject receipt | — |
| **Unknown Task** | Task ID not in registry | Reject receipt | — |
| **Output Mismatch** | Replicated verification differs | Slash provider | 50% |
| **Duplicate Receipt** | Same task_id already processed | Reject receipt | — |
| **Stale Receipt** | Timestamp too old | Reject receipt | — |
| **Unregistered Provider** | Provider not in set | Reject receipt | — |
| **Collusion Detected** | Statistical anomaly in results | Slash all parties | 100% |

---

### 2.D Consensus Integrity Checks

#### D.1 Fork Choice Checks

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `FC-01` | Chain Valid | All ancestors valid | `E_INVALID_CHAIN` |
| `FC-02` | Weight Computed | `chain_weight == Σ block_weights` | `E_WEIGHT_ERROR` |
| `FC-03` | Canonical Selected | `selected == argmax(chains, weight)` | `E_WRONG_FORK` |
| `FC-04` | Tie Resolved | If weights equal, lower hash wins | `E_TIE_UNRESOLVED` |
| `FC-05` | Finality Respected | Finalized blocks not reverted | `E_FINALITY_VIOLATION` |

```
FORK CHOICE ALGORITHM:

function select_canonical(heads):
    best = null
    best_weight = 0
    
    for head in heads:
        weight = compute_chain_weight(head)
        if weight > best_weight:
            best = head
            best_weight = weight
        else if weight == best_weight:
            // Tie-break: lower block hash wins
            if head.hash < best.hash:
                best = head
    
    // Verify finality not violated
    for block in chain(best):
        if is_finalized(block.parent) and block.parent != best_ancestor:
            return ERROR(E_FINALITY_VIOLATION)
    
    return best
```

#### D.2 Re-org Protection

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `RO-01` | Finalized Immutable | `∀ b: finalized(b) ⟹ ¬reverted(b)` | `E_FINALITY_VIOLATION` |
| `RO-02` | Depth Limit | `reorg_depth ≤ MAX_REORG_DEPTH` | `E_REORG_TOO_DEEP` |
| `RO-03` | Weight Required | `new_chain_weight > old_chain_weight` | `E_INSUFFICIENT_WEIGHT` |
| `RO-04` | Lock Check | `locked_block respected in new chain` | `E_LOCK_VIOLATION` |

#### D.3 Timeout & Fallback Voting

| Rule ID | Rule | Condition | Error Code |
|---------|------|-----------|------------|
| `TO-01` | Proposal Timeout | `now - slot_start > PROPOSAL_TIMEOUT` | Trigger NIL vote |
| `TO-02` | Prevote Timeout | `now - prevote_start > PREVOTE_TIMEOUT` | Force round change |
| `TO-03` | Precommit Timeout | `now - precommit_start > PRECOMMIT_TIMEOUT` | Force round change |
| `TO-04` | Max Rounds | `round < MAX_ROUNDS_PER_SLOT` | `E_MAX_ROUNDS_EXCEEDED` |

```
TIMEOUT CONFIGURATION:

PROPOSAL_TIMEOUT   = 500ms    // Wait for block proposal
PREVOTE_TIMEOUT    = 1000ms   // Wait for ⅔ prevotes
PRECOMMIT_TIMEOUT  = 500ms    // Wait for ⅔ precommits
SLOT_DURATION      = 2000ms   // Total slot time
MAX_ROUNDS         = 10       // Maximum rounds before skip
```

#### D.4 Invariant Checks

| Invariant | Check | Frequency | Error Code |
|-----------|-------|-----------|------------|
| **State Hash** | `computed_root == block.state_root` | Every block | `E_STATE_ROOT_MISMATCH` |
| **Gas Model** | `total_gas ≤ block.gas_limit` | Every block | `E_GAS_EXCEEDED` |
| **Block Score** | `score == base + attestations + pouw` | Every block | `E_SCORE_MISMATCH` |
| **Supply** | `Σ balances == TOTAL_SUPPLY` | Every epoch | `E_SUPPLY_MISMATCH` |
| **Validator Set** | `validators == compute_validators(state)` | Every epoch | `E_VALIDATOR_MISMATCH` |

---

## 3. Validation Pipeline Diagram

### 3.1 Full Validation Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CONSENSUS VALIDATION PIPELINE                                   │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  Incoming Message (Proposal / Vote / Receipt)
                │
                ▼
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 1: MESSAGE PARSING
  ══════════════════════════════════════════════════════════════════════════════════════
                │
                ▼
        ┌───────────────┐
        │ Deserialize   │
        │ Message       │
        └───────┬───────┘
                │
        ┌───────▼───────┐
        │ Check Message │──── Invalid ────▶ DROP (E_MALFORMED)
        │ Type & Format │
        └───────┬───────┘
                │ Valid
                ▼
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 2: PRELIMINARY VALIDATION
  ══════════════════════════════════════════════════════════════════════════════════════
                │
                ▼
        ┌───────────────┐
        │ Signature     │──── Invalid ────▶ REJECT (E_INVALID_SIGNATURE)
        │ Verification  │
        └───────┬───────┘
                │ Valid
                ▼
        ┌───────────────┐
        │ Slot / Round  │──── Invalid ────▶ REJECT (E_INVALID_SLOT)
        │ Check         │
        └───────┬───────┘
                │ Valid
                ▼
        ┌───────────────┐
        │ Sender        │──── Invalid ────▶ REJECT (E_UNAUTHORIZED)
        │ Authorization │
        └───────┬───────┘
                │ Valid
                ▼
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 3: TYPE-SPECIFIC VALIDATION
  ══════════════════════════════════════════════════════════════════════════════════════
                │
        ┌───────┴───────┬───────────────┬───────────────┐
        │               │               │               │
        ▼               ▼               ▼               ▼
  ┌───────────┐   ┌───────────┐   ┌───────────┐   ┌───────────┐
  │ PROPOSAL  │   │ PREVOTE   │   │ PRECOMMIT │   │ RECEIPT   │
  │ Validation│   │ Validation│   │ Validation│   │ Validation│
  └─────┬─────┘   └─────┬─────┘   └─────┬─────┘   └─────┬─────┘
        │               │               │               │
        │ ┌─────────────┘               │               │
        │ │ ┌───────────────────────────┘               │
        │ │ │ ┌─────────────────────────────────────────┘
        │ │ │ │
        ▼ ▼ ▼ ▼
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 4: VOTE AGGREGATION (for votes)
  ══════════════════════════════════════════════════════════════════════════════════════
                │
                ▼
        ┌───────────────┐
        │ Compute Vote  │
        │ Weight        │
        │ (50%S + 50%G) │
        └───────┬───────┘
                │
                ▼
        ┌───────────────┐
        │ Check for     │──── Conflict ───▶ SLASH (emit SlashEvent)
        │ Conflicts     │
        └───────┬───────┘
                │ No Conflict
                ▼
        ┌───────────────┐
        │ Accumulate    │
        │ Total Weight  │
        └───────┬───────┘
                │
                ▼
        ┌───────────────┐     ┌───────────────┐
        │ Quorum        │─Yes─│ Progress to   │
        │ Reached?      │     │ Next Phase    │
        │ (≥ 66.7%)     │     └───────────────┘
        └───────┬───────┘
                │ No
                ▼
        ┌───────────────┐
        │ Wait for      │
        │ More Votes    │
        └───────────────┘
                │
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 5: RECEIPT VALIDATION (for proposals with receipts)
  ══════════════════════════════════════════════════════════════════════════════════════
                │
                ▼
        ┌───────────────┐
        │ Verify Each   │
        │ PoUW Receipt  │
        └───────┬───────┘
                │
        ┌───────┴───────┐
        │               │
        ▼               ▼
   All Valid      Some Invalid
        │               │
        │               ▼
        │        ┌───────────────┐
        │        │ Remove Invalid│
        │        │ Flag Provider │
        │        └───────┬───────┘
        │                │
        └────────┬───────┘
                 │
                 ▼
        ┌───────────────┐
        │ Compute Total │
        │ PoUW Score    │
        └───────┬───────┘
                │
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 6: FINALITY CHECK
  ══════════════════════════════════════════════════════════════════════════════════════
                │
                ▼
        ┌───────────────┐
        │ ⅔ PRECOMMIT   │──── No ────▶ Wait / Timeout
        │ Weight?       │
        └───────┬───────┘
                │ Yes
                ▼
        ┌───────────────┐
        │ Lock Block    │
        └───────┬───────┘
                │
                ▼
        ┌───────────────┐
        │ Mark Block    │
        │ FINALIZED     │
        └───────┬───────┘
                │
  ══════════════════════════════════════════════════════════════════════════════════════
  STAGE 7: COMMIT
  ══════════════════════════════════════════════════════════════════════════════════════
                │
                ▼
        ┌───────────────┐
        │ Execute Block │
        │ (State Trans.)│
        └───────┬───────┘
                │
                ▼
        ┌───────────────┐
        │ Verify State  │──── Mismatch ───▶ REJECT (E_STATE_MISMATCH)
        │ Root          │
        └───────┬───────┘
                │ Match
                ▼
        ┌───────────────┐
        │ Persist to    │
        │ Storage       │
        └───────┬───────┘
                │
                ▼
        ┌───────────────┐
        │ Broadcast     │
        │ FINALIZE Msg  │
        └───────┬───────┘
                │
                ▼
           ┌─────────┐
           │  DONE   │
           └─────────┘
```

### 3.2 Stage Summary

| Stage | Input | Output | Errors |
|-------|-------|--------|--------|
| **1. Parsing** | Raw bytes | Typed message | `E_MALFORMED` |
| **2. Preliminary** | Message | Authorized message | `E_INVALID_SIGNATURE`, `E_INVALID_SLOT`, `E_UNAUTHORIZED` |
| **3. Type-Specific** | Authorized message | Validated message | Type-specific errors |
| **4. Aggregation** | Votes | Weight totals | `E_CONFLICT`, `E_DUPLICATE` |
| **5. Receipt** | PoUW receipts | Verified receipts | `E_INVALID_RECEIPT` |
| **6. Finality** | Aggregated state | Final decision | `E_NO_QUORUM` |
| **7. Commit** | Finalized block | Persisted state | `E_STATE_MISMATCH` |

---

## 4. Failure Modes & Detection

### 4.1 Failure Categories

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CONSENSUS FAILURE TAXONOMY                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CATEGORY           │ FAILURES                      │ SEVERITY │ RECOVERY             │
│   ────────           │ ────────                      │ ──────── │ ────────             │
│                      │                               │          │                      │
│   PROPOSAL           │ • InvalidProposal             │ Medium   │ Wait next slot       │
│   FAILURES           │ • MalformedBlock              │ Low      │ Reject message       │
│                      │ • WrongProposer               │ Medium   │ Ignore proposal      │
│                      │ • DoubleProposal              │ High     │ Slash proposer       │
│                      │                               │          │                      │
│   HEADER             │ • HeaderMismatch              │ Medium   │ Reject block         │
│   FAILURES           │ • InvalidParent               │ Medium   │ Request sync         │
│                      │ • TimestampInvalid            │ Low      │ Reject/hold          │
│                      │ • RootMismatch                │ High     │ Reject block         │
│                      │                               │          │                      │
│   RECEIPT            │ • ReceiptMismatch             │ Medium   │ Remove receipt       │
│   FAILURES           │ • InvalidProviderSig          │ Low      │ Reject receipt       │
│                      │ • TaskNotFound                │ Low      │ Reject receipt       │
│                      │ • OutputMismatch              │ High     │ Slash provider       │
│                      │                               │          │                      │
│   WEIGHT             │ • WeightInsufficiency         │ Medium   │ Wait for votes       │
│   FAILURES           │ • WeightOverflow              │ Low      │ Cap at maximum       │
│                      │ • StalePoUWScore              │ Low      │ Use fallback         │
│                      │                               │          │                      │
│   VOTE               │ • ConflictingPrevote          │ Critical │ Slash voter          │
│   FAILURES           │ • ConflictingPrecommit        │ Critical │ Slash voter          │
│                      │ • InvalidVoteSignature        │ Low      │ Reject vote          │
│                      │ • DuplicateVote               │ Low      │ Ignore               │
│                      │                               │          │                      │
│   FIELD              │ • MissingFields               │ Low      │ Reject message       │
│   FAILURES           │ • InvalidFieldFormat          │ Low      │ Reject message       │
│                      │ • ExtraFields                 │ Low      │ Ignore extras        │
│                      │                               │          │                      │
│   TIMEOUT            │ • ProposalTimeout             │ Medium   │ NIL vote             │
│   FAILURES           │ • VoteTimeout                 │ Medium   │ Round change         │
│                      │ • FinalityTimeout             │ Medium   │ View change          │
│                      │                               │          │                      │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Detection Mechanisms

#### 4.2.1 Proposal Invalid

```rust
fn detect_invalid_proposal(proposal: &Proposal) -> Option<ValidationError> {
    // Check proposer legitimacy
    if !is_slot_leader(proposal.proposer, proposal.slot, proposal.round) {
        return Some(E_NOT_LEADER);
    }
    
    // Check header consistency
    if !validate_header(&proposal.block.header) {
        return Some(E_INVALID_HEADER);
    }
    
    // Check signature
    if !verify_signature(&proposal.proposer, &proposal.signature, &proposal.hash()) {
        return Some(E_INVALID_SIGNATURE);
    }
    
    // Check for double proposal
    if seen_proposals.contains(&(proposal.proposer, proposal.slot, proposal.round)) {
        emit_slash_event(proposal.proposer, DOUBLE_PROPOSAL);
        return Some(E_DOUBLE_PROPOSAL);
    }
    
    None
}
```

#### 4.2.2 Header Mismatch

```rust
fn detect_header_mismatch(header: &BlockHeader) -> Option<ValidationError> {
    // Parent existence
    if !known_blocks.contains(&header.parent_hash) {
        return Some(E_UNKNOWN_PARENT);
    }
    
    let parent = get_block(header.parent_hash);
    
    // Height check
    if header.height != parent.height + 1 {
        return Some(E_INVALID_HEIGHT);
    }
    
    // Timestamp check
    if header.timestamp <= parent.timestamp {
        return Some(E_INVALID_TIMESTAMP);
    }
    
    // Merkle root checks
    if merkle_root(&header.transactions) != header.tx_root {
        return Some(E_TX_ROOT_MISMATCH);
    }
    
    None
}
```

#### 4.2.3 Receipt Mismatch

```rust
fn detect_receipt_mismatch(receipt: &ComputeReceipt) -> Option<ValidationError> {
    // Task existence
    let task = match task_registry.get(&receipt.task_id) {
        Some(t) => t,
        None => return Some(E_TASK_NOT_FOUND),
    };
    
    // Input hash match
    if receipt.input_hash != task.input_hash {
        return Some(E_INPUT_MISMATCH);
    }
    
    // Output verification (if selected for verification)
    if should_verify(receipt.task_id) {
        let expected = recompute_output(&task);
        if receipt.output_hash != expected {
            emit_slash_event(receipt.provider, INVALID_COMPUTE);
            return Some(E_OUTPUT_MISMATCH);
        }
    }
    
    None
}
```

#### 4.2.4 Weight Insufficiency

```rust
fn detect_weight_insufficiency(votes: &[Vote], quorum: f64) -> Option<ValidationError> {
    let total_weight: f64 = votes.iter()
        .map(|v| compute_vote_weight(v))
        .sum();
    
    if total_weight < quorum {
        return Some(E_INSUFFICIENT_QUORUM);
    }
    
    None
}
```

#### 4.2.5 Conflicting Votes

```rust
fn detect_conflicting_votes(vote: &Vote) -> Option<(Vote, Vote)> {
    let key = (vote.voter, vote.slot, vote.round, vote.vote_type);
    
    if let Some(existing) = seen_votes.get(&key) {
        if existing.block_hash != vote.block_hash {
            return Some((existing.clone(), vote.clone()));
        }
    }
    
    seen_votes.insert(key, vote.clone());
    None
}
```

#### 4.2.6 Missing Fields

```rust
fn detect_missing_fields<T: ConsensusMessage>(msg: &T) -> Option<ValidationError> {
    let required_fields = T::required_fields();
    
    for field in required_fields {
        if !msg.has_field(field) {
            return Some(E_MISSING_FIELD(field));
        }
    }
    
    None
}
```

#### 4.2.7 Timeout

```rust
fn detect_timeout(phase: Phase, start_time: Instant) -> Option<TimeoutEvent> {
    let elapsed = start_time.elapsed();
    let timeout = match phase {
        Phase::Proposal => PROPOSAL_TIMEOUT,
        Phase::Prevote => PREVOTE_TIMEOUT,
        Phase::Precommit => PRECOMMIT_TIMEOUT,
    };
    
    if elapsed > timeout {
        return Some(TimeoutEvent { phase, elapsed });
    }
    
    None
}
```

### 4.3 Error Code Reference

| Code | Name | Description |
|------|------|-------------|
| `E_MALFORMED` | Malformed Message | Cannot deserialize |
| `E_INVALID_SIGNATURE` | Invalid Signature | Signature verification failed |
| `E_INVALID_SLOT` | Invalid Slot | Slot out of valid range |
| `E_UNAUTHORIZED` | Unauthorized | Sender not authorized |
| `E_NOT_LEADER` | Not Slot Leader | Proposer not elected leader |
| `E_INVALID_HEADER` | Invalid Header | Header validation failed |
| `E_UNKNOWN_PARENT` | Unknown Parent | Parent block not found |
| `E_TX_ROOT_MISMATCH` | Tx Root Mismatch | Merkle root incorrect |
| `E_TASK_NOT_FOUND` | Task Not Found | PoUW task unknown |
| `E_OUTPUT_MISMATCH` | Output Mismatch | Compute result incorrect |
| `E_INSUFFICIENT_QUORUM` | Insufficient Quorum | <⅔ weight reached |
| `E_CONFLICTING_VOTE` | Conflicting Vote | Equivocation detected |
| `E_MISSING_FIELD` | Missing Field | Required field absent |
| `E_TIMEOUT` | Timeout | Phase timeout exceeded |

---

## 5. Developer Notes

### 5.1 Rust Module Structure

```
pow/
├── Cargo.toml
├── src/
│   ├── lib.rs                      // Module exports
│   ├── consensus/
│   │   ├── mod.rs                  // Consensus submodule
│   │   ├── state.rs                // Consensus state machine
│   │   ├── leader.rs               // Leader election
│   │   └── finality.rs             // Finality tracking
│   │
│   ├── validation/
│   │   ├── mod.rs                  // Validation exports
│   │   ├── proposal.rs             // Proposal validation [PRO-*]
│   │   ├── vote.rs                 // Vote validation [PV-*, PC-*]
│   │   ├── receipt.rs              // Receipt validation [RCP-*]
│   │   ├── weight.rs               // Weight calculation [WGT-*]
│   │   ├── integrity.rs            // Integrity checks [FC-*, RO-*, TO-*]
│   │   └── conflict.rs             // Conflict detection [CNF-*]
│   │
│   ├── messages/
│   │   ├── mod.rs
│   │   ├── proposal.rs             // PROPOSE message
│   │   ├── vote.rs                 // PREVOTE, PRECOMMIT
│   │   ├── receipt.rs              // RECEIPT message
│   │   └── finalize.rs             // FINALIZE message
│   │
│   └── errors.rs                   // Error definitions
│
└── tests/
    ├── proposal_tests.rs
    ├── vote_tests.rs
    ├── receipt_tests.rs
    └── integration_tests.rs
```

### 5.2 Validation Function Signatures

```rust
// ═══════════════════════════════════════════════════════════════════════════════
// PROPOSAL VALIDATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Validate a block proposal
/// 
/// # Arguments
/// * `proposal` - The proposal message to validate
/// * `state` - Current consensus state
/// 
/// # Returns
/// * `Ok(ValidatedProposal)` - Proposal passed all checks
/// * `Err(ValidationError)` - Proposal failed validation with specific error
pub fn validate_proposal(
    proposal: &Proposal,
    state: &ConsensusState,
) -> Result<ValidatedProposal, ValidationError>;

/// Validate block header fields
pub fn validate_header(
    header: &BlockHeader,
    parent: &BlockHeader,
) -> Result<(), ValidationError>;

/// Verify proposer is legitimate leader for slot/round
pub fn verify_proposer(
    proposer: &ValidatorId,
    slot: u64,
    round: u32,
    validator_set: &ValidatorSet,
) -> Result<(), ValidationError>;


// ═══════════════════════════════════════════════════════════════════════════════
// VOTE VALIDATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Validate a prevote message
pub fn validate_prevote(
    vote: &Prevote,
    state: &ConsensusState,
) -> Result<ValidatedVote, ValidationError>;

/// Validate a precommit message
pub fn validate_precommit(
    vote: &Precommit,
    state: &ConsensusState,
    prevote_quorum: &VoteQuorum,
) -> Result<ValidatedVote, ValidationError>;

/// Compute vote weight (50% stake + 50% PoUW)
pub fn compute_vote_weight(
    voter: &ValidatorId,
    stake_weights: &StakeWeights,
    pouw_scores: &PoUWScores,
) -> VoteWeight;

/// Check for conflicting votes (equivocation)
pub fn check_conflicts(
    vote: &Vote,
    seen_votes: &SeenVotes,
) -> Result<(), ConflictError>;


// ═══════════════════════════════════════════════════════════════════════════════
// RECEIPT VALIDATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Validate a PoUW compute receipt
pub fn validate_receipt(
    receipt: &ComputeReceipt,
    task_registry: &TaskRegistry,
    provider_set: &ProviderSet,
) -> Result<ValidatedReceipt, ValidationError>;

/// Verify receipt output against task (replicated verification)
pub fn verify_receipt_output(
    receipt: &ComputeReceipt,
    task: &ComputeTask,
) -> Result<(), VerificationError>;

/// Compute PoUW score contribution from receipt
pub fn compute_pouw_score(
    receipt: &ValidatedReceipt,
    difficulty: u64,
) -> u64;


// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRITY CHECKS
// ═══════════════════════════════════════════════════════════════════════════════

/// Apply fork choice rule to select canonical chain
pub fn select_canonical_chain(
    heads: &[BlockHash],
    weights: &ChainWeights,
    finalized: &FinalizedBlocks,
) -> Result<BlockHash, ForkChoiceError>;

/// Check if reorg is valid
pub fn validate_reorg(
    old_head: &BlockHash,
    new_head: &BlockHash,
    finalized: &FinalizedBlocks,
) -> Result<(), ReorgError>;

/// Check all consensus invariants
pub fn check_invariants(
    block: &Block,
    state: &ConsensusState,
) -> Result<(), InvariantError>;
```

### 5.3 Error Surfacing

```rust
/// Consensus validation error with context
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub code: ErrorCode,
    pub message: String,
    pub context: ErrorContext,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub slot: Option<u64>,
    pub round: Option<u32>,
    pub block_hash: Option<Hash>,
    pub validator: Option<ValidatorId>,
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Low,      // Log and continue
    Medium,   // Reject message, log warning
    High,     // Reject message, emit event
    Critical, // Slash offender, alert
}

impl ValidationError {
    /// Create error with full context
    pub fn with_context(code: ErrorCode, ctx: ErrorContext) -> Self {
        Self {
            code,
            message: code.default_message(),
            context: ctx,
            severity: code.default_severity(),
        }
    }
    
    /// Surface error appropriately based on severity
    pub fn surface(&self) {
        match self.severity {
            Severity::Low => {
                log::debug!("Validation warning: {:?}", self);
            }
            Severity::Medium => {
                log::warn!("Validation error: {:?}", self);
                metrics::increment("consensus.validation.errors", &[("code", self.code.as_str())]);
            }
            Severity::High => {
                log::error!("Validation failure: {:?}", self);
                metrics::increment("consensus.validation.failures", &[("code", self.code.as_str())]);
                events::emit(Event::ValidationFailure(self.clone()));
            }
            Severity::Critical => {
                log::error!("CRITICAL validation failure: {:?}", self);
                metrics::increment("consensus.validation.critical", &[("code", self.code.as_str())]);
                events::emit(Event::CriticalFailure(self.clone()));
                alerts::send_alert(Alert::ConsensusViolation(self.clone()));
            }
        }
    }
}
```

### 5.4 Integration Test Expectations

```rust
// ═══════════════════════════════════════════════════════════════════════════════
// TEST EXPECTATIONS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    // ──────────────────────────────────────────────────────────────────────────
    // PROPOSAL VALIDATION TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_valid_proposal_accepted() {
        // Valid proposal from correct leader should pass
        let proposal = create_valid_proposal(slot: 10, round: 0);
        let state = create_state_with_leader(slot: 10, leader: proposal.proposer);
        
        assert!(validate_proposal(&proposal, &state).is_ok());
    }
    
    #[test]
    fn test_wrong_proposer_rejected() {
        // Proposal from non-leader should fail with E_NOT_LEADER
        let proposal = create_valid_proposal(slot: 10, round: 0);
        let state = create_state_with_leader(slot: 10, leader: OTHER_VALIDATOR);
        
        let result = validate_proposal(&proposal, &state);
        assert_eq!(result.unwrap_err().code, E_NOT_LEADER);
    }
    
    #[test]
    fn test_double_proposal_slashed() {
        // Second proposal from same proposer should trigger slash
        let proposal1 = create_valid_proposal(slot: 10, round: 0, block: BLOCK_A);
        let proposal2 = create_valid_proposal(slot: 10, round: 0, block: BLOCK_B);
        let mut state = create_default_state();
        
        validate_proposal(&proposal1, &state).unwrap();
        state.record_proposal(&proposal1);
        
        let result = validate_proposal(&proposal2, &state);
        assert_eq!(result.unwrap_err().code, E_DOUBLE_PROPOSAL);
        assert!(state.pending_slashes.contains(&proposal2.proposer));
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // VOTE VALIDATION TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_valid_prevote_accepted() {
        let vote = create_valid_prevote(voter: VALIDATOR_A, block: BLOCK_X);
        let state = create_state_with_proposal(BLOCK_X);
        
        assert!(validate_prevote(&vote, &state).is_ok());
    }
    
    #[test]
    fn test_conflicting_prevotes_detected() {
        let vote1 = create_valid_prevote(voter: VALIDATOR_A, block: BLOCK_X);
        let vote2 = create_valid_prevote(voter: VALIDATOR_A, block: BLOCK_Y);
        let mut seen = SeenVotes::new();
        
        check_conflicts(&vote1, &seen).unwrap();
        seen.record(&vote1);
        
        let conflict = check_conflicts(&vote2, &seen);
        assert!(conflict.is_err());
    }
    
    #[test]
    fn test_weight_calculation_correct() {
        // 50% stake + 50% PoUW
        let stake_weights = StakeWeights::new(vec![
            (VALIDATOR_A, 1000),
            (VALIDATOR_B, 500),
        ]);
        let pouw_scores = PoUWScores::new(vec![
            (VALIDATOR_A, 100),
            (VALIDATOR_B, 400),
        ]);
        
        let weight_a = compute_vote_weight(&VALIDATOR_A, &stake_weights, &pouw_scores);
        // A: (1000/1500)*0.50 + (100/500)*0.50 = 0.333 + 0.10 = 0.433
        assert_approx_eq!(weight_a, 0.527, 0.001);
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // RECEIPT VALIDATION TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_valid_receipt_accepted() {
        let receipt = create_valid_receipt(task: TASK_1, provider: PROVIDER_A);
        let registry = create_registry_with_task(TASK_1);
        let providers = create_provider_set_with(PROVIDER_A);
        
        assert!(validate_receipt(&receipt, &registry, &providers).is_ok());
    }
    
    #[test]
    fn test_unknown_task_rejected() {
        let receipt = create_valid_receipt(task: UNKNOWN_TASK, provider: PROVIDER_A);
        let registry = TaskRegistry::empty();
        let providers = create_provider_set_with(PROVIDER_A);
        
        let result = validate_receipt(&receipt, &registry, &providers);
        assert_eq!(result.unwrap_err().code, E_TASK_NOT_FOUND);
    }
    
    #[test]
    fn test_output_mismatch_slashes_provider() {
        let receipt = create_receipt_with_wrong_output(task: TASK_1, provider: PROVIDER_A);
        let registry = create_registry_with_task(TASK_1);
        let providers = create_provider_set_with(PROVIDER_A);
        
        let result = verify_receipt_output(&receipt, &registry.get(TASK_1));
        assert!(result.is_err());
        // Provider should be flagged for slashing
    }
    
    // ──────────────────────────────────────────────────────────────────────────
    // INTEGRITY CHECK TESTS
    // ──────────────────────────────────────────────────────────────────────────
    
    #[test]
    fn test_finality_prevents_reorg() {
        let finalized = FinalizedBlocks::new(vec![BLOCK_1, BLOCK_2, BLOCK_3]);
        let old_head = BLOCK_4; // child of BLOCK_3
        let new_head = BLOCK_ALT; // forks before BLOCK_3
        
        let result = validate_reorg(&old_head, &new_head, &finalized);
        assert_eq!(result.unwrap_err().code, E_FINALITY_VIOLATION);
    }
    
    #[test]
    fn test_quorum_threshold() {
        let votes = create_votes_with_weights(vec![0.3, 0.2, 0.15]); // Total: 0.65
        
        // Below 66.7% threshold
        assert!(detect_weight_insufficiency(&votes, 0.667).is_some());
        
        // Add one more vote
        let more_votes = create_votes_with_weights(vec![0.3, 0.2, 0.15, 0.05]); // Total: 0.70 (above threshold)
        assert!(detect_weight_insufficiency(&more_votes, 0.667).is_none());
    }
}
```

### 5.5 CI/CD Validation Commands

```bash
# Run all consensus validation tests
cargo test -p pow validation:: --all-features

# Run specific validation module tests
cargo test -p pow validation::proposal::
cargo test -p pow validation::vote::
cargo test -p pow validation::receipt::

# Run with coverage
cargo tarpaulin -p pow --out Html -- validation::

# Run integration tests
cargo test -p pow --test integration_tests

# Benchmark validation performance
cargo bench -p pow -- validation

# Check for validation-related clippy warnings
cargo clippy -p pow -- -D clippy::unwrap_used -D clippy::expect_used
```

---

## Appendix: Validation Checklist

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CONSENSUS VALIDATION CHECKLIST                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PROPOSAL                    │  VOTE                        │  RECEIPT                │
│   ────────                    │  ────                        │  ───────                │
│   □ PRO-S01 Slot current      │  □ PV-01 Voter valid         │  □ RCP-F01 Task ID      │
│   □ PRO-S02 Slot not past     │  □ PV-02 Slot match          │  □ RCP-F02 Provider     │
│   □ PRO-S03 Round valid       │  □ PV-03 Round match         │  □ RCP-F03 Hashes       │
│   □ PRO-P01 In validator set  │  □ PV-04 Block known         │  □ RCP-O01 Task exists  │
│   □ PRO-P02 Is slot leader    │  □ PV-05 Signature valid     │  □ RCP-O02 Input match  │
│   □ PRO-H01 Parent exists     │  □ PV-06 Not duplicate       │  □ RCP-O03 Output valid │
│   □ PRO-H02 Height sequential │  □ PV-07 Not conflicting     │  □ RCP-G01 Registered   │
│   □ PRO-H06 Tx root matches   │  □ PC-06 Prevote quorum      │  □ RCP-G04 Signature    │
│   □ PRO-SIG02 Signature valid │  □ PC-09 Lock respected      │                         │
│                               │                              │                         │
│   INTEGRITY                   │  WEIGHT                      │  TIMEOUT                │
│   ─────────                   │  ──────                      │  ───────                │
│   □ FC-01 Chain valid         │  □ WGT-01 Stake positive     │  □ TO-01 Proposal       │
│   □ FC-05 Finality respected  │  □ WGT-02 Weight bounded     │  □ TO-02 Prevote        │
│   □ RO-01 Finalized immutable │  □ WGT-03 Total valid        │  □ TO-03 Precommit      │
│   □ RO-02 Depth limit         │  □ WGT-04 No double count    │  □ TO-04 Max rounds     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

*This document is the canonical consensus validation specification for Mbongo Chain. All consensus validation implementations must conform to these rules.*

