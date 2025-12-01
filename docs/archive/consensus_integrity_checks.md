<!-- Verified against tokenomics.md -->
# Mbongo Chain — Consensus Integrity Checks

> **Document Type:** Technical Specification  
> **Last Updated:** November 2025  
> **Status:** Canonical Reference

---

## Table of Contents

1. [Purpose](#1-purpose)
2. [Integrity Check Categories](#2-integrity-check-categories)
3. [Integrity Pipeline Diagram](#3-integrity-pipeline-diagram)
4. [Failure Conditions Table](#4-failure-conditions-table)
5. [Developer Notes](#5-developer-notes)

---

## 1. Purpose

### 1.1 Why Integrity Checks Matter

In a hybrid PoS + PoUW consensus system, integrity checks serve as the foundational defense layer ensuring that:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     INTEGRITY CHECK OBJECTIVES                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CONSENSUS SAFETY                                                                      │
│   ────────────────                                                                      │
│   • No conflicting blocks are finalized at the same height                             │
│   • Validator weights (stake + compute) are correctly computed                         │
│   • Proposer eligibility follows deterministic rules                                   │
│   • PoUW receipts are authentic and verified                                           │
│                                                                                         │
│   CONSENSUS LIVENESS                                                                    │
│   ─────────────────                                                                     │
│   • Progress continues even under partial validator failure                            │
│   • Timeouts trigger appropriate fallback mechanisms                                   │
│   • Network partitions don't cause permanent forks                                     │
│                                                                                         │
│   ECONOMIC SECURITY                                                                     │
│   ─────────────────                                                                     │
│   • Stake manipulation cannot influence consensus unfairly                             │
│   • PoUW scores cannot be fabricated                                                   │
│   • Slashing conditions are correctly triggered                                        │
│                                                                                         │
│   STATE CONSISTENCY                                                                     │
│   ─────────────────                                                                     │
│   • All nodes converge to identical state                                              │
│   • Execution results match across network                                             │
│   • No state corruption from invalid inputs                                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Component Dependencies

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     COMPONENTS DEPENDING ON INTEGRITY CHECKS                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                          CONSENSUS LAYER                                         │  │
│   │   Primary consumer of all integrity checks                                       │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                         │                                              │
│            ┌────────────────────────────┼────────────────────────────┐                │
│            │                            │                            │                │
│            ▼                            ▼                            ▼                │
│   ┌─────────────────┐          ┌─────────────────┐          ┌─────────────────┐       │
│   │   EXECUTION     │          │    MEMPOOL      │          │   NETWORKING    │       │
│   │    ENGINE       │          │                 │          │                 │       │
│   └────────┬────────┘          └────────┬────────┘          └────────┬────────┘       │
│            │                            │                            │                │
│   Depends on:                  Depends on:                  Depends on:               │
│   • State consistency          • Header integrity           • Message integrity       │
│   • Receipt integrity          • Proposer checks            • Gossip integrity        │
│   • Fork-choice result         • Fork-choice for            • Duplicate detection     │
│   • Finality signals             tx ordering                • Peer legitimacy         │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

| Component | Integrity Dependencies | Failure Impact |
|-----------|------------------------|----------------|
| **Execution** | State hash, Receipt, Fork-choice | Invalid state transitions |
| **Mempool** | Header, Proposer, Fork-choice | Wrong tx ordering |
| **Networking** | Message, Gossip, Duplicate | Message corruption/flooding |
| **Storage** | State consistency, Finality | Data corruption |
| **Compute Engine** | Receipt, Weight | Invalid PoUW scores |

---

## 2. Integrity Check Categories

### 2.A Header Integrity Checks

#### A.1 Parent Linkage

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `HDR-PL01` | Parent Exists | `block.parent_hash ∈ known_blocks` | `E_UNKNOWN_PARENT` |
| `HDR-PL02` | Parent Finalized or Pending | `status(parent) ∈ {finalized, pending}` | `E_ORPHAN_BLOCK` |
| `HDR-PL03` | No Circular Reference | `block.hash ≠ block.parent_hash` | `E_CIRCULAR_REF` |
| `HDR-PL04` | Parent Not Reverted | `¬reverted(parent)` | `E_REVERTED_PARENT` |

```
PARENT LINKAGE VERIFICATION:

function verify_parent_linkage(block):
    if block.parent_hash not in known_blocks:
        return E_UNKNOWN_PARENT
    
    parent = known_blocks[block.parent_hash]
    
    if parent.status == REVERTED:
        return E_REVERTED_PARENT
    
    if block.hash == block.parent_hash:
        return E_CIRCULAR_REF
    
    return OK
```

#### A.2 Height/Slot Correctness

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `HDR-HS01` | Height Sequential | `block.height == parent.height + 1` | `E_INVALID_HEIGHT` |
| `HDR-HS02` | Slot Progressive | `block.slot > parent.slot` | `E_SLOT_REGRESSION` |
| `HDR-HS03` | Slot Within Bounds | `block.slot ≤ current_slot + 1` | `E_FUTURE_SLOT` |
| `HDR-HS04` | Height Matches Slot | `block.height ≤ block.slot` | `E_HEIGHT_SLOT_MISMATCH` |

#### A.3 Difficulty / Score Rules

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `HDR-DS01` | Difficulty Positive | `block.difficulty > 0` | `E_ZERO_DIFFICULTY` |
| `HDR-DS02` | Score Computed | `block.score == compute_score(block)` | `E_SCORE_MISMATCH` |
| `HDR-DS03` | Cumulative Valid | `block.cumulative_score == parent.cumulative_score + block.score` | `E_CUMULATIVE_ERROR` |
| `HDR-DS04` | PoUW Bonus Valid | `block.pouw_bonus ≤ MAX_POUW_BONUS` | `E_POUW_OVERFLOW` |

```
SCORE COMPUTATION:

block_score = base_difficulty 
            + attestation_weight × ATTESTATION_MULTIPLIER
            + pouw_score × POUW_MULTIPLIER

Where:
  base_difficulty     = FIXED_BASE (e.g., 1000)
  attestation_weight  = Σ(attester.weight) for attesters
  pouw_score         = Σ(receipt.score) for receipts in block
  ATTESTATION_MULTIPLIER = 1.0
  POUW_MULTIPLIER        = 0.5
```

#### A.4 Hash and Domain Separation

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `HDR-HD01` | Hash Correct | `hash(block.header) == block.hash` | `E_HASH_MISMATCH` |
| `HDR-HD02` | Domain Tagged | `block.hash.domain == BLOCK_DOMAIN` | `E_WRONG_DOMAIN` |
| `HDR-HD03` | Tx Root Valid | `merkle_root(block.txs) == block.tx_root` | `E_TX_ROOT_INVALID` |
| `HDR-HD04` | Receipts Root Valid | `merkle_root(block.receipts) == block.receipts_root` | `E_RECEIPTS_ROOT_INVALID` |
| `HDR-HD05` | State Root Format | `len(block.state_root) == 32` | `E_STATE_ROOT_FORMAT` |

---

### 2.B Proposer Legitimacy Checks

#### B.1 Eligibility Computation

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `PRP-EL01` | In Validator Set | `proposer ∈ active_validators` | `E_NOT_VALIDATOR` |
| `PRP-EL02` | Stake Sufficient | `stake[proposer] ≥ MIN_STAKE` | `E_INSUFFICIENT_STAKE` |
| `PRP-EL03` | Not Slashed | `¬slashed[proposer]` | `E_PROPOSER_SLASHED` |
| `PRP-EL04` | Not Jailed | `jail_until[proposer] < current_slot` | `E_PROPOSER_JAILED` |
| `PRP-EL05` | Registered | `registration_slot[proposer] < current_slot - ACTIVATION_DELAY` | `E_NOT_ACTIVATED` |

#### B.2 Stake-Weighted + Compute-Weighted Selection

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     PROPOSER SELECTION ALGORITHM                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   STEP 1: Compute Selection Score                                                       │
│   ────────────────────────────────                                                      │
│   For each validator v:                                                                 │
│     stake_component = (stake[v] / total_stake) × 0.50                                  │
│     pouw_component  = (pouw_score[v] / total_pouw) × 0.50                              │
│     base_score      = stake_component + pouw_component                                 │
│                                                                                         │
│   STEP 2: Apply VRF Randomness                                                          │
│   ────────────────────────────                                                          │
│     vrf_output = VRF(slot, round, v.secret_key)                                        │
│     selection_score = base_score × vrf_output                                          │
│                                                                                         │
│   STEP 3: Select Leader                                                                 │
│   ─────────────────────                                                                 │
│     leader = argmax(validators, selection_score)                                       │
│                                                                                         │
│   VERIFICATION:                                                                         │
│   ─────────────                                                                         │
│     verify_vrf(slot, round, proposer.public_key, vrf_proof)                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `PRP-SW01` | VRF Proof Valid | `verify_vrf(slot, round, pubkey, proof)` | `E_INVALID_VRF` |
| `PRP-SW02` | Selection Deterministic | `compute_leader(slot, round) == proposer` | `E_NOT_LEADER` |
| `PRP-SW03` | Weights Current | `weights_epoch == current_epoch` | `E_STALE_WEIGHTS` |
| `PRP-SW04` | No Weight Manipulation | `weights == compute_weights(state)` | `E_WEIGHT_TAMPERED` |

#### B.3 Rotation Schedule and Sampling

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `PRP-RS01` | Rotation Period | `proposer_set_epoch == floor(slot / EPOCH_LENGTH)` | `E_WRONG_EPOCH` |
| `PRP-RS02` | Set Deterministic | `proposer_set == compute_set(epoch_seed)` | `E_SET_MISMATCH` |
| `PRP-RS03` | Seed Valid | `epoch_seed == hash(prev_epoch_randomness, prev_state_root)` | `E_INVALID_SEED` |
| `PRP-RS04` | Sample Fair | `selection_distribution ≈ weight_distribution` | `E_UNFAIR_SAMPLING` |

---

### 2.C Voting Integrity Checks

#### C.1 PREVOTE Conflict Prevention

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `VOT-PV01` | Single Vote Per Round | `count(prevotes[v][slot][round]) ≤ 1` | `E_DOUBLE_PREVOTE` |
| `VOT-PV02` | Vote For Known Block | `vote.block_hash ∈ {known_blocks, NIL}` | `E_UNKNOWN_BLOCK` |
| `VOT-PV03` | Slot Current | `vote.slot == current_slot` | `E_WRONG_SLOT` |
| `VOT-PV04` | Round Valid | `vote.round == current_round` | `E_WRONG_ROUND` |

```
PREVOTE CONFLICT DETECTION:

function check_prevote_conflict(new_vote):
    key = (new_vote.voter, new_vote.slot, new_vote.round)
    
    if key in prevote_cache:
        existing = prevote_cache[key]
        if existing.block_hash != new_vote.block_hash:
            // EQUIVOCATION DETECTED
            evidence = EquivocationEvidence {
                vote1: existing,
                vote2: new_vote,
                type: DOUBLE_PREVOTE
            }
            emit_slash_event(new_vote.voter, evidence)
            return E_DOUBLE_PREVOTE
    
    prevote_cache[key] = new_vote
    return OK
```

#### C.2 PRECOMMIT Conflict Prevention

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `VOT-PC01` | Single Commit Per Round | `count(precommits[v][slot][round]) ≤ 1` | `E_DOUBLE_PRECOMMIT` |
| `VOT-PC02` | Prevote Quorum Exists | `prevote_weight[block] ≥ QUORUM` | `E_NO_PREVOTE_QUORUM` |
| `VOT-PC03` | Lock Respected | `locked_round == null ∨ vote.block == locked_block` | `E_LOCK_VIOLATION` |
| `VOT-PC04` | Block Not NIL | `vote.block_hash ≠ NIL` | `E_COMMIT_NIL` |

#### C.3 Double-Vote Detection

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     DOUBLE-VOTE DETECTION MATRIX                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SCENARIO                          │ DETECTION        │ SLASH   │ EVIDENCE            │
│   ────────                          │ ─────────        │ ─────   │ ────────            │
│   Same voter, slot, round,          │                  │         │                     │
│   different block (PREVOTE)         │ Cache lookup     │ 100%    │ Both signed votes   │
│                                     │                  │         │                     │
│   Same voter, slot, round,          │                  │         │                     │
│   different block (PRECOMMIT)       │ Cache lookup     │ 100%    │ Both signed commits │
│                                     │                  │         │                     │
│   PRECOMMIT without matching        │                  │         │                     │
│   PREVOTE                           │ Quorum check     │ 0%      │ N/A (reject only)   │
│                                     │                  │         │                     │
│   Vote for block conflicting        │                  │         │                     │
│   with locked block                 │ Lock check       │ 50%     │ Lock + new vote     │
│                                     │                  │         │                     │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### C.4 Missing or Malformed Votes

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `VOT-MF01` | Signature Present | `vote.signature ≠ null` | `E_MISSING_SIGNATURE` |
| `VOT-MF02` | Signature Valid | `verify(voter_pubkey, vote_hash, signature)` | `E_INVALID_SIGNATURE` |
| `VOT-MF03` | Fields Complete | `all_required_fields_present(vote)` | `E_INCOMPLETE_VOTE` |
| `VOT-MF04` | Voter In Set | `vote.voter ∈ active_validators` | `E_UNKNOWN_VOTER` |

---

### 2.D Weight Integrity

#### D.1 Combined PoS (50%) + PoUW (50%) Weight Accumulation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     WEIGHT ACCUMULATION FORMULA                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   INDIVIDUAL VALIDATOR WEIGHT:                                                          │
│   ────────────────────────────                                                          │
│   W(v) = α × S(v) + β × P(v)                                                           │
│                                                                                         │
│   Where:                                                                                │
│     α = 0.50 (stake coefficient)                                                       │
│     β = 0.50 (PoUW coefficient)                                                        │
│     S(v) = stake[v] / Σ(stake[all])     (normalized stake)                             │
│     P(v) = pouw[v] / Σ(pouw[all])       (normalized PoUW score)                        │
│                                                                                         │
│   TOTAL VOTING WEIGHT:                                                                  │
│   ────────────────────                                                                  │
│   Total(block) = Σ W(v) for v ∈ voters(block)                                          │
│                                                                                         │
│   QUORUM CONDITION:                                                                     │
│   ─────────────────                                                                     │
│   Quorum reached ⟺ Total(block) ≥ 2/3                                                  │
│                                                                                         │
│   EXAMPLE:                                                                              │
│   ────────                                                                              │
│   Validator A: stake=1000 (20%), pouw=500 (50%)                                        │
│   W(A) = 0.50 × 0.20 + 0.50 × 0.50 = 0.10 + 0.25 = 0.35 (35%)                         │
│                                                                                         │
│   Validator B: stake=3000 (60%), pouw=200 (20%)                                        │
│   W(B) = 0.50 × 0.60 + 0.50 × 0.20 = 0.30 + 0.10 = 0.40 (40%)                         │
│                                                                                         │
│   Validator C: stake=1000 (20%), pouw=300 (30%)                                        │
│   W(C) = 0.50 × 0.20 + 0.50 × 0.30 = 0.10 + 0.15 = 0.25 (25%)                         │
│                                                                                         │
│   Total = 0.29 + 0.48 + 0.23 = 1.00 (100%) ✓                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### D.2 Threshold Formulas

| Threshold | Formula | Value | Purpose |
|-----------|---------|-------|---------|
| **Quorum** | `total_weight ≥ ⅔` | 66.67% | Finality requirement |
| **Proposal Accept** | `total_weight ≥ ½` | 50% | Block acceptance |
| **Slash Evidence** | `evidence_weight ≥ ⅓` | 33.33% | Slash confirmation |
| **View Change** | `nil_weight ≥ ⅔` | 66.67% | Round advancement |

#### D.3 Invalid Weight Conditions

| Check ID | Condition | Detection | Error |
|----------|-----------|-----------|-------|
| `WGT-IV01` | `W(v) < 0` | Bounds check | `E_NEGATIVE_WEIGHT` |
| `WGT-IV02` | `W(v) > 1.0` | Bounds check | `E_WEIGHT_OVERFLOW` |
| `WGT-IV03` | `Σ W(v) > 1.0 + ε` | Sum check | `E_TOTAL_OVERFLOW` |
| `WGT-IV04` | `stake[v] == 0 ∧ voted` | Eligibility | `E_ZERO_STAKE_VOTE` |
| `WGT-IV05` | `W(v) ≠ compute_weight(v)` | Recomputation | `E_WEIGHT_MISMATCH` |
| `WGT-IV06` | Duplicate voter in sum | Uniqueness check | `E_DOUBLE_COUNTED` |
| `WGT-IV07` | Stale PoUW scores used | Epoch check | `E_STALE_POUW` |

---

### 2.E Receipt Integrity (PoUW)

#### E.1 Execution Output Hash Integrity

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `RCP-OH01` | Hash Format | `len(output_hash) == 32` | `E_INVALID_HASH_FORMAT` |
| `RCP-OH02` | Hash Non-Zero | `output_hash ≠ 0x00...00` | `E_ZERO_HASH` |
| `RCP-OH03` | Hash Deterministic | `recompute(input) == output_hash` | `E_HASH_MISMATCH` |
| `RCP-OH04` | Input Referenced | `input_hash == task.input_hash` | `E_INPUT_MISMATCH` |

```
OUTPUT HASH VERIFICATION:

function verify_output_hash(receipt, task):
    // Method 1: Replicated execution
    if selected_for_replication(receipt.task_id):
        expected = execute_task(task.input)
        if hash(expected) != receipt.output_hash:
            slash(receipt.provider, INVALID_OUTPUT)
            return E_HASH_MISMATCH
    
    // Method 2: ZK proof (future)
    if receipt.zk_proof != null:
        if !verify_zk(receipt.zk_proof, task.circuit):
            return E_INVALID_PROOF
    
    return OK
```

#### E.2 GPU Identity Validation

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `RCP-GI01` | Provider Registered | `provider ∈ registered_providers` | `E_UNKNOWN_PROVIDER` |
| `RCP-GI02` | Provider Staked | `provider_stake[p] ≥ MIN_PROVIDER_STAKE` | `E_UNSTAKED_PROVIDER` |
| `RCP-GI03` | Hardware Attested | `verify_attestation(provider.hw_proof)` | `E_INVALID_ATTESTATION` |
| `RCP-GI04` | Capacity Available | `provider.available_capacity > 0` | `E_NO_CAPACITY` |

#### E.3 Proof-of-Execution Aggregation

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `RCP-PE01` | Receipt Signed | `verify(provider_pubkey, receipt_hash, sig)` | `E_INVALID_RECEIPT_SIG` |
| `RCP-PE02` | Task Completed | `receipt.status == COMPLETED` | `E_INCOMPLETE_TASK` |
| `RCP-PE03` | Time Bounded | `receipt.exec_time ≤ task.max_time` | `E_TIMEOUT_EXCEEDED` |
| `RCP-PE04` | Resources Valid | `receipt.resources ≤ task.max_resources` | `E_RESOURCE_EXCEEDED` |
| `RCP-PE05` | Score Computed | `receipt.score == compute_pouw_score(receipt)` | `E_SCORE_MISMATCH` |

#### E.4 Invalid Receipt Types

| Type | Description | Detection | Action |
|------|-------------|-----------|--------|
| **Forged Receipt** | Invalid provider signature | Signature verification | Reject |
| **Duplicate Receipt** | Same task_id processed | Cache lookup | Reject |
| **Stale Receipt** | Timestamp too old | Age check | Reject |
| **Output Fraud** | Wrong computation result | Replicated verification | Slash |
| **Task Mismatch** | Receipt for unknown task | Registry lookup | Reject |
| **Collusion** | Statistical anomaly | Pattern detection | Slash all |

---

### 2.F Re-org Protection

#### F.1 Finality Distance

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `RRG-FD01` | Finalized Immutable | `finalized_blocks ∩ reorg_set == ∅` | `E_FINALITY_VIOLATION` |
| `RRG-FD02` | Depth Limited | `reorg_depth ≤ MAX_REORG_DEPTH` | `E_REORG_TOO_DEEP` |
| `RRG-FD03` | Checkpoint Respected | `reorg_target.height > last_checkpoint.height` | `E_CHECKPOINT_VIOLATION` |

```
FINALITY DISTANCE RULES:

MAX_REORG_DEPTH = 100 blocks (configurable)
CHECKPOINT_INTERVAL = 1000 blocks

finality_distance(block) = tip.height - block.height

is_reorg_allowed(old_head, new_head):
    // Find common ancestor
    ancestor = find_common_ancestor(old_head, new_head)
    reorg_depth = old_head.height - ancestor.height
    
    // Check depth limit
    if reorg_depth > MAX_REORG_DEPTH:
        return E_REORG_TOO_DEEP
    
    // Check finality
    if any(block in finalized for block in chain(ancestor, old_head)):
        return E_FINALITY_VIOLATION
    
    // Check checkpoint
    if ancestor.height < last_checkpoint.height:
        return E_CHECKPOINT_VIOLATION
    
    return OK
```

#### F.2 Allowed vs Forbidden Reorganizations

| Scenario | Allowed | Reason |
|----------|---------|--------|
| Reorg within non-finalized blocks | ✓ | Normal fork resolution |
| Reorg with higher weight | ✓ | Heaviest chain wins |
| Reorg reverting finalized block | ✗ | Finality violation |
| Reorg deeper than MAX_DEPTH | ✗ | Long-range attack |
| Reorg crossing checkpoint | ✗ | Checkpoint anchor |
| Reorg with equal weight | ✓ | Lower hash wins |

#### F.3 Long-Range Attack Protection

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     LONG-RANGE ATTACK MITIGATION                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ATTACK: Adversary creates alternative chain from distant past                        │
│                                                                                         │
│   Main Chain:    [G]──[1]──[2]──[3]──...──[999]──[1000]                               │
│                    ╲                                                                    │
│   Attack Chain:    [1']──[2']──[3']──...──[999']──[1000']                             │
│                                                                                         │
│   DEFENSES:                                                                             │
│   ─────────                                                                             │
│   1. CHECKPOINTS                                                                        │
│      • Periodic finality anchors                                                       │
│      • Social consensus on checkpoint                                                  │
│      • Reorg cannot cross checkpoint                                                   │
│                                                                                         │
│   2. WEAK SUBJECTIVITY                                                                 │
│      • New nodes must sync from recent state                                           │
│      • Weak subjectivity period: 2 weeks                                               │
│      • Nodes joining after period need trusted checkpoint                              │
│                                                                                         │
│   3. STAKE LOCK PERIOD                                                                 │
│      • Stake cannot be withdrawn immediately                                           │
│      • Unbonding period: 21 days                                                       │
│      • Historical validators remain slashable                                          │
│                                                                                         │
│   4. FINALITY GADGET                                                                   │
│      • Explicit finality after ⅔ commits                                               │
│      • Finalized blocks immutable                                                      │
│      • Conflicting finalization triggers halt                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### F.4 Canonical Chain Selection Logic

```rust
fn select_canonical_chain(heads: &[BlockHash], state: &ChainState) -> Result<BlockHash> {
    let mut best_head = None;
    let mut best_weight = 0;
    
    for head in heads {
        let chain = build_chain(head, state)?;
        
        // Check finality constraint
        if violates_finality(&chain, &state.finalized) {
            continue; // Skip chains that violate finality
        }
        
        // Compute chain weight
        let weight = compute_chain_weight(&chain);
        
        // Compare weights
        if weight > best_weight {
            best_head = Some(*head);
            best_weight = weight;
        } else if weight == best_weight {
            // Tie-break: lower hash wins
            if head < &best_head.unwrap() {
                best_head = Some(*head);
            }
        }
    }
    
    best_head.ok_or(E_NO_VALID_CHAIN)
}
```

---

### 2.G Fork-Choice Integrity

#### G.1 Scoring Function

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     FORK-CHOICE SCORING                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CHAIN WEIGHT:                                                                         │
│   ─────────────                                                                         │
│   chain_weight(chain) = Σ block_weight(b) for b ∈ chain                                │
│                                                                                         │
│   BLOCK WEIGHT:                                                                         │
│   ─────────────                                                                         │
│   block_weight(b) = base_weight                                                        │
│                   + attestation_bonus(b)                                               │
│                   + pouw_bonus(b)                                                      │
│                                                                                         │
│   Where:                                                                                │
│     base_weight        = 1000 (constant)                                               │
│     attestation_bonus  = Σ W(attester) × 1000                                          │
│     pouw_bonus         = Σ receipt.score × 0.5                                         │
│                                                                                         │
│   FORK CHOICE RULE:                                                                     │
│   ─────────────────                                                                     │
│   canonical = argmax(chains, chain_weight)                                             │
│                                                                                         │
│   TIE-BREAKING:                                                                         │
│   ─────────────                                                                         │
│   If chain_weight(A) == chain_weight(B):                                               │
│     canonical = chain with lower tip_hash                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `FRK-SC01` | Weight Positive | `chain_weight > 0` | `E_ZERO_WEIGHT` |
| `FRK-SC02` | Weight Deterministic | `compute_weight(chain) == claimed_weight` | `E_WEIGHT_MISMATCH` |
| `FRK-SC03` | Canonical Heaviest | `weight(canonical) ≥ weight(any_other)` | `E_NOT_HEAVIEST` |
| `FRK-SC04` | Tie Resolved | `if equal_weight: lower_hash wins` | `E_TIE_UNRESOLVED` |

#### G.2 Attacker Scenarios

| Scenario | Attack | Detection | Mitigation |
|----------|--------|-----------|------------|
| **Selfish Mining** | Withhold blocks | Timing analysis | Attestation deadlines |
| **Balance Attack** | Split votes evenly | Weight monitoring | Clear quorum rules |
| **Stake Grinding** | Manipulate VRF | VRF verification | Strong randomness |
| **Nothing-at-Stake** | Vote all forks | Conflict detection | Slashing |

#### G.3 Invalid Fork Transitions

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `FRK-TR01` | Parent Valid | `new_block.parent ∈ valid_chain` | `E_INVALID_PARENT` |
| `FRK-TR02` | No Skip | `new_block.height == parent.height + 1` | `E_HEIGHT_SKIP` |
| `FRK-TR03` | State Continuous | `new_block.pre_state == parent.post_state` | `E_STATE_DISCONTINUITY` |
| `FRK-TR04` | Time Forward | `new_block.timestamp > parent.timestamp` | `E_TIME_REGRESSION` |

---

### 2.H Timeout & Fallback Integrity

#### H.1 Timeout Rules

| Phase | Timeout | Trigger | Action |
|-------|---------|---------|--------|
| **Proposal** | 500ms | No proposal received | Broadcast NIL PREVOTE |
| **Prevote** | 1000ms | No ⅔ prevotes | Force view change |
| **Precommit** | 500ms | No ⅔ precommits | Force view change |
| **Finality** | 2000ms | No finalization | Emergency timeout |

```
TIMEOUT STATE MACHINE:

State: WAITING_PROPOSAL
  on proposal_received:
    → WAITING_PREVOTES
  on timeout(500ms):
    broadcast(NIL_PREVOTE)
    → WAITING_PREVOTES

State: WAITING_PREVOTES
  on prevote_quorum(block):
    broadcast(PRECOMMIT(block))
    → WAITING_PRECOMMITS
  on prevote_quorum(NIL):
    → VIEW_CHANGE
  on timeout(1000ms):
    → VIEW_CHANGE

State: WAITING_PRECOMMITS
  on precommit_quorum(block):
    finalize(block)
    → FINALIZED
  on timeout(500ms):
    → VIEW_CHANGE

State: VIEW_CHANGE
  increment_round()
  → WAITING_PROPOSAL
```

#### H.2 Fallback Proposer Activation

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `TIM-FB01` | Primary Failed | `timeout_reached(slot, round)` | Trigger fallback |
| `TIM-FB02` | Fallback Eligible | `fallback == compute_leader(slot, round+1)` | `E_WRONG_FALLBACK` |
| `TIM-FB03` | Round Advanced | `new_round == old_round + 1` | `E_ROUND_SKIP` |
| `TIM-FB04` | Max Rounds | `round < MAX_ROUNDS_PER_SLOT` | `E_MAX_ROUNDS` |

#### H.3 Liveness Guarantees

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     LIVENESS GUARANTEES                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   GUARANTEE 1: Bounded Finalization Time                                               │
│   ──────────────────────────────────────                                                │
│   If >⅔ validators are honest and online:                                              │
│     finalization_time ≤ SLOT_DURATION + MAX_ROUNDS × ROUND_DURATION                    │
│                                                                                         │
│   GUARANTEE 2: Progress Under Partition                                                │
│   ─────────────────────────────────────                                                 │
│   If network heals within PARTITION_TOLERANCE:                                         │
│     consensus resumes without manual intervention                                      │
│                                                                                         │
│   GUARANTEE 3: No Permanent Stall                                                      │
│   ───────────────────────────────                                                       │
│   If no ⅔ quorum after MAX_ROUNDS:                                                     │
│     slot is skipped, next slot begins                                                  │
│                                                                                         │
│   PARAMETERS:                                                                           │
│     SLOT_DURATION = 2000ms                                                             │
│     ROUND_DURATION = 2000ms                                                            │
│     MAX_ROUNDS = 10                                                                    │
│     PARTITION_TOLERANCE = 30 seconds                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

### 2.I State Consistency Integrity

#### I.1 State Hash Invariants

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `STC-SH01` | Root Computed | `computed_root == block.state_root` | `E_STATE_ROOT_MISMATCH` |
| `STC-SH02` | Root Deterministic | `replay(txs, pre_state) == post_state` | `E_NONDETERMINISTIC` |
| `STC-SH03` | Root Format | `len(state_root) == 32` | `E_INVALID_ROOT_FORMAT` |
| `STC-SH04` | Trie Consistent | `verify_trie_integrity(state_root)` | `E_TRIE_CORRUPT` |

#### I.2 Gas Model Invariants

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `STC-GS01` | Total Within Limit | `Σ gas_used ≤ block.gas_limit` | `E_BLOCK_GAS_EXCEEDED` |
| `STC-GS02` | Individual Within Limit | `∀ tx: tx.gas_used ≤ tx.gas_limit` | `E_TX_GAS_EXCEEDED` |
| `STC-GS03` | Fee Collected | `fee_recipient.balance += Σ fees` | `E_FEE_MISMATCH` |
| `STC-GS04` | Refund Correct | `refund = (gas_limit - gas_used) × gas_price` | `E_REFUND_ERROR` |

#### I.3 Nonce / Replay Protection Invariants

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `STC-NR01` | Nonce Sequential | `tx.nonce == sender.nonce` | `E_INVALID_NONCE` |
| `STC-NR02` | Nonce Incremented | `sender.nonce' == sender.nonce + 1` | `E_NONCE_NOT_INC` |
| `STC-NR03` | Chain ID Bound | `tx.chain_id == CHAIN_ID` | `E_WRONG_CHAIN` |
| `STC-NR04` | No Replay | `tx.hash ∉ processed_txs` | `E_REPLAY_DETECTED` |

#### I.4 Execution Alignment with Receipts

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `STC-EA01` | Receipt Count | `len(receipts) == len(txs)` | `E_RECEIPT_COUNT` |
| `STC-EA02` | Receipt Order | `receipts[i].tx_hash == txs[i].hash` | `E_RECEIPT_ORDER` |
| `STC-EA03` | Status Correct | `receipt.status == execution_result` | `E_STATUS_MISMATCH` |
| `STC-EA04` | Gas Matches | `receipt.gas_used == actual_gas_used` | `E_GAS_MISMATCH` |

---

### 2.J Network & Gossip Integrity

#### J.1 Message Propagation Guarantees

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `NET-MP01` | Delivery Bound | `delivery_time ≤ MAX_PROPAGATION_TIME` | `E_SLOW_PROPAGATION` |
| `NET-MP02` | Reliable Delivery | `sent ⟹ eventually_received` | `E_MESSAGE_LOST` |
| `NET-MP03` | Order Preserved | `causal_order_maintained` | `E_ORDER_VIOLATION` |
| `NET-MP04` | No Amplification | `forwarded_once_per_peer` | `E_AMPLIFICATION` |

#### J.2 Duplicate Detection

```
DUPLICATE DETECTION:

message_cache: LRU<MessageHash, Timestamp>
CACHE_TTL = 5 minutes

function receive_message(msg):
    hash = compute_hash(msg)
    
    if hash in message_cache:
        if message_cache[hash].age < CACHE_TTL:
            return E_DUPLICATE_MESSAGE
    
    message_cache[hash] = now()
    process(msg)
    return OK
```

| Check ID | Rule | Condition | Error |
|----------|------|-----------|-------|
| `NET-DD01` | Not In Cache | `msg.hash ∉ recent_messages` | `E_DUPLICATE` |
| `NET-DD02` | Cache Fresh | `cache_entry.age < CACHE_TTL` | — |
| `NET-DD03` | Hash Unique | `msg.hash == compute_hash(msg)` | `E_HASH_MISMATCH` |

#### J.3 Malformed Message Handling

| Message Issue | Detection | Response |
|---------------|-----------|----------|
| Invalid encoding | Deserialization fails | Drop message |
| Missing required fields | Schema validation | Drop message |
| Oversized message | Size check | Drop + rate limit peer |
| Invalid signature | Signature verification | Drop message |
| Unknown message type | Type check | Drop message |
| Future version | Version check | Drop or upgrade |

---

## 3. Integrity Pipeline Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     CONSENSUS INTEGRITY PIPELINE                                        │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  Incoming Block / Vote / Receipt
              │
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 1: HEADER INTEGRITY
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ Parent Link   │──── Invalid ────▶ REJECT (E_UNKNOWN_PARENT)
      │ Check         │
      └───────┬───────┘
              │ Valid
              ▼
      ┌───────────────┐
      │ Height/Slot   │──── Invalid ────▶ REJECT (E_INVALID_HEIGHT)
      │ Check         │
      └───────┬───────┘
              │ Valid
              ▼
      ┌───────────────┐
      │ Hash/Root     │──── Invalid ────▶ REJECT (E_HASH_MISMATCH)
      │ Check         │
      └───────┬───────┘
              │ Valid
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 2: PROPOSER CHECK
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ Validator     │──── Not Found ──▶ REJECT (E_NOT_VALIDATOR)
      │ Set Check     │
      └───────┬───────┘
              │ Found
              ▼
      ┌───────────────┐
      │ Leader        │──── Not Leader ─▶ REJECT (E_NOT_LEADER)
      │ Election      │
      └───────┬───────┘
              │ Is Leader
              ▼
      ┌───────────────┐
      │ VRF Proof     │──── Invalid ────▶ REJECT (E_INVALID_VRF)
      │ Verify        │
      └───────┬───────┘
              │ Valid
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 3: VOTE VALIDATION
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ Signature     │──── Invalid ────▶ REJECT (E_INVALID_SIGNATURE)
      │ Verify        │
      └───────┬───────┘
              │ Valid
              ▼
      ┌───────────────┐
      │ Conflict      │──── Conflict ───▶ SLASH + REJECT
      │ Detection     │
      └───────┬───────┘
              │ No Conflict
              ▼
      ┌───────────────┐
      │ Round/Slot    │──── Mismatch ───▶ REJECT (E_WRONG_ROUND)
      │ Match         │
      └───────┬───────┘
              │ Match
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 4: WEIGHT VALIDATION
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ Compute       │
      │ Vote Weight   │
      │ (50%S + 50%P) │
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Accumulate    │
      │ Total Weight  │
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Check Quorum  │──── Below ⅔ ────▶ WAIT (E_NO_QUORUM)
      │ (≥ 66.7%)     │
      └───────┬───────┘
              │ Quorum
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 5: RECEIPT VALIDATION
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ Provider      │──── Unknown ────▶ REJECT RECEIPT
      │ Check         │
      └───────┬───────┘
              │ Known
              ▼
      ┌───────────────┐
      │ Output Hash   │──── Mismatch ───▶ SLASH PROVIDER
      │ Verify        │
      └───────┬───────┘
              │ Valid
              ▼
      ┌───────────────┐
      │ Compute PoUW  │
      │ Score         │
      └───────┬───────┘
              │
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 6: FORK-CHOICE
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ Compute Chain │
      │ Weight        │
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Compare to    │──── Not Heavier ─▶ IGNORE (keep current)
      │ Current Best  │
      └───────┬───────┘
              │ Heavier
              ▼
      ┌───────────────┐
      │ Check Reorg   │──── Forbidden ──▶ REJECT (E_FINALITY_VIOLATION)
      │ Allowed       │
      └───────┬───────┘
              │ Allowed
              ▼
      ┌───────────────┐
      │ Update        │
      │ Canonical     │
      └───────┬───────┘
              │
              ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 7: FINALITY
  ════════════════════════════════════════════════════════════════════════════════════════
              │
              ▼
      ┌───────────────┐
      │ ⅔ Precommits? │──── No ─────────▶ PENDING
      └───────┬───────┘
              │ Yes
              ▼
      ┌───────────────┐
      │ Mark Block    │
      │ FINALIZED     │
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Execute Block │
      │ Update State  │
      └───────┬───────┘
              │
              ▼
      ┌───────────────┐
      │ Persist &     │
      │ Broadcast     │
      └───────────────┘
              │
              ▼
         ┌─────────┐
         │  DONE   │
         └─────────┘
```

---

## 4. Failure Conditions Table

### 4.1 Category A: Header Integrity Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Unknown parent hash | DB lookup | Block rejected | `E_UNKNOWN_PARENT` |
| Height not sequential | Arithmetic check | Block rejected | `E_INVALID_HEIGHT` |
| Slot regression | Comparison | Block rejected | `E_SLOT_REGRESSION` |
| Future timestamp | Clock check | Block held | `E_FUTURE_TIMESTAMP` |
| Hash mismatch | Recomputation | Block rejected | `E_HASH_MISMATCH` |
| Root mismatch | Merkle verification | Block rejected | `E_ROOT_MISMATCH` |

### 4.2 Category B: Proposer Legitimacy Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Not in validator set | Set membership | Block rejected | `E_NOT_VALIDATOR` |
| Not slot leader | VRF verification | Block rejected | `E_NOT_LEADER` |
| Slashed proposer | Status check | Block rejected | `E_PROPOSER_SLASHED` |
| Invalid VRF proof | Cryptographic verify | Block rejected | `E_INVALID_VRF` |
| Stale weights | Epoch check | Block rejected | `E_STALE_WEIGHTS` |

### 4.3 Category C: Voting Integrity Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Double prevote | Cache lookup | Slash + reject | `E_DOUBLE_PREVOTE` |
| Double precommit | Cache lookup | Slash + reject | `E_DOUBLE_PRECOMMIT` |
| Invalid signature | Crypto verify | Vote rejected | `E_INVALID_SIGNATURE` |
| Wrong round | Comparison | Vote rejected | `E_WRONG_ROUND` |
| Lock violation | Lock check | Slash + reject | `E_LOCK_VIOLATION` |

### 4.4 Category D: Weight Integrity Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Negative weight | Bounds check | Reject | `E_NEGATIVE_WEIGHT` |
| Weight overflow | Bounds check | Cap at max | `E_WEIGHT_OVERFLOW` |
| Double-counted vote | Uniqueness check | Reject duplicate | `E_DOUBLE_COUNTED` |
| Weight mismatch | Recomputation | Reject | `E_WEIGHT_MISMATCH` |
| Stale PoUW score | Epoch check | Use fallback | `E_STALE_POUW` |

### 4.5 Category E: Receipt Integrity Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Unknown provider | Registry lookup | Receipt rejected | `E_UNKNOWN_PROVIDER` |
| Invalid signature | Crypto verify | Receipt rejected | `E_INVALID_RECEIPT_SIG` |
| Output mismatch | Replicated verify | Slash provider | `E_OUTPUT_MISMATCH` |
| Task not found | Registry lookup | Receipt rejected | `E_TASK_NOT_FOUND` |
| Stale receipt | Timestamp check | Receipt rejected | `E_STALE_RECEIPT` |

### 4.6 Category F: Re-org Protection Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Revert finalized | Finality check | Reorg rejected | `E_FINALITY_VIOLATION` |
| Reorg too deep | Depth check | Reorg rejected | `E_REORG_TOO_DEEP` |
| Cross checkpoint | Checkpoint check | Reorg rejected | `E_CHECKPOINT_VIOLATION` |
| No weight gain | Weight comparison | Reorg rejected | `E_NO_WEIGHT_GAIN` |

### 4.7 Category G: Fork-Choice Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Zero chain weight | Sum check | Chain invalid | `E_ZERO_WEIGHT` |
| Weight mismatch | Recomputation | Reject claim | `E_WEIGHT_MISMATCH` |
| Invalid parent | Chain traversal | Reject fork | `E_INVALID_PARENT` |
| State discontinuity | State check | Reject fork | `E_STATE_DISCONTINUITY` |

### 4.8 Category H: Timeout Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Proposal timeout | Timer | NIL vote | `E_PROPOSAL_TIMEOUT` |
| Prevote timeout | Timer | View change | `E_PREVOTE_TIMEOUT` |
| Precommit timeout | Timer | View change | `E_PRECOMMIT_TIMEOUT` |
| Max rounds exceeded | Counter | Slot skipped | `E_MAX_ROUNDS` |

### 4.9 Category I: State Consistency Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| State root mismatch | Recomputation | Block rejected | `E_STATE_ROOT_MISMATCH` |
| Gas exceeded | Sum check | Block rejected | `E_GAS_EXCEEDED` |
| Invalid nonce | Sequence check | Tx rejected | `E_INVALID_NONCE` |
| Replay detected | Cache lookup | Tx rejected | `E_REPLAY_DETECTED` |

### 4.10 Category J: Network Integrity Failures

| Trigger | Detection | Outcome | Error Code |
|---------|-----------|---------|------------|
| Duplicate message | Cache lookup | Message dropped | `E_DUPLICATE` |
| Malformed message | Deserialization | Message dropped | `E_MALFORMED` |
| Oversized message | Size check | Drop + rate limit | `E_OVERSIZED` |
| Invalid encoding | Schema check | Message dropped | `E_INVALID_ENCODING` |

---

## 5. Developer Notes

### 5.1 Rust Module Mapping

```
pow/
├── src/
│   ├── integrity/
│   │   ├── mod.rs                    // Module exports
│   │   ├── header.rs                 // Category A: HDR-* checks
│   │   ├── proposer.rs               // Category B: PRP-* checks
│   │   ├── voting.rs                 // Category C: VOT-* checks
│   │   ├── weight.rs                 // Category D: WGT-* checks
│   │   ├── receipt.rs                // Category E: RCP-* checks
│   │   ├── reorg.rs                  // Category F: RRG-* checks
│   │   ├── fork_choice.rs            // Category G: FRK-* checks
│   │   ├── timeout.rs                // Category H: TIM-* checks
│   │   ├── state.rs                  // Category I: STC-* checks
│   │   └── network.rs                // Category J: NET-* checks
│   │
│   └── errors/
│       └── integrity_errors.rs       // All error codes
│
└── tests/
    └── integrity/
        ├── header_tests.rs
        ├── proposer_tests.rs
        ├── voting_tests.rs
        ├── weight_tests.rs
        ├── receipt_tests.rs
        ├── reorg_tests.rs
        ├── fork_choice_tests.rs
        ├── timeout_tests.rs
        ├── state_tests.rs
        └── network_tests.rs
```

### 5.2 Writing Integrity Tests

```rust
// Example: Header integrity test
#[cfg(test)]
mod header_tests {
    use super::*;
    
    #[test]
    fn test_parent_linkage_valid() {
        let parent = create_block(height: 10);
        let child = create_block(height: 11, parent_hash: parent.hash);
        let state = ChainState::with_block(parent);
        
        let result = check_header_integrity(&child, &state);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_parent_linkage_unknown() {
        let child = create_block(height: 11, parent_hash: UNKNOWN_HASH);
        let state = ChainState::empty();
        
        let result = check_header_integrity(&child, &state);
        assert_eq!(result.unwrap_err().code, E_UNKNOWN_PARENT);
    }
    
    #[test]
    fn test_height_sequential() {
        let parent = create_block(height: 10);
        let child = create_block(height: 12, parent_hash: parent.hash); // Skip
        let state = ChainState::with_block(parent);
        
        let result = check_header_integrity(&child, &state);
        assert_eq!(result.unwrap_err().code, E_INVALID_HEIGHT);
    }
}

// Example: Weight integrity test
#[cfg(test)]
mod weight_tests {
    use super::*;
    
    #[test]
    fn test_weight_formula_correct() {
        let stake_weights = StakeWeights::new(vec![
            (VALIDATOR_A, 1000),  // 50%
            (VALIDATOR_B, 1000),  // 50%
        ]);
        let pouw_scores = PoUWScores::new(vec![
            (VALIDATOR_A, 300),   // 30%
            (VALIDATOR_B, 700),   // 70%
        ]);
        
        // W(A) = 0.50 × 0.50 + 0.50 × 0.30 = 0.25 + 0.15 = 0.40
        let weight_a = compute_weight(&VALIDATOR_A, &stake_weights, &pouw_scores);
        assert_approx_eq!(weight_a, 0.44, 0.001);
        
        // W(B) = 0.50 × 0.50 + 0.50 × 0.70 = 0.25 + 0.35 = 0.60
        let weight_b = compute_weight(&VALIDATOR_B, &stake_weights, &pouw_scores);
        assert_approx_eq!(weight_b, 0.56, 0.001);
        
        // Total should be 1.0
        assert_approx_eq!(weight_a + weight_b, 1.0, 0.001);
    }
    
    #[test]
    fn test_quorum_threshold() {
        let votes = create_votes_with_weights(vec![0.34, 0.33]); // Total: 0.67
        
        // Just above ⅔ threshold
        assert!(check_quorum(&votes, 0.667).is_ok());
        
        let insufficient = create_votes_with_weights(vec![0.34, 0.32]); // Total: 0.66
        assert_eq!(check_quorum(&insufficient, 0.667).unwrap_err().code, E_NO_QUORUM);
    }
}
```

### 5.3 CI Validation Integration

```yaml
# .github/workflows/integrity-checks.yml
name: Consensus Integrity Tests

on: [push, pull_request]

jobs:
  integrity-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run Header Integrity Tests
        run: cargo test -p pow integrity::header --all-features
      
      - name: Run Proposer Integrity Tests
        run: cargo test -p pow integrity::proposer --all-features
      
      - name: Run Voting Integrity Tests
        run: cargo test -p pow integrity::voting --all-features
      
      - name: Run Weight Integrity Tests
        run: cargo test -p pow integrity::weight --all-features
      
      - name: Run Receipt Integrity Tests
        run: cargo test -p pow integrity::receipt --all-features
      
      - name: Run Reorg Integrity Tests
        run: cargo test -p pow integrity::reorg --all-features
      
      - name: Run Fork-Choice Tests
        run: cargo test -p pow integrity::fork_choice --all-features
      
      - name: Run Timeout Tests
        run: cargo test -p pow integrity::timeout --all-features
      
      - name: Run State Consistency Tests
        run: cargo test -p pow integrity::state --all-features
      
      - name: Run Network Integrity Tests
        run: cargo test -p pow integrity::network --all-features
      
      - name: Run Full Integration Tests
        run: cargo test -p pow --test integration_tests
```

### 5.4 Local Development Commands

```bash
# Run all integrity checks
cargo test -p pow integrity:: --all-features

# Run specific category
cargo test -p pow integrity::header::
cargo test -p pow integrity::weight::
cargo test -p pow integrity::receipt::

# Run with verbose output
cargo test -p pow integrity:: -- --nocapture

# Run benchmarks
cargo bench -p pow -- integrity

# Check for integrity-related clippy warnings
cargo clippy -p pow -- -D clippy::unwrap_used

# Generate coverage report
cargo tarpaulin -p pow --out Html -- integrity::
```

---

## Appendix: Integrity Check Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                     INTEGRITY CHECK QUICK REFERENCE                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   A. HEADER (HDR-*)       │  B. PROPOSER (PRP-*)      │  C. VOTING (VOT-*)             │
│   ─────────────────       │  ────────────────────     │  ──────────────────            │
│   PL01: Parent exists     │  EL01: In validator set   │  PV01: Single prevote          │
│   PL02: Parent valid      │  EL02: Stake sufficient   │  PV02: Known block             │
│   HS01: Height sequential │  SW01: VRF proof valid    │  PC01: Single precommit        │
│   HS02: Slot progressive  │  SW02: Is leader          │  PC02: Prevote quorum          │
│   DS01: Difficulty > 0    │  RS01: Rotation period    │  PC03: Lock respected          │
│   HD01: Hash correct      │  RS02: Set deterministic  │  MF01-04: Format valid         │
│                           │                           │                                 │
│   D. WEIGHT (WGT-*)       │  E. RECEIPT (RCP-*)       │  F. REORG (RRG-*)              │
│   ─────────────────       │  ─────────────────        │  ────────────────              │
│   IV01: Weight ≥ 0        │  OH01-04: Hash valid      │  FD01: Finalized immutable     │
│   IV02: Weight ≤ 1        │  GI01-04: Provider valid  │  FD02: Depth limited           │
│   IV03: Total ≤ 1         │  PE01-05: Execution OK    │  FD03: Checkpoint respected    │
│   IV04: Stake > 0         │                           │                                 │
│                           │                           │                                 │
│   G. FORK-CHOICE (FRK-*)  │  H. TIMEOUT (TIM-*)       │  I. STATE (STC-*)              │
│   ──────────────────────  │  ─────────────────        │  ────────────────              │
│   SC01: Weight positive   │  FB01: Primary failed     │  SH01: Root matches            │
│   SC02: Deterministic     │  FB02: Fallback eligible  │  GS01-04: Gas valid            │
│   SC03: Heaviest wins     │  FB03: Round advanced     │  NR01-04: Nonce/replay         │
│   TR01-04: Transitions    │  FB04: Max rounds         │  EA01-04: Receipts aligned     │
│                           │                           │                                 │
│   J. NETWORK (NET-*)                                                                   │
│   ──────────────────                                                                   │
│   MP01-04: Propagation    │  DD01-03: Duplicates      │  Malformed handling            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

*This document is the canonical integrity checks specification for Mbongo Chain consensus. All implementations must enforce these checks.*

