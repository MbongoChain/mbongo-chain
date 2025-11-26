# Mbongo Chain — Validation Specification Summary

> **Document Type:** Technical Specification  
> **Last Updated:** November 2025  
> **Status:** Canonical Reference

---

## Table of Contents

1. [Overview](#1-overview)
2. [Validation Flow Summary](#2-validation-flow-summary)
3. [Core Validation Rules Table](#3-core-validation-rules-table)
4. [Failure Modes](#4-failure-modes)
5. [Guarantees](#5-guarantees)
6. [Developer Notes](#6-developer-notes)

---

## 1. Overview

### 1.1 Purpose

This document provides a **comprehensive summary of all validation rules** enforced by the Mbongo Chain protocol. It serves as a single reference for understanding what conditions must be satisfied at each stage of transaction and block processing.

### 1.2 Position in Protocol Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATION IN PROTOCOL PIPELINE                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   External Input                                                                        │
│        │                                                                                │
│        ▼                                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                          NETWORKING LAYER                                        │  │
│   │   Validation: Message format, size limits, peer authorization                   │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│        │                                                                                │
│        ▼                                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           MEMPOOL LAYER                                          │  │
│   │   Validation: Signatures, nonces, balances, fee requirements                    │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│        │                                                                                │
│        ▼                                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                          CONSENSUS LAYER                                         │  │
│   │   Validation: Block headers, proposer eligibility, PoUW receipts, attestations  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│        │                                                                                │
│        ▼                                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                          EXECUTION LAYER                                         │  │
│   │   Validation: State transitions, gas consumption, invariants, receipts          │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│        │                                                                                │
│        ▼                                                                                │
│   Finalized State                                                                       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Participating Layers

| Layer | Validation Scope | Failure Impact |
|-------|------------------|----------------|
| **Networking** | Message integrity, peer trust | Message dropped |
| **Mempool** | Transaction validity, resource limits | Transaction rejected |
| **Consensus** | Block validity, proposer rights | Block rejected |
| **Execution** | State correctness, determinism | Transaction fails or block invalid |

---

## 2. Validation Flow Summary

### 2.1 Complete Validation Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           FULL VALIDATION PIPELINE                                      │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  External Input (Transaction / Block)
           │
           ▼
  ┌─────────────────┐
  │    RECEIVE      │  Network layer receives bytes
  │                 │  • Connection authorized
  │                 │  • Rate limits checked
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │    DECODE       │  Deserialize message
  │                 │  • Format validation
  │                 │  • Size limits
  │                 │  • Version compatibility
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  PRE-CHECKS     │  Syntactic validation
  │                 │  • Required fields present
  │                 │  • Value bounds
  │                 │  • Chain ID match
  └────────┬────────┘
           │
           ├─────────────────────────────────────────────────────────┐
           │ (Transaction Path)                                      │ (Block Path)
           ▼                                                         ▼
  ┌─────────────────┐                                       ┌─────────────────┐
  │    MEMPOOL      │                                       │   CONSENSUS     │
  │   ADMISSION     │                                       │   PROPOSAL      │
  │                 │                                       │   VALIDATION    │
  │  • Signature    │                                       │                 │
  │  • Nonce        │                                       │  • Header valid │
  │  • Balance      │                                       │  • Parent exists│
  │  • Gas price    │                                       │  • Slot correct │
  │  • Pool limits  │                                       │  • Proposer OK  │
  └────────┬────────┘                                       └────────┬────────┘
           │                                                         │
           ▼                                                         ▼
  ┌─────────────────┐                                       ┌─────────────────┐
  │    GOSSIP       │                                       │     BLOCK       │
  │    RELAY        │                                       │   VALIDATION    │
  │   VALIDATION    │                                       │                 │
  │                 │                                       │  • Tx root      │
  │  • Not seen     │                                       │  • All txs valid│
  │  • Propagation  │                                       │  • PoUW receipts│
  │    rules        │                                       │  • Signatures   │
  └────────┬────────┘                                       └────────┬────────┘
           │                                                         │
           │                                                         ▼
           │                                                ┌─────────────────┐
           │                                                │   EXECUTION     │
           │                                                │   VALIDATION    │
           │                                                │                 │
           │                                                │  • Execute txs  │
           │                                                │  • State diff   │
           │                                                │  • Gas metering │
           │                                                │  • Invariants   │
           │                                                └────────┬────────┘
           │                                                         │
           │                                                         ▼
           │                                                ┌─────────────────┐
           │                                                │    RECEIPT      │
           │                                                │   VALIDATION    │
           │                                                │                 │
           │                                                │  • PoUW proofs  │
           │                                                │  • Signatures   │
           │                                                │  • Task IDs     │
           │                                                │  • Score calc   │
           │                                                └────────┬────────┘
           │                                                         │
           │                                                         ▼
           │                                                ┌─────────────────┐
           │                                                │    FINALITY     │
           │                                                │  VERIFICATION   │
           │                                                │                 │
           │                                                │  • State root   │
           │                                                │  • Receipts root│
           │                                                │  • Checkpoint   │
           └────────────────────────────────────────────────┤  • Attestations │
                                                            └────────┬────────┘
                                                                     │
                                                                     ▼
                                                              ┌─────────────┐
                                                              │  FINALIZED  │
                                                              └─────────────┘
```

### 2.2 Stage Descriptions

| Stage | Layer | Input | Output | Failure Action |
|-------|-------|-------|--------|----------------|
| **Receive** | Network | Raw bytes | Connection context | Drop connection |
| **Decode** | Network | Bytes | Typed message | Reject message |
| **Pre-Checks** | Network | Message | Validated message | Reject message |
| **Mempool Admission** | Mempool | Transaction | Pending tx | Reject transaction |
| **Gossip Relay** | Network | Pending tx | Propagation | Skip relay |
| **Consensus Proposal** | Consensus | Block | Candidate block | Reject block |
| **Block Validation** | Consensus | Candidate | Valid block | Reject block |
| **Execution** | Execution | Valid block | State diff | Reject block |
| **Receipt Validation** | Execution | PoUW receipts | Verified receipts | Reject receipts |
| **Finality** | Consensus | Executed block | Final block | Pending |

---

## 3. Core Validation Rules Table

### 3.1 Signature Validation

| Rule | Condition | Error Code |
|------|-----------|------------|
| **SIG-001** | Signature bytes are well-formed (64 or 65 bytes) | `MalformedSignature` |
| **SIG-002** | Recovery ID valid (0, 1, 27, 28) | `InvalidRecoveryId` |
| **SIG-003** | Recovered signer matches `tx.sender` | `SignerMismatch` |
| **SIG-004** | Signature scheme matches chain config | `UnsupportedScheme` |
| **SIG-005** | Block proposer signature valid | `InvalidProposerSig` |
| **SIG-006** | Attestation signatures valid | `InvalidAttestation` |
| **SIG-007** | PoUW receipt provider signature valid | `InvalidReceiptSig` |

### 3.2 Replay Protection

| Rule | Condition | Error Code |
|------|-----------|------------|
| **RPL-001** | `tx.chain_id == CHAIN_ID` | `WrongChainId` |
| **RPL-002** | `tx.nonce == account.nonce` | `InvalidNonce` |
| **RPL-003** | Transaction hash not in recent history | `DuplicateTransaction` |
| **RPL-004** | Block not already processed | `DuplicateBlock` |

### 3.3 Nonce Correctness

| Rule | Condition | Error Code |
|------|-----------|------------|
| **NCE-001** | `tx.nonce >= account.nonce` | `NonceTooLow` |
| **NCE-002** | `tx.nonce == account.nonce` for execution | `NonceGap` |
| **NCE-003** | `tx.nonce <= account.nonce + MAX_PENDING` | `NonceTooHigh` |
| **NCE-004** | Post-execution: `account.nonce' == account.nonce + 1` | `NonceNotIncremented` |

### 3.4 Gas & Fees Rules

| Rule | Condition | Error Code |
|------|-----------|------------|
| **GAS-001** | `tx.gas_limit >= INTRINSIC_GAS` | `GasLimitTooLow` |
| **GAS-002** | `tx.gas_limit <= BLOCK_GAS_LIMIT` | `GasLimitTooHigh` |
| **GAS-003** | `tx.gas_price >= MIN_GAS_PRICE` | `GasPriceTooLow` |
| **GAS-004** | `account.balance >= tx.gas_limit * tx.gas_price` | `InsufficientBalance` |
| **GAS-005** | `gas_used <= tx.gas_limit` during execution | `OutOfGas` |
| **GAS-006** | `Σ(tx.gas_limit) <= BLOCK_GAS_LIMIT` | `BlockGasExceeded` |
| **GAS-007** | Refund: `refund <= gas_used / 2` | `RefundExceeded` |

### 3.5 State Integrity Rules

| Rule | Condition | Error Code |
|------|-----------|------------|
| **STT-001** | `account.balance >= 0` always | `NegativeBalance` |
| **STT-002** | `account.nonce` monotonically increasing | `NonceRegression` |
| **STT-003** | `Σ(balances) == TOTAL_SUPPLY` (epoch check) | `SupplyMismatch` |
| **STT-004** | `computed_state_root == block.state_root` | `StateRootMismatch` |
| **STT-005** | `computed_receipts_root == block.receipts_root` | `ReceiptsRootMismatch` |
| **STT-006** | `computed_tx_root == block.transactions_root` | `TxRootMismatch` |

### 3.6 Cross-Module Validation

| Rule | Modules | Condition | Error Code |
|------|---------|-----------|------------|
| **XMD-001** | Network→Mempool | Tx passes format checks | `InvalidFormat` |
| **XMD-002** | Mempool→Consensus | Tx available for inclusion | `TxNotFound` |
| **XMD-003** | Consensus→Execution | Block header valid | `InvalidHeader` |
| **XMD-004** | Execution→Storage | State diff applicable | `StateApplyFailed` |
| **XMD-005** | Consensus→Network | Block propagation authorized | `PropagationDenied` |
| **XMD-006** | Compute→Consensus | PoUW receipts valid | `InvalidPoUWReceipt` |

### 3.7 Compute Receipt Validation [V2]

| Rule | Condition | Error Code |
|------|-----------|------------|
| **CMP-001** | `receipt.task_id` exists in task registry | `TaskNotFound` |
| **CMP-002** | `receipt.provider` is registered | `ProviderNotRegistered` |
| **CMP-003** | `receipt.output_hash` matches verified result | `OutputMismatch` |
| **CMP-004** | `receipt.signature` valid from provider | `InvalidProviderSig` |
| **CMP-005** | `receipt.verification_status == Verified` | `UnverifiedReceipt` |
| **CMP-006** | `receipt.pouw_score` within bounds | `ScoreOutOfBounds` |
| **CMP-007** | Receipt not already processed | `DuplicateReceipt` |
| **CMP-008** | ZK proof valid (future) | `InvalidZKProof` |

---

## 4. Failure Modes

### 4.1 Signature Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `InvalidSignature` | `E1001` | Signature verification failed | Reject transaction |
| `MalformedSignature` | `E1002` | Signature bytes malformed | Reject transaction |
| `SignerMismatch` | `E1003` | Recovered signer ≠ claimed sender | Reject transaction |
| `UnsupportedScheme` | `E1004` | Unknown signature algorithm | Reject transaction |

### 4.2 State Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `StateInvariantViolation` | `E2001` | State invariant broken | Reject block, investigate |
| `NegativeBalance` | `E2002` | Balance would go negative | Reject transaction |
| `StateRootMismatch` | `E2003` | Computed root ≠ claimed root | Reject block |
| `NonceRegression` | `E2004` | Nonce decreased | Reject block |

### 4.3 Gas Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `OutOfGas` | `E3001` | Execution exceeded gas limit | Fail transaction, consume gas |
| `GasLimitTooLow` | `E3002` | Gas limit below intrinsic cost | Reject transaction |
| `GasPriceTooLow` | `E3003` | Gas price below minimum | Reject transaction |
| `BlockGasExceeded` | `E3004` | Block gas limit exceeded | Reject block |

### 4.4 Serialization Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `DeserializationError` | `E4001` | Failed to decode message | Drop message |
| `InvalidEncoding` | `E4002` | Invalid RLP/SSZ encoding | Drop message |
| `UnexpectedFormat` | `E4003` | Wrong message type | Drop message |
| `OversizedMessage` | `E4004` | Message exceeds size limit | Drop message |

### 4.5 Consensus Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `ConsensusMismatch` | `E5001` | Block violates consensus rules | Reject block |
| `InvalidProposer` | `E5002` | Wrong proposer for slot | Reject block |
| `InvalidAttestation` | `E5003` | Attestation verification failed | Ignore attestation |
| `ForkChoiceViolation` | `E5004` | Block not on canonical chain | Handle reorg |
| `FinalityViolation` | `E5005` | Attempt to revert finalized block | Reject, alert |

### 4.6 Compute Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `ComputeReceiptInvalid` | `E6001` | PoUW receipt verification failed | Reject receipt, slash |
| `TaskNotFound` | `E6002` | Referenced task doesn't exist | Reject receipt |
| `ProviderNotRegistered` | `E6003` | Unknown compute provider | Reject receipt |
| `OutputMismatch` | `E6004` | Result doesn't match verification | Slash provider |

### 4.7 Header Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `HeaderMismatch` | `E7001` | Header fields inconsistent | Reject block |
| `InvalidParent` | `E7002` | Parent block not found | Request sync |
| `InvalidTimestamp` | `E7003` | Timestamp out of bounds | Reject block |
| `InvalidBlockNumber` | `E7004` | Block number not sequential | Reject block |

### 4.8 Checkpoint Failures

| Error | Code | Description | Recovery |
|-------|------|-------------|----------|
| `CheckpointFailure` | `E8001` | Checkpoint verification failed | Re-sync from earlier |
| `CheckpointMissing` | `E8002` | Required checkpoint not found | Request from peers |
| `CheckpointCorrupted` | `E8003` | Checkpoint data corrupted | Re-download |
| `CheckpointTooOld` | `E8004` | Checkpoint beyond retention | Use newer checkpoint |

---

## 5. Guarantees

### 5.1 Safety

Successful validation guarantees that:
- No invalid state transitions are applied
- No double-spending is possible
- No unauthorized actions are executed
- All cryptographic proofs are verified
- Consensus rules are enforced

```
SAFETY INVARIANT:
  ∀ block B: valid(B) ⟹ safe_state_transition(B)
  
  Where safe_state_transition means:
  • No balance goes negative
  • No nonce regresses
  • No signature is forged
  • No consensus rule is violated
```

### 5.2 Determinism

Successful validation guarantees that:
- Given identical inputs, all nodes produce identical outputs
- No non-deterministic operations are executed
- State transitions are reproducible
- Execution order is canonical

```
DETERMINISM INVARIANT:
  ∀ node N1, N2:
    state(N1) == state(N2) ∧ block(B) ⟹
    execute(N1, B) == execute(N2, B)
```

### 5.3 Reproducibility

Successful validation guarantees that:
- Any node can replay the chain from genesis
- Historical state is verifiable
- Merkle proofs are valid for any committed state
- Checkpoints accurately represent state

```
REPRODUCIBILITY INVARIANT:
  ∀ block B at height H:
    replay(genesis, blocks[0..H]) == state(B)
```

### 5.4 State Invariants

| Invariant | Description | Check Frequency |
|-----------|-------------|-----------------|
| **Balance Non-Negative** | `∀ account: balance ≥ 0` | Every transaction |
| **Nonce Monotonic** | `∀ account: nonce' ≥ nonce` | Every transaction |
| **Supply Conservation** | `Σ balances == TOTAL_SUPPLY` | Every epoch |
| **Root Consistency** | `merkle_root(state) == block.state_root` | Every block |

### 5.5 Mempool Consistency

Successful validation guarantees that:
- Only valid transactions enter the pool
- Duplicate transactions are rejected
- Evicted transactions can be re-submitted
- Pool state is consistent across operations

```
MEMPOOL INVARIANT:
  ∀ tx in mempool:
    valid_signature(tx) ∧
    nonce_valid(tx) ∧
    balance_sufficient(tx)
```

### 5.6 Consensus Correctness

Successful validation guarantees that:
- Only eligible proposers create blocks
- Fork choice follows heaviest chain
- Finalized blocks are never reverted
- Attestations are from valid validators

```
CONSENSUS INVARIANT:
  ∀ finalized block B:
    ¬∃ valid chain C: B ∉ C
    (Finalized blocks cannot be removed from canonical chain)
```

### 5.7 Execution Integrity

Successful validation guarantees that:
- All transactions execute correctly or fail gracefully
- Gas is properly metered and charged
- State diffs are correctly applied
- Receipts accurately reflect execution

```
EXECUTION INVARIANT:
  ∀ tx in block:
    execute(tx) produces (state_diff, receipt)
    where:
      apply(state, state_diff) == new_state
      receipt.status reflects execution outcome
```

### 5.8 Compute Receipt Soundness (PoUW)

Successful validation guarantees that:
- Compute results are verified
- Provider signatures are authentic
- PoUW scores are correctly calculated
- Invalid results are rejected and slashed

```
POUW INVARIANT:
  ∀ receipt R in block:
    verified(R.result) ∧
    valid_signature(R.provider, R) ∧
    score(R) ∈ valid_range
```

---

## 6. Developer Notes

### 6.1 Validation Scripts and Tools

#### Rust Test Commands

```bash
# Run all validation tests
cargo test --workspace --all-features

# Run specific validation module tests
cargo test -p runtime validation::
cargo test -p consensus validation::
cargo test -p network validation::

# Run with verbose output
cargo test --workspace -- --nocapture

# Run validation benchmarks
cargo bench --workspace validation
```

#### CI/CD Checks

```bash
# Full validation pipeline (run before PR)
./scripts/check.sh      # Linux/macOS
./scripts/check.ps1     # Windows

# Individual checks
cargo fmt --all -- --check          # Format validation
cargo clippy --workspace -- -D warnings  # Lint validation
cargo build --workspace             # Compilation validation
cargo test --workspace              # Test validation
```

#### Clippy Lints for Validation

```toml
# .clippy.toml - validation-relevant lints
warn = [
    "clippy::unwrap_used",          # Avoid panics in validation
    "clippy::expect_used",          # Explicit error handling
    "clippy::integer_arithmetic",   # Check for overflow
    "clippy::indexing_slicing",     # Bounds checking
]
```

### 6.2 Debug Strategy for Failures

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           VALIDATION FAILURE DEBUG FLOW                                 │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  Validation Failure
         │
         ▼
  ┌─────────────────┐
  │ Identify Layer  │  Network? Mempool? Consensus? Execution?
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │ Check Error Code│  E1xxx = Signature, E2xxx = State, etc.
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │ Examine Input   │  Log transaction/block bytes
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │ Trace Execution │  Enable debug logging, step through
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │ Compare States  │  Pre-state vs expected vs actual
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │ Verify Determin │  Run same input on multiple nodes
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │ Check Invariants│  Run invariant assertions
  └─────────────────┘
```

#### Debug Logging Levels

| Level | Use Case |
|-------|----------|
| `ERROR` | Validation failures that reject input |
| `WARN` | Suspicious but non-fatal conditions |
| `INFO` | Validation stage transitions |
| `DEBUG` | Detailed validation checks |
| `TRACE` | Byte-level inspection |

#### Common Debug Commands

```bash
# Enable validation debug logging
RUST_LOG=validation=debug cargo run

# Trace specific validation module
RUST_LOG=runtime::validation=trace cargo run

# Capture validation failure details
RUST_LOG=error cargo run 2>&1 | grep -E "E[0-9]{4}"
```

### 6.3 Cross-Reference Documents

| Document | Relevance to Validation |
|----------|-------------------------|
| [consensus_validation.md](./consensus_validation.md) | Block and proposer validation rules |
| [execution_engine_overview.md](./execution_engine_overview.md) | Transaction execution validation |
| [state_machine_validation.md](./state_machine_validation.md) | State transition invariants |
| [block_validation_pipeline.md](./block_validation_pipeline.md) | Block verification stages |
| [mempool_overview.md](./mempool_overview.md) | Transaction admission rules |
| [compute_engine_overview.md](./compute_engine_overview.md) | PoUW receipt validation |
| [error_categories.md](./error_categories.md) | Complete error taxonomy |
| [security_model.md](./security_model.md) | Security implications of validation |

### 6.4 Validation Module Locations

```
src/
├── network/
│   └── validation/
│       ├── message.rs      # Message format validation
│       ├── peer.rs         # Peer authorization
│       └── rate_limit.rs   # Rate limiting
│
├── runtime/
│   └── validation/
│       ├── transaction.rs  # Transaction validation
│       ├── state.rs        # State invariants
│       ├── gas.rs          # Gas metering
│       └── receipt.rs      # Receipt validation
│
├── pow/ (consensus)
│   └── validation/
│       ├── block.rs        # Block header validation
│       ├── proposer.rs     # Proposer eligibility
│       ├── attestation.rs  # Attestation verification
│       └── pouw.rs         # PoUW receipt validation
│
└── crypto/
    └── validation/
        ├── signature.rs    # Signature verification
        └── hash.rs         # Hash validation
```

---

## Appendix: Validation Rule Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                        VALIDATION RULES QUICK REFERENCE                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│  SIGNATURE (SIG-xxx)         │  STATE (STT-xxx)           │  GAS (GAS-xxx)             │
│  ─────────────────           │  ────────────────          │  ────────────              │
│  001: Well-formed            │  001: Balance ≥ 0          │  001: Limit ≥ intrinsic    │
│  002: Recovery ID            │  002: Nonce monotonic      │  002: Limit ≤ block max    │
│  003: Signer match           │  003: Supply conserved     │  003: Price ≥ minimum      │
│  004: Scheme supported       │  004: State root match     │  004: Balance covers fee   │
│  005: Proposer sig           │  005: Receipts root        │  005: Used ≤ limit         │
│  006: Attestation sig        │  006: Tx root              │  006: Block total ≤ max    │
│  007: Provider sig           │                            │  007: Refund capped        │
│                              │                            │                            │
│  REPLAY (RPL-xxx)            │  NONCE (NCE-xxx)           │  COMPUTE (CMP-xxx)         │
│  ────────────────            │  ──────────────            │  ─────────────────         │
│  001: Chain ID               │  001: Nonce ≥ current      │  001: Task exists          │
│  002: Nonce exact            │  002: Nonce == current     │  002: Provider registered  │
│  003: Not duplicate tx       │  003: Nonce ≤ max pending  │  003: Output matches       │
│  004: Not duplicate block    │  004: Nonce incremented    │  004: Signature valid      │
│                              │                            │  005: Status verified      │
│                              │                            │  006: Score in bounds      │
│                              │                            │  007: Not duplicate        │
│                              │                            │  008: ZK proof valid [V2]  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

*This document is the canonical validation specification for Mbongo Chain. All validation implementations must conform to these rules.*

