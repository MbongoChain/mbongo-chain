execution_engine.md — Canonical Version v1.0
# Mbongo Chain — Execution Engine  
Status: Canonical  
Version: v1.0

The execution engine defines how Mbongo Chain processes transactions, applies state transitions, executes PoS staking logic, validates PoUW compute results, and integrates AIDA-based fee regulation into the runtime.

The engine is designed around four principles:

1. **Determinism** (execution must be fully reproducible)
2. **Modularity** (WASM runtime + native modules)
3. **Safety by Construction** (Rust + strict resource bounds)
4. **Compute-Native Architecture** (PoUW integration)

This document specifies the canonical execution architecture for Mbongo Chain.

---

# 1. Overview

The execution engine is responsible for:

- transaction validation  
- gas accounting  
- state transition application  
- staking logic  
- slashing and reward distribution  
- PoUW receipt verification  
- AIDA fee adjustments  
- WASM smart contract execution (future)  
- Merkle root computation  

The execution engine operates **inside the node**, in a deterministic Rust environment, independent of networking or block production.

---

# 2. Deterministic State Machine

Mbongo Chain uses a deterministic, Rust-based state machine with:

- unique state root per block  
- Sparse Merkle Tree (SMT) structure  
- strict ordering of transactions  
- fixed arithmetic semantics  
- strict gas rules  
- replay protection  

Each block execution produces:



(state_root, receipts_root, pouw_receipts_root)


These roots are included in block headers and validated by all nodes.

---

# 3. Transaction Model

Transactions consist of:

- sender  
- signature  
- nonce  
- gas_limit  
- gas_price  
- payload (module call, PoUW job submission, staking action, etc.)  

The engine processes transactions in the following steps:

1. **Signature verification**  
2. **Nonce validation**  
3. **Balance check**  
4. **Gas pre-charge**  
5. **Module dispatch**  
6. **State transition**  
7. **Refund unused gas**  

Transactions that fail at any stage are rejected.

---

# 4. Gas & Fee Model

Mbongo Chain uses a **dual-layer gas model**:

| Layer | Description | Used for |
|-------|-------------|-----------|
| **E-Gas** | Execution Gas | Normal state transitions (accounts, staking, governance, etc.) |
| **C-Gas** | Compute Gas | PoUW job execution & verification |

Both types of gas are tracked independently.

---

## 4.1 E-Gas (Execution Gas)

E-Gas covers:

- state reads/writes  
- signature verification  
- hashing  
- module calls  
- WASM execution (future)  

E-Gas is priced based on:

- deterministic cost tables  
- block capacity (`E-Gas limit per block`)  
- AIDA-regulated base fee multiplier  

### E-Gas Formula



effective_base_fee = base_fee * AIDA.base_fee_multiplier
total_fee = gas_used * effective_base_fee


AIDA does **not** change gas tables —  
only the **multiplier**, within safe boundaries:



0.5 ≤ base_fee_multiplier ≤ 3.0


---

## 4.2 C-Gas (Compute Gas)

C-Gas applies to:

- AI inference  
- rendering jobs  
- simulation workloads  
- ZK proof generation  
- any PoUW GPU/TPU/NPU task  

C-Gas is measured as:



C-Gas = compute_time * hardware_weight


Where `hardware_weight` is derived from:

- GPU VRAM  
- FLOPS capability  
- compute score based on PoC module  

C-Gas pricing uses:



compute_fee = C-Gas * AIDA.pouw_multiplier


AIDA multiplier range:



0.8 ≤ pouw_multiplier ≤ 1.2


This maintains predictable cost for compute.

---

# 5. AIDA Integration in Execution

AIDA influences **fee economics**, not execution logic.

## AIDA can modify:

- base_fee_multiplier  
- burn_rate  
- PoUW multiplier  

## AIDA cannot modify:

- runtime execution  
- module behavior  
- gas tables  
- PoUW verification rules  
- WASM VM behavior  
- block state transitions  
- reward distribution algorithms  

This ensures safety and determinism.

---

# 6. Module Architecture

The runtime is modular and supports native Rust modules:

- **accounts**  
- **staking**  
- **governance**  
- **PoUW**  
- **AIDA**  
- **compute marketplace**  
- **reward distribution**  
- **slashing**  
- **future WASM smart contract module**

Each module implements:



fn validate(tx: &Transaction) -> Result<()>;
fn execute(tx: &Transaction, state: &mut State) -> Result<()>;


Modules are deterministic and isolated.

---

# 7. PoUW Execution Pipeline

The execution engine integrates PoUW through:

1. **Job Submission**
   - user submits a compute task  
   - C-Gas pre-charged  
   - job added to PoUW marketplace  

2. **Compute Execution**
   - compute node retrieves job  
   - executes GPU/TPU/CPU workload  
   - generates a receipt  

3. **Receipt Verification**
   - validator verifies the receipt deterministically  
   - redundant verification optional for high-value tasks  

4. **Inclusion**
   - verified receipt included in block  
   - rewards distributed (PoUW share of block reward + compute fees)  

5. **Fraud Proofs (Optional)**
   - challengers submit fraud proofs  
   - faulty compute nodes slashed  
   - job re-executed or reassigned  

---

# 8. Reward Distribution Execution

The execution engine handles:

- PoS rewards  
- PoUW rewards  
- fee distribution  
- burn (AIDA-regulated)  

At block execution:



block_reward = emission_per_block
pos_reward = block_reward * 0.50
pouw_reward = block_reward * 0.50
burn_amount = total_fees * AIDA.burn_rate
validator_fee = total_fees - burn_amount


Burn is deterministic and included in block state.

---

# 9. WASM Execution Environment (Future)

WASM support is modular and isolated:

- `no_std` Rust contracts  
- deterministic gas metering  
- bounded memory  
- no syscalls  
- deterministic host functions  
- versioned VM for upgrades  

AIDA has **zero influence** on WASM logic.

---

# 10. Error Handling & Safety

Execution halts if:

- gas runs out  
- signature invalid  
- insufficient balance  
- module throws an error  

PoUW receipts that fail validation:

- are discarded  
- generate slashing penalties  
- do not reward compute nodes  

Execution is guaranteed deterministic:

- no floating-point math  
- no hardware-specific ops  
- fixed byte order  
- fixed hashing algorithms  

---

# 11. Block Execution Summary

Each block executes:

1. Apply AIDA parameter updates  
2. Process transactions in order  
3. Execute PoS module  
4. Execute PoUW receipt verification  
5. Apply burns and reward distribution  
6. Compute new state root  
7. Commit storage  

The same inputs → same outputs → same state.

---

# 12. Security Guarantees

The execution engine ensures:

- deterministic state transitions  
- non-bypassable PoUW verification  
- strict runtime isolation  
- bounded fee manipulation via AIDA  
- resistance to economic attacks  
- predictable block economics  

It is designed for:

- high throughput  
- verifiable compute  
- long-term decentralization  
- stable economic behavior  

---

# 13. Summary

The Mbongo Chain Execution Engine:

- is Rust-native  
- deterministic  
- modular  
- compute-aware  
- integrates PoS + PoUW  
- enforces fixed supply  
- supports AIDA for bounded dynamic fees  
- integrates dual-layer gas (E-Gas + C-Gas)  
- includes verifiable GPU compute  
- prepares for future WASM smart contracts  

It is the core of Mbongo Chain’s **verifiable compute architecture**.
