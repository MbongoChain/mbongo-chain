# Mbongo Chain — Runtime Architecture

This document describes the architecture of the Mbongo Chain runtime, the core component responsible for state transitions and transaction execution.

---

## 1. Introduction

### What is the Runtime?

The **runtime** is the deterministic state machine at the heart of Mbongo Chain. It defines the rules for how transactions modify blockchain state and ensures that all nodes reach identical conclusions when processing the same inputs.

### Role in State Transition and Execution

The runtime is responsible for:

| Function | Description |
|----------|-------------|
| **State Definition** | Define what constitutes valid blockchain state |
| **Transition Rules** | Specify how transactions modify state |
| **Validation Logic** | Determine which transactions are valid |
| **Execution Dispatch** | Route transactions to appropriate handlers |

### Runtime vs Node vs Execution Engine

| Component | Responsibility | Scope |
|-----------|---------------|-------|
| **Node** | Orchestration, networking, consensus | Infrastructure |
| **Runtime** | State machine logic, transition rules | Business logic |
| **Execution Engine** | Low-level transaction processing | Compute |

```
┌──────────────────────────────────────────────────────────────────┐
│                            NODE                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                         RUNTIME                            │  │
│  │  ┌──────────────────────────────────────────────────────┐  │  │
│  │  │                  EXECUTION ENGINE                    │  │  │
│  │  └──────────────────────────────────────────────────────┘  │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

The **node** provides the environment, the **runtime** defines the rules, and the **execution engine** performs the actual computation.

---

## 2. Design Goals

### Determinism

Every node must produce identical outputs given identical inputs:

- Same transaction + same state = same result
- No external dependencies that vary between nodes
- Bit-for-bit reproducible execution

### Safety

The runtime enforces invariants that protect the system:

- Balance conservation (no creation or destruction of value)
- Authorization (only valid signatures can spend)
- Ordering (nonce-based transaction sequencing)

### Modularity

The runtime is composed of independent modules:

- Each module handles specific functionality
- Modules can be upgraded independently
- Clear interfaces between modules

### Verifiability for PoUW

The runtime supports Proof of Useful Work verification:

- Compute proofs can be validated on-chain
- Verification is deterministic and efficient
- Results are cryptographically committed

---

## 3. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              RUNTIME                                    │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          TRANSACTION INPUT                              │
│                                                                         │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │
│  │ Sender  │  │  Nonce  │  │ Action  │  │  Data   │  │   Sig   │       │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘       │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           PRE-VALIDATION                                │
│                                                                         │
│  • Signature verification                                               │
│  • Nonce check                                                          │
│  • Balance check                                                        │
│  • Format validation                                                    │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         EXECUTION ENGINE                                │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      EXECUTION CONTEXT                          │   │
│  │  • Caller address                                               │   │
│  │  • Gas limit / remaining                                        │   │
│  │  • Block context (height, timestamp)                            │   │
│  │  • State accessor                                               │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      MODULE DISPATCH                            │   │
│  │  • Transfer module                                              │   │
│  │  • Staking module (planned)                                     │   │
│  │  • Compute module                                               │   │
│  │  • Governance module (planned)                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          STATE TRANSITION                               │
│                                                                         │
│  ┌─────────────────┐         ┌─────────────────┐                       │
│  │   Pre-State     │ ──────▶ │   Post-State    │                       │
│  │   (Root: 0x...)│         │   (Root: 0x...)│                       │
│  └─────────────────┘         └─────────────────┘                       │
└───────────────────────────────────┬─────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              RECEIPT                                    │
│                                                                         │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐       │
│  │ Status  │  │Gas Used │  │  Logs   │  │ Output  │  │  Root   │       │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘       │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Core Components

### 4.1 State Machine

The state machine defines valid states and transitions:

```rust
// State machine interface (conceptual)
pub trait StateMachine {
    /// Apply a transaction to the current state
    fn apply_transaction(
        &mut self,
        tx: &Transaction,
        context: &ExecutionContext,
    ) -> Result<Receipt, RuntimeError>;
    
    /// Validate a transaction without executing
    fn validate_transaction(
        &self,
        tx: &Transaction,
    ) -> Result<(), ValidationError>;
    
    /// Get the current state root
    fn state_root(&self) -> Hash;
}
```

### 4.2 Transaction Format

Transactions contain all information needed for execution:

| Field | Type | Description |
|-------|------|-------------|
| `sender` | Address | Transaction originator |
| `nonce` | u64 | Sequence number for ordering |
| `action` | Action | Operation to perform |
| `data` | Bytes | Action-specific payload |
| `gas_limit` | u64 | Maximum compute units |
| `gas_price` | u128 | Fee per compute unit |
| `signature` | Signature | Authorization proof |

### 4.3 Execution Context

The execution context provides environment information:

```rust
// Execution context (conceptual)
pub struct ExecutionContext {
    /// Transaction sender
    pub caller: Address,
    /// Remaining gas
    pub gas_remaining: u64,
    /// Current block height
    pub block_height: u64,
    /// Current block timestamp
    pub block_timestamp: u64,
    /// State accessor
    pub state: StateAccessor,
    /// Depth of call stack
    pub depth: u32,
}
```

### 4.4 Gas / Metering

*Status: Placeholder implementation*

Gas metering controls resource consumption:

| Operation | Gas Cost |
|-----------|----------|
| Base transaction | 21,000 |
| Storage write | 20,000 |
| Storage read | 2,100 |
| Hash computation | 30 |
| Signature verify | 3,000 |

Future implementation will include:

- Dynamic gas pricing
- Compute-weighted metering for PoUW
- Gas refunds for storage cleanup

### 4.5 Error Handling Model

Errors are categorized by severity and recoverability:

| Category | Examples | Behavior |
|----------|----------|----------|
| **Validation** | Invalid signature, bad nonce | Transaction rejected |
| **Execution** | Out of gas, arithmetic overflow | Transaction reverted |
| **System** | State corruption, invariant violation | Node halt |

```rust
// Error types (conceptual)
pub enum RuntimeError {
    // Validation errors (transaction rejected)
    InvalidSignature,
    InvalidNonce { expected: u64, got: u64 },
    InsufficientBalance { required: u128, available: u128 },
    
    // Execution errors (transaction reverted)
    OutOfGas,
    ExecutionFailed(String),
    
    // System errors (critical)
    StateCorruption,
    InvariantViolation(String),
}
```

### 4.6 Runtime Modules

Current modules:

| Module | Status | Function |
|--------|--------|----------|
| **Balances** | Implemented | Token transfers and balance tracking |
| **System** | Implemented | Block metadata, nonce management |
| **Compute** | Placeholder | PoUW proof submission and verification |

Planned modules:

| Module | Status | Function |
|--------|--------|----------|
| **Staking** | Planned | Validator registration and delegation |
| **Governance** | Planned | On-chain proposals and voting |
| **Contracts** | Planned | Smart contract deployment and execution |

---

## 5. Execution Flow

### Complete Flow Diagram

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│  Input  │────▶│Validate │────▶│Dispatch │────▶│ Execute │────▶│ Commit  │
│   Tx    │     │         │     │         │     │         │     │         │
└─────────┘     └─────────┘     └─────────┘     └─────────┘     └─────────┘
                    │               │               │               │
                    ▼               ▼               ▼               ▼
               ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
               │ Reject  │    │ Module  │    │  State  │    │ Receipt │
               │   or    │    │  Lookup │    │ Changes │    │  Gen    │
               │ Accept  │    │         │    │         │    │         │
               └─────────┘    └─────────┘    └─────────┘    └─────────┘
```

### 5.1 Pre-Validation

Before execution, transactions are validated:

1. **Format Check** — Transaction structure is well-formed
2. **Signature Verification** — Signature matches sender
3. **Nonce Check** — Nonce equals expected value
4. **Balance Check** — Sender can pay maximum fees
5. **Gas Limit Check** — Gas limit within block limits

### 5.2 Dispatch to Runtime

Valid transactions are routed to the appropriate module:

```rust
// Dispatch logic (conceptual)
fn dispatch(tx: &Transaction, ctx: &ExecutionContext) -> Result<Output> {
    match tx.action {
        Action::Transfer { to, amount } => {
            balances::transfer(ctx.caller, to, amount)
        }
        Action::SubmitProof { proof } => {
            compute::submit_proof(ctx.caller, proof)
        }
        Action::Stake { validator, amount } => {
            staking::stake(ctx.caller, validator, amount)
        }
        // ... other actions
    }
}
```

### 5.3 Execution via Engine

The execution engine processes the action:

1. **Load State** — Read relevant state entries
2. **Apply Logic** — Execute module-specific rules
3. **Generate Changes** — Produce state diff
4. **Meter Gas** — Deduct gas for operations

### 5.4 State Transition Commit

After successful execution:

1. **Apply Changes** — Write state diff to state tree
2. **Update Root** — Compute new state root
3. **Deduct Fees** — Transfer gas fees to block producer

### 5.5 Receipt Generation

Each transaction produces a receipt:

```rust
// Receipt structure (conceptual)
pub struct Receipt {
    /// Execution status
    pub status: ExecutionStatus,
    /// Gas consumed
    pub gas_used: u64,
    /// Output data (if any)
    pub output: Option<Bytes>,
    /// Logs emitted
    pub logs: Vec<Log>,
    /// Post-execution state root
    pub state_root: Hash,
}
```

---

## 6. State Model

### 6.1 Global State Tree

State is organized as a Merkle Patricia Trie:

```
                    ┌───────────────┐
                    │  State Root   │
                    │   (Hash)      │
                    └───────┬───────┘
                            │
          ┌─────────────────┼─────────────────┐
          │                 │                 │
          ▼                 ▼                 ▼
    ┌───────────┐     ┌───────────┐     ┌───────────┐
    │ Account A │     │ Account B │     │ Account C │
    └─────┬─────┘     └─────┬─────┘     └─────┬─────┘
          │                 │                 │
          ▼                 ▼                 ▼
    ┌───────────┐     ┌───────────┐     ┌───────────┐
    │  Balance  │     │  Balance  │     │  Balance  │
    │  Nonce    │     │  Nonce    │     │  Nonce    │
    │  Storage  │     │  Storage  │     │  Storage  │
    └───────────┘     └───────────┘     └───────────┘
```

### 6.2 Accounts

Each account contains:

| Field | Type | Description |
|-------|------|-------------|
| `balance` | u128 | Token balance |
| `nonce` | u64 | Transaction count |
| `code_hash` | Hash | Contract code (if any) |
| `storage_root` | Hash | Storage tree root |

### 6.3 Storage Layout

Account storage is key-value based:

```
Account Storage
├── Key: 0x0000...0001 → Value: ...
├── Key: 0x0000...0002 → Value: ...
└── Key: 0x0000...0003 → Value: ...
```

### 6.4 State Access Patterns

The runtime provides controlled state access:

```rust
// State accessor interface (conceptual)
pub trait StateAccessor {
    /// Read account balance
    fn get_balance(&self, address: &Address) -> u128;
    
    /// Read account nonce
    fn get_nonce(&self, address: &Address) -> u64;
    
    /// Read storage value
    fn get_storage(&self, address: &Address, key: &Key) -> Value;
    
    /// Write balance (internal only)
    fn set_balance(&mut self, address: &Address, balance: u128);
    
    /// Write storage (internal only)
    fn set_storage(&mut self, address: &Address, key: &Key, value: Value);
}
```

---

## 7. Interaction with Execution Engine

### 7.1 Interface Definition

The runtime communicates with the execution engine through a defined interface:

```rust
// Execution engine interface (conceptual)
pub trait ExecutionEngine {
    /// Execute a single transaction
    fn execute(
        &self,
        state: &mut dyn StateAccessor,
        tx: &Transaction,
        context: &ExecutionContext,
    ) -> ExecutionResult;
    
    /// Verify a compute proof
    fn verify_proof(
        &self,
        proof: &ComputeProof,
        commitment: &Hash,
    ) -> bool;
}
```

### 7.2 Deterministic Execution Requirements

The execution engine must guarantee determinism:

| Requirement | Enforcement |
|-------------|-------------|
| No floating point | Use fixed-point arithmetic |
| No randomness | Require VRF for random values |
| No external I/O | Sandboxed execution |
| No time variance | Use block timestamp only |
| Bounded execution | Gas limits enforced |

### 7.3 PoUW Constraints

For Proof of Useful Work verification:

- **Proof Format** — Standardized proof structure
- **Verification Cost** — Verification << computation
- **Determinism** — Same proof always yields same result
- **Commitment** — Results cryptographically committed

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Compute Task   │────▶│     Proof       │────▶│   Verification  │
│  (Off-chain)    │     │  (Submitted)    │     │   (On-chain)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │
        ▼                       ▼                       ▼
   Expensive              Compact               Cheap
   (GPU hours)           (KB-sized)            (Milliseconds)
```

---

## 8. Runtime Modules

### 8.1 Module Architecture

Modules are self-contained units of functionality:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           RUNTIME                                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐    │
│  │   System    │  │  Balances   │  │   Compute   │  │   Staking   │    │
│  │   Module    │  │   Module    │  │   Module    │  │   Module    │    │
│  │             │  │             │  │             │  │  (planned)  │    │
│  │ - Blocks    │  │ - Transfer  │  │ - Submit    │  │ - Stake     │    │
│  │ - Nonces    │  │ - Balance   │  │ - Verify    │  │ - Delegate  │    │
│  │ - Params    │  │ - Reserve   │  │ - Reward    │  │ - Slash     │    │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Current Modules

| Module | Functions | Status |
|--------|-----------|--------|
| **System** | Block metadata, nonces, parameters | Implemented |
| **Balances** | Transfers, balance queries, reserves | Implemented |
| **Compute** | Proof submission, verification | Placeholder |

### 8.3 Planned Modules

| Module | Functions | Timeline |
|--------|-----------|----------|
| **Staking** | Validator management, delegation | Q2 |
| **Governance** | Proposals, voting, execution | Q3 |
| **Contracts** | WASM deployment, execution | Q4 |

### 8.4 Plug-in Model

Modules follow a standard interface for extensibility:

```rust
// Module trait (conceptual)
pub trait RuntimeModule {
    /// Module identifier
    fn name(&self) -> &str;
    
    /// Handle a call to this module
    fn dispatch(
        &self,
        state: &mut dyn StateAccessor,
        context: &ExecutionContext,
        call: &Call,
    ) -> Result<Output, ModuleError>;
    
    /// Validate a call without executing
    fn validate(&self, call: &Call) -> Result<(), ValidationError>;
}
```

---

## 9. Determinism Requirements

### 9.1 Forbidden Operations

The following operations are prohibited in runtime execution:

| Category | Forbidden | Reason |
|----------|-----------|--------|
| **System Calls** | File I/O, network, process | External state |
| **Time** | System clock, sleep | Non-reproducible |
| **Random** | rand(), /dev/random | Non-deterministic |
| **Floating Point** | IEEE 754 operations | Platform variance |
| **Threads** | Spawn, join, atomics | Race conditions |

### 9.2 No Syscalls

Runtime execution is fully sandboxed:

- No file system access
- No network access
- No process spawning
- No environment variables

### 9.3 No Randomness Without VRF

Randomness must be verifiable:

```rust
// Verifiable random function (conceptual)
pub fn get_random(seed: &Hash, domain: &[u8]) -> Hash {
    // VRF output is deterministic given seed
    // Seed derived from block hash + domain separator
    vrf_output(seed, domain)
}
```

### 9.4 No Non-Deterministic I/O

All I/O is through the state accessor:

- Reads return consistent values
- Writes are applied atomically
- No external data sources

### 9.5 Determinism Checklist

| Aspect | Requirement |
|--------|-------------|
| Arithmetic | Integer-only, checked overflow |
| Hashing | Standardized algorithms (SHA-256, Blake3) |
| Serialization | Canonical encoding (no ambiguity) |
| Ordering | Deterministic iteration order |
| Errors | Predictable error handling |

---

## 10. Future Extensions

### 10.1 WASM-Based Contracts

*Status: Planned*

WebAssembly smart contracts will enable:

- User-deployed programs
- Multiple language support (Rust, AssemblyScript)
- Deterministic execution sandbox
- Metered gas consumption

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Contract Code  │────▶│   WASM Engine   │────▶│     Output      │
│     (WASM)      │     │   (Sandboxed)   │     │    (State)      │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

### 10.2 Parallel Execution Lanes

*Status: Research*

Parallel execution will improve throughput:

- Detect independent transactions
- Execute on separate lanes
- Merge results deterministically
- Maintain serializability

```
┌─────────────────────────────────────────────────────────────────┐
│                    PARALLEL EXECUTION                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Lane 0: [Tx1] ─────────▶ [Tx4] ─────────▶                     │
│  Lane 1: [Tx2] ─────────▶ [Tx5] ─────────▶      [Merge]        │
│  Lane 2: [Tx3] ─────────▶ [Tx6] ─────────▶                     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 10.3 Zero-Knowledge Execution Support

*Status: Research*

ZK execution will enable:

- Private transactions
- Succinct state proofs
- Cross-chain verification
- Scalability via rollups

### 10.4 Additional Roadmap

- [ ] Gas optimization and refunds
- [ ] Precompiled contracts for common operations
- [ ] Cross-module calls
- [ ] Upgradeable modules via governance
- [ ] Runtime versioning and migrations

---

## Summary

The Mbongo Chain runtime is a deterministic state machine designed for safety, modularity, and verifiability. It provides the foundation for all on-chain computation, from simple transfers to complex PoUW verification.

For node-level architecture, see [Node Architecture](node_architecture.md).

For the overall system design, see [Architecture Overview](architecture_overview.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

