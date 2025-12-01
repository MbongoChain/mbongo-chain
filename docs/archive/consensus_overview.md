# Mbongo Chain — Consensus Overview

This document describes the hybrid consensus model used by Mbongo Chain, combining Proof of Stake (PoS) with Proof of Useful Work (PoUW) to achieve security, decentralization, and compute usefulness.

---

## 1. Introduction

### Hybrid Consensus Model

Mbongo Chain employs a **hybrid PoS + PoUW consensus mechanism** that leverages the strengths of both approaches:

| Component | Role |
|-----------|------|
| **Proof of Stake (PoS)** | Economic security and block finality |
| **Proof of Useful Work (PoUW)** | Verifiable compute and resource allocation |

This hybrid design ensures that securing the network produces real-world value through useful computation, rather than wasting energy on arbitrary puzzles.

### Goals

The consensus mechanism is designed to achieve:

- **Security** — High cost to attack, rapid finality
- **Decentralization** — Open participation for validators and compute providers
- **Compute Usefulness** — Network security produces valuable compute outputs
- **Sustainability** — Economically viable for all participants

---

## 2. Design Principles

### Useful Work Strengthens Security

Unlike traditional Proof of Work, Mbongo Chain's PoUW ensures that computational effort produces valuable outputs:

- AI model inference and training
- Scientific simulations
- Data processing pipelines
- Rendering and media encoding

Every compute cycle contributes to both network security and real-world utility.

### Hybrid Economics

The economic model combines stake-based and compute-based incentives:

```
┌─────────────────────────────────────────────────────────────────┐
│                    HYBRID ECONOMIC MODEL                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────┐       ┌─────────────────────┐         │
│  │     STAKE-BASED     │       │    COMPUTE-BASED    │         │
│  │                     │       │                     │         │
│  │  • Lock tokens      │       │  • Provide compute  │         │
│  │  • Validate blocks  │   +   │  • Submit proofs    │         │
│  │  • Earn rewards     │       │  • Earn rewards     │         │
│  │  • Risk slashing    │       │  • Build reputation │         │
│  └─────────────────────┘       └─────────────────────┘         │
│                                                                 │
│                    Combined Security Budget                     │
└─────────────────────────────────────────────────────────────────┘
```

### Deterministic Verification

Compute tasks must be deterministically verifiable:

- Same inputs always produce same outputs
- Verification is significantly cheaper than computation
- Proofs are compact and efficiently validated on-chain

### Long-Term Sustainability

The consensus model is designed for long-term operation:

- Predictable reward schedules
- Balanced incentives between validators and compute providers
- Resistance to centralization pressures

---

## 3. Components of Consensus

### 3.1 Validators (PoS)

Validators are responsible for block production and finalization:

| Responsibility | Description |
|----------------|-------------|
| Block Proposal | Assemble and propose new blocks |
| Attestation | Vote on block validity |
| Finalization | Participate in finality protocol |
| Slashing | Subject to penalties for misbehavior |

**Requirements:**
- Minimum stake threshold
- Reliable infrastructure
- Network connectivity

### 3.2 Compute Nodes (PoUW Contributors)

Compute nodes perform useful work and submit proofs:

| Responsibility | Description |
|----------------|-------------|
| Task Execution | Perform assigned compute tasks |
| Proof Generation | Create verifiable proofs of work |
| Proof Submission | Submit proofs to the network |
| Result Delivery | Provide compute outputs to requesters |

**Requirements:**
- Capable hardware (GPU, specialized compute)
- Network connectivity
- Registered identity

### 3.3 Coordination Layer

The coordination layer manages block proposer selection:

```
┌─────────────────────────────────────────────────────────────────┐
│                    COORDINATION LAYER                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Epoch N                                                        │
│  ├── Slot 0: Validator A proposes                              │
│  ├── Slot 1: Validator B proposes                              │
│  ├── Slot 2: Validator C proposes                              │
│  └── ...                                                        │
│                                                                 │
│  Selection: Weighted random based on stake                      │
│  Rotation: Per-slot assignment                                  │
│  Backup: Secondary proposers if primary fails                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 3.4 Proof Verifier

The proof verifier validates PoUW submissions:

| Function | Description |
|----------|-------------|
| Format Check | Verify proof structure |
| Computation Verify | Validate proof against commitment |
| Duplicate Detection | Reject previously submitted proofs |
| Reward Trigger | Signal reward distribution on success |

---

## 4. Proof-of-Stake (PoS) Overview

### Role of Staking

Staking serves multiple purposes:

- **Security Deposit** — Validators lock tokens as collateral
- **Voting Power** — Stake determines influence in consensus
- **Alignment** — Economic incentive to behave honestly
- **Decentralization** — Open participation with sufficient stake

### Validator Requirements

| Requirement | Specification |
|-------------|---------------|
| Minimum Stake | TBD (governance parameter) |
| Hardware | Reliable server with SSD storage |
| Network | Low-latency, high-availability connection |
| Uptime | 99%+ expected availability |

### Block Proposal Rules

Validators propose blocks according to:

1. **Slot Assignment** — Determined by randomness beacon + stake weight
2. **Transaction Selection** — Choose transactions from mempool
3. **PoUW Inclusion** — Include verified compute proofs
4. **State Root** — Compute post-execution state root
5. **Signature** — Sign block with validator key

### Misbehavior Detection

The protocol detects and punishes misbehavior:

| Violation | Detection Method |
|-----------|------------------|
| Double Signing | Conflicting signatures for same slot |
| Surround Voting | Attestations that surround or are surrounded |
| Inactivity | Missing attestations over threshold |

### Slashing

*Status: Placeholder for future implementation*

Slashing penalties will include:

- Partial stake reduction for minor violations
- Full stake forfeiture for severe attacks
- Forced exit from validator set
- Cooldown period before re-entry

---

## 5. Proof-of-Useful-Work (PoUW) Overview

### Types of Compute Tasks

PoUW supports various compute workloads:

| Category | Examples |
|----------|----------|
| **AI/ML** | Model inference, training batches, fine-tuning |
| **Scientific** | Simulations, data analysis, optimization |
| **Media** | Rendering, encoding, transcoding |
| **Cryptographic** | ZK proof generation, signature aggregation |

### Verification Model

Compute proofs follow a verification pipeline:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Compute Task   │────▶│   Execution     │────▶│     Proof       │
│  (Commitment)   │     │   (Off-chain)   │     │   (Generated)   │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│     Reward      │◀────│   On-Chain      │◀────│   Submission    │
│   (Distributed) │     │  (Verification) │     │  (Transaction)  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

**Verification Properties:**

- **Soundness** — Invalid proofs are rejected with high probability
- **Efficiency** — Verification is orders of magnitude faster than computation
- **Determinism** — Same proof always yields same verification result

### Receipt Creation

Successful verification produces a compute receipt:

```rust
// Compute receipt (conceptual)
pub struct ComputeReceipt {
    /// Task identifier
    pub task_id: Hash,
    /// Compute provider
    pub provider: Address,
    /// Proof hash
    pub proof_hash: Hash,
    /// Verification timestamp
    pub verified_at: u64,
    /// Reward amount
    pub reward: u128,
}
```

### Integration into Block Metadata

Compute proofs are included in blocks:

```
Block Header
├── Parent Hash
├── State Root
├── Transactions Root
├── Receipts Root
├── PoUW Proofs Root      ← Merkle root of included proofs
├── Timestamp
└── Validator Signature
```

---

## 6. Hybrid Model Interaction

### Mutual Reinforcement

PoS and PoUW strengthen each other:

```
┌─────────────────────────────────────────────────────────────────┐
│                   MUTUAL REINFORCEMENT                          │
└─────────────────────────────────────────────────────────────────┘

     PoS Security                           PoUW Utility
         │                                       │
         │  Validators secure                    │  Compute providers
         │  compute marketplace   ────────────▶  │  add economic value
         │                                       │
         │  Economic value                       │  Useful work
         │  strengthens stake   ◀────────────    │  funds security
         │                                       │
         ▼                                       ▼
    
    Higher stake value              More compute demand
         │                                       │
         └───────────────┬───────────────────────┘
                         │
                         ▼
                 NETWORK SECURITY
```

### Reward Distribution

Rewards flow to both participant types:

| Recipient | Source | Basis |
|-----------|--------|-------|
| Validators | Block rewards | Stake weight × participation |
| Compute Providers | Compute rewards | Verified useful work |
| Delegators | Shared rewards | Delegation to validators |

*Note: Detailed tokenomics are specified separately.*

### Why Hybrid Increases Security

The hybrid model raises attack costs:

| Attack Type | PoS-Only Cost | Hybrid Cost |
|-------------|---------------|-------------|
| 51% Attack | Acquire 51% stake | Acquire 51% stake + compute |
| Censorship | Control block production | Control blocks + compute market |
| State Manipulation | Corrupt validators | Corrupt validators + fake proofs |

Attackers must compromise both economic stake AND compute infrastructure.

---

## 7. Block Finalization Path

### Complete Path

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│Proposer │────▶│Validate │────▶│ Include │────▶│ Attest  │────▶│ Finalize│
│ Select  │     │  Block  │     │  PoUW   │     │  Block  │     │  Block  │
└─────────┘     └─────────┘     └─────────┘     └─────────┘     └─────────┘
     │               │               │               │               │
     ▼               ▼               ▼               ▼               ▼
  Slot           Tx Exec         Proof           Vote            Finality
  Assign         Verify          Verify          Count           Gadget
```

### Stage Details

#### 1. Block Proposal

- Validator assigned to slot proposes block
- Includes pending transactions and compute proofs
- Computes state root after execution

#### 2. Block Validation

- Other validators receive proposed block
- Verify transaction execution
- Verify state transition correctness

#### 3. PoUW Receipt Inclusion

- Validate included compute proofs
- Generate compute receipts
- Update proof verification state

#### 4. Attestation

- Validators sign attestations for valid blocks
- Attestations aggregated for efficiency
- Threshold required for progression

#### 5. Finality Gadget

*Status: Placeholder for future implementation*

Finality mechanism will provide:

- Probabilistic finality after N confirmations
- Economic finality via attestation threshold
- Absolute finality via finality gadget (planned)

---

## 8. Security Assumptions

### Honest Majority of Stake

The protocol assumes:

| Assumption | Threshold |
|------------|-----------|
| Safety | >1/3 stake honest |
| Liveness | >2/3 stake online and honest |
| Finality | >2/3 stake attesting |

### Verifiable Compute Correctness

PoUW security relies on:

- **Proof Soundness** — False proofs rejected with overwhelming probability
- **Deterministic Tasks** — Same inputs produce same outputs
- **Verification Integrity** — On-chain verifier correctly implemented

### Network-Level Assumptions

| Assumption | Requirement |
|------------|-------------|
| Synchrony | Messages delivered within Δ time |
| Connectivity | Honest nodes can communicate |
| Clock Sync | Clocks within bounded drift |

---

## 9. Attack Mitigation

### Sybil Resistance

Multiple identity attacks are mitigated by:

| Mechanism | Protection |
|-----------|------------|
| Stake Requirement | Creating validators requires capital |
| Compute Registration | Providers must prove capability |
| Reputation System | History affects opportunities |

### Long-Range Attacks

*Status: Placeholder for future implementation*

Mitigations will include:

- Weak subjectivity checkpoints
- Finalized state commitments
- Social consensus on chain history

### Grinding Resistance

Block proposer selection resists grinding:

- Randomness derived from previous blocks
- Stake-weighted selection
- Limited influence over future slots

### Compute Fraud Prevention

PoUW verification prevents compute fraud:

```
┌─────────────────────────────────────────────────────────────────┐
│                    FRAUD PREVENTION                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Attack: Submit false proof claiming work was done              │
│                                                                 │
│  Defense:                                                       │
│  ┌───────────────┐                                             │
│  │ Proof Submit  │                                             │
│  └───────┬───────┘                                             │
│          │                                                      │
│          ▼                                                      │
│  ┌───────────────┐     ┌───────────────┐                       │
│  │   Verifier    │────▶│ Check against │                       │
│  │   (On-chain)  │     │  commitment   │                       │
│  └───────────────┘     └───────┬───────┘                       │
│                                │                                │
│          ┌─────────────────────┼─────────────────────┐         │
│          │                     │                     │         │
│          ▼                     ▼                     ▼         │
│    ┌───────────┐        ┌───────────┐        ┌───────────┐    │
│    │  VALID    │        │  INVALID  │        │  FRAUD    │    │
│    │  Reward   │        │  Reject   │        │  Penalty  │    │
│    └───────────┘        └───────────┘        └───────────┘    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. Future Extensions

### ZK-Accelerated Compute Verification

*Status: Research*

Zero-knowledge proofs will enable:

- Succinct verification of complex computations
- Privacy-preserving compute tasks
- Reduced on-chain verification costs
- Batch verification of multiple proofs

### Multi-Engine Consensus

*Status: Planned*

Support for multiple consensus engines:

- Pluggable finality gadgets
- Configurable PoS parameters
- Alternative PoUW verification schemes
- Testnet experimentation

### Cross-Chain Finality Bridges

*Status: Research*

Cross-chain integration will provide:

- Light client bridges to other chains
- Shared security models
- Cross-chain compute verification
- Unified liquidity across networks

### Additional Roadmap

- [ ] Validator rotation optimization
- [ ] Compute task scheduling improvements
- [ ] Reputation-weighted proof assignment
- [ ] Decentralized task marketplace
- [ ] Hardware attestation for compute nodes

---

## Summary

Mbongo Chain's hybrid PoS + PoUW consensus combines economic security with useful computation. Validators secure the network through staked capital, while compute providers contribute valuable work verified on-chain. This design achieves security, decentralization, and real-world utility in a single protocol.

For runtime details, see [Runtime Architecture](runtime_architecture.md).

For node-level architecture, see [Node Architecture](node_architecture.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

