# Proof of Useful Work (PoUW) - Verification Strategy

## Overview

This document defines the comprehensive verification strategy for Mbongo Chain's Proof of Useful Work consensus mechanism. The strategy employs a multi-layered approach that evolves across three phases, balancing security, efficiency, and decentralization.

## Table of Contents

1. [Verification Architecture](#verification-architecture)
2. [Phase 1: Redundant Execution](#phase-1-redundant-execution)
3. [Phase 2: TEE Integration](#phase-2-tee-integration)
4. [Phase 3: ZK-ML Proofs](#phase-3-zk-ml-proofs)
5. [Fraud Proofs System](#fraud-proofs-system)
6. [Slashing Mechanism](#slashing-mechanism)
7. [Security Analysis](#security-analysis)

---

## Verification Architecture

### Multi-Layer Defense Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                    Verification Layers                       │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Redundant Execution (All Phases)                  │
│           ├─ Random validator selection                     │
│           ├─ Majority consensus (2/3)                       │
│           └─ Result comparison                              │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: Optimistic Fraud Proofs (All Phases)              │
│           ├─ Challenge period: 100 blocks                   │
│           ├─ Open challenge submission                      │
│           └─ On-chain dispute resolution                    │
├─────────────────────────────────────────────────────────────┤
│  Layer 3: TEE Attestation (Phase 2+)                        │
│           ├─ Intel SGX / AMD SEV                            │
│           ├─ Remote attestation                             │
│           └─ Sealed execution environment                   │
├─────────────────────────────────────────────────────────────┤
│  Layer 4: ZK-ML Proofs (Phase 3+)                           │
│           ├─ Zero-knowledge computation proofs              │
│           ├─ Succinct verification                          │
│           └─ Privacy-preserving validation                  │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Redundant Execution

### Overview

Phase 1 relies on **redundant execution** where multiple randomly selected validators independently verify each compute task. This approach provides immediate security while more advanced cryptographic methods are developed.

### Configuration Parameters

```yaml
redundancy:
  default_validators: 3
  minimum_validators: 3
  maximum_validators: 7

consensus:
  threshold: 0.67  # 2/3 majority required

selection:
  method: "weighted_random"
  refresh_interval: 10  # blocks
```

### Validator Selection Algorithm

#### Selection Formula

The probability of validator `i` being selected is:

```
P(i) = (stake_i × reputation_i) / Σ(stake_j × reputation_j)
```

Where:
- `stake_i` = Validator's staked amount
- `reputation_i` = Validator's reputation score (0.0 - 1.0)
- `Σ` = Sum over all eligible validators

#### Selection Process

```
┌─────────────────────────────────────────────────────────────┐
│                 Validator Selection Flow                     │
└─────────────────────────────────────────────────────────────┘

    Task Submitted
         ↓
    ┌────────────────────────┐
    │  Generate Random Seed  │
    │  (block_hash + task_id)│
    └───────────┬────────────┘
                ↓
    ┌────────────────────────┐
    │   Filter Eligible      │
    │   Validators           │
    │   • Minimum stake      │
    │   • Reputation > 0.5   │
    │   • Not slashed        │
    └───────────┬────────────┘
                ↓
    ┌────────────────────────┐
    │   Calculate Weights    │
    │   W = stake × rep      │
    └───────────┬────────────┘
                ↓
    ┌────────────────────────┐
    │   Weighted Random      │
    │   Selection (n=3)      │
    └───────────┬────────────┘
                ↓
    Selected Validators
```

#### Implementation Pseudocode

```python
def select_validators(task_id, block_hash, num_validators=3):
    """
    Selects validators using weighted random selection
    """
    # Generate deterministic random seed
    seed = hash(block_hash + task_id)

    # Filter eligible validators
    eligible = [
        v for v in all_validators
        if v.stake >= MIN_STAKE
        and v.reputation >= 0.5
        and not v.is_slashed
    ]

    # Calculate selection weights
    weights = [
        v.stake * v.reputation
        for v in eligible
    ]

    # Normalize weights
    total_weight = sum(weights)
    probabilities = [w / total_weight for w in weights]

    # Select without replacement
    selected = weighted_random_sample(
        population=eligible,
        weights=probabilities,
        k=num_validators,
        seed=seed
    )

    return selected
```

### Verification Process

```
┌─────────────────────────────────────────────────────────────┐
│              Redundant Execution Timeline                    │
└─────────────────────────────────────────────────────────────┘

Block N: Task Assignment
    │
    │    ┌──────────────┐
    ├───→│ Validator 1  │──→ Execute & Submit Result
    │    └──────────────┘
    │    ┌──────────────┐
    ├───→│ Validator 2  │──→ Execute & Submit Result
    │    └──────────────┘
    │    ┌──────────────┐
    └───→│ Validator 3  │──→ Execute & Submit Result
         └──────────────┘
              ↓
Block N+1: Result Collection
              ↓
    ┌─────────────────────┐
    │  Compare Results    │
    │  • Hash comparison  │
    │  • Output matching  │
    └──────────┬──────────┘
               ↓
    ┌─────────────────────┐
    │  Consensus Check    │
    │  (2/3 agreement)    │
    └──────────┬──────────┘
               ↓
         ┌─────────┐
         │ Accept  │  OR  ┌─────────┐
         │ Result  │      │ Reject  │
         └─────────┘      └─────────┘
               ↓                ↓
    Update Reputation    Slash Minority
```

### Result Comparison

Results are considered matching if:

```python
def results_match(result_1, result_2):
    """
    Compares two computation results
    """
    return (
        hash(result_1.output) == hash(result_2.output)
        and abs(result_1.gas_used - result_2.gas_used) < GAS_TOLERANCE
        and result_1.exit_code == result_2.exit_code
    )
```

### Consensus Decision

```python
def determine_consensus(results):
    """
    Determines if consensus is reached
    """
    # Group identical results
    result_groups = group_by_hash(results)

    # Find largest group
    largest_group = max(result_groups, key=len)

    # Check if majority (2/3)
    if len(largest_group) >= (2 * len(results)) / 3:
        return {
            'consensus': True,
            'accepted_result': largest_group[0],
            'agreeing_validators': largest_group,
            'disagreeing_validators': [r for r in results if r not in largest_group]
        }
    else:
        return {
            'consensus': False,
            'requires_arbitration': True
        }
```

---

## Phase 2: TEE Integration

### Overview

Phase 2 introduces **Trusted Execution Environment (TEE)** support, allowing validators to prove their computations were executed in a secure, isolated environment. This reduces the need for full redundancy while maintaining security.

### Supported TEE Technologies

- **Intel SGX** (Software Guard Extensions)
- **AMD SEV** (Secure Encrypted Virtualization)
- **ARM TrustZone**

### TEE Attestation Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    TEE Attestation Process                   │
└─────────────────────────────────────────────────────────────┘

1. Validator Initialization
    │
    ├─→ Create TEE Enclave
    │
    ├─→ Generate Attestation Report
    │      • Enclave measurement (MRENCLAVE)
    │      • Signing key (MRSIGNER)
    │      • Security version
    │
    └─→ Submit to Chain
           ↓
2. Attestation Verification
    │
    ├─→ Verify Intel/AMD Signature
    │
    ├─→ Check Enclave Identity
    │
    ├─→ Validate Security Level
    │
    └─→ Register TEE Validator
           ↓
3. Task Execution
    │
    ├─→ Load Task in Enclave
    │
    ├─→ Execute in Sealed Environment
    │
    ├─→ Generate Quote + Result
    │
    └─→ Submit with Attestation
           ↓
4. On-Chain Verification
    │
    ├─→ Verify Quote Signature
    │
    ├─→ Check Enclave Identity Matches
    │
    └─→ Accept Result (if valid)
```

### TEE Configuration

```yaml
tee:
  enabled: true  # Phase 2+

  required_attestation_types:
    - "intel_sgx"
    - "amd_sev"

  enclave_requirements:
    min_security_version: 2
    require_debug_disabled: true
    require_production_mode: true

  redundancy_reduction:
    tee_validator_count: 1  # Only 1 TEE validator needed
    fallback_validator_count: 2  # + 2 non-TEE validators
```

### Hybrid Verification Model

In Phase 2, tasks can be verified either through:

1. **TEE + Redundancy**: 1 TEE validator + 2 standard validators
2. **Full Redundancy**: 3 standard validators (fallback)

```
┌─────────────────────────────────────────────────────────────┐
│              Phase 2 Verification Decision Tree              │
└─────────────────────────────────────────────────────────────┘

                    Task Submitted
                         ↓
              ┌──────────────────────┐
              │ TEE Validators       │
              │ Available?           │
              └──────────┬───────────┘
                         ↓
           Yes ┌─────────┴─────────┐ No
               ↓                   ↓
      ┌────────────────┐    ┌──────────────┐
      │ TEE Hybrid     │    │ Full         │
      │ 1 TEE +        │    │ Redundancy   │
      │ 2 Standard     │    │ 3 Validators │
      └────────┬───────┘    └──────┬───────┘
               ↓                   ↓
         Reduced Cost        Higher Cost
         Higher Security     Standard Security
```

---

## Phase 3: ZK-ML Proofs

### Overview

Phase 3 introduces **Zero-Knowledge Machine Learning (ZK-ML)** proofs, enabling validators to prove correct computation execution without revealing the computation itself. This provides:

- **Privacy**: Computation details remain confidential
- **Efficiency**: Verification is O(1) instead of re-execution
- **Scalability**: Single proof validates complex computations

### ZK-ML Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  ZK-ML Proof Generation                      │
└─────────────────────────────────────────────────────────────┘

Compute Provider Side:
    │
    ├─→ 1. Execute ML Task
    │      • Run inference/training
    │      • Record execution trace
    │
    ├─→ 2. Generate Circuit
    │      • Convert computation to arithmetic circuit
    │      • Optimize for proof size
    │
    ├─→ 3. Generate ZK Proof
    │      • Prove correct execution
    │      • Prove input/output relationship
    │      • No private data revealed
    │
    └─→ 4. Submit Proof + Result
           ↓
Validator Side:
    │
    ├─→ 5. Verify ZK Proof
    │      • O(1) verification time
    │      • Check proof validity
    │
    └─→ 6. Accept/Reject
           • No re-execution needed
```

### Supported Proof Systems

```yaml
zk_proofs:
  enabled: true  # Phase 3+

  proof_systems:
    primary: "groth16"      # Fast verification
    fallback: "plonk"       # Universal setup

  circuit_optimization:
    max_constraints: 1000000
    parallelization: true

  verification:
    on_chain: true
    gas_limit: 500000       # Gas for proof verification
```

### ZK Proof Structure

```rust
struct ZKMLProof {
    // Proof data
    proof: Vec<u8>,

    // Public inputs
    public_inputs: PublicInputs {
        task_id: Hash,
        input_hash: Hash,
        output_hash: Hash,
        model_hash: Hash,
        gas_used: u64,
    },

    // Verification key
    vk_hash: Hash,

    // Metadata
    proof_system: ProofSystem,
    circuit_version: u32,
}

struct PublicInputs {
    task_id: [u8; 32],
    input_hash: [u8; 32],
    output_hash: [u8; 32],
    model_hash: [u8; 32],
    gas_used: u64,
}
```

### Verification Flow

```python
def verify_zkml_proof(proof, public_inputs):
    """
    Verifies a ZK-ML proof on-chain
    """
    # Load verification key
    vk = load_verification_key(proof.vk_hash)

    # Verify proof structure
    if not validate_proof_format(proof):
        return False

    # Verify public inputs match task
    if not verify_public_inputs(public_inputs):
        return False

    # Cryptographic verification
    if not zksnark_verify(vk, proof.proof, public_inputs):
        return False

    # Check gas limits
    if public_inputs.gas_used > MAX_GAS:
        return False

    return True
```

### Cost-Benefit Analysis

| Method | Verification Time | Gas Cost | Security | Privacy |
|--------|------------------|----------|----------|---------|
| Redundant Execution | O(n) | High | High | Low |
| TEE Attestation | O(1) | Medium | High | Medium |
| ZK-ML Proofs | O(1) | Low | Very High | High |

---

## Fraud Proofs System

### Overview

The **Optimistic Fraud Proofs** system allows anyone to challenge incorrect computation results during a challenge period. This provides an additional security layer across all phases.

### Challenge Period

```yaml
fraud_proofs:
  challenge_period: 100  # blocks (~10 minutes)

  submission:
    min_stake: 100  # MBO tokens
    challenge_bond: 50  # MBO tokens

  resolution:
    arbitration_validators: 5
    timeout: 500  # blocks
```

### Challenge Timeline

```
┌─────────────────────────────────────────────────────────────┐
│                  Fraud Proof Timeline                        │
└─────────────────────────────────────────────────────────────┘

Block N: Result Submitted
    │
    ├─→ Result Hash Posted
    │   Rewards Locked
    │
    ↓
Block N → N+100: Challenge Period (100 blocks)
    │
    │   Anyone can submit challenge:
    │   • Alternative result
    │   • Fraud proof
    │   • Challenge bond
    │
    ↓
    ┌─────────────────────┐
    │ Challenge Submitted?│
    └──────────┬──────────┘
               ↓
     Yes ┌────┴────┐ No
         ↓         ↓
    ┌─────────┐ ┌──────────────┐
    │Arbitrate│ │Finalize      │
    │(Re-exec)│ │Accept Result │
    └────┬────┘ └──────────────┘
         ↓
    ┌─────────────────────┐
    │Compare:             │
    │• Original Result    │
    │• Challenge Result   │
    │• Arbitrator Results │
    └─────────┬───────────┘
              ↓
         ┌────────┐
         │Winner? │
         └───┬────┘
             ↓
    ┌────────┴────────┐
    ↓                 ↓
Original Correct  Challenge Correct
    ↓                 ↓
Slash Challenger  Slash Original Validator
Return Bond       Reward Challenger
Pay Rewards       Reverse Rewards
```

### Challenge Submission

```rust
struct FraudChallenge {
    // Challenge identification
    challenge_id: Hash,
    task_id: Hash,

    // Challenger information
    challenger: Address,
    challenge_bond: u64,

    // Challenge data
    alternative_result: ComputeResult,
    fraud_proof: FraudProof,

    // Timing
    submission_block: u64,
    deadline_block: u64,
}

struct FraudProof {
    // Evidence of fraud
    execution_trace: Vec<TraceStep>,
    divergence_point: u64,
    expected_state: StateHash,
    actual_state: StateHash,

    // Supporting data
    witness_data: Vec<u8>,
}
```

### Arbitration Process

```python
def arbitrate_challenge(original_result, challenge):
    """
    Resolves a fraud proof challenge
    """
    # Select arbitration validators
    arbitrators = select_validators(
        task_id=challenge.task_id,
        num_validators=5,
        exclude=[original_result.validator, challenge.challenger]
    )

    # Re-execute task
    arbitration_results = []
    for arbitrator in arbitrators:
        result = arbitrator.execute_task(challenge.task_id)
        arbitration_results.append(result)

    # Determine consensus
    consensus = determine_consensus(arbitration_results)

    # Compare with original and challenge
    if consensus.result == original_result:
        # Original was correct
        slash(challenge.challenger, challenge.challenge_bond)
        return original_result
    elif consensus.result == challenge.alternative_result:
        # Challenge was correct
        slash(original_result.validator, VALIDATOR_SLASH_AMOUNT)
        reward(challenge.challenger, CHALLENGE_REWARD)
        return challenge.alternative_result
    else:
        # Both wrong - slash both
        slash(original_result.validator, VALIDATOR_SLASH_AMOUNT)
        slash(challenge.challenger, challenge.challenge_bond)
        return consensus.result
```

---

## Slashing Mechanism

### Overview

The slashing mechanism penalizes malicious or negligent validators to maintain network integrity.

### Slashing Parameters

```yaml
slashing:
  # Slash amounts (in MBO tokens)
  incorrect_result: 1000
  missed_validation: 100
  invalid_tee_attestation: 2000
  invalid_zk_proof: 2000

  # Reputation impact
  reputation_penalty:
    incorrect_result: -0.1
    missed_validation: -0.02
    fraud_attempt: -0.5

  # Recovery
  reputation_recovery_rate: 0.001  # per successful validation
  min_reputation: 0.0
  max_reputation: 1.0

  # Jail (temporary ban)
  jail_threshold: 0.3  # reputation below this = jailed
  jail_duration: 1000  # blocks
```

### Slash Conditions

| Violation | Stake Slash | Reputation Penalty | Jail? |
|-----------|-------------|-------------------|-------|
| Incorrect result (minority) | 1000 MBO | -0.1 | No |
| Incorrect result (proven fraud) | 5000 MBO | -0.5 | Yes |
| Missed validation | 100 MBO | -0.02 | No |
| Invalid TEE attestation | 2000 MBO | -0.3 | Yes |
| Invalid ZK proof | 2000 MBO | -0.3 | Yes |
| Multiple violations (3+) | 10000 MBO | -0.8 | Yes |

### Slashing Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    Slashing Process                          │
└─────────────────────────────────────────────────────────────┘

    Violation Detected
         ↓
    ┌────────────────────────┐
    │  Record Violation      │
    │  • Type                │
    │  • Validator           │
    │  • Evidence            │
    └───────────┬────────────┘
                ↓
    ┌────────────────────────┐
    │  Calculate Penalties   │
    │  • Stake slash amount  │
    │  • Reputation penalty  │
    └───────────┬────────────┘
                ↓
    ┌────────────────────────┐
    │  Execute Slash         │
    │  • Burn tokens         │
    │  • Update reputation   │
    └───────────┬────────────┘
                ↓
    ┌────────────────────────┐
    │  Check Jail Condition  │
    │  (reputation < 0.3)    │
    └───────────┬────────────┘
                ↓
         ┌──────┴──────┐
         │ Jail?       │
         └──────┬──────┘
                ↓
       Yes ┌────┴────┐ No
           ↓         ↓
    ┌──────────┐  ┌────────┐
    │ Jail     │  │ Active │
    │ Validator│  │        │
    │ 1000 blk │  │        │
    └──────────┘  └────────┘
```

### Reputation Management

```python
class ReputationManager:
    """
    Manages validator reputation scores
    """

    def __init__(self):
        self.min_reputation = 0.0
        self.max_reputation = 1.0
        self.recovery_rate = 0.001

    def apply_penalty(self, validator, violation_type):
        """
        Applies reputation penalty for violation
        """
        penalty = REPUTATION_PENALTIES[violation_type]
        new_reputation = max(
            self.min_reputation,
            validator.reputation + penalty
        )
        validator.reputation = new_reputation

        # Check if jailing required
        if new_reputation < JAIL_THRESHOLD:
            self.jail_validator(validator)

    def apply_reward(self, validator):
        """
        Increases reputation for successful validation
        """
        new_reputation = min(
            self.max_reputation,
            validator.reputation + self.recovery_rate
        )
        validator.reputation = new_reputation

    def jail_validator(self, validator):
        """
        Temporarily bans validator from participation
        """
        validator.jailed = True
        validator.jail_release_block = (
            current_block() + JAIL_DURATION
        )
```

### Slash Distribution

Slashed tokens are distributed as follows:

```
Slashed Amount (100%)
    ↓
    ├─→ 50% Burned (deflationary)
    ├─→ 30% To Treasury (governance)
    └─→ 20% To Challenger (if applicable)
```

---

## Security Analysis

### Threat Model

| Attack Vector | Mitigation | Success Probability |
|--------------|------------|-------------------|
| **Collusion (3 validators)** | Random selection, high validator pool | < 0.01% |
| **Sybil Attack** | Minimum stake requirement, reputation | < 0.1% |
| **TEE Compromise** | Fallback to redundancy, attestation | < 0.5% |
| **ZK Proof Forgery** | Cryptographic soundness, verification | Negligible |
| **Long-range Attack** | Fraud proofs, checkpointing | < 0.01% |

### Collusion Resistance

Probability of 2/3 colluding validators:

```
P(collusion) = C(m,k) / C(n,k)

Where:
- n = total validator pool
- m = colluding validators
- k = selected validators (3)
- C = combination function
```

Example with 100 validators, 10 colluding:

```
P(2/3 collusion) = [C(10,2) × C(90,1) + C(10,3)] / C(100,3)
                 = [45 × 90 + 120] / 161,700
                 = 4,170 / 161,700
                 ≈ 0.0258 (2.58%)
```

### Economic Security

Total economic security derives from:

```
Security Budget = Σ(slash_amount × P(violation))

With default parameters:
- 1000 validators
- 10,000 MBO average stake
- 1000 MBO slash per violation

Expected security: 10,000,000 MBO at risk
```

### Verification Cost Comparison

| Phase | Method | Validators | Cost per Task | Verification Time |
|-------|--------|-----------|---------------|------------------|
| **Phase 1** | Redundant | 3 | 3× compute | 3× time |
| **Phase 2** | TEE + Redundant | 1+2 | 2× compute | 2× time |
| **Phase 3** | ZK Proof | 1 | 1× + proof gen | Proof verification (~1s) |

### Liveness Guarantees

The system guarantees liveness if:

```
honest_validators ≥ (2/3) × selected_validators
```

With random selection from a large pool:

```
P(liveness) = P(≥2 honest | 3 selected)
            = 1 - P(all malicious) - P(2 malicious)

With 90% honest validators:
P(liveness) ≈ 99.7%
```

---

## Implementation Roadmap

### Phase 1 (Current)
- ✅ Redundant execution framework
- ✅ Weighted random selection
- ✅ Fraud proof system
- ✅ Basic slashing mechanism

### Phase 2 (Q2 2025)
- ⏳ Intel SGX integration
- ⏳ AMD SEV support
- ⏳ TEE attestation verification
- ⏳ Hybrid verification model

### Phase 3 (Q4 2025)
- 📋 ZK-ML circuit design
- 📋 Proof generation integration
- 📋 On-chain proof verification
- 📋 Privacy-preserving validation

---

## References

- [PoUW Consensus Mechanics](./consensus_mechanics.md)
- [Validator Setup Guide](./validator_setup.md)
- [Compute Provider Guide](./compute_provider_setup.md)
- [Economic Model](./economic_model.md)

## Changelog

- **2025-11-30**: Initial verification strategy documentation
  - Redundant execution specification
  - TEE integration plan
  - ZK-ML roadmap
  - Fraud proofs system
  - Slashing mechanism
