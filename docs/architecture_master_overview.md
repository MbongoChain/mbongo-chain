# Mbongo Chain — Architecture Master Overview

> **Document Version:** 1.0  
> **Last Updated:** November 2025  
> **Status:** Living Document  

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [System Overview](#2-system-overview)
3. [Architecture Layers](#3-architecture-layers)
4. [Data Flow Overview](#4-data-flow-overview)
5. [Node Architecture Summary](#5-node-architecture-summary)
6. [Consensus Summary](#6-consensus-summary)
7. [Execution Summary](#7-execution-summary)
8. [Storage Summary](#8-storage-summary)
9. [Message Types Summary](#9-message-types-summary)
10. [Cross-Component Relationships](#10-cross-component-relationships)
11. [Final Architecture Diagram](#11-final-architecture-diagram)

---

## 1. Purpose of This Document

This document serves as the **canonical high-level architecture reference** for the Mbongo Chain project. It provides a comprehensive technical summary that unifies all sub-systems into a coherent whole, enabling architects and engineers to understand system boundaries, data flows, and component interactions.

### Scope

This document links together the following sub-systems:

| Sub-System | Description |
|------------|-------------|
| **Consensus** | Hybrid PoS + PoUW finality mechanism |
| **Networking** | P2P gossip, peer discovery, message propagation |
| **State** | Account model, balances, contract storage |
| **Execution** | Transaction processing, state transitions |
| **PoUW** | Proof-of-Useful-Work compute coordination |
| **Nodes** | Full, Validator, Guardian, Light node types |
| **Mempool** | Transaction staging, prioritization, eviction |
| **Validation** | Signature verification, semantic checks |
| **Storage** | Persistent block store, state trie, indices |

### Audience

This document is intended for:

- **Senior Blockchain Engineers** implementing core protocol logic
- **System Architects** designing integrations and extensions
- **Security Reviewers** auditing protocol correctness
- **Open-Source Contributors** seeking context before contributing

### Conventions

- ASCII diagrams are provided for offline/terminal readability
- Component names match Rust module names where applicable
- `[FUTURE]` tags indicate planned but unimplemented features

---

## 2. System Overview

Mbongo Chain is a **Rust-native, compute-first blockchain** designed for global GPU coordination and high-throughput state execution.

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Mbongo Chain — 10-Line System Summary                                  │
├─────────────────────────────────────────────────────────────────────────┤
│  1. Rust-native implementation for memory safety and performance        │
│  2. Hybrid PoS + PoUW consensus for security and useful computation     │
│  3. Compute-first design: GPU workloads are first-class citizens        │
│  4. Global GPU coordination via Guardian nodes and PoUW scoring         │
│  5. Fast state execution through optimized transaction pipelines        │
│  6. Modular runtime enabling pluggable execution environments           │
│  7. Deterministic execution guarantees for consensus correctness        │
│  8. P2P networking via libp2p for robust peer discovery and gossip      │
│  9. Checkpoint-based finality for fast sync and state recovery          │
│ 10. Future WASM support for smart contract execution                    │
└─────────────────────────────────────────────────────────────────────────┘
```

### Design Principles

| Principle | Description |
|-----------|-------------|
| **Modularity** | Each sub-system is an independent Rust crate with explicit dependencies |
| **Determinism** | All state transitions are reproducible given the same inputs |
| **Compute-First** | GPU workloads integrated at the consensus layer via PoUW |
| **Safety** | Rust's ownership model prevents memory corruption classes |
| **Extensibility** | Runtime traits enable pluggable execution backends |

---

## 3. Architecture Layers

Mbongo Chain is organized into **six primary layers**, each with distinct responsibilities and interfaces.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         APPLICATION / CLI LAYER                         │
│                    (User commands, RPC, API interfaces)                 │
├─────────────────────────────────────────────────────────────────────────┤
│                            NODE LAYER                                   │
│           (Full Node │ Validator Node │ Guardian Node │ Light Node)     │
├─────────────────────────────────────────────────────────────────────────┤
│                          CONSENSUS LAYER                                │
│              (PoS + PoUW │ Leader Election │ Finality)                  │
├─────────────────────────────────────────────────────────────────────────┤
│                          EXECUTION LAYER                                │
│         (State Transitions │ Runtime │ WASM [FUTURE] │ GPU Paths)       │
├─────────────────────────────────────────────────────────────────────────┤
│                           MEMPOOL LAYER                                 │
│            (Transaction Queue │ Prioritization │ Eviction)              │
├─────────────────────────────────────────────────────────────────────────┤
│                         NETWORKING LAYER                                │
│             (P2P │ Gossip │ Peer Discovery │ Sync Protocol)             │
├─────────────────────────────────────────────────────────────────────────┤
│                          STORAGE LAYER                                  │
│           (Block Store │ State Trie │ Indices │ Checkpoints)            │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Networking Layer

**Module:** `network/`

Responsible for all peer-to-peer communication, including:

- **Peer Discovery:** DHT-based discovery via libp2p Kademlia
- **Gossip Protocol:** Efficient transaction and block propagation
- **Request/Response:** Direct peer queries for sync and state retrieval
- **Connection Management:** Peer scoring, banning, connection limits

```
┌──────────────────────────────────────────────────────────────┐
│                     NETWORKING LAYER                         │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│   │   libp2p    │    │   Gossip    │    │    Sync     │     │
│   │  Transport  │───▶│   Router    │───▶│  Protocol   │     │
│   └─────────────┘    └─────────────┘    └─────────────┘     │
│         │                   │                  │             │
│         ▼                   ▼                  ▼             │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│   │    Peer     │    │   Message   │    │    Block    │     │
│   │  Discovery  │    │   Handler   │    │  Requester  │     │
│   └─────────────┘    └─────────────┘    └─────────────┘     │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 3.2 Mempool Layer

**Module:** `runtime/` (mempool component)

Manages pending transactions before block inclusion:

- **Transaction Queue:** Priority-ordered pending transactions
- **Fee Prioritization:** Gas price and urgency-based ordering
- **Eviction Policy:** LRU/fee-based eviction under memory pressure
- **Duplicate Detection:** Prevents redundant transaction storage

```
┌──────────────────────────────────────────────────────────────┐
│                      MEMPOOL LAYER                           │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────────────────────────────────────────────┐   │
│   │                  Transaction Pool                    │   │
│   │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌────────┐  │   │
│   │  │ Pending │  │ Queued  │  │ Priority│  │ Evict  │  │   │
│   │  │  Queue  │  │  Queue  │  │  Heap   │  │ Policy │  │   │
│   │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬───┘  │   │
│   │       └────────────┴────────────┴────────────┘      │   │
│   └─────────────────────────────────────────────────────┘   │
│                            │                                 │
│                            ▼                                 │
│                  ┌─────────────────┐                        │
│                  │  Block Builder  │                        │
│                  └─────────────────┘                        │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 3.3 Consensus Layer

**Module:** `pow/` + consensus components

Implements the hybrid PoS + PoUW consensus mechanism:

- **Validator Set Management:** Stake-weighted validator selection
- **Leader Election:** Deterministic per-slot leader derivation
- **PoUW Integration:** Useful work proofs influence block weight
- **Fork Choice:** Heaviest-chain rule with PoUW scoring
- **Finality:** Checkpoint-based finality gadget

```
┌──────────────────────────────────────────────────────────────┐
│                     CONSENSUS LAYER                          │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌──────────────┐         ┌──────────────┐                 │
│   │   PoS Core   │◀───────▶│   PoUW Core  │                 │
│   │  (Staking)   │         │  (Compute)   │                 │
│   └──────┬───────┘         └──────┬───────┘                 │
│          │                        │                          │
│          └───────────┬───────────┘                          │
│                      ▼                                       │
│            ┌─────────────────┐                              │
│            │ Leader Election │                              │
│            └────────┬────────┘                              │
│                     ▼                                        │
│   ┌──────────────────────────────────────────────────────┐  │
│   │                   Fork Choice Rule                    │  │
│   │        (Heaviest Chain + PoUW Score Weight)          │  │
│   └──────────────────────────────────────────────────────┘  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 3.4 Execution Layer

**Module:** `runtime/`

Processes transactions and computes state transitions:

- **Transaction Execution:** Sequential/parallel tx processing
- **State Machine:** Deterministic state transition function
- **WASM Runtime:** [FUTURE] Smart contract execution
- **GPU Paths:** Optimized execution for compute workloads
- **Gas Metering:** Resource accounting and limits

```
┌──────────────────────────────────────────────────────────────┐
│                     EXECUTION LAYER                          │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌───────────────────────────────────────────────────────┐ │
│   │                  Transaction Executor                  │ │
│   └───────────────────────────┬───────────────────────────┘ │
│                               │                              │
│              ┌────────────────┼────────────────┐            │
│              ▼                ▼                ▼            │
│   ┌─────────────────┐ ┌─────────────┐ ┌─────────────────┐  │
│   │  Native Runtime │ │ WASM Engine │ │   GPU Executor  │  │
│   │   (Transfers)   │ │  [FUTURE]   │ │  (PoUW Tasks)   │  │
│   └────────┬────────┘ └──────┬──────┘ └────────┬────────┘  │
│            │                 │                  │            │
│            └─────────────────┴──────────────────┘           │
│                              │                               │
│                              ▼                               │
│                    ┌─────────────────┐                      │
│                    │  State Commit   │                      │
│                    └─────────────────┘                      │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 3.5 Storage Layer

**Module:** Storage components within `node/` and `runtime/`

Persists blockchain data durably:

- **Block Store:** Sequential block storage with fast retrieval
- **State Trie:** Merkle Patricia Trie for account/storage state
- **Index Tables:** Secondary indices for queries (txhash → block)
- **Checkpoint System:** Periodic state snapshots for fast sync

```
┌──────────────────────────────────────────────────────────────┐
│                      STORAGE LAYER                           │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│   │   Block     │    │   State     │    │   Index     │     │
│   │   Store     │    │   Trie      │    │   Tables    │     │
│   │  (RocksDB)  │    │  (Merkle)   │    │  (KV Maps)  │     │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘     │
│          │                  │                  │             │
│          └──────────────────┴──────────────────┘            │
│                             │                                │
│                             ▼                                │
│                  ┌─────────────────────┐                    │
│                  │  Checkpoint Manager │                    │
│                  │   (State Snapshots) │                    │
│                  └─────────────────────┘                    │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### 3.6 Node Layer

**Module:** `node/`

Orchestrates all components into cohesive node implementations:

- **Full Node:** Complete chain history, full validation
- **Validator Node:** Full Node + block production + staking
- **Guardian Node:** Validator + PoUW compute coordination
- **Light Node:** [FUTURE] Header-only verification

```
┌──────────────────────────────────────────────────────────────┐
│                       NODE LAYER                             │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│   ┌─────────────┐ ┌─────────────┐ ┌─────────────┐           │
│   │  Full Node  │ │  Validator  │ │  Guardian   │           │
│   │             │ │    Node     │ │    Node     │           │
│   └──────┬──────┘ └──────┬──────┘ └──────┬──────┘           │
│          │               │               │                   │
│          │    ┌──────────┴──────────┐    │                   │
│          │    │                     │    │                   │
│          ▼    ▼                     ▼    ▼                   │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              Node Orchestration Engine              │   │
│   │  (Networking + Consensus + Execution + Storage)     │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                              │
│   ┌─────────────┐                                           │
│   │ Light Node  │ [FUTURE]                                  │
│   │ (Headers)   │                                           │
│   └─────────────┘                                           │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 4. Data Flow Overview

This section describes the complete lifecycle of a transaction from submission to final state commitment.

### 4.1 Transaction Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    TRANSACTION LIFECYCLE FLOW                           │
└─────────────────────────────────────────────────────────────────────────┘

  ┌──────────┐
  │  Client  │
  │  (User)  │
  └────┬─────┘
       │ 1. Submit Transaction
       ▼
  ┌──────────────┐
  │   RPC/API    │
  │   Endpoint   │
  └──────┬───────┘
         │ 2. Initial Validation (signature, format, nonce)
         ▼
  ┌──────────────┐
  │   Mempool    │◀──────────────────────────────────────┐
  │              │                                        │
  └──────┬───────┘                                        │
         │ 3. Gossip to Peers                             │
         ▼                                                │
  ┌──────────────┐                                        │
  │   Network    │────────────▶ Peer Nodes ───────────────┘
  │   Gossip     │              (receive & validate)
  └──────┬───────┘
         │ 4. Block Producer Selects Transactions
         ▼
  ┌──────────────┐
  │    Block     │
  │   Builder    │
  └──────┬───────┘
         │ 5. Propose Block
         ▼
  ┌──────────────┐
  │  Consensus   │
  │  (PoS+PoUW)  │
  └──────┬───────┘
         │ 6. Block Accepted (fork choice)
         ▼
  ┌──────────────┐
  │  Execution   │
  │   Engine     │
  └──────┬───────┘
         │ 7. Execute Transactions → State Transitions
         ▼
  ┌──────────────┐
  │    State     │
  │   Update     │
  └──────┬───────┘
         │ 8. Compute New State Root
         ▼
  ┌──────────────┐
  │   Storage    │
  │   Commit     │
  └──────┬───────┘
         │ 9. Persist Block + State
         ▼
  ┌──────────────┐
  │   Finality   │
  │ (Checkpoint) │
  └──────────────┘
```

### 4.2 Detailed Flow Description

| Step | Component | Action |
|------|-----------|--------|
| **1** | Client | User signs and submits transaction via RPC |
| **2** | RPC Endpoint | Validates signature, format, nonce, balance |
| **3** | Mempool | Stores transaction, assigns priority score |
| **4** | Gossip | Propagates transaction to connected peers |
| **5** | Block Builder | Leader selects transactions for block |
| **6** | Consensus | Network agrees on block via PoS + PoUW |
| **7** | Execution | Transactions executed in deterministic order |
| **8** | State Update | Account balances, storage modified |
| **9** | Storage | Block and state trie persisted to disk |
| **10** | Finality | Checkpoint confirms irreversibility |

### 4.3 Block Propagation Flow

```
  Proposer                    Validators                   Full Nodes
     │                            │                            │
     │  1. Build Block            │                            │
     │─────────────────────────▶  │                            │
     │      (Block Announce)      │                            │
     │                            │  2. Validate Block         │
     │                            │─────────────────────────▶  │
     │                            │      (Block Gossip)        │
     │                            │                            │
     │  3. Attestations           │                            │
     │◀─────────────────────────  │                            │
     │                            │                            │
     │  4. Block Finalized        │                            │
     │─────────────────────────▶  │─────────────────────────▶  │
     │                            │                            │
```

---

## 5. Node Architecture Summary

Mbongo Chain supports multiple node types optimized for different roles and resource profiles.

### 5.1 Full Node

**Purpose:** Complete chain validation and state verification.

| Attribute | Description |
|-----------|-------------|
| **Chain History** | Stores complete block history |
| **Validation** | Full transaction and block validation |
| **State** | Maintains full state trie |
| **Networking** | Participates in gossip, serves sync requests |
| **Block Production** | No (read-only participant) |

**Hardware Requirements:**

```
┌────────────────────────────────────────┐
│           FULL NODE SPECS              │
├────────────────────────────────────────┤
│  CPU:     4+ cores                     │
│  RAM:     16 GB minimum                │
│  Storage: 500 GB SSD (grows over time) │
│  Network: 100 Mbps stable connection   │
└────────────────────────────────────────┘
```

### 5.2 Validator Node

**Purpose:** Block production and consensus participation.

| Attribute | Description |
|-----------|-------------|
| **Chain History** | Complete block history |
| **Validation** | Full validation + attestation signing |
| **State** | Full state trie + pending state |
| **Networking** | Priority gossip, low-latency peers |
| **Block Production** | Yes, when elected as leader |
| **Staking** | Requires minimum stake deposit |

**Hardware Requirements:**

```
┌────────────────────────────────────────┐
│         VALIDATOR NODE SPECS           │
├────────────────────────────────────────┤
│  CPU:     8+ cores (high single-thread)│
│  RAM:     32 GB minimum                │
│  Storage: 1 TB NVMe SSD                │
│  Network: 1 Gbps, low-latency          │
│  Uptime:  99.9% target                 │
└────────────────────────────────────────┘
```

### 5.3 Guardian Node

**Purpose:** PoUW compute coordination and GPU workload management.

| Attribute | Description |
|-----------|-------------|
| **Chain History** | Complete block history |
| **Validation** | Full validation + PoUW verification |
| **State** | Full state + compute task queue |
| **Networking** | Compute node coordination channels |
| **Block Production** | Yes, with PoUW score boost |
| **GPU Management** | Coordinates GPU compute workers |

**Hardware Requirements:**

```
┌────────────────────────────────────────┐
│         GUARDIAN NODE SPECS            │
├────────────────────────────────────────┤
│  CPU:     16+ cores                    │
│  RAM:     64 GB minimum                │
│  Storage: 2 TB NVMe SSD                │
│  GPU:     NVIDIA RTX 3090+ or A100     │
│  VRAM:    24 GB+ per GPU               │
│  Network: 10 Gbps, datacenter-grade    │
└────────────────────────────────────────┘
```

### 5.4 Light Node [FUTURE]

**Purpose:** Resource-efficient chain verification.

| Attribute | Description |
|-----------|-------------|
| **Chain History** | Headers only |
| **Validation** | Header verification + Merkle proofs |
| **State** | On-demand state retrieval |
| **Networking** | Minimal gossip, request-based |
| **Block Production** | No |

**Hardware Requirements:**

```
┌────────────────────────────────────────┐
│          LIGHT NODE SPECS              │
├────────────────────────────────────────┤
│  CPU:     2+ cores                     │
│  RAM:     4 GB minimum                 │
│  Storage: 10 GB SSD                    │
│  Network: 10 Mbps                      │
└────────────────────────────────────────┘
```

### 5.5 Node Comparison Matrix

```
┌─────────────────┬──────────┬───────────┬──────────┬───────────┐
│    Capability   │   Full   │ Validator │ Guardian │   Light   │
├─────────────────┼──────────┼───────────┼──────────┼───────────┤
│ Full History    │    ✓     │     ✓     │    ✓     │     ✗     │
│ Full Validation │    ✓     │     ✓     │    ✓     │     ✗     │
│ Block Production│    ✗     │     ✓     │    ✓     │     ✗     │
│ Staking         │    ✗     │     ✓     │    ✓     │     ✗     │
│ PoUW Compute    │    ✗     │     ✗     │    ✓     │     ✗     │
│ GPU Required    │    ✗     │     ✗     │    ✓     │     ✗     │
│ Serve Sync      │    ✓     │     ✓     │    ✓     │     ✗     │
│ Header Proofs   │    ✓     │     ✓     │    ✓     │     ✓     │
└─────────────────┴──────────┴───────────┴──────────┴───────────┘
```

---

## 6. Consensus Summary

Mbongo Chain implements a **hybrid Proof-of-Stake (PoS) + Proof-of-Useful-Work (PoUW)** consensus mechanism.

### 6.1 Consensus Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        CONSENSUS ARCHITECTURE                           │
└─────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────────────────┐
                    │      Validator Set          │
                    │  (Stake-Weighted Members)   │
                    └─────────────┬───────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │      Leader Election        │
                    │   (VRF + Stake + PoUW)      │
                    └─────────────┬───────────────┘
                                  │
              ┌───────────────────┼───────────────────┐
              ▼                   ▼                   ▼
     ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
     │   PoS Weight    │ │   PoUW Score    │ │  Randomness     │
     │  (Stake-based)  │ │ (Compute-based) │ │  (VRF Output)   │
     └────────┬────────┘ └────────┬────────┘ └────────┬────────┘
              │                   │                   │
              └───────────────────┼───────────────────┘
                                  ▼
                    ┌─────────────────────────────┐
                    │      Block Proposal         │
                    └─────────────┬───────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │      Fork Choice Rule       │
                    │   Heaviest Chain + PoUW     │
                    └─────────────┬───────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │       Finality Gadget       │
                    │     (Checkpoint-based)      │
                    └─────────────────────────────┘
```

### 6.2 Leader Election

The leader election process combines multiple inputs for fair and unpredictable selection:

1. **Stake Weight:** Higher stake increases selection probability
2. **PoUW Score:** Accumulated useful work contributes to weight
3. **VRF Randomness:** Verifiable random function ensures unpredictability
4. **Slot Assignment:** Deterministic leader per time slot

```
Leader_Score = (Stake_Weight × α) + (PoUW_Score × β) + VRF(slot, secret_key)

Where:
  α = PoS weight coefficient (e.g., 0.6)
  β = PoUW weight coefficient (e.g., 0.4)
```

### 6.3 PoUW Score Integration

Guardian nodes earn PoUW scores by coordinating useful GPU computations:

| Score Component | Description |
|-----------------|-------------|
| **Task Completion** | Successfully completed compute tasks |
| **Result Verification** | Correct results verified by other nodes |
| **Uptime** | Consistent availability for task assignment |
| **Efficiency** | Resource utilization and response time |

### 6.4 Fork Choice Rule

The canonical chain is determined by:

1. **Chain Weight:** Sum of block difficulties
2. **PoUW Contribution:** Blocks with higher PoUW scores add more weight
3. **Attestation Count:** More validator attestations increase weight
4. **Tie-Breaking:** Lower block hash wins ties

```
Chain_Weight = Σ (Block_Difficulty + PoUW_Bonus + Attestation_Weight)
```

### 6.5 Reorg Rules

| Condition | Action |
|-----------|--------|
| **Depth < Finality** | Reorg allowed if new chain is heavier |
| **Depth ≥ Finality** | Reorg rejected (checkpoint prevents) |
| **Same Height** | Heavier chain wins; hash tie-break |
| **Equivocation** | Slashing triggered for double-signing |

### 6.6 Safety & Finality

- **Probabilistic Finality:** Increases with block depth
- **Checkpoint Finality:** Explicit finality every N blocks
- **Slashing Conditions:** Punish equivocation, invalid blocks
- **Recovery Mode:** Network halt procedures for safety violations

---

## 7. Execution Summary

The execution layer processes transactions and produces deterministic state transitions.

### 7.1 Execution Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        EXECUTION PIPELINE                               │
└─────────────────────────────────────────────────────────────────────────┘

  Input Block                                              Output State
      │                                                         ▲
      ▼                                                         │
 ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
 │  Parse  │───▶│Validate │───▶│ Execute │───▶│ Commit  │───▶│  Root   │
 │  Block  │    │   Txs   │    │   Txs   │    │  State  │    │  Hash   │
 └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
                    │              │
                    ▼              ▼
              ┌───────────────────────────────┐
              │       Execution Backends      │
              ├───────────┬───────────────────┤
              │  Native   │  WASM   │   GPU   │
              │ Transfers │ [FUTURE]│  PoUW   │
              └───────────┴─────────┴─────────┘
```

### 7.2 State Transitions

Each transaction produces a state transition function:

```
S' = STF(S, Tx)

Where:
  S  = Pre-state (accounts, balances, storage)
  Tx = Transaction (sender, receiver, amount, data)
  S' = Post-state (updated accounts, balances, storage)
```

**Transition Types:**

| Type | Description |
|------|-------------|
| **Transfer** | Move tokens between accounts |
| **Stake** | Deposit tokens for validation rights |
| **Unstake** | Withdraw staked tokens (with delay) |
| **PoUW Submit** | Submit compute task results |
| **Contract Call** | [FUTURE] Execute smart contract |

### 7.3 WASM Execution [FUTURE]

Planned smart contract support via WebAssembly:

- **Sandboxed Execution:** Isolated contract environments
- **Metered Gas:** Deterministic resource limits
- **Host Functions:** Bridge to native chain functionality
- **Upgradability:** Contract upgrade mechanisms

### 7.4 GPU-Optimized Execution Paths

For PoUW compute tasks:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     GPU EXECUTION PATH                                  │
└─────────────────────────────────────────────────────────────────────────┘

   ┌──────────────┐         ┌──────────────┐         ┌──────────────┐
   │  Task Queue  │────────▶│  GPU Kernel  │────────▶│   Result     │
   │  (Pending)   │         │  Execution   │         │  Validation  │
   └──────────────┘         └──────────────┘         └──────────────┘
          │                        │                        │
          │                        ▼                        │
          │                 ┌──────────────┐               │
          │                 │   CUDA/ROCm  │               │
          │                 │   Backend    │               │
          │                 └──────────────┘               │
          │                                                 │
          └────────────────────────────────────────────────┘
                           (Batch Processing)
```

### 7.5 Deterministic Execution Guarantees

All execution is fully deterministic:

- **Ordered Execution:** Transactions execute in block order
- **No External I/O:** No network/filesystem access during execution
- **Fixed Arithmetic:** No floating-point; fixed-point only
- **Reproducibility:** Same inputs always produce same outputs

### 7.6 Commit Logic

After execution, state changes are committed atomically:

1. **Execute all transactions** in block
2. **Compute state diff** (changed keys/values)
3. **Update state trie** with new values
4. **Compute new state root** hash
5. **Persist to storage** atomically
6. **Prune old state** if configured

---

## 8. Storage Summary

The storage layer provides durable persistence for all chain data.

### 8.1 Storage Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        STORAGE ARCHITECTURE                             │
└─────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────────────────┐
                    │      Storage Manager        │
                    │   (Unified Access Layer)    │
                    └─────────────┬───────────────┘
                                  │
         ┌────────────────────────┼────────────────────────┐
         │                        │                        │
         ▼                        ▼                        ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Block Store   │    │   State Trie    │    │  Index Tables   │
│                 │    │                 │    │                 │
│  ┌───────────┐  │    │  ┌───────────┐  │    │  ┌───────────┐  │
│  │  Headers  │  │    │  │ Accounts  │  │    │  │  TxHash   │  │
│  ├───────────┤  │    │  ├───────────┤  │    │  │  → Block  │  │
│  │  Bodies   │  │    │  │ Balances  │  │    │  ├───────────┤  │
│  ├───────────┤  │    │  ├───────────┤  │    │  │  Address  │  │
│  │ Receipts  │  │    │  │ Storage   │  │    │  │  → Txs    │  │
│  └───────────┘  │    │  ├───────────┤  │    │  ├───────────┤  │
│                 │    │  │  Code     │  │    │  │  Block    │  │
│                 │    │  └───────────┘  │    │  │  → Height │  │
│                 │    │                 │    │  └───────────┘  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                        │                        │
         └────────────────────────┴────────────────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │     Underlying DB Engine    │
                    │   (RocksDB / LevelDB / etc) │
                    └─────────────────────────────┘
```

### 8.2 Block Store

Stores the complete blockchain:

| Component | Description |
|-----------|-------------|
| **Headers** | Block headers indexed by hash and height |
| **Bodies** | Transaction lists per block |
| **Receipts** | Execution receipts with logs and status |
| **Uncles** | Uncle block references (if applicable) |

**Key Schema:**

```
header:{block_hash}     → BlockHeader
body:{block_hash}       → BlockBody
receipt:{block_hash}    → Vec<Receipt>
height:{block_number}   → block_hash
```

### 8.3 State Trie

Merkle Patricia Trie storing account state:

| Data Type | Description |
|-----------|-------------|
| **Accounts** | Address → (nonce, balance, code_hash, storage_root) |
| **Balances** | Implicitly in account structure |
| **Storage** | Contract storage key-value pairs |
| **Code** | Smart contract bytecode (by code_hash) |

**Trie Properties:**

- **Merkle Proofs:** Any value provable with O(log n) proof
- **Efficient Updates:** Only modified paths recomputed
- **State Root:** Single 32-byte commitment to entire state

### 8.4 Index Tables

Secondary indices for efficient queries:

| Index | Mapping |
|-------|---------|
| **TxHash Index** | transaction_hash → (block_hash, tx_index) |
| **Address Index** | address → Vec<(block_hash, tx_index)> |
| **Block Height** | height → block_hash |
| **Validator Index** | validator_pubkey → stake_info |

### 8.5 Checkpoint System

Periodic state snapshots for fast synchronization:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       CHECKPOINT SYSTEM                                 │
└─────────────────────────────────────────────────────────────────────────┘

  Block 0        Block 1000      Block 2000      Block 3000
     │               │               │               │
     ▼               ▼               ▼               ▼
 ┌───────┐       ┌───────┐       ┌───────┐       ┌───────┐
 │Genesis│       │Chkpt 1│       │Chkpt 2│       │Chkpt 3│
 │ State │       │ State │       │ State │       │ State │
 └───────┘       └───────┘       └───────┘       └───────┘
     │               │               │               │
     └───────────────┴───────────────┴───────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │  Fast Sync: Download  │
              │  Latest Checkpoint +  │
              │  Apply Recent Blocks  │
              └───────────────────────┘
```

**Checkpoint Properties:**

- **Interval:** Every N blocks (configurable, e.g., 1000)
- **Contents:** Full state snapshot + block hash + signatures
- **Finality:** Checkpoints are considered final
- **Pruning:** Old checkpoints can be pruned after newer ones exist

---

## 9. Message Types Summary

Network messages are categorized by function.

### 9.1 Control Messages

Messages for network coordination and peer management:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `Handshake` | Bidirectional | Initial peer connection setup |
| `Ping` | Request | Liveness check |
| `Pong` | Response | Liveness response |
| `GetPeers` | Request | Request peer list |
| `Peers` | Response | Peer list response |
| `Disconnect` | Notification | Graceful disconnection |
| `Status` | Bidirectional | Chain status exchange |

### 9.2 Data Messages

Messages for blockchain data exchange:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `NewBlock` | Broadcast | Announce new block |
| `NewTransaction` | Broadcast | Announce new transaction |
| `GetBlocks` | Request | Request blocks by hash |
| `Blocks` | Response | Block data response |
| `GetHeaders` | Request | Request headers by range |
| `Headers` | Response | Header data response |
| `GetReceipts` | Request | Request transaction receipts |
| `Receipts` | Response | Receipt data response |
| `GetState` | Request | Request state trie nodes |
| `State` | Response | State trie node response |

### 9.3 Sync Messages

Messages for chain synchronization:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `GetCheckpoint` | Request | Request checkpoint data |
| `Checkpoint` | Response | Checkpoint data response |
| `GetBlockBodies` | Request | Request block bodies |
| `BlockBodies` | Response | Block body response |
| `GetStateRange` | Request | Request state range for fast sync |
| `StateRange` | Response | State range response |

### 9.4 Consensus Messages

Messages for consensus protocol:

| Message | Direction | Purpose |
|---------|-----------|---------|
| `BlockProposal` | Broadcast | Propose new block |
| `Attestation` | Broadcast | Vote for block |
| `AggregateAttestation` | Broadcast | Aggregated votes |
| `PoUWProof` | Broadcast | Submit PoUW proof |
| `ValidatorRegistration` | Broadcast | Announce new validator |

### 9.5 Message Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      MESSAGE FLOW PATTERNS                              │
└─────────────────────────────────────────────────────────────────────────┘

  Request/Response Pattern:
  ┌──────┐                    ┌──────┐
  │Node A│─── GetBlocks ────▶│Node B│
  │      │◀───  Blocks  ─────│      │
  └──────┘                    └──────┘

  Broadcast Pattern:
  ┌──────┐                    ┌──────┐
  │Node A│─── NewBlock ─────▶│Node B│
  │      │                    │      │──── NewBlock ────▶ Node C
  └──────┘                    └──────┘
      │
      └────── NewBlock ─────▶ Node D

  Gossip Pattern:
  ┌──────┐    ┌──────┐    ┌──────┐    ┌──────┐
  │Node A│◀──▶│Node B│◀──▶│Node C│◀──▶│Node D│
  └──────┘    └──────┘    └──────┘    └──────┘
      │           │           │           │
      └───────────┴───────────┴───────────┘
              (Mesh propagation)
```

---

## 10. Cross-Component Relationships

This section describes the critical interfaces between major components.

### 10.1 Consensus ↔ Execution

```
┌───────────────────────────────────────────────────────────────┐
│              CONSENSUS ↔ EXECUTION INTERFACE                  │
└───────────────────────────────────────────────────────────────┘

  Consensus                                          Execution
     │                                                   │
     │  1. Finalize Block                               │
     │──────────────────────────────────────────────────▶│
     │     (block_header, transactions)                  │
     │                                                   │
     │  2. Request State Root                           │
     │◀──────────────────────────────────────────────────│
     │     (state_root, receipts_root)                   │
     │                                                   │
     │  3. Validate Execution                           │
     │──────────────────────────────────────────────────▶│
     │     (expected_state_root)                         │
     │                                                   │
     │  4. Confirm/Reject                               │
     │◀──────────────────────────────────────────────────│
     │     (bool)                                        │
```

**Interface Contract:**

| Function | Input | Output |
|----------|-------|--------|
| `execute_block` | Block, ParentState | StateRoot, Receipts |
| `validate_state_root` | Block, ExpectedRoot | bool |
| `revert_block` | BlockHash | Result<()> |

### 10.2 Networking ↔ Mempool

```
┌───────────────────────────────────────────────────────────────┐
│              NETWORKING ↔ MEMPOOL INTERFACE                   │
└───────────────────────────────────────────────────────────────┘

  Networking                                          Mempool
     │                                                   │
     │  1. Receive Transaction                          │
     │──────────────────────────────────────────────────▶│
     │     (raw_transaction)                             │
     │                                                   │
     │  2. Validation Result                            │
     │◀──────────────────────────────────────────────────│
     │     (accepted: bool, reason: Option<Error>)       │
     │                                                   │
     │  3. Broadcast Request (for local txs)            │
     │◀──────────────────────────────────────────────────│
     │     (transaction, peer_filter)                    │
     │                                                   │
```

**Interface Contract:**

| Function | Input | Output |
|----------|-------|--------|
| `submit_transaction` | SignedTx | Result<TxHash> |
| `get_pending` | MaxCount | Vec<SignedTx> |
| `remove_transactions` | Vec<TxHash> | () |

### 10.3 Mempool ↔ Validator

```
┌───────────────────────────────────────────────────────────────┐
│              MEMPOOL ↔ VALIDATOR INTERFACE                    │
└───────────────────────────────────────────────────────────────┘

  Mempool                                            Validator
     │                                                   │
     │  1. Get Pending Transactions                     │
     │◀──────────────────────────────────────────────────│
     │     (max_count, max_gas)                          │
     │                                                   │
     │  2. Return Sorted Transactions                   │
     │──────────────────────────────────────────────────▶│
     │     (Vec<SignedTx>)                               │
     │                                                   │
     │  3. Mark Included                                │
     │◀──────────────────────────────────────────────────│
     │     (Vec<TxHash>)                                 │
     │                                                   │
```

**Interface Contract:**

| Function | Input | Output |
|----------|-------|--------|
| `get_block_transactions` | MaxGas, MaxCount | Vec<SignedTx> |
| `mark_included` | Vec<TxHash> | () |
| `reinsert_transactions` | Vec<SignedTx> | () |

### 10.4 Execution ↔ Storage

```
┌───────────────────────────────────────────────────────────────┐
│              EXECUTION ↔ STORAGE INTERFACE                    │
└───────────────────────────────────────────────────────────────┘

  Execution                                           Storage
     │                                                   │
     │  1. Read State                                   │
     │──────────────────────────────────────────────────▶│
     │     (address, key)                                │
     │                                                   │
     │  2. Return Value                                 │
     │◀──────────────────────────────────────────────────│
     │     (Option<Value>)                               │
     │                                                   │
     │  3. Write State Batch                            │
     │──────────────────────────────────────────────────▶│
     │     (Vec<(key, value)>)                           │
     │                                                   │
     │  4. Commit                                       │
     │──────────────────────────────────────────────────▶│
     │     (block_hash, state_root)                      │
     │                                                   │
```

**Interface Contract:**

| Function | Input | Output |
|----------|-------|--------|
| `get_account` | Address | Option<Account> |
| `get_storage` | Address, Key | Option<Value> |
| `commit_state` | StateDiff | StateRoot |
| `get_state_proof` | Address, Keys | MerkleProof |

### 10.5 Guardian Nodes ↔ Full Nodes

```
┌───────────────────────────────────────────────────────────────┐
│           GUARDIAN NODES ↔ FULL NODES INTERFACE               │
└───────────────────────────────────────────────────────────────┘

  Guardian Node                                      Full Node
     │                                                   │
     │  1. Broadcast PoUW Results                       │
     │──────────────────────────────────────────────────▶│
     │     (task_id, result, proof)                      │
     │                                                   │
     │  2. Request Task Verification                    │
     │◀──────────────────────────────────────────────────│
     │     (task_id)                                     │
     │                                                   │
     │  3. Submit Verification                          │
     │──────────────────────────────────────────────────▶│
     │     (task_id, is_valid, signature)                │
     │                                                   │
     │  4. Request Compute Tasks                        │
     │◀──────────────────────────────────────────────────│
     │     (compute_capacity)                            │
     │                                                   │
     │  5. Assign Tasks                                 │
     │──────────────────────────────────────────────────▶│
     │     (Vec<ComputeTask>)                            │
     │                                                   │
```

**Interface Contract:**

| Function | Input | Output |
|----------|-------|--------|
| `submit_pouw_result` | TaskId, Result, Proof | bool |
| `verify_pouw_result` | TaskId, Result | bool |
| `get_pending_tasks` | Capacity | Vec<ComputeTask> |
| `report_task_completion` | TaskId, Duration | () |

### 10.6 Relationship Matrix

```
┌────────────────────────────────────────────────────────────────────────┐
│                    COMPONENT RELATIONSHIP MATRIX                        │
├───────────┬──────────┬─────────┬───────────┬─────────┬─────────────────┤
│           │Networking│ Mempool │ Consensus │Execution│ Storage         │
├───────────┼──────────┼─────────┼───────────┼─────────┼─────────────────┤
│Networking │    -     │   R/W   │    R/W    │    -    │       -         │
├───────────┼──────────┼─────────┼───────────┼─────────┼─────────────────┤
│ Mempool   │   R/W    │    -    │     R     │    -    │       -         │
├───────────┼──────────┼─────────┼───────────┼─────────┼─────────────────┤
│Consensus  │   R/W    │    R    │     -     │   R/W   │       R         │
├───────────┼──────────┼─────────┼───────────┼─────────┼─────────────────┤
│Execution  │    -     │    -    │     W     │    -    │      R/W        │
├───────────┼──────────┼─────────┼───────────┼─────────┼─────────────────┤
│ Storage   │    -     │    -    │     W     │   R/W   │       -         │
└───────────┴──────────┴─────────┴───────────┴─────────┴─────────────────┘

Legend: R = Reads from, W = Writes to, R/W = Both, - = No direct interaction
```

---

## 11. Final Architecture Diagram

The complete system architecture showing all major components and their connections:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                         │
│                           MBONGO CHAIN — COMPLETE ARCHITECTURE                          │
│                                                                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│    ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│    │                              EXTERNAL INTERFACES                                │  │
│    │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │  │
│    │  │  JSON-RPC   │  │  WebSocket  │  │  REST API   │  │   CLI (mbongo-cli)      │ │  │
│    │  │   Server    │  │   Server    │  │   Server    │  │                         │ │  │
│    │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └────────────┬────────────┘ │  │
│    └─────────┼────────────────┼────────────────┼──────────────────────┼──────────────┘  │
│              │                │                │                      │                 │
│              └────────────────┴────────────────┴──────────────────────┘                 │
│                                        │                                                │
│    ┌───────────────────────────────────▼───────────────────────────────────────────┐   │
│    │                                NODE LAYER                                      │   │
│    │                                                                                │   │
│    │    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌───────────┐  │   │
│    │    │  Full Node   │    │  Validator   │    │   Guardian   │    │   Light   │  │   │
│    │    │              │    │    Node      │    │    Node      │    │   Node    │  │   │
│    │    │  ┌────────┐  │    │  ┌────────┐  │    │  ┌────────┐  │    │  [FUTURE] │  │   │
│    │    │  │Validate│  │    │  │ Stake  │  │    │  │  GPU   │  │    │           │  │   │
│    │    │  │ Blocks │  │    │  │ & Sign │  │    │  │Compute │  │    │           │  │   │
│    │    │  └────────┘  │    │  └────────┘  │    │  └────────┘  │    │           │  │   │
│    │    └──────┬───────┘    └──────┬───────┘    └──────┬───────┘    └───────────┘  │   │
│    │           └───────────────────┴───────────────────┘                           │   │
│    │                               │                                                │   │
│    └───────────────────────────────┼────────────────────────────────────────────────┘   │
│                                    │                                                    │
│    ┌───────────────────────────────┼────────────────────────────────────────────────┐   │
│    │                               ▼                                                │   │
│    │  ┌────────────────────────────────────────────────────────────────────────┐   │   │
│    │  │                         CONSENSUS LAYER                                 │   │   │
│    │  │                                                                         │   │   │
│    │  │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                │   │   │
│    │  │   │    PoS      │◀──▶│   Leader    │◀──▶│    PoUW     │                │   │   │
│    │  │   │   Engine    │    │  Election   │    │   Engine    │                │   │   │
│    │  │   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                │   │   │
│    │  │          │                  │                  │                        │   │   │
│    │  │          └──────────────────┼──────────────────┘                        │   │   │
│    │  │                             ▼                                           │   │   │
│    │  │                    ┌─────────────────┐                                  │   │   │
│    │  │                    │   Fork Choice   │                                  │   │   │
│    │  │                    │      Rule       │                                  │   │   │
│    │  │                    └─────────────────┘                                  │   │   │
│    │  │                                                                         │   │   │
│    │  └──────────────────────────────┬──────────────────────────────────────────┘   │   │
│    │                                 │                                              │   │
│    │  ┌──────────────────────────────▼──────────────────────────────────────────┐   │   │
│    │  │                        EXECUTION LAYER                                   │   │   │
│    │  │                                                                          │   │   │
│    │  │   ┌─────────────────────────────────────────────────────────────────┐   │   │   │
│    │  │   │                    Transaction Executor                          │   │   │   │
│    │  │   └─────────────────────────────┬───────────────────────────────────┘   │   │   │
│    │  │                                 │                                        │   │   │
│    │  │          ┌──────────────────────┼──────────────────────┐                │   │   │
│    │  │          ▼                      ▼                      ▼                │   │   │
│    │  │   ┌─────────────┐       ┌─────────────┐       ┌─────────────┐          │   │   │
│    │  │   │   Native    │       │    WASM     │       │     GPU     │          │   │   │
│    │  │   │   Runtime   │       │  [FUTURE]   │       │  Executor   │          │   │   │
│    │  │   └─────────────┘       └─────────────┘       └─────────────┘          │   │   │
│    │  │                                                                          │   │   │
│    │  └──────────────────────────────┬───────────────────────────────────────────┘   │   │
│    │                                 │                                               │   │
│    │  ┌──────────────────────────────▼───────────────────────────────────────────┐   │   │
│    │  │                         MEMPOOL LAYER                                     │   │   │
│    │  │                                                                           │   │   │
│    │  │    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │   │   │
│    │  │    │   Pending   │───▶│  Priority   │───▶│   Block     │                 │   │   │
│    │  │    │    Pool     │    │   Queue     │    │  Builder    │                 │   │   │
│    │  │    └─────────────┘    └─────────────┘    └─────────────┘                 │   │   │
│    │  │                                                                           │   │   │
│    │  └──────────────────────────────┬────────────────────────────────────────────┘   │   │
│    │                                 │                                                │   │
│    │  ┌──────────────────────────────▼────────────────────────────────────────────┐   │   │
│    │  │                        NETWORKING LAYER                                    │   │   │
│    │  │                                                                            │   │   │
│    │  │   ┌───────────┐   ┌───────────┐   ┌───────────┐   ┌───────────┐           │   │   │
│    │  │   │  libp2p   │──▶│  Gossip   │──▶│   Sync    │──▶│   Peer    │           │   │   │
│    │  │   │ Transport │   │  Router   │   │ Protocol  │   │ Discovery │           │   │   │
│    │  │   └───────────┘   └───────────┘   └───────────┘   └───────────┘           │   │   │
│    │  │                                                                            │   │   │
│    │  └──────────────────────────────┬─────────────────────────────────────────────┘   │   │
│    │                                 │                                                 │   │
│    │  ┌──────────────────────────────▼─────────────────────────────────────────────┐   │   │
│    │  │                         STORAGE LAYER                                       │   │   │
│    │  │                                                                             │   │   │
│    │  │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐   ┌────────────┐   │   │   │
│    │  │   │   Block     │    │   State     │    │   Index     │   │ Checkpoint │   │   │   │
│    │  │   │   Store     │    │   Trie      │    │   Tables    │   │  Manager   │   │   │   │
│    │  │   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘   └─────┬──────┘   │   │   │
│    │  │          │                  │                  │                │          │   │   │
│    │  │          └──────────────────┴──────────────────┴────────────────┘          │   │   │
│    │  │                                    │                                        │   │   │
│    │  │                         ┌──────────▼──────────┐                            │   │   │
│    │  │                         │   RocksDB Engine    │                            │   │   │
│    │  │                         └─────────────────────┘                            │   │   │
│    │  │                                                                             │   │   │
│    │  └─────────────────────────────────────────────────────────────────────────────┘   │   │
│    │                                                                                    │   │
│    │                              CORE MODULES                                          │   │
│    └────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                             │
│    ┌────────────────────────────────────────────────────────────────────────────────────┐   │
│    │                           CRYPTOGRAPHIC PRIMITIVES                                 │   │
│    │                                                                                    │   │
│    │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐        │   │
│    │   │   Hashing   │    │  Signatures │    │   Key Mgmt  │    │   Merkle    │        │   │
│    │   │  (Blake3,   │    │  (Ed25519,  │    │  (BIP-32,   │    │   Trees     │        │   │
│    │   │   SHA-256)  │    │   ECDSA)    │    │   BIP-39)   │    │             │        │   │
│    │   └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘        │   │
│    │                                                                                    │   │
│    └────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                             │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                             │
│   P2P NETWORK:  ◀═══════════════════════════════════════════════════════════════════════▶  │
│                        Peer Discovery │ Block Gossip │ Tx Propagation │ Sync               │
│                                                                                             │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Component Legend

| Symbol | Meaning |
|--------|---------|
| `─▶` | Data flow direction |
| `◀──▶` | Bidirectional communication |
| `═══` | Network boundary |
| `[FUTURE]` | Planned feature |

### Module Mapping

| Architecture Layer | Rust Module |
|-------------------|-------------|
| CLI | `cli/` |
| Node | `node/` |
| Consensus | `pow/` |
| Execution | `runtime/` |
| Networking | `network/` |
| Crypto | `crypto/` |
| Storage | Embedded in `node/`, `runtime/` |

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | November 2025 | Architecture Team | Initial document |

---

## References

- [Mbongo Chain Repository](https://github.com/mbongo-chain/mbongo-chain)
- [Rust Programming Language](https://www.rust-lang.org/)
- [libp2p Specification](https://libp2p.io/)
- [Merkle Patricia Trie](https://ethereum.org/en/developers/docs/data-structures-and-encoding/patricia-merkle-trie/)

---

*This document is maintained by the Mbongo Chain Architecture Team. For questions or contributions, please open an issue or pull request in the main repository.*

