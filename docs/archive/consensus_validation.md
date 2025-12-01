<!-- Verified against tokenomics.md -->
# Mbongo Chain — Consensus Validation

This document provides a comprehensive technical specification of the consensus validation mechanisms in Mbongo Chain, covering the hybrid PoS + PoUW architecture, block validation pipeline, fork-choice rules, and security model.

---

## 1. Consensus Overview

### Hybrid PoS + PoUW Architecture

Mbongo Chain implements a **hybrid consensus model** combining Proof of Stake (PoS) for economic security with Proof of Useful Work (PoUW) for compute validation.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     HYBRID CONSENSUS ARCHITECTURE                           │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────┐
                              │   CONSENSUS     │
                              │     LAYER       │
                              └────────┬────────┘
                                       │
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
              ▼                        ▼                        ▼
    ┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
    │      PoS        │      │     PoUW        │      │    FINALITY     │
    │   COMPONENT     │      │   COMPONENT     │      │    GADGET       │
    │                 │      │                 │      │                 │
    │ • Leader elect  │      │ • Compute proof │      │ • Checkpoints   │
    │ • Stake weight  │      │ • Task verify   │      │ • Attestations  │
    │ • Slashing      │      │ • Reward calc   │      │ • Finalization  │
    └────────┬────────┘      └────────┬────────┘      └────────┬────────┘
             │                        │                        │
             └────────────────────────┼────────────────────────┘
                                      │
                              ┌───────▼───────┐
                              │  CHAIN WEIGHT │
                              │  CALCULATION  │
                              │               │
                              │ W = Σ(stake)  │
                              │   + Σ(pouw)   │
                              └───────────────┘
```

### Deterministic PoS Leader Selection

Leader selection is deterministic given the same inputs:

```rust
// Leader selection algorithm (conceptual)
fn select_leader(
    slot: Slot,
    epoch_randomness: Hash,
    validator_set: &ValidatorSet,
) -> ValidatorId {
    // Combine slot and epoch randomness for seed
    let seed = hash(slot.to_le_bytes(), epoch_randomness);
    
    // Calculate cumulative stake
    let total_stake: u128 = validator_set.iter().map(|v| v.stake).sum();
    
    // Select based on stake-weighted random
    let selection_point = u128::from_le_bytes(seed[0..16]) % total_stake;
    
    let mut cumulative = 0u128;
    for validator in validator_set.iter() {
        cumulative += validator.stake;
        if cumulative > selection_point {
            return validator.id;
        }
    }
    
    unreachable!("validator set is non-empty")
}
```

### PoUW Compute Validation

Compute proofs are validated on-chain:

| Stage | Description |
|-------|-------------|
| **Assignment** | Task assigned to compute provider |
| **Execution** | Provider executes task off-chain |
| **Proof** | Provider generates verifiable proof |
| **Verification** | On-chain verification of proof correctness |
| **Reward** | Provider receives compute rewards |

### Finality via Checkpointing

Blocks become final through checkpoint attestations:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     FINALITY CHECKPOINTING                                  │
└─────────────────────────────────────────────────────────────────────────────┘

  Epoch N-1                    Epoch N                      Epoch N+1
  ─────────                    ───────                      ─────────
  
  [Block]─[Block]─[Block]──►[Checkpoint]─[Block]─[Block]──►[Checkpoint]
                                  │                              │
                                  │                              │
                            >2/3 stake                     >2/3 stake
                            attestations                   attestations
                                  │                              │
                                  ▼                              ▼
                             FINALIZED                      FINALIZED
```

---

## 2. PoS Leader Selection

### Validator Eligibility Rules

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     VALIDATOR ELIGIBILITY                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  MUST satisfy ALL conditions:                                               │
│                                                                             │
│  □ Stake >= MINIMUM_STAKE (e.g., 32,000 tokens)                            │
│  □ Registered in validator set before epoch boundary                        │
│  □ Not currently slashed or jailed                                          │
│  □ Activation delay elapsed (e.g., 2 epochs)                               │
│  □ Valid withdrawal credentials                                             │
│  □ Node online and responding                                               │
│                                                                             │
│  MAY be temporarily ineligible if:                                          │
│                                                                             │
│  □ Pending exit (exit_epoch <= current_epoch)                              │
│  □ Under investigation for slashable offense                                │
│  □ Voluntary inactive status                                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stake Weighting

Validators are selected proportionally to their stake:

```rust
// Stake weight calculation
struct Validator {
    id: ValidatorId,
    pubkey: PublicKey,
    stake: u128,
    effective_stake: u128,  // After caps and penalties
    activation_epoch: Epoch,
    exit_epoch: Epoch,
}

impl Validator {
    fn calculate_effective_stake(&self, config: &Config) -> u128 {
        // Apply maximum effective stake cap
        let capped = self.stake.min(config.max_effective_stake);
        
        // Apply any active penalties
        let penalized = capped.saturating_sub(self.pending_penalties);
        
        penalized
    }
    
    fn selection_weight(&self, total_effective_stake: u128) -> f64 {
        self.effective_stake as f64 / total_effective_stake as f64
    }
}
```

### VRF-Based Leader Election

Verifiable Random Function ensures unpredictable but verifiable leader selection:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     VRF LEADER ELECTION                                     │
└─────────────────────────────────────────────────────────────────────────────┘

                     ┌─────────────────────────┐
                     │     EPOCH RANDOMNESS    │
                     │  (from previous epoch)  │
                     └───────────┬─────────────┘
                                 │
                                 ▼
  ┌─────────────┐      ┌─────────────────────────┐      ┌─────────────┐
  │   SLOT N    │─────▶│       VRF COMPUTE       │─────▶│   LEADER    │
  │             │      │                         │      │  SELECTED   │
  └─────────────┘      │  vrf_output = VRF(      │      └─────────────┘
                       │    sk_validator,        │
                       │    slot || epoch_rand   │
                       │  )                      │
                       │                         │
                       │  if vrf_output < thresh │
                       │    → validator is leader│
                       └─────────────────────────┘

  Threshold Calculation:
  ──────────────────────
  threshold = (effective_stake / total_stake) * MAX_VRF_OUTPUT
```

```rust
// VRF-based leader election (conceptual)
struct VrfProof {
    output: [u8; 32],
    proof: [u8; 64],
}

impl Validator {
    fn compute_vrf(&self, slot: Slot, epoch_randomness: &Hash) -> VrfProof {
        let input = [slot.to_le_bytes(), epoch_randomness.as_bytes()].concat();
        vrf_prove(&self.secret_key, &input)
    }
    
    fn is_leader(&self, slot: Slot, epoch_randomness: &Hash, total_stake: u128) -> bool {
        let vrf_proof = self.compute_vrf(slot, epoch_randomness);
        let threshold = self.calculate_threshold(total_stake);
        
        // Compare VRF output against stake-weighted threshold
        u256_from_bytes(&vrf_proof.output) < threshold
    }
    
    fn calculate_threshold(&self, total_stake: u128) -> U256 {
        // probability = effective_stake / total_stake
        // threshold = probability * MAX_U256
        U256::MAX * U256::from(self.effective_stake) / U256::from(total_stake)
    }
}
```

### Slashing Rules

| Offense | Detection | Penalty | Evidence Required |
|---------|-----------|---------|-------------------|
| **Double Sign** | Two blocks for same slot | 100% stake | Both signed blocks |
| **Invalid Header** | Malformed block proposal | 10% stake | Invalid block |
| **Surround Vote** | Conflicting attestations | 100% stake | Both attestations |
| **Downtime** | Missing N consecutive slots | 0.1% stake per slot | Absence proof |

```rust
// Slashing condition detection
enum SlashableOffense {
    DoubleSign {
        slot: Slot,
        block_a: SignedBlockHeader,
        block_b: SignedBlockHeader,
    },
    SurroundVote {
        attestation_a: SignedAttestation,
        attestation_b: SignedAttestation,
    },
    InvalidHeader {
        block: SignedBlockHeader,
        reason: InvalidReason,
    },
}

fn detect_double_sign(a: &SignedBlockHeader, b: &SignedBlockHeader) -> bool {
    a.slot == b.slot 
        && a.proposer == b.proposer 
        && a.hash() != b.hash()
        && verify_signature(&a.proposer_pubkey, &a.hash(), &a.signature)
        && verify_signature(&b.proposer_pubkey, &b.hash(), &b.signature)
}

fn calculate_slash_amount(offense: &SlashableOffense, stake: u128) -> u128 {
    match offense {
        SlashableOffense::DoubleSign { .. } => stake,  // 100%
        SlashableOffense::SurroundVote { .. } => stake,  // 100%
        SlashableOffense::InvalidHeader { .. } => stake / 10,  // 10%
    }
}
```

---

## 3. PoUW Execution Validation

### Compute Task Assignment

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     COMPUTE TASK ASSIGNMENT                                 │
└─────────────────────────────────────────────────────────────────────────────┘

  Task Requester              Network                 Compute Provider
        │                        │                          │
        │── Submit Task ────────▶│                          │
        │   (commitment, fee)    │                          │
        │                        │                          │
        │                        │── Broadcast Task ───────▶│
        │                        │                          │
        │                        │                    ┌─────┴─────┐
        │                        │                    │  Accept   │
        │                        │                    │  Task     │
        │                        │                    └─────┬─────┘
        │                        │                          │
        │                        │◀── Task Acceptance ──────│
        │                        │                          │
        │                        │                    ┌─────┴─────┐
        │                        │                    │  Execute  │
        │                        │                    │  Off-chain│
        │                        │                    └─────┬─────┘
        │                        │                          │
        │                        │◀── Submit Proof ─────────│
        │                        │                          │
        │                  ┌─────┴─────┐                    │
        │                  │  Verify   │                    │
        │                  │  On-chain │                    │
        │                  └─────┬─────┘                    │
        │                        │                          │
        │◀── Result ─────────────│                          │
        │                        │── Reward ───────────────▶│
        │                        │                          │
```

### Execution Correctness Receipt

```rust
// Compute execution receipt
struct ComputeReceipt {
    /// Task identifier
    task_id: Hash,
    
    /// Compute provider
    provider: Address,
    
    /// Input commitment (hash of task input)
    input_commitment: Hash,
    
    /// Output commitment (hash of result)
    output_commitment: Hash,
    
    /// Proof of correct execution
    proof: ComputeProof,
    
    /// Compute units consumed
    compute_units: u64,
    
    /// Timestamp of completion
    completed_at: Timestamp,
    
    /// Provider signature
    signature: Signature,
}

// Compute proof structure
enum ComputeProof {
    /// Deterministic replay (re-execute to verify)
    Replay {
        execution_trace: Vec<u8>,
    },
    
    /// SNARK proof (succinct verification)
    Snark {
        proof: [u8; 256],
        public_inputs: Vec<u8>,
    },
    
    /// Trusted execution (TEE attestation)
    TeeAttestation {
        attestation: TeeAttestation,
        measurement: Hash,
    },
}

impl ComputeReceipt {
    fn verify(&self, task: &ComputeTask) -> Result<(), VerificationError> {
        // 1. Verify input commitment matches task
        if self.input_commitment != task.input_commitment {
            return Err(VerificationError::InputMismatch);
        }
        
        // 2. Verify provider signature
        if !verify_signature(&self.provider, &self.hash(), &self.signature) {
            return Err(VerificationError::InvalidSignature);
        }
        
        // 3. Verify proof based on type
        match &self.proof {
            ComputeProof::Replay { execution_trace } => {
                self.verify_replay(task, execution_trace)
            }
            ComputeProof::Snark { proof, public_inputs } => {
                self.verify_snark(proof, public_inputs)
            }
            ComputeProof::TeeAttestation { attestation, measurement } => {
                self.verify_tee(attestation, measurement)
            }
        }
    }
}
```

### Useful Compute Scoring

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     COMPUTE SCORING                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Score = base_compute_units × quality_multiplier × demand_factor           │
│                                                                             │
│  Where:                                                                     │
│  • base_compute_units = raw compute measured                               │
│  • quality_multiplier = 1.0 + (accuracy_bonus) - (latency_penalty)        │
│  • demand_factor = network_demand / supply (capped at 2.0)                 │
│                                                                             │
│  Quality Multiplier Components:                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  accuracy_bonus:                                                    │   │
│  │    • Perfect result match: +0.1                                     │   │
│  │    • Within tolerance: +0.05                                        │   │
│  │    • Edge case handling: +0.02                                      │   │
│  │                                                                     │   │
│  │  latency_penalty:                                                   │   │
│  │    • > 2x expected time: -0.1                                       │   │
│  │    • > 5x expected time: -0.3                                       │   │
│  │    • timeout: -1.0 (no reward)                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```rust
// Compute scoring
struct ComputeScore {
    base_units: u64,
    quality_multiplier: f64,
    demand_factor: f64,
}

impl ComputeScore {
    fn calculate(receipt: &ComputeReceipt, market: &ComputeMarket) -> Self {
        let base_units = receipt.compute_units;
        
        let quality_multiplier = Self::calculate_quality(receipt);
        let demand_factor = Self::calculate_demand(market).min(2.0);
        
        Self {
            base_units,
            quality_multiplier,
            demand_factor,
        }
    }
    
    fn total_score(&self) -> u64 {
        (self.base_units as f64 * self.quality_multiplier * self.demand_factor) as u64
    }
    
    fn calculate_quality(receipt: &ComputeReceipt) -> f64 {
        let mut multiplier = 1.0;
        
        // Accuracy bonus
        if receipt.accuracy == Accuracy::Perfect {
            multiplier += 0.1;
        }
        
        // Latency penalty
        let expected_time = receipt.task_expected_duration();
        let actual_time = receipt.actual_duration();
        
        if actual_time > expected_time * 5 {
            multiplier -= 0.3;
        } else if actual_time > expected_time * 2 {
            multiplier -= 0.1;
        }
        
        multiplier.max(0.0)
    }
}
```

### Future Compute Market Integration

*Status: Placeholder*

The PoUW layer is a heterogeneous compute layer (GPU/TPU/CPU/FPGA/ASIC) that accepts proofs from any supported accelerator hardware, provided the computation is deterministic and verifiable on-chain.

For full compute layer details, see:
- [docs/compute_engine_overview.md](./compute_engine_overview.md) — Compute engine architecture
- [docs/compute_value.md](./compute_value.md) — Compute economics and provider incentives

```
┌─────────────────────────────────────────────────────────────────────────────┐
│             COMPUTE MARKET INTEGRATION (FUTURE - Heterogeneous)             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Components:                                                                │
│  • Compute Provider Registry (GPU/TPU/CPU/FPGA/ASIC)                        │
│  • Hardware Attestation & Type Detection                                    │
│  • Real-time Pricing Oracle (normalized across hardware)                    │
│  • Task-Accelerator Matching Algorithm                                      │
│  • SLA Enforcement                                                          │
│                                                                             │
│  Integration Points (Heterogeneous Hardware):                               │
│  • NVIDIA CUDA attestation (GPU)                                            │
│  • AMD ROCm verification (GPU)                                              │
│  • Intel oneAPI support (CPU/GPU/FPGA)                                      │
│  • Google TPU API (TPU)                                                     │
│  • Custom ASIC drivers (ASIC)                                               │
│  • Apple Metal (future)                                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Full Block Validation Pipeline

### Pipeline Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     BLOCK VALIDATION PIPELINE                               │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐
  │ PROPOSE │──▶│ GOSSIP  │──▶│VALIDATE │──▶│VALIDATE │──▶│VALIDATE │
  │         │   │         │   │ HEADER  │   │  PoUW   │   │   TXS   │
  └─────────┘   └─────────┘   └─────────┘   └─────────┘   └─────────┘
       │             │             │             │             │
       ▼             ▼             ▼             ▼             ▼
  [Leader       [Broadcast   [Signature,  [Compute     [Format,
   creates]     to peers]    parent,      receipts]    signatures,
                             slot]                     balances]
                                                            │
  ┌─────────┐   ┌─────────┐   ┌─────────┐                   │
  │BROADCAST│◀──│ COMMIT  │◀──│  APPLY  │◀──────────────────┘
  │ COMMIT  │   │  BLOCK  │   │  STATE  │
  └─────────┘   └─────────┘   └─────────┘
       │             │             │
       ▼             ▼             ▼
  [Announce     [Persist     [Execute txs,
   finality]    to disk]     update state]
```

---

### Step 1: Propose

**Inputs:**
- Parent block hash
- Pending transactions from mempool
- Pending compute receipts
- Slot number
- Validator private key

**Outputs:**
- Signed block proposal

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| No transactions | Empty mempool | Propose empty block |
| Invalid parent | Stale view | Resync chain tip |
| Slot passed | Slow execution | Wait for next slot |

```rust
// Block proposal
fn propose_block(
    state: &State,
    mempool: &Mempool,
    compute_pool: &ComputePool,
    slot: Slot,
    validator_key: &ValidatorKey,
) -> Result<SignedBlock, ProposeError> {
    // 1. Select transactions
    let transactions = mempool.select_for_block(
        state,
        MAX_BLOCK_GAS,
        MAX_BLOCK_SIZE,
    )?;
    
    // 2. Select compute receipts
    let compute_receipts = compute_pool.select_verified(
        MAX_RECEIPTS_PER_BLOCK,
    )?;
    
    // 3. Build header
    let header = BlockHeader {
        parent_hash: state.head_hash(),
        height: state.height() + 1,
        slot,
        timestamp: current_timestamp(),
        transactions_root: merkle_root(&transactions),
        receipts_root: merkle_root(&compute_receipts),
        state_root: Hash::default(),  // Filled after execution
        proposer: validator_key.public_key(),
    };
    
    // 4. Execute and get state root
    let execution_result = execute_block(state, &header, &transactions)?;
    let header = header.with_state_root(execution_result.state_root);
    
    // 5. Sign block
    let signature = validator_key.sign(&header.hash());
    
    Ok(SignedBlock {
        header,
        transactions,
        compute_receipts,
        signature,
    })
}
```

---

### Step 2: Gossip

**Inputs:**
- Signed block proposal
- Peer list

**Outputs:**
- Block distributed to network

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| Network partition | Connectivity loss | Retry with backoff |
| Peer rejection | Invalid block | Check local validation |
| Timeout | Slow peers | Use faster peers |

```rust
// Block gossip
async fn gossip_block(
    network: &Network,
    block: &SignedBlock,
) -> Result<GossipStats, GossipError> {
    let block_hash = block.header.hash();
    let mut stats = GossipStats::default();
    
    // 1. Announce block hash
    let announcement = BlockAnnouncement {
        hash: block_hash,
        height: block.header.height,
        slot: block.header.slot,
    };
    
    // 2. Send to all connected peers
    for peer in network.connected_peers() {
        match network.send(peer, Message::NewBlock(announcement.clone())).await {
            Ok(_) => stats.announced += 1,
            Err(e) => stats.failed.push((peer, e)),
        }
    }
    
    // 3. Serve full block on request
    network.register_handler(block_hash, block.clone());
    
    Ok(stats)
}
```

---

### Step 3: Validate Header

**Inputs:**
- Block header
- Current chain state
- Validator set

**Outputs:**
- Header validation result

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| Invalid signature | Wrong proposer or tampered | Reject block |
| Wrong parent | Fork or stale | Check fork choice |
| Future slot | Clock skew | Wait or reject |
| Invalid proposer | Not assigned to slot | Reject block |

```rust
// Header validation
fn validate_header(
    header: &BlockHeader,
    signature: &Signature,
    state: &State,
    validator_set: &ValidatorSet,
) -> Result<(), HeaderValidationError> {
    // 1. Check parent exists
    let parent = state.get_header(&header.parent_hash)
        .ok_or(HeaderValidationError::UnknownParent)?;
    
    // 2. Check height
    if header.height != parent.height + 1 {
        return Err(HeaderValidationError::InvalidHeight {
            expected: parent.height + 1,
            got: header.height,
        });
    }
    
    // 3. Check slot
    if header.slot <= parent.slot {
        return Err(HeaderValidationError::InvalidSlot);
    }
    
    // 4. Check timestamp
    if header.timestamp <= parent.timestamp {
        return Err(HeaderValidationError::InvalidTimestamp);
    }
    
    let max_future = current_timestamp() + MAX_FUTURE_SLOT_TIME;
    if header.timestamp > max_future {
        return Err(HeaderValidationError::FutureBlock);
    }
    
    // 5. Check proposer is assigned to slot
    let expected_proposer = select_leader(
        header.slot,
        state.epoch_randomness(),
        validator_set,
    );
    
    if header.proposer != expected_proposer {
        return Err(HeaderValidationError::WrongProposer {
            expected: expected_proposer,
            got: header.proposer,
        });
    }
    
    // 6. Verify signature
    let proposer_pubkey = validator_set.get_pubkey(&header.proposer)?;
    if !verify_signature(&proposer_pubkey, &header.hash(), signature) {
        return Err(HeaderValidationError::InvalidSignature);
    }
    
    Ok(())
}
```

---

### Step 4: Validate PoUW Receipt

**Inputs:**
- Compute receipts in block
- Task registry
- Provider registry

**Outputs:**
- Validated receipts

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| Invalid proof | Wrong computation | Reject receipt |
| Unknown task | Task not registered | Reject receipt |
| Duplicate | Already claimed | Reject receipt |
| Expired | Task timeout | Reject receipt |

```rust
// PoUW receipt validation
fn validate_compute_receipts(
    receipts: &[ComputeReceipt],
    task_registry: &TaskRegistry,
    provider_registry: &ProviderRegistry,
    state: &State,
) -> Result<Vec<ValidatedReceipt>, ReceiptValidationError> {
    let mut validated = Vec::with_capacity(receipts.len());
    
    for receipt in receipts {
        // 1. Check task exists and is active
        let task = task_registry.get(&receipt.task_id)
            .ok_or(ReceiptValidationError::UnknownTask)?;
        
        if task.status != TaskStatus::Active {
            return Err(ReceiptValidationError::TaskNotActive);
        }
        
        // 2. Check provider is registered
        let provider = provider_registry.get(&receipt.provider)
            .ok_or(ReceiptValidationError::UnknownProvider)?;
        
        if !provider.is_eligible() {
            return Err(ReceiptValidationError::ProviderNotEligible);
        }
        
        // 3. Check not already claimed
        if state.is_receipt_claimed(&receipt.task_id, &receipt.provider) {
            return Err(ReceiptValidationError::AlreadyClaimed);
        }
        
        // 4. Verify proof
        receipt.verify(&task)?;
        
        // 5. Calculate score
        let score = ComputeScore::calculate(receipt, &state.compute_market());
        
        validated.push(ValidatedReceipt {
            receipt: receipt.clone(),
            score,
        });
    }
    
    Ok(validated)
}
```

---

### Step 5: Validate Transactions

**Inputs:**
- Transaction list
- Current state
- Block context

**Outputs:**
- Validated transactions

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| Invalid signature | Tampered or wrong key | Reject transaction |
| Insufficient balance | Sender can't pay | Reject transaction |
| Invalid nonce | Replay or out-of-order | Reject transaction |
| Gas limit exceeded | Block full | Reject remaining |

```rust
// Transaction validation
fn validate_transactions(
    transactions: &[Transaction],
    state: &State,
    block_context: &BlockContext,
) -> Result<Vec<ValidatedTransaction>, TxValidationError> {
    let mut validated = Vec::with_capacity(transactions.len());
    let mut gas_used = 0u64;
    let mut state_changes = StateChanges::new();
    
    for tx in transactions {
        // 1. Check block gas limit
        if gas_used + tx.gas_limit > block_context.gas_limit {
            break;  // Block full
        }
        
        // 2. Verify signature
        let sender = recover_sender(tx)?;
        
        // 3. Check nonce
        let expected_nonce = state.get_nonce(&sender) + state_changes.nonce_delta(&sender);
        if tx.nonce != expected_nonce {
            return Err(TxValidationError::InvalidNonce {
                expected: expected_nonce,
                got: tx.nonce,
            });
        }
        
        // 4. Check balance
        let balance = state.get_balance(&sender) - state_changes.balance_delta(&sender);
        let max_cost = tx.gas_limit * tx.gas_price + tx.value;
        if balance < max_cost {
            return Err(TxValidationError::InsufficientBalance {
                required: max_cost,
                available: balance,
            });
        }
        
        // 5. Track changes for next iteration
        state_changes.increment_nonce(&sender);
        state_changes.deduct_balance(&sender, max_cost);
        
        validated.push(ValidatedTransaction {
            transaction: tx.clone(),
            sender,
        });
        
        gas_used += tx.gas_limit;
    }
    
    Ok(validated)
}
```

---

### Step 6: Apply State Transition

**Inputs:**
- Validated transactions
- Validated compute receipts
- Current state

**Outputs:**
- New state root
- Execution receipts

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| Execution error | Invalid operation | Revert transaction |
| State root mismatch | Non-determinism | Reject block |
| Storage error | Disk failure | Halt and recover |

```rust
// State transition
fn apply_state_transition(
    state: &mut State,
    transactions: &[ValidatedTransaction],
    compute_receipts: &[ValidatedReceipt],
    block_context: &BlockContext,
) -> Result<StateTransitionResult, TransitionError> {
    let pre_state_root = state.root();
    let mut execution_receipts = Vec::new();
    let mut total_gas_used = 0u64;
    let mut total_compute_rewarded = 0u64;
    
    // 1. Apply transactions
    for validated_tx in transactions {
        let receipt = state.execute_transaction(
            &validated_tx.transaction,
            &validated_tx.sender,
            block_context,
        )?;
        
        total_gas_used += receipt.gas_used;
        execution_receipts.push(receipt);
    }
    
    // 2. Apply compute rewards
    for validated_receipt in compute_receipts {
        let reward = calculate_compute_reward(
            &validated_receipt.score,
            &state.compute_market(),
        );
        
        state.credit_balance(&validated_receipt.receipt.provider, reward)?;
        state.mark_receipt_claimed(
            &validated_receipt.receipt.task_id,
            &validated_receipt.receipt.provider,
        )?;
        
        total_compute_rewarded += reward;
    }
    
    // 3. Apply block rewards
    let block_reward = calculate_block_reward(block_context, total_gas_used);
    state.credit_balance(&block_context.proposer, block_reward)?;
    
    // 4. Compute new state root
    let post_state_root = state.compute_root();
    
    Ok(StateTransitionResult {
        pre_state_root,
        post_state_root,
        execution_receipts,
        total_gas_used,
        total_compute_rewarded,
        block_reward,
    })
}
```

---

### Step 7: Commit Block

**Inputs:**
- Validated block
- State transition result

**Outputs:**
- Persisted block

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| State root mismatch | Invalid execution | Reject block |
| Storage write fail | Disk error | Retry or halt |
| Concurrent write | Race condition | Acquire lock |

```rust
// Block commit
fn commit_block(
    storage: &mut Storage,
    block: &SignedBlock,
    transition_result: &StateTransitionResult,
) -> Result<CommitResult, CommitError> {
    // 1. Verify state root matches
    if block.header.state_root != transition_result.post_state_root {
        return Err(CommitError::StateRootMismatch {
            expected: block.header.state_root,
            computed: transition_result.post_state_root,
        });
    }
    
    // 2. Begin atomic write
    let mut batch = storage.begin_batch();
    
    // 3. Write block
    batch.put_block(&block)?;
    
    // 4. Write receipts
    for (i, receipt) in transition_result.execution_receipts.iter().enumerate() {
        batch.put_receipt(&block.header.hash(), i as u32, receipt)?;
    }
    
    // 5. Update chain head
    batch.put_head(&block.header.hash())?;
    
    // 6. Commit atomically
    batch.commit()?;
    
    Ok(CommitResult {
        block_hash: block.header.hash(),
        height: block.header.height,
        state_root: transition_result.post_state_root,
    })
}
```

---

### Step 8: Broadcast Commit

**Inputs:**
- Committed block
- Commit result

**Outputs:**
- Network acknowledgment

**Failure Modes:**
| Failure | Cause | Recovery |
|---------|-------|----------|
| Network error | Connectivity | Retry |
| Peer timeout | Slow response | Skip peer |

```rust
// Broadcast commit
async fn broadcast_commit(
    network: &Network,
    commit_result: &CommitResult,
) -> Result<BroadcastStats, BroadcastError> {
    let mut stats = BroadcastStats::default();
    
    let commit_msg = CommitMessage {
        block_hash: commit_result.block_hash,
        height: commit_result.height,
        state_root: commit_result.state_root,
    };
    
    // Broadcast to all peers
    for peer in network.connected_peers() {
        match network.send(peer, Message::BlockCommit(commit_msg.clone())).await {
            Ok(_) => stats.success += 1,
            Err(_) => stats.failed += 1,
        }
    }
    
    // Update local metrics
    metrics::BLOCKS_COMMITTED.inc();
    metrics::CHAIN_HEIGHT.set(commit_result.height as i64);
    
    Ok(stats)
}
```

---

## 5. Fork-Choice Rule

### Weight Calculation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     FORK-CHOICE WEIGHT                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Chain Weight = Σ(block_weight) for all blocks in chain                    │
│                                                                             │
│  Block Weight = stake_attestation_weight + pouw_work_score                 │
│                                                                             │
│  Where:                                                                     │
│  • stake_attestation_weight = Σ(attester_stake) for attestations           │
│  • pouw_work_score = Σ(compute_score) for verified receipts                │
│                                                                             │
│  Normalization:                                                             │
│  • PoS weight: 50% of total                                                │
│  • PoUW weight: 50% of total                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```rust
// Fork-choice weight calculation
struct ChainWeight {
    stake_weight: u128,
    pouw_weight: u128,
}

impl ChainWeight {
    fn total(&self) -> u128 {
        // 50% PoS, 50% PoUW
        (self.stake_weight * 50 + self.pouw_weight * 50) / 100
    }
    
    fn add_block(&mut self, block: &Block, attestations: &[Attestation]) {
        // Add stake weight from attestations
        for attestation in attestations {
            self.stake_weight += attestation.attester_stake;
        }
        
        // Add PoUW weight from compute receipts
        for receipt in &block.compute_receipts {
            self.pouw_weight += receipt.score.total_score() as u128;
        }
    }
}

fn calculate_chain_weight(chain: &[Block], state: &State) -> ChainWeight {
    let mut weight = ChainWeight::default();
    
    for block in chain {
        let attestations = state.get_attestations(&block.header.hash());
        weight.add_block(block, &attestations);
    }
    
    weight
}
```

### Checkpointing Logic

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     CHECKPOINTING                                           │
└─────────────────────────────────────────────────────────────────────────────┘

  Checkpoint Creation:
  ────────────────────
  
  At epoch boundary (every N slots):
  
  1. Validators vote on checkpoint candidate
  2. Candidate = highest justified block in epoch
  3. If >2/3 stake votes: checkpoint is justified
  4. If justified checkpoint has justified parent: both become finalized
  
  
  Justification Flow:
  ───────────────────
  
       Genesis         Epoch 1          Epoch 2          Epoch 3
          │               │                │                │
          ▼               ▼                ▼                ▼
       [CP0] ─────────▶ [CP1] ─────────▶ [CP2] ─────────▶ [CP3]
     FINALIZED        JUSTIFIED        JUSTIFIED        PENDING
                          │                │
                          └── >2/3 ────────┘
                              votes on CP2
                              finalizes CP1
```

```rust
// Checkpoint management
struct Checkpoint {
    epoch: Epoch,
    block_hash: Hash,
    block_height: u64,
    state_root: Hash,
    attestations: Vec<SignedAttestation>,
}

impl Checkpoint {
    fn is_justified(&self, validator_set: &ValidatorSet) -> bool {
        let attesting_stake: u128 = self.attestations
            .iter()
            .filter(|a| self.verify_attestation(a, validator_set))
            .map(|a| validator_set.get_stake(&a.validator))
            .sum();
        
        let total_stake = validator_set.total_stake();
        attesting_stake * 3 > total_stake * 2  // >2/3
    }
    
    fn finalize(
        &self,
        parent_checkpoint: &Checkpoint,
        validator_set: &ValidatorSet,
    ) -> bool {
        // Both must be justified
        self.is_justified(validator_set) 
            && parent_checkpoint.is_justified(validator_set)
            // And consecutive epochs
            && self.epoch == parent_checkpoint.epoch + 1
    }
}
```

### Fork Resolution Algorithm

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     FORK RESOLUTION                                         │
└─────────────────────────────────────────────────────────────────────────────┘

                           ┌─────────┐
                           │ Genesis │
                           └────┬────┘
                                │
                           ┌────▼────┐
                           │Block 1  │
                           └────┬────┘
                                │
                           ┌────▼────┐
                           │Block 2  │◄─── Last Finalized
                           └────┬────┘
                                │
              ┌─────────────────┼─────────────────┐
              │                 │                 │
         ┌────▼────┐       ┌────▼────┐       ┌────▼────┐
         │Block 3a │       │Block 3b │       │Block 3c │
         │ W: 150  │       │ W: 200  │       │ W: 180  │
         └────┬────┘       └────┬────┘       └─────────┘
              │                 │
         ┌────▼────┐       ┌────▼────┐
         │Block 4a │       │Block 4b │◄─── Canonical (highest weight)
         │ W: 100  │       │ W: 250  │
         └─────────┘       └─────────┘

  Resolution: Follow chain with highest cumulative weight from last finalized
```

```rust
// Fork resolution
fn resolve_fork(
    state: &State,
    candidates: &[Hash],
) -> Hash {
    let finalized = state.last_finalized_checkpoint();
    
    // Calculate weight for each candidate chain
    let mut best_chain = finalized.block_hash;
    let mut best_weight = ChainWeight::default();
    
    for candidate in candidates {
        // Build chain from finalized to candidate
        let chain = state.get_chain_between(&finalized.block_hash, candidate);
        
        if chain.is_none() {
            continue;  // Not descendant of finalized
        }
        
        let weight = calculate_chain_weight(&chain.unwrap(), state);
        
        if weight.total() > best_weight.total() {
            best_weight = weight;
            best_chain = *candidate;
        }
    }
    
    best_chain
}

fn reorg_to_chain(
    state: &mut State,
    new_head: &Hash,
) -> Result<ReorgResult, ReorgError> {
    let current_head = state.head();
    
    // Find common ancestor
    let ancestor = state.find_common_ancestor(&current_head, new_head)?;
    
    // Check reorg depth
    let reorg_depth = state.height() - state.get_height(&ancestor)?;
    if reorg_depth > MAX_REORG_DEPTH {
        return Err(ReorgError::TooDeep { depth: reorg_depth });
    }
    
    // Cannot reorg past finalized
    if state.get_height(&ancestor)? < state.last_finalized_height() {
        return Err(ReorgError::PastFinalized);
    }
    
    // Revert to ancestor
    state.revert_to(&ancestor)?;
    
    // Apply new chain
    let new_chain = state.get_chain_between(&ancestor, new_head)?;
    for block in new_chain {
        state.apply_block(&block)?;
    }
    
    Ok(ReorgResult {
        old_head: current_head,
        new_head: *new_head,
        reorg_depth,
    })
}
```

---

## 6. Security Model

### Threat Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     THREAT MODEL                                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Adversary Capabilities:                                                    │
│  • Control up to f validators (f < n/3 for safety)                         │
│  • Control network between honest nodes (bounded delay)                     │
│  • Observe all network traffic                                              │
│  • Unlimited compute resources                                              │
│  • Adaptive: can corrupt validators over time                               │
│                                                                             │
│  Adversary Limitations:                                                     │
│  • Cannot break cryptographic primitives                                    │
│  • Cannot control >2/3 stake instantaneously                               │
│  • Cannot prevent all message delivery indefinitely                         │
│                                                                             │
│  Security Properties:                                                       │
│  • Safety: No two honest nodes finalize conflicting blocks                 │
│  • Liveness: Transactions eventually included (if fee sufficient)          │
│  • Censorship Resistance: Cannot exclude specific transactions forever     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Honest Majority Assumptions

| Property | Assumption | Consequence if Violated |
|----------|------------|-------------------------|
| **Safety** | <1/3 Byzantine stake | Conflicting finalizations |
| **Liveness** | >2/3 online honest stake | Chain halts |
| **Censorship** | <1/2 colluding stake | Targeted tx exclusion |

### Sybil Resistance

```rust
// Sybil resistance mechanisms
struct SybilDefense {
    /// Minimum stake to become validator
    minimum_stake: u128,
    
    /// Stake required per additional validator slot
    stake_per_slot: u128,
    
    /// Maximum validators from single entity (based on stake cap)
    effective_stake_cap: u128,
    
    /// Activation delay (epochs)
    activation_delay: u64,
}

impl SybilDefense {
    fn cost_to_control_fraction(&self, fraction: f64, total_stake: u128) -> u128 {
        // Cost to acquire fraction of stake
        let required_stake = (total_stake as f64 * fraction) as u128;
        
        // Additional cost from activation delay
        let opportunity_cost = required_stake * self.activation_delay as u128 / 365;
        
        required_stake + opportunity_cost
    }
    
    fn max_validators_per_entity(&self, entity_stake: u128) -> u64 {
        // Limited by effective stake cap
        (entity_stake / self.effective_stake_cap).min(MAX_VALIDATORS_PER_ENTITY)
    }
}
```

### Worst-Case Attack Vectors

| Attack | Cost | Impact | Mitigation |
|--------|------|--------|------------|
| **51% Attack** | >50% stake | Double spend | Finality checkpoints |
| **Long-Range** | Old keys | Rewrite history | Weak subjectivity |
| **Grinding** | Compute | Bias randomness | VRF-based selection |
| **Eclipse** | Network control | Isolate nodes | Peer diversity |
| **DoS** | Bandwidth | Halt liveness | Rate limiting |

### Recovery Strategy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     RECOVERY PROCEDURES                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. SAFETY VIOLATION DETECTED                                               │
│     ─────────────────────────                                               │
│     • Alert all nodes                                                       │
│     • Halt block production                                                 │
│     • Identify conflicting finalizations                                    │
│     • Social consensus on canonical chain                                   │
│     • Hard fork if necessary                                                │
│                                                                             │
│  2. LIVENESS FAILURE                                                        │
│     ────────────────────                                                    │
│     • Identify offline validators                                           │
│     • Reduce finality threshold temporarily                                 │
│     • Activate backup validators                                            │
│     • Slash persistently offline validators                                 │
│                                                                             │
│  3. NETWORK PARTITION                                                       │
│     ───────────────────                                                     │
│     • Detect partition via missing attestations                             │
│     • Smaller partition halts (no finality)                                 │
│     • Larger partition continues                                            │
│     • Reconcile on partition heal                                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Validation Test Vectors

*Status: Placeholder for future implementation*

### Header Validation Vectors

```rust
// Test vector structure (placeholder)
struct HeaderValidationVector {
    name: &'static str,
    input: HeaderInput,
    expected: ValidationResult,
}

// Example vectors (to be expanded)
const HEADER_VECTORS: &[HeaderValidationVector] = &[
    HeaderValidationVector {
        name: "valid_header",
        input: HeaderInput { /* ... */ },
        expected: ValidationResult::Ok,
    },
    HeaderValidationVector {
        name: "invalid_signature",
        input: HeaderInput { /* ... */ },
        expected: ValidationResult::Err(HeaderValidationError::InvalidSignature),
    },
    // ... more vectors
];
```

### PoUW Validation Vectors

```rust
// PoUW test vector (placeholder)
struct PouwValidationVector {
    name: &'static str,
    task: ComputeTask,
    receipt: ComputeReceipt,
    expected: ValidationResult,
}
```

### Fork-Choice Vectors

```rust
// Fork-choice test vector (placeholder)
struct ForkChoiceVector {
    name: &'static str,
    chains: Vec<Chain>,
    attestations: Vec<Attestation>,
    expected_head: Hash,
}
```

---

## 8. Future Extensions

### ZK-Verified Compute

Zero-knowledge proofs for compute verification:

| Feature | Benefit | Status |
|---------|---------|--------|
| SNARK proofs | Succinct verification | Research |
| STARK proofs | No trusted setup | Research |
| Recursive proofs | Proof aggregation | Planned |

### Delegated Compute

Allow stakers to delegate compute tasks:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     DELEGATED COMPUTE (FUTURE)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Staker ──delegate──▶ Compute Provider ──execute──▶ Result                 │
│     │                        │                          │                   │
│     │                        │                          │                   │
│     └────────reward split────┴──────────────────────────┘                   │
│                                                                             │
│  Benefits:                                                                  │
│  • Stakers without hardware can participate                                │
│  • Compute providers get guaranteed work                                   │
│  • Network gets more compute capacity                                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### QUIC Transport

Upgrade networking layer:

| Feature | Improvement |
|---------|-------------|
| Multiplexing | Multiple streams per connection |
| 0-RTT | Faster reconnection |
| Migration | Connection mobility |

### Additional Roadmap

- [ ] Sharded consensus
- [ ] Cross-shard transactions
- [ ] BLS signature aggregation
- [ ] Data availability sampling
- [ ] Proposer-builder separation

---

## Summary

Mbongo Chain's consensus validation provides a robust framework for hybrid PoS + PoUW consensus. The eight-step block validation pipeline ensures correctness, while the fork-choice rule and checkpointing provide finality guarantees. The security model assumes an honest supermajority with well-defined recovery procedures for edge cases.

For implementation details, see [Runtime Architecture](runtime_architecture.md).

For network-level details, see [Networking Overview](networking_overview.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

