# Mbongo Chain — Block Validation Pipeline

This document provides a comprehensive specification of the block validation pipeline in Mbongo Chain, covering each phase from proposal through commit with detailed validation rules, pseudocode, and security considerations.

---

## 1. Overview

### Block Lifecycle

The complete block lifecycle proceeds through six distinct phases:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     BLOCK VALIDATION PIPELINE                               │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │                                                                          │
  │   PROPOSAL        GOSSIP         VALIDATION         EXECUTION    COMMIT  │
  │      │              │                │                  │           │    │
  │      ▼              ▼                ▼                  ▼           ▼    │
  │   ┌─────┐       ┌─────┐       ┌───────────┐       ┌─────────┐  ┌──────┐ │
  │   │Build│──────▶│Broad│──────▶│  Header   │──────▶│ Execute │─▶│Commit│ │
  │   │Block│       │cast │       │  PoUW     │       │  Txs    │  │State │ │
  │   └─────┘       └─────┘       │  Txs      │       │         │  └──────┘ │
  │                               └───────────┘       └─────────┘           │
  │                                                                          │
  └──────────────────────────────────────────────────────────────────────────┘

  Time ──────────────────────────────────────────────────────────────────────▶
        ~50ms          ~200ms          ~100ms           ~500ms        ~50ms
```

### System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     VALIDATION SYSTEM ARCHITECTURE                          │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────┐
                              │   BLOCK INPUT   │
                              │   (from peer)   │
                              └────────┬────────┘
                                       │
                                       ▼
              ┌────────────────────────────────────────────────┐
              │              VALIDATION ORCHESTRATOR           │
              │                                                │
              │  ┌──────────────────────────────────────────┐  │
              │  │           PARALLEL VALIDATION            │  │
              │  │                                          │  │
              │  │  ┌────────────┐  ┌────────────┐         │  │
              │  │  │  Header    │  │   PoUW     │         │  │
              │  │  │  Validator │  │  Verifier  │         │  │
              │  │  └─────┬──────┘  └─────┬──────┘         │  │
              │  │        │               │                │  │
              │  │        └───────┬───────┘                │  │
              │  │                │                        │  │
              │  └────────────────┼────────────────────────┘  │
              │                   │                           │
              │  ┌────────────────▼────────────────────────┐  │
              │  │         SEQUENTIAL VALIDATION           │  │
              │  │                                          │  │
              │  │  ┌────────────┐  ┌────────────┐         │  │
              │  │  │Transaction │  │   State    │         │  │
              │  │  │ Validator  │──▶│  Executor  │         │  │
              │  │  └────────────┘  └─────┬──────┘         │  │
              │  │                        │                │  │
              │  └────────────────────────┼────────────────┘  │
              │                           │                   │
              └───────────────────────────┼───────────────────┘
                                          │
                                          ▼
                              ┌─────────────────┐
                              │  STATE COMMIT   │
                              │    (storage)    │
                              └─────────────────┘
```

### Design Principles

| Principle | Implementation |
|-----------|----------------|
| **Determinism** | Identical inputs always produce identical outputs |
| **Parallelism** | Header and PoUW validation run concurrently |
| **Fail-Fast** | Early rejection of invalid blocks |
| **Auditability** | Complete validation trace for debugging |

---

## 2. Header Validation

### Header Structure

```rust
/// Block header structure
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BlockHeader {
    /// Hash of the parent block
    pub parent_hash: Hash,
    
    /// Block height (parent.height + 1)
    pub height: u64,
    
    /// Slot number in consensus
    pub slot: Slot,
    
    /// Unix timestamp (seconds)
    pub timestamp: u64,
    
    /// Merkle root of post-execution state
    pub state_root: Hash,
    
    /// Merkle root of transaction receipts
    pub receipts_root: Hash,
    
    /// Merkle root of transactions
    pub transactions_root: Hash,
    
    /// Merkle root of PoUW compute receipts
    pub pouw_receipts_root: Hash,
    
    /// Accumulated PoUW work in this block
    pub pouw_work: u64,
    
    /// Block proposer (validator public key)
    pub proposer: PublicKey,
    
    /// Proposer signature over header hash
    pub signature: Signature,
    
    /// Extra data (max 32 bytes)
    pub extra_data: [u8; 32],
}

impl BlockHeader {
    /// Compute header hash (excludes signature)
    pub fn hash(&self) -> Hash {
        let mut hasher = Blake3::new();
        hasher.update(&self.parent_hash);
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.slot.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(&self.state_root);
        hasher.update(&self.receipts_root);
        hasher.update(&self.transactions_root);
        hasher.update(&self.pouw_receipts_root);
        hasher.update(&self.pouw_work.to_le_bytes());
        hasher.update(&self.proposer.as_bytes());
        hasher.update(&self.extra_data);
        hasher.finalize()
    }
    
    /// Compute hash for signing (same as hash())
    pub fn signing_hash(&self) -> Hash {
        self.hash()
    }
}
```

### Deterministic Validation Rules

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     HEADER VALIDATION RULES                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Rule 1: Parent Validity                                                    │
│  ─────────────────────────                                                  │
│  • parent_hash MUST reference known, valid block                           │
│  • parent_hash MUST NOT reference block beyond finalized checkpoint        │
│                                                                             │
│  Rule 2: Height Continuity                                                  │
│  ─────────────────────────                                                  │
│  • height MUST equal parent.height + 1                                     │
│  • No gaps or duplicates allowed                                           │
│                                                                             │
│  Rule 3: Slot Progression                                                   │
│  ────────────────────────                                                   │
│  • slot MUST be > parent.slot                                              │
│  • slot MUST be <= current_slot + MAX_FUTURE_SLOTS                         │
│                                                                             │
│  Rule 4: Timestamp Bounds                                                   │
│  ────────────────────────                                                   │
│  • timestamp MUST be > parent.timestamp                                    │
│  • timestamp MUST be <= now() + MAX_FUTURE_TIME (15 seconds)               │
│  • timestamp MUST align with slot (within tolerance)                       │
│                                                                             │
│  Rule 5: Proposer Authorization                                             │
│  ──────────────────────────────                                             │
│  • proposer MUST be assigned to slot via VRF                               │
│  • proposer MUST be active validator (not slashed/exited)                  │
│                                                                             │
│  Rule 6: Signature Validity                                                 │
│  ─────────────────────────                                                  │
│  • signature MUST verify against proposer public key                       │
│  • signature MUST be over header.signing_hash()                            │
│                                                                             │
│  Rule 7: Merkle Root Format                                                 │
│  ───────────────────────────                                                │
│  • All roots MUST be 32-byte Blake3 hashes                                 │
│  • Empty tree root = hash(0x00)                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Header Validation Implementation

```rust
/// Header validation result
pub enum HeaderValidationResult {
    Valid,
    Invalid(HeaderValidationError),
}

/// Header validation errors
#[derive(Debug, Clone)]
pub enum HeaderValidationError {
    UnknownParent { hash: Hash },
    InvalidHeight { expected: u64, got: u64 },
    InvalidSlot { reason: &'static str },
    InvalidTimestamp { reason: &'static str },
    InvalidProposer { expected: PublicKey, got: PublicKey },
    InvalidSignature,
    ProposerNotActive { validator: PublicKey },
    ParentBeyondFinalized,
}

/// Validate block header
pub fn validate_header(
    header: &BlockHeader,
    chain_state: &ChainState,
    validator_set: &ValidatorSet,
    current_time: u64,
) -> HeaderValidationResult {
    // Rule 1: Parent validity
    let parent = match chain_state.get_header(&header.parent_hash) {
        Some(p) => p,
        None => return HeaderValidationResult::Invalid(
            HeaderValidationError::UnknownParent { hash: header.parent_hash }
        ),
    };
    
    // Check parent is not beyond finalized
    if parent.height < chain_state.finalized_height() {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::ParentBeyondFinalized
        );
    }
    
    // Rule 2: Height continuity
    let expected_height = parent.height + 1;
    if header.height != expected_height {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidHeight {
                expected: expected_height,
                got: header.height,
            }
        );
    }
    
    // Rule 3: Slot progression
    if header.slot <= parent.slot {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidSlot {
                reason: "slot must be greater than parent slot"
            }
        );
    }
    
    let max_future_slot = chain_state.current_slot() + MAX_FUTURE_SLOTS;
    if header.slot > max_future_slot {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidSlot {
                reason: "slot too far in future"
            }
        );
    }
    
    // Rule 4: Timestamp bounds
    if header.timestamp <= parent.timestamp {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidTimestamp {
                reason: "timestamp must be greater than parent"
            }
        );
    }
    
    let max_future_time = current_time + MAX_FUTURE_TIME_SECS;
    if header.timestamp > max_future_time {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidTimestamp {
                reason: "timestamp too far in future"
            }
        );
    }
    
    // Rule 5: Proposer authorization
    let expected_proposer = select_proposer(
        header.slot,
        chain_state.epoch_randomness(),
        validator_set,
    );
    
    if header.proposer != expected_proposer {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidProposer {
                expected: expected_proposer,
                got: header.proposer,
            }
        );
    }
    
    // Check proposer is active
    if !validator_set.is_active(&header.proposer) {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::ProposerNotActive {
                validator: header.proposer,
            }
        );
    }
    
    // Rule 6: Signature validity
    let signing_hash = header.signing_hash();
    if !verify_signature(&header.proposer, &signing_hash, &header.signature) {
        return HeaderValidationResult::Invalid(
            HeaderValidationError::InvalidSignature
        );
    }
    
    HeaderValidationResult::Valid
}
```

### Header Validation Failure Modes

| Failure | Cause | Detection | Action |
|---------|-------|-----------|--------|
| Unknown parent | Missing block | DB lookup | Request from peers |
| Invalid height | Fork or corruption | Arithmetic check | Reject block |
| Future slot | Clock skew | Slot comparison | Wait or reject |
| Past timestamp | Invalid block | Timestamp check | Reject block |
| Wrong proposer | Consensus violation | VRF verification | Reject + penalize |
| Invalid signature | Tampered block | Crypto verify | Reject + ban peer |

---

## 3. PoUW Receipt Verification

### Verification Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     PoUW RECEIPT VERIFICATION FLOW                          │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────┐
  │  Compute        │
  │  Receipt        │
  │  (from block)   │
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  1. SIGNATURE   │────▶│  Verify provider signed receipt                 │
  │     CHECK       │     │  • recover_signer(receipt.hash, signature)      │
  │                 │     │  • signer == receipt.provider                   │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  2. TASK        │────▶│  Verify task exists and is claimable            │
  │     LOOKUP      │     │  • task_registry.get(task_id)                   │
  │                 │     │  • task.status == Active                        │
  └────────┬────────┘     │  • task.deadline > current_time                 │
           │              └─────────────────────────────────────────────────┘
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  3. PROVIDER    │────▶│  Verify provider is eligible                    │
  │     CHECK       │     │  • provider_registry.get(provider)              │
  │                 │     │  • provider.status == Active                    │
  └────────┬────────┘     │  • provider.stake >= minimum                    │
           │              └─────────────────────────────────────────────────┘
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  4. DUPLICATE   │────▶│  Check receipt not already claimed              │
  │     CHECK       │     │  • !state.is_claimed(task_id, provider)         │
  │                 │     │                                                 │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  5. PROOF       │────▶│  Verify execution correctness (placeholder)     │
  │     VERIFY      │     │  • verify_proof(task, receipt.proof)            │
  │                 │     │  • output_commitment matches                    │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  6. SCORE       │────▶│  Calculate useful work score                    │
  │     COMPUTE     │     │  • base_units * quality * demand                │
  │                 │     │  • update block pouw_work accumulator           │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐
  │  VERIFIED       │
  │  RECEIPT        │
  └─────────────────┘
```

### Receipt Structure

```rust
/// PoUW compute receipt
#[derive(Clone, Debug)]
pub struct ComputeReceipt {
    /// Unique task identifier
    pub task_id: Hash,
    
    /// Compute provider address
    pub provider: Address,
    
    /// Hash of task input data
    pub input_commitment: Hash,
    
    /// Hash of computation output
    pub output_commitment: Hash,
    
    /// Compute units consumed
    pub compute_units: u64,
    
    /// Execution proof
    pub proof: ExecutionProof,
    
    /// Completion timestamp
    pub completed_at: u64,
    
    /// Provider signature
    pub signature: Signature,
}

/// Execution proof variants
#[derive(Clone, Debug)]
pub enum ExecutionProof {
    /// Deterministic replay proof
    Replay {
        /// Execution trace hash
        trace_hash: Hash,
        /// Key checkpoints for verification
        checkpoints: Vec<StateCheckpoint>,
    },
    
    /// Zero-knowledge proof (future)
    Snark {
        /// Proof bytes
        proof: Vec<u8>,
        /// Public inputs
        public_inputs: Vec<u8>,
    },
    
    /// Trusted execution environment attestation
    Tee {
        /// TEE attestation report
        attestation: Vec<u8>,
        /// Enclave measurement
        measurement: Hash,
    },
    
    /// Placeholder for development
    Placeholder {
        /// Self-declared correctness (NOT for production)
        declared_correct: bool,
    },
}
```

### Receipt Verification Implementation

```rust
/// Receipt verification result
pub struct VerifiedReceipt {
    pub receipt: ComputeReceipt,
    pub score: ComputeScore,
    pub reward: u128,
}

/// Verify a compute receipt
pub fn verify_compute_receipt(
    receipt: &ComputeReceipt,
    task_registry: &TaskRegistry,
    provider_registry: &ProviderRegistry,
    state: &State,
    block_time: u64,
) -> Result<VerifiedReceipt, ReceiptVerificationError> {
    // Step 1: Signature verification
    let receipt_hash = receipt.compute_hash();
    let recovered_signer = recover_signer(&receipt_hash, &receipt.signature)?;
    
    if recovered_signer != receipt.provider {
        return Err(ReceiptVerificationError::InvalidSignature {
            expected: receipt.provider,
            recovered: recovered_signer,
        });
    }
    
    // Step 2: Task lookup
    let task = task_registry.get(&receipt.task_id)
        .ok_or(ReceiptVerificationError::UnknownTask {
            task_id: receipt.task_id,
        })?;
    
    if task.status != TaskStatus::Active {
        return Err(ReceiptVerificationError::TaskNotActive {
            status: task.status,
        });
    }
    
    if task.deadline < block_time {
        return Err(ReceiptVerificationError::TaskExpired {
            deadline: task.deadline,
            current: block_time,
        });
    }
    
    // Step 3: Provider eligibility
    let provider = provider_registry.get(&receipt.provider)
        .ok_or(ReceiptVerificationError::UnknownProvider)?;
    
    if provider.status != ProviderStatus::Active {
        return Err(ReceiptVerificationError::ProviderNotActive);
    }
    
    if provider.stake < MIN_PROVIDER_STAKE {
        return Err(ReceiptVerificationError::InsufficientStake {
            required: MIN_PROVIDER_STAKE,
            actual: provider.stake,
        });
    }
    
    // Step 4: Duplicate check
    if state.is_receipt_claimed(&receipt.task_id, &receipt.provider) {
        return Err(ReceiptVerificationError::AlreadyClaimed);
    }
    
    // Step 5: Proof verification
    verify_execution_proof(&task, receipt)?;
    
    // Step 6: Score computation
    let score = compute_work_score(receipt, &task, state);
    let reward = calculate_reward(&score, &state.compute_market());
    
    Ok(VerifiedReceipt {
        receipt: receipt.clone(),
        score,
        reward,
    })
}

/// Verify execution proof (placeholder implementation)
fn verify_execution_proof(
    task: &ComputeTask,
    receipt: &ComputeReceipt,
) -> Result<(), ReceiptVerificationError> {
    // Verify input commitment matches task
    if receipt.input_commitment != task.input_commitment {
        return Err(ReceiptVerificationError::InputMismatch);
    }
    
    match &receipt.proof {
        ExecutionProof::Replay { trace_hash, checkpoints } => {
            // Verify replay proof by checking trace consistency
            verify_replay_proof(task, trace_hash, checkpoints)
        }
        
        ExecutionProof::Snark { proof, public_inputs } => {
            // Verify SNARK proof (future implementation)
            verify_snark_proof(task, proof, public_inputs)
        }
        
        ExecutionProof::Tee { attestation, measurement } => {
            // Verify TEE attestation
            verify_tee_attestation(task, attestation, measurement)
        }
        
        ExecutionProof::Placeholder { declared_correct } => {
            // Development only - accept if declared correct
            if *declared_correct {
                Ok(())
            } else {
                Err(ReceiptVerificationError::ProofFailed)
            }
        }
    }
}

/// Compute useful work score
fn compute_work_score(
    receipt: &ComputeReceipt,
    task: &ComputeTask,
    state: &State,
) -> ComputeScore {
    let base_units = receipt.compute_units;
    
    // Quality multiplier based on accuracy and latency
    let quality = calculate_quality_multiplier(receipt, task);
    
    // Demand factor from market conditions
    let demand = state.compute_market().demand_factor().min(2.0);
    
    ComputeScore {
        base_units,
        quality_multiplier: quality,
        demand_factor: demand,
        total: (base_units as f64 * quality * demand) as u64,
    }
}
```

---

## 4. Transaction Validation

### Validation Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     TRANSACTION VALIDATION                                  │
└─────────────────────────────────────────────────────────────────────────────┘

  For each transaction in block.transactions:

  ┌─────────────────┐
  │  Raw Transaction│
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  1. FORMAT      │────▶│  Decode and validate structure                  │
  │     VALIDATION  │     │  • RLP/Bincode decode succeeds                  │
  │                 │     │  • All required fields present                  │
  └────────┬────────┘     │  • Field sizes within limits                    │
           │              └─────────────────────────────────────────────────┘
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  2. SIGNATURE   │────▶│  Verify and recover sender                      │
  │     VERIFICATION│     │  • ECDSA signature valid                        │
  │                 │     │  • Sender = ecrecover(tx_hash, sig)             │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  3. REPLAY      │────▶│  Prevent replay attacks                         │
  │     PROTECTION  │     │  • tx.nonce == account.nonce                    │
  │                 │     │  • tx.chain_id == current_chain_id              │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  4. GAS         │────▶│  Validate gas parameters                        │
  │     ACCOUNTING  │     │  • gas_limit >= intrinsic_gas                   │
  │                 │     │  • gas_price >= min_gas_price                   │
  └────────┬────────┘     │  • cumulative_gas <= block_gas_limit            │
           │              └─────────────────────────────────────────────────┘
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  5. BALANCE     │────▶│  Verify sender can pay                          │
  │     CHECK       │     │  • balance >= gas_limit * gas_price + value     │
  │                 │     │  • Account for pending txs in block             │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐
  │  VALIDATED TX   │
  └─────────────────┘
```

### Replay Protection

```rust
/// Replay protection validation
pub fn validate_replay_protection(
    tx: &Transaction,
    account_state: &AccountState,
    chain_config: &ChainConfig,
) -> Result<(), ReplayProtectionError> {
    // Check chain ID (EIP-155 style)
    if let Some(chain_id) = tx.chain_id {
        if chain_id != chain_config.chain_id {
            return Err(ReplayProtectionError::WrongChainId {
                expected: chain_config.chain_id,
                got: chain_id,
            });
        }
    } else if chain_config.require_chain_id {
        return Err(ReplayProtectionError::MissingChainId);
    }
    
    // Check nonce
    if tx.nonce != account_state.nonce {
        return Err(ReplayProtectionError::InvalidNonce {
            expected: account_state.nonce,
            got: tx.nonce,
        });
    }
    
    Ok(())
}
```

### Signature Verification

```rust
/// Signature verification and sender recovery
pub fn verify_and_recover_sender(
    tx: &Transaction,
) -> Result<Address, SignatureError> {
    // Compute transaction hash for signing
    let tx_hash = tx.signing_hash();
    
    // Verify signature format
    if !is_valid_signature_format(&tx.signature) {
        return Err(SignatureError::MalformedSignature);
    }
    
    // Recover public key from signature
    let public_key = ecrecover(&tx_hash, &tx.signature)
        .map_err(|_| SignatureError::RecoveryFailed)?;
    
    // Derive address from public key
    let sender = public_key_to_address(&public_key);
    
    // Verify recovered address matches (if specified)
    if let Some(from) = tx.from {
        if from != sender {
            return Err(SignatureError::SenderMismatch {
                declared: from,
                recovered: sender,
            });
        }
    }
    
    Ok(sender)
}
```

### Gas Accounting

```rust
/// Gas accounting for transaction validation
pub struct GasAccounting {
    /// Gas used so far in block
    pub cumulative_gas: u64,
    /// Block gas limit
    pub block_gas_limit: u64,
    /// Minimum gas price
    pub min_gas_price: u128,
}

impl GasAccounting {
    /// Validate and account for transaction gas
    pub fn validate_transaction(
        &mut self,
        tx: &Transaction,
    ) -> Result<(), GasAccountingError> {
        // Calculate intrinsic gas
        let intrinsic_gas = calculate_intrinsic_gas(tx);
        
        // Gas limit must cover intrinsic cost
        if tx.gas_limit < intrinsic_gas {
            return Err(GasAccountingError::GasLimitBelowIntrinsic {
                limit: tx.gas_limit,
                intrinsic: intrinsic_gas,
            });
        }
        
        // Gas price must meet minimum
        if tx.gas_price < self.min_gas_price {
            return Err(GasAccountingError::GasPriceTooLow {
                price: tx.gas_price,
                minimum: self.min_gas_price,
            });
        }
        
        // Check block gas limit
        let new_cumulative = self.cumulative_gas.saturating_add(tx.gas_limit);
        if new_cumulative > self.block_gas_limit {
            return Err(GasAccountingError::BlockGasLimitExceeded {
                cumulative: new_cumulative,
                limit: self.block_gas_limit,
            });
        }
        
        // Update accounting (actual gas used updated after execution)
        self.cumulative_gas = new_cumulative;
        
        Ok(())
    }
}

/// Calculate intrinsic gas for transaction
fn calculate_intrinsic_gas(tx: &Transaction) -> u64 {
    let mut gas = TX_BASE_GAS;  // 21,000
    
    // Data costs
    for byte in &tx.data {
        if *byte == 0 {
            gas += TX_DATA_ZERO_GAS;      // 4
        } else {
            gas += TX_DATA_NONZERO_GAS;   // 16
        }
    }
    
    // Contract creation
    if tx.to.is_none() {
        gas += TX_CREATE_GAS;  // 32,000
    }
    
    // Access list (EIP-2930)
    if let Some(access_list) = &tx.access_list {
        for entry in access_list {
            gas += TX_ACCESS_LIST_ADDRESS_GAS;  // 2,400
            gas += entry.storage_keys.len() as u64 * TX_ACCESS_LIST_STORAGE_GAS;  // 1,900
        }
    }
    
    gas
}
```

### Merkle Root Computation

```rust
/// Compute transactions merkle root
pub fn compute_transactions_root(transactions: &[Transaction]) -> Hash {
    if transactions.is_empty() {
        return EMPTY_TRIE_ROOT;
    }
    
    // Build merkle tree from transaction hashes
    let leaves: Vec<Hash> = transactions
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            // Key is RLP-encoded index
            let key = rlp_encode_u64(i as u64);
            // Value is RLP-encoded transaction
            let value = tx.rlp_encode();
            hash_leaf(&key, &value)
        })
        .collect();
    
    compute_merkle_root(&leaves)
}

/// Compute receipts merkle root
pub fn compute_receipts_root(receipts: &[TransactionReceipt]) -> Hash {
    if receipts.is_empty() {
        return EMPTY_TRIE_ROOT;
    }
    
    let leaves: Vec<Hash> = receipts
        .iter()
        .enumerate()
        .map(|(i, receipt)| {
            let key = rlp_encode_u64(i as u64);
            let value = receipt.rlp_encode();
            hash_leaf(&key, &value)
        })
        .collect();
    
    compute_merkle_root(&leaves)
}
```

### Transaction Validation Failure Modes

| Failure | Cause | Detection | Severity |
|---------|-------|-----------|----------|
| Decode error | Malformed bytes | RLP/Bincode | Reject tx |
| Invalid signature | Wrong key or tampered | ECDSA verify | Reject tx |
| Wrong chain ID | Cross-chain replay | Chain ID check | Reject tx |
| Invalid nonce | Replay or gap | State lookup | Reject tx |
| Insufficient balance | Can't pay fees | Balance check | Reject tx |
| Gas limit too low | Under intrinsic | Calculation | Reject tx |
| Block gas exceeded | Block full | Accumulator | Stop processing |

---

## 5. State Execution Phase

### Deterministic Execution Rules

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     DETERMINISTIC EXECUTION RULES                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  MUST be deterministic:                                                     │
│  ─────────────────────                                                      │
│  • Same transaction + same state = same result (always)                    │
│  • Execution order matches transaction order in block                      │
│  • Gas consumption identical across all nodes                              │
│  • State changes identical across all nodes                                │
│                                                                             │
│  FORBIDDEN operations:                                                      │
│  ─────────────────────                                                      │
│  • System time (use block.timestamp only)                                  │
│  • Random number generation (use VRF/block hash only)                      │
│  • Floating point arithmetic                                               │
│  • External network calls                                                  │
│  • File system access                                                      │
│  • Thread spawning                                                         │
│                                                                             │
│  ALLOWED sources of "randomness":                                          │
│  ────────────────────────────────                                          │
│  • block.hash (previous block)                                             │
│  • VRF output (verifiable)                                                 │
│  • Combination of block data and tx data                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### VM Architecture (Placeholder)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     VM ARCHITECTURE (FUTURE)                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Current: Native Rust Execution                                             │
│  ────────────────────────────────                                           │
│  • Direct function calls for transfers                                      │
│  • No smart contracts                                                       │
│  • Deterministic by Rust guarantees                                         │
│                                                                             │
│  Planned: WASM VM                                                           │
│  ───────────────────                                                        │
│  • WebAssembly bytecode execution                                          │
│  • Wasmer/Wasmtime runtime                                                 │
│  • Gas metering via fuel                                                   │
│  • Memory isolation                                                        │
│                                                                             │
│  Research: RISC-V VM                                                        │
│  ───────────────────                                                        │
│  • RISC-V bytecode for ZK compatibility                                    │
│  • Enables ZK-rollup execution proofs                                      │
│  • Similar gas model to WASM                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### State Trie Operations

```rust
/// State trie interface
pub trait StateTrie {
    /// Get value at key
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    
    /// Set value at key
    fn set(&mut self, key: &[u8], value: Vec<u8>);
    
    /// Delete key
    fn delete(&mut self, key: &[u8]);
    
    /// Compute root hash
    fn root(&self) -> Hash;
    
    /// Create checkpoint for rollback
    fn checkpoint(&mut self) -> CheckpointId;
    
    /// Rollback to checkpoint
    fn rollback(&mut self, checkpoint: CheckpointId);
    
    /// Commit checkpoint (discard rollback option)
    fn commit(&mut self, checkpoint: CheckpointId);
}

/// Account state in trie
#[derive(Clone, Debug, Default)]
pub struct AccountState {
    pub nonce: u64,
    pub balance: u128,
    pub storage_root: Hash,
    pub code_hash: Hash,
}

impl AccountState {
    /// Encode for trie storage
    pub fn encode(&self) -> Vec<u8> {
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&self.nonce.to_le_bytes());
        encoded.extend_from_slice(&self.balance.to_le_bytes());
        encoded.extend_from_slice(&self.storage_root);
        encoded.extend_from_slice(&self.code_hash);
        encoded
    }
    
    /// Decode from trie storage
    pub fn decode(data: &[u8]) -> Result<Self, DecodeError> {
        if data.len() != 8 + 16 + 32 + 32 {
            return Err(DecodeError::InvalidLength);
        }
        
        Ok(Self {
            nonce: u64::from_le_bytes(data[0..8].try_into().unwrap()),
            balance: u128::from_le_bytes(data[8..24].try_into().unwrap()),
            storage_root: data[24..56].try_into().unwrap(),
            code_hash: data[56..88].try_into().unwrap(),
        })
    }
}
```

### Storage Access Rules

```rust
/// Storage access tracking for gas
pub struct StorageAccess {
    /// Cold access set (first access in tx)
    cold_accessed: HashSet<(Address, Hash)>,
    /// Warm access set (subsequent access)
    warm_accessed: HashSet<(Address, Hash)>,
    /// Original values for refund calculation
    original_values: HashMap<(Address, Hash), Hash>,
}

impl StorageAccess {
    /// Record storage read
    pub fn read(&mut self, address: Address, slot: Hash) -> AccessCost {
        let key = (address, slot);
        
        if self.warm_accessed.contains(&key) {
            AccessCost::Warm(WARM_STORAGE_READ_GAS)  // 100
        } else {
            self.cold_accessed.insert(key);
            self.warm_accessed.insert(key);
            AccessCost::Cold(COLD_STORAGE_READ_GAS)  // 2,100
        }
    }
    
    /// Record storage write
    pub fn write(
        &mut self,
        address: Address,
        slot: Hash,
        old_value: Hash,
        new_value: Hash,
    ) -> WriteCost {
        let key = (address, slot);
        
        // Store original value for refund
        self.original_values.entry(key).or_insert(old_value);
        
        let base_cost = if self.warm_accessed.contains(&key) {
            WARM_STORAGE_READ_GAS  // 100
        } else {
            self.cold_accessed.insert(key);
            self.warm_accessed.insert(key);
            COLD_STORAGE_READ_GAS  // 2,100
        };
        
        // Calculate write cost based on state change
        let write_cost = if old_value == new_value {
            0  // No change, no cost beyond read
        } else if old_value == Hash::default() {
            STORAGE_SET_GAS  // 20,000 (zero -> nonzero)
        } else if new_value == Hash::default() {
            STORAGE_RESET_GAS  // 2,900 (nonzero -> zero, refund later)
        } else {
            STORAGE_RESET_GAS  // 2,900 (nonzero -> nonzero)
        };
        
        WriteCost {
            base: base_cost,
            write: write_cost,
        }
    }
}
```

### Transaction Application

```rust
/// Apply a single transaction to state
pub fn apply_transaction(
    state: &mut StateTrie,
    tx: &ValidatedTransaction,
    block_context: &BlockContext,
) -> Result<TransactionReceipt, ExecutionError> {
    // Create checkpoint for potential rollback
    let checkpoint = state.checkpoint();
    
    // Get sender account
    let sender = tx.sender;
    let mut sender_account = state.get_account(&sender)?;
    
    // Deduct maximum gas cost upfront
    let max_gas_cost = tx.gas_limit as u128 * tx.gas_price;
    if sender_account.balance < max_gas_cost + tx.value {
        state.rollback(checkpoint);
        return Err(ExecutionError::InsufficientBalance);
    }
    
    sender_account.balance -= max_gas_cost;
    sender_account.nonce += 1;
    state.set_account(&sender, &sender_account)?;
    
    // Execute transaction
    let execution_result = match tx.to {
        Some(recipient) => {
            // Transfer or contract call
            execute_call(state, &sender, &recipient, tx.value, &tx.data, tx.gas_limit)
        }
        None => {
            // Contract creation
            execute_create(state, &sender, tx.value, &tx.data, tx.gas_limit)
        }
    };
    
    // Handle execution result
    let (status, gas_used, logs, output) = match execution_result {
        Ok(result) => {
            state.commit(checkpoint);
            (
                ExecutionStatus::Success,
                result.gas_used,
                result.logs,
                result.output,
            )
        }
        Err(ExecutionError::Revert { gas_used, output }) => {
            state.rollback(checkpoint);
            // Re-apply nonce increment and gas deduction
            sender_account.nonce += 1;
            sender_account.balance -= gas_used as u128 * tx.gas_price;
            state.set_account(&sender, &sender_account)?;
            
            (
                ExecutionStatus::Revert,
                gas_used,
                vec![],
                output,
            )
        }
        Err(e) => {
            state.rollback(checkpoint);
            return Err(e);
        }
    };
    
    // Refund unused gas
    let gas_refund = (tx.gas_limit - gas_used) as u128 * tx.gas_price;
    sender_account.balance += gas_refund;
    state.set_account(&sender, &sender_account)?;
    
    // Pay block producer
    let gas_payment = gas_used as u128 * tx.gas_price;
    let mut producer_account = state.get_account(&block_context.proposer)?;
    producer_account.balance += gas_payment;
    state.set_account(&block_context.proposer, &producer_account)?;
    
    Ok(TransactionReceipt {
        tx_hash: tx.hash(),
        status,
        gas_used,
        cumulative_gas: block_context.cumulative_gas + gas_used,
        logs,
        output,
        state_root: state.root(),
    })
}
```

---

## 6. Commit Phase

### Commit Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     BLOCK COMMIT PHASE                                      │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────┐
  │  Executed Block │
  │  + State Root   │
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  1. STATE ROOT  │────▶│  Compare computed vs header state_root          │
  │     COMPARISON  │     │  • computed_root == header.state_root ?         │
  │                 │     │  • If mismatch: REJECT block                    │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  2. RECEIPTS    │────▶│  Verify receipts merkle root                    │
  │     ROOT CHECK  │     │  • compute_receipts_root(receipts)              │
  │                 │     │  • computed == header.receipts_root             │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  3. ATOMIC      │────▶│  Persist all data atomically                    │
  │     WRITE       │     │  • Block header                                 │
  │                 │     │  • Block body (txs, receipts)                   │
  └────────┬────────┘     │  • State trie nodes                             │
           │              │  • Chain head pointer                           │
           │              └─────────────────────────────────────────────────┘
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  4. UPDATE      │────▶│  Update in-memory indices                       │
  │     INDICES     │     │  • Height -> Hash                               │
  │                 │     │  • Latest block cache                           │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐     ┌─────────────────────────────────────────────────┐
  │  5. BROADCAST   │────▶│  Announce commit to network                     │
  │     COMMIT MSG  │     │  • Send BlockCommit message                     │
  │                 │     │  • Update sync state                            │
  └────────┬────────┘     └─────────────────────────────────────────────────┘
           │
           ▼
  ┌─────────────────┐
  │  BLOCK COMMITTED│
  └─────────────────┘
```

### Header Commit Logic

```rust
/// Commit validated block to storage
pub fn commit_block(
    storage: &mut Storage,
    block: &ValidatedBlock,
    execution_result: &ExecutionResult,
) -> Result<CommitResult, CommitError> {
    // Step 1: State root comparison
    if execution_result.state_root != block.header.state_root {
        return Err(CommitError::StateRootMismatch {
            expected: block.header.state_root,
            computed: execution_result.state_root,
        });
    }
    
    // Step 2: Receipts root verification
    let computed_receipts_root = compute_receipts_root(&execution_result.receipts);
    if computed_receipts_root != block.header.receipts_root {
        return Err(CommitError::ReceiptsRootMismatch {
            expected: block.header.receipts_root,
            computed: computed_receipts_root,
        });
    }
    
    // Step 3: Atomic write
    let mut batch = storage.begin_batch();
    
    // Write block header
    batch.put(
        &block_header_key(&block.header.hash()),
        &block.header.encode(),
    );
    
    // Write block body
    batch.put(
        &block_body_key(&block.header.hash()),
        &block.body.encode(),
    );
    
    // Write transaction receipts
    for (i, receipt) in execution_result.receipts.iter().enumerate() {
        batch.put(
            &receipt_key(&block.header.hash(), i as u32),
            &receipt.encode(),
        );
    }
    
    // Write state trie changes
    for (key, value) in &execution_result.state_changes {
        batch.put(&state_key(key), value);
    }
    
    // Update chain head
    batch.put(
        HEAD_KEY,
        &block.header.hash(),
    );
    
    // Update height index
    batch.put(
        &height_key(block.header.height),
        &block.header.hash(),
    );
    
    // Commit batch atomically
    batch.commit()?;
    
    // Step 4: Update in-memory indices
    // (handled by caller)
    
    Ok(CommitResult {
        block_hash: block.header.hash(),
        height: block.header.height,
        state_root: execution_result.state_root,
        tx_count: block.body.transactions.len(),
        gas_used: execution_result.total_gas_used,
    })
}
```

### Broadcast Commit Message

```rust
/// Broadcast block commit to peers
pub async fn broadcast_commit(
    network: &Network,
    commit_result: &CommitResult,
) -> BroadcastStats {
    let mut stats = BroadcastStats::default();
    
    let commit_msg = BlockCommitMessage {
        block_hash: commit_result.block_hash,
        height: commit_result.height,
        state_root: commit_result.state_root,
        timestamp: current_timestamp(),
    };
    
    let message = Message::BlockCommit(commit_msg);
    
    for peer in network.connected_peers() {
        match network.send_async(peer, message.clone()).await {
            Ok(_) => {
                stats.success += 1;
            }
            Err(e) => {
                stats.failed += 1;
                log::warn!("Failed to send commit to {}: {}", peer, e);
            }
        }
    }
    
    // Emit metrics
    metrics::blocks_committed_total().inc();
    metrics::chain_height().set(commit_result.height as i64);
    metrics::last_block_time().set(current_timestamp() as i64);
    
    stats
}
```

---

## 7. Validation Table

### Stage Summary

| Stage | Inputs | Outputs | CPU Intensive | GPU Opportunity |
|-------|--------|---------|---------------|-----------------|
| **Header Validation** | BlockHeader, ChainState, ValidatorSet | HeaderValidationResult | Low | None |
| **Signature Verification** | Transactions, Signatures | Recovered senders | Medium | BLS aggregation |
| **PoUW Verification** | ComputeReceipts, TaskRegistry | VerifiedReceipts | High | SNARK verification |
| **Transaction Validation** | Transactions, State | ValidatedTransactions | Low | None |
| **State Execution** | ValidatedTxs, State | ExecutionResult | High | Parallel execution |
| **Merkle Computation** | Txs, Receipts | MerkleRoots | Medium | Tree hashing |
| **State Root** | StateTrie | Hash | Medium | MPT hashing |
| **Commit** | Block, State | CommitResult | Low (I/O bound) | None |

### Parallelization Opportunities

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     PARALLELIZATION OPPORTUNITIES                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CPU Parallelism:                                                           │
│  ────────────────                                                           │
│  • Signature verification (per-tx parallel)                                │
│  • Merkle tree computation (tree parallelism)                              │
│  • PoUW receipt verification (per-receipt parallel)                        │
│                                                                             │
│  GPU Acceleration:                                                          │
│  ─────────────────                                                          │
│  • BLS signature aggregation (batched)                                     │
│  • SNARK proof verification (compute-heavy)                                │
│  • Hash computation (bulk operations)                                      │
│                                                                             │
│  Pipeline Parallelism:                                                      │
│  ─────────────────────                                                      │
│  • Header validation || PoUW verification                                  │
│  • Signature recovery || Nonce validation                                  │
│  • Execution || Next block header validation                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Security Considerations

### Consensus-Critical Invariants

```rust
/// Consensus-critical invariants that MUST hold
mod invariants {
    /// Block height must increase monotonically
    pub fn height_monotonic(parent: &Block, child: &Block) -> bool {
        child.header.height == parent.header.height + 1
    }
    
    /// State root must match execution result
    pub fn state_root_matches(header: &BlockHeader, execution: &ExecutionResult) -> bool {
        header.state_root == execution.state_root
    }
    
    /// Total supply must be conserved
    pub fn supply_conserved(pre_state: &State, post_state: &State, block_reward: u128) -> bool {
        pre_state.total_supply() + block_reward == post_state.total_supply()
    }
    
    /// Nonces must be sequential per account
    pub fn nonces_sequential(account: &Address, txs: &[Transaction]) -> bool {
        let mut expected_nonce = txs[0].nonce;
        for tx in txs {
            if tx.from == *account {
                if tx.nonce != expected_nonce {
                    return false;
                }
                expected_nonce += 1;
            }
        }
        true
    }
    
    /// Finalized blocks cannot be reverted
    pub fn finality_respected(reorg_target: &Block, finalized: &Block) -> bool {
        reorg_target.header.height > finalized.header.height
    }
}
```

### Invalid State Prevention

| Check | Location | Consequence if Skipped |
|-------|----------|------------------------|
| Signature verification | Tx validation | Unauthorized transfers |
| Nonce check | Tx validation | Replay attacks |
| Balance check | Tx validation | Overdrafts |
| Gas accounting | Execution | DoS via infinite loops |
| State root check | Commit | State divergence |
| Parent validation | Header | Chain split |

### Execution Determinism

```rust
/// Determinism requirements for execution
pub struct DeterminismConfig {
    /// Fixed gas costs (no runtime variation)
    pub static_gas_costs: bool,
    
    /// Timestamp from block only (no system time)
    pub block_timestamp_only: bool,
    
    /// Deterministic hash function
    pub hash_algorithm: HashAlgorithm,
    
    /// No floating point in consensus
    pub integer_only_arithmetic: bool,
    
    /// Fixed iteration limits
    pub bounded_loops: bool,
}

impl Default for DeterminismConfig {
    fn default() -> Self {
        Self {
            static_gas_costs: true,
            block_timestamp_only: true,
            hash_algorithm: HashAlgorithm::Blake3,
            integer_only_arithmetic: true,
            bounded_loops: true,
        }
    }
}
```

### Common Attack Vectors

| Attack | Target | Mitigation |
|--------|--------|------------|
| **Signature malleability** | Tx uniqueness | Canonical signature format |
| **Integer overflow** | Balance/gas | Checked arithmetic |
| **Reentrancy** | State consistency | Checks-effects-interactions |
| **Front-running** | Tx ordering | Commit-reveal schemes |
| **Timestamp manipulation** | Time-dependent logic | Bounds checking |
| **Gas griefing** | Resource exhaustion | Minimum gas price |
| **State bloat** | Storage costs | Storage rent (future) |

---

## 9. Future Extensions

### ZK-Execution Validation

*Status: Research*

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     ZK-EXECUTION VALIDATION (FUTURE)                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Concept:                                                                   │
│  • Generate ZK proof of correct execution                                  │
│  • Verify proof instead of re-executing                                    │
│  • Enable light client full validation                                     │
│                                                                             │
│  Components:                                                                │
│  • RISC-V or WASM VM with ZK circuit                                       │
│  • State transition proof                                                  │
│  • Recursive proof aggregation                                             │
│                                                                             │
│  Benefits:                                                                  │
│  • Constant-time verification                                              │
│  • Trustless light clients                                                 │
│  • Cross-chain verification                                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Parallel Block Execution

*Status: Planned*

```rust
/// Parallel execution configuration (future)
pub struct ParallelExecutionConfig {
    /// Number of execution threads
    pub thread_count: usize,
    
    /// Transaction dependency analyzer
    pub dependency_analysis: bool,
    
    /// Speculative execution enabled
    pub speculative_execution: bool,
    
    /// Conflict resolution strategy
    pub conflict_resolution: ConflictStrategy,
}

/// Execute transactions in parallel where possible
pub async fn execute_parallel(
    state: &State,
    transactions: &[ValidatedTransaction],
    config: &ParallelExecutionConfig,
) -> Result<ExecutionResult, ExecutionError> {
    // Analyze dependencies
    let dependency_graph = analyze_dependencies(transactions);
    
    // Group independent transactions
    let execution_groups = dependency_graph.independent_groups();
    
    // Execute groups in parallel
    let mut results = Vec::new();
    for group in execution_groups {
        let group_results = futures::future::join_all(
            group.iter().map(|tx| execute_transaction(state.clone(), tx))
        ).await;
        
        results.extend(group_results);
    }
    
    // Merge results and handle conflicts
    merge_execution_results(results, config.conflict_resolution)
}
```

### Light-Client Friendly Execution Receipts

*Status: Planned*

```rust
/// Enhanced receipt for light client verification
pub struct LightClientReceipt {
    /// Standard receipt data
    pub receipt: TransactionReceipt,
    
    /// Merkle proof of inclusion
    pub inclusion_proof: MerkleProof,
    
    /// State proof for affected accounts
    pub state_proofs: Vec<AccountStateProof>,
    
    /// Witness data for verification
    pub witness: ExecutionWitness,
}

/// Account state proof
pub struct AccountStateProof {
    pub address: Address,
    pub pre_state: AccountState,
    pub post_state: AccountState,
    pub proof: MerkleProof,
}
```

---

## Summary

The Mbongo Chain block validation pipeline provides a robust, deterministic framework for validating blocks from proposal through commit. Each phase has well-defined inputs, outputs, and failure modes, enabling consistent behavior across all nodes. The modular design allows for future optimizations including parallel execution and ZK validation.

For consensus details, see [Consensus Validation](consensus_validation.md).

For networking details, see [Networking Overview](networking_overview.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

