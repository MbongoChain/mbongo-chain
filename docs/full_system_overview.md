<!-- Verified against tokenomics.md -->
# Mbongo Chain — Full System Overview

> **Document Version:** 1.0  
> **Last Updated:** November 2025  
> **Status:** Canonical Reference  
> **Audience:** New Engineers, Architects, Contributors

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [High-Level Architecture Diagram](#2-high-level-architecture-diagram)
3. [System Components Summary](#3-system-components-summary)
4. [Data Flow Overview](#4-data-flow-overview)
5. [Block Lifecycle Overview](#5-block-lifecycle-overview)
6. [Consensus Summary](#6-consensus-summary)
7. [Execution Pipeline Summary](#7-execution-pipeline-summary)
8. [Storage Layer Summary](#8-storage-layer-summary)
9. [Node Roles Summary](#9-node-roles-summary)
10. [Network Model Summary](#10-network-model-summary)
11. [Security Architecture Summary](#11-security-architecture-summary)
12. [Future Roadmap](#12-future-roadmap)

---

## 1. Purpose of This Document

This document serves as the **canonical entry point** for understanding Mbongo Chain's complete system architecture. It provides a unified view of all subsystems—from networking through consensus to execution—enabling engineers to quickly understand how components interact.

### Document Scope

```
┌─────────────────────────────────────────────────────────────────────┐
│                    DOCUMENT COVERAGE                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   This Document Covers:                                             │
│   ─────────────────────                                             │
│   ✓ Complete architecture overview                                 │
│   ✓ All major subsystem interactions                               │
│   ✓ Data flow from transaction to finality                         │
│   ✓ Node types and their responsibilities                          │
│   ✓ Security considerations                                        │
│   ✓ Future development direction                                   │
│                                                                     │
│   For Detailed Specifications, See:                                │
│   ─────────────────────────────────                                 │
│   • architecture_master_overview.md — Layer-by-layer breakdown     │
│   • compute_engine_overview.md — PoUW and GPU coordination        │
│   • execution_engine_overview.md — Runtime and state machine      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Target Audience

| Audience | Focus Areas |
|----------|-------------|
| **New Engineers** | Sections 1-4: Architecture and data flow |
| **Protocol Developers** | Sections 5-7: Block lifecycle and execution |
| **Infrastructure Engineers** | Sections 8-10: Storage and networking |
| **Security Reviewers** | Section 11: Security architecture |
| **Technical Leadership** | Section 12: Future roadmap |

### Core Design Philosophy

Mbongo Chain is built on four foundational principles:

1. **Compute-First:** GPU workloads are protocol-native, not afterthoughts
2. **Deterministic:** All state transitions are reproducible across nodes
3. **Modular:** Each subsystem has clear boundaries and interfaces
4. **Secure:** Economic incentives align with protocol correctness

---

## 2. High-Level Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                 │
│                              MBONGO CHAIN — FULL SYSTEM ARCHITECTURE                            │
│                                                                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                 │
│  ┌───────────────────────────────────────────────────────────────────────────────────────────┐  │
│  │                                    EXTERNAL INTERFACES                                    │  │
│  │                                                                                           │  │
│  │    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────────────┐  │  │
│  │    │ JSON-RPC │    │WebSocket │    │ REST API │    │   CLI    │    │  SDK Clients     │  │  │
│  │    └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘    └────────┬─────────┘  │  │
│  │         └───────────────┴───────────────┴───────────────┴───────────────────┘            │  │
│  └─────────────────────────────────────────────┬─────────────────────────────────────────────┘  │
│                                                │                                                │
│  ┌─────────────────────────────────────────────▼─────────────────────────────────────────────┐  │
│  │                                      NODE LAYER                                           │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                      │  │
│  │  │  FULL NODE  │  │  VALIDATOR  │  │  GUARDIAN   │  │ LIGHT NODE  │                      │  │
│  │  │             │  │    NODE     │  │    NODE     │  │  [FUTURE]   │                      │  │
│  │  │ • Validate  │  │ • Validate  │  │ • Validate  │  │ • Headers   │                      │  │
│  │  │ • Relay     │  │ • Propose   │  │ • Propose   │  │ • Proofs    │                      │  │
│  │  │ • Serve     │  │ • Stake     │  │ • GPU Coord │  │             │                      │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────────────┘                      │  │
│  │         └────────────────┴────────────────┘                                              │  │
│  └─────────────────────────────────────────────┬─────────────────────────────────────────────┘  │
│                                                │                                                │
│  ┌─────────────────────────────────────────────▼─────────────────────────────────────────────┐  │
│  │                                  CONSENSUS LAYER                                          │  │
│  │                                                                                           │  │
│  │    ┌─────────────────────┐         ┌─────────────────────┐                               │  │
│  │    │      PoS ENGINE     │◀───────▶│     PoUW ENGINE     │                               │  │
│  │    │                     │         │                     │                               │  │
│  │    │  • Stake tracking   │         │  • Task assignment  │                               │  │
│  │    │  • Validator set    │         │  • Result verify    │                               │  │
│  │    │  • Slashing rules   │         │  • Score compute    │                               │  │
│  │    └──────────┬──────────┘         └──────────┬──────────┘                               │  │
│  │               │                               │                                          │  │
│  │               └───────────┬───────────────────┘                                          │  │
│  │                           ▼                                                              │  │
│  │               ┌─────────────────────┐                                                    │  │
│  │               │   LEADER ELECTION   │                                                    │  │
│  │               │   (VRF + Stake +    │                                                    │  │
│  │               │    PoUW Score)      │                                                    │  │
│  │               └──────────┬──────────┘                                                    │  │
│  │                          │                                                               │  │
│  │               ┌──────────▼──────────┐                                                    │  │
│  │               │    FORK CHOICE      │                                                    │  │
│  │               │  (Heaviest Chain)   │                                                    │  │
│  │               └─────────────────────┘                                                    │  │
│  │                                                                                           │  │
│  └─────────────────────────────────────────────┬─────────────────────────────────────────────┘  │
│                                                │                                                │
│  ┌──────────────────────────┬──────────────────┴──────────────────┬──────────────────────────┐  │
│  │                          │                                     │                          │  │
│  │    ┌─────────────────────▼───────────────────┐   ┌────────────▼────────────────┐         │  │
│  │    │            EXECUTION ENGINE             │   │      COMPUTE ENGINE         │         │  │
│  │    │                                         │   │                             │         │  │
│  │    │  ┌─────────────────────────────────┐   │   │  ┌─────────────────────┐    │         │  │
│  │    │  │        TRANSACTION PIPELINE     │   │   │  │    TASK MANAGER     │    │         │  │
│  │    │  │  Receive → Validate → Execute   │   │   │  │  Queue → Assign →   │    │         │  │
│  │    │  └─────────────┬───────────────────┘   │   │  │  Execute → Verify   │    │         │  │
│  │    │                │                       │   │  └──────────┬──────────┘    │         │  │
│  │    │  ┌─────────────▼───────────────────┐   │   │             │               │         │  │
│  │    │  │         STATE MACHINE           │   │   │  ┌──────────▼──────────┐    │         │  │
│  │    │  │  S' = STF(S, Tx)               │   │   │  │   GPU PROVIDERS     │    │         │  │
│  │    │  │  • Deterministic               │   │   │  │  • Job execution    │    │         │  │
│  │    │  │  • Isolated                    │   │   │  │  • Proof generation │    │         │  │
│  │    │  │  • Verifiable                  │   │   │  └─────────────────────┘    │         │  │
│  │    │  └─────────────────────────────────┘   │   │                             │         │  │
│  │    │                                         │   │                             │         │  │
│  │    └──────────────────────┬──────────────────┘   └──────────────┬─────────────┘         │  │
│  │                           │                                      │                       │  │
│  │  MEMPOOL LAYER            │                                      │                       │  │
│  │  ┌────────────────────────┼──────────────────────────────────────┼─────────────────────┐ │  │
│  │  │                        │                                      │                     │ │  │
│  │  │  ┌──────────────┐      │      ┌──────────────┐               │                     │ │  │
│  │  │  │  Pending Txs │◀─────┘      │ Compute Tasks│◀──────────────┘                     │ │  │
│  │  │  │  • Priority  │             │ • GPU Queue  │                                     │ │  │
│  │  │  │  • Eviction  │             │ • Assignment │                                     │ │  │
│  │  │  └──────────────┘             └──────────────┘                                     │ │  │
│  │  │                                                                                     │ │  │
│  │  └─────────────────────────────────────────────────────────────────────────────────────┘ │  │
│  │                                                                                          │  │
│  └──────────────────────────────────────────────┬───────────────────────────────────────────┘  │
│                                                 │                                              │
│  ┌──────────────────────────────────────────────▼──────────────────────────────────────────┐   │
│  │                                    STORAGE LAYER                                        │   │
│  │                                                                                         │   │
│  │    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐    │   │
│  │    │ BLOCK STORE │    │ STATE TRIE  │    │   INDEXES   │    │    CHECKPOINTS      │    │   │
│  │    │             │    │             │    │             │    │                     │    │   │
│  │    │ • Headers   │    │ • Accounts  │    │ • TxHash    │    │ • State snapshots   │    │   │
│  │    │ • Bodies    │    │ • Balances  │    │ • Address   │    │ • Fast sync         │    │   │
│  │    │ • Receipts  │    │ • Storage   │    │ • Block     │    │ • Recovery          │    │   │
│  │    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────────────┘    │   │
│  │                                                                                         │   │
│  └──────────────────────────────────────────────┬──────────────────────────────────────────┘   │
│                                                 │                                              │
│  ┌──────────────────────────────────────────────▼──────────────────────────────────────────┐   │
│  │                                   NETWORKING LAYER                                      │   │
│  │                                                                                         │   │
│  │    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐    │   │
│  │    │   libp2p    │───▶│   GOSSIP    │───▶│    SYNC     │───▶│   PEER DISCOVERY    │    │   │
│  │    │  Transport  │    │   Router    │    │  Protocol   │    │     (Kademlia)      │    │   │
│  │    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────────────┘    │   │
│  │                                                                                         │   │
│  └─────────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                                │
│  ═══════════════════════════════════════════════════════════════════════════════════════════  │
│                                        P2P NETWORK                                             │
│  ═══════════════════════════════════════════════════════════════════════════════════════════  │
│                                                                                                │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Layer Interaction Summary

| Layer | Interacts With | Primary Data |
|-------|----------------|--------------|
| **Networking** | All layers | Messages, blocks, transactions |
| **Mempool** | Networking, Execution | Pending transactions |
| **Consensus** | Execution, Compute | Blocks, votes, PoUW scores |
| **Execution** | Consensus, Storage | State transitions |
| **Compute** | Consensus, Networking | Task results, receipts |
| **Storage** | All layers | Persistent state |

---

## 3. System Components Summary

### 3.1 Networking Module

**Purpose:** P2P communication and message propagation.

```
┌────────────────────────────────────────────────────────────────┐
│                    NETWORKING MODULE                           │
├────────────────────────────────────────────────────────────────┤
│  Transport:    libp2p (TCP, QUIC, WebSocket)                  │
│  Discovery:    Kademlia DHT                                   │
│  Propagation:  GossipSub protocol                             │
│  Sync:         Request/Response for blocks and state          │
│  Security:     Peer scoring, connection limits, banning       │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Mempool Module

**Purpose:** Transaction staging and prioritization before block inclusion.

| Function | Description |
|----------|-------------|
| **Receive** | Accept transactions from network/RPC |
| **Validate** | Check signatures, nonces, balances |
| **Prioritize** | Order by fee, urgency |
| **Evict** | Remove low-priority under pressure |
| **Serve** | Provide sorted transactions to block builder |

### 3.3 Consensus Module

**Purpose:** Achieve network-wide agreement on canonical chain.

```
Consensus = PoS (Stake Weight) + PoUW (Compute Score)

Block Selection:
  weight(block) = base_difficulty 
                + attestation_weight 
                + pouw_bonus

Leader Election:
  score = (α × stake) + (β × pouw_score) + VRF(slot)
```

### 3.4 Execution Engine

**Purpose:** Process transactions and compute state transitions.

| Phase | Action |
|-------|--------|
| **Pre-check** | Format, version, size validation |
| **Signature** | Cryptographic verification |
| **Gas** | Fee and limit validation |
| **Execute** | Apply state changes |
| **Commit** | Merkle root computation, persist |

### 3.5 Compute Engine

**Purpose:** Coordinate useful GPU workloads for PoUW.

```
Task Lifecycle:
  Submit → Queue → Assign → Execute → Verify → Reward

Supported Workloads:
  • ML inference / training
  • ZK proof generation
  • Batch computations
  • Rendering / encoding
```

### 3.6 Storage Engine

**Purpose:** Durable persistence of blockchain data.

| Store | Contents |
|-------|----------|
| **Block Store** | Headers, bodies, receipts |
| **State Trie** | Accounts, balances, storage |
| **Indexes** | TxHash→Block, Address→Txs |
| **Checkpoints** | Periodic state snapshots |

### 3.7 Node Roles

| Role | Primary Function |
|------|------------------|
| **Full Node** | Validate and relay |
| **Validator** | Propose blocks, stake |
| **Guardian** | Coordinate GPU compute |
| **Light Node** | Header verification [FUTURE] |

### 3.8 Validation Pipeline

```
Transaction ──▶ Format ──▶ Signature ──▶ Nonce ──▶ Balance ──▶ Execute
                  │            │           │          │           │
                  ▼            ▼           ▼          ▼           ▼
               Reject       Reject     Reject     Reject    Success/Fail
```

### 3.9 State Synchronization Model

| Mode | Description | Use Case |
|------|-------------|----------|
| **Full Sync** | Download all blocks, replay | New node, archive |
| **Fast Sync** | Download checkpoint + recent blocks | Quick bootstrap |
| **Snap Sync** | Download state trie + recent blocks | Fastest startup |
| **Light Sync** | Headers only + proofs | Resource-constrained |

---

## 4. Data Flow Overview

### 4.1 Transaction Lifecycle (8 Steps)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TRANSACTION LIFECYCLE FLOW                                      │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  ┌──────────┐
  │  CLIENT  │
  └────┬─────┘
       │
       │ ① Transaction Received (Network/RPC)
       ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                              NETWORKING LAYER                                        │
  │   • Decode transaction bytes                                                         │
  │   • Basic format validation                                                          │
  │   • Forward to mempool                                                               │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ② Mempool Validation          │
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                MEMPOOL LAYER                                         │
  │   • Signature verification                                                           │
  │   • Nonce check against current state                                               │
  │   • Balance sufficiency check                                                        │
  │   • Priority assignment                                                              │
  │   • Add to pending pool                                                              │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ③ Block Proposal              │
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                              CONSENSUS LAYER                                         │
  │   • Leader election (VRF + Stake + PoUW)                                            │
  │   • If elected: select transactions from mempool                                    │
  │   • Build block with PoUW receipts                                                  │
  │   • Sign and broadcast proposal                                                      │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ④ PoS Selection + PoUW Scoring│
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                              CONSENSUS LAYER                                         │
  │   • Validate block header                                                            │
  │   • Verify proposer eligibility                                                      │
  │   • Validate PoUW receipts                                                          │
  │   • Compute block weight                                                             │
  │   • Apply fork choice rule                                                           │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ⑤ Execution Engine Transition │
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                             EXECUTION ENGINE                                         │
  │   • Load parent state                                                                │
  │   • Execute transactions sequentially                                               │
  │   • Process PoUW receipts (rewards)                                                 │
  │   • Generate execution receipts                                                      │
  │   • Compute state diff                                                               │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ⑥ State Root Update           │
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                              STATE MACHINE                                           │
  │   • Apply state diff to trie                                                         │
  │   • Compute new state root                                                           │
  │   • Compute receipts root                                                            │
  │   • Verify roots match block header                                                  │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ⑦ Storage Commit              │
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                              STORAGE LAYER                                           │
  │   • Persist block to block store                                                     │
  │   • Persist state changes to trie                                                    │
  │   • Update indexes                                                                   │
  │   • Update canonical head pointer                                                    │
  │   • Create checkpoint if needed                                                      │
  └────────────────────────────────────┬─────────────────────────────────────────────────┘
                                       │
       │ ⑧ Broadcast to Network        │
       ▼                               ▼
  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                              NETWORKING LAYER                                        │
  │   • Gossip block to peers                                                            │
  │   • Remove included txs from mempool                                                │
  │   • Update sync status                                                               │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
                                  ┌─────────┐
                                  │FINALIZED│
                                  └─────────┘
```

### 4.2 Step-by-Step Description

| Step | Component | Actions | Outputs |
|------|-----------|---------|---------|
| **①** | Network | Receive, decode, validate format | Raw transaction |
| **②** | Mempool | Verify signature, check nonce/balance, prioritize | Pending tx |
| **③** | Consensus | Leader builds block from mempool | Block proposal |
| **④** | Consensus | Verify proposer, validate PoUW, compute weight | Valid block |
| **⑤** | Execution | Execute transactions, process receipts | State diff |
| **⑥** | State | Apply diff, compute roots | New state root |
| **⑦** | Storage | Persist block, state, indexes | Durable storage |
| **⑧** | Network | Broadcast, cleanup mempool | Network propagation |

---

## 5. Block Lifecycle Overview

### 5.1 Block Lifecycle Phases

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              BLOCK LIFECYCLE                                            │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────┐
  │  PROPOSAL       │  Leader creates block
  │  PHASE          │  • Select transactions
  │                 │  • Include PoUW receipts
  │                 │  • Sign block header
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  PRE-BLOCK      │  Validators verify
  │  CHECKS         │  • Header validity
  │                 │  • Proposer eligibility
  │                 │  • Parent exists
  │                 │  • Timestamp valid
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  EXECUTION      │  Process contents
  │  PHASE          │  • Execute all transactions
  │                 │  • Process PoUW receipts
  │                 │  • Compute state root
  │                 │  • Verify roots match
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  PoUW           │  Integrate compute
  │  INTEGRATION    │  • Validate receipts
  │                 │  • Update provider scores
  │                 │  • Distribute rewards
  │                 │  • Add to block weight
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  FINALITY       │  Achieve consensus
  │  (Placeholder)  │  • Attestations collected
  │                 │  • Checkpoint created
  │                 │  • Block considered final
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  STORAGE        │  Persist block
  │  COMMIT         │  • Write to disk
  │                 │  • Update indexes
  │                 │  • Prune old data
  └─────────────────┘
```

### 5.2 Block Metadata Structure

```rust
// Block header fields
BlockHeader {
    // Identity
    block_hash: Hash,
    parent_hash: Hash,
    block_number: u64,
    
    // Timing
    timestamp: u64,
    slot: u64,
    
    // State
    state_root: Hash,
    transactions_root: Hash,
    receipts_root: Hash,
    
    // PoUW
    compute_receipts_root: Hash,
    total_pouw_score: u64,
    
    // Consensus
    proposer: Address,
    proposer_signature: Signature,
}
```

### 5.3 Pre-Block Validation Checklist

| Check | Condition | Error |
|-------|-----------|-------|
| Parent exists | `parent_hash` in chain | `UnknownParent` |
| Block number | `number == parent.number + 1` | `InvalidNumber` |
| Timestamp | `timestamp > parent.timestamp` | `InvalidTimestamp` |
| Proposer | Valid for this slot | `InvalidProposer` |
| Signature | Valid over header | `InvalidSignature` |

---

## 6. Consensus Summary

### 6.1 Hybrid PoS + PoUW Model

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         HYBRID CONSENSUS MODEL                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           VALIDATOR SELECTION                                    │  │
│   │                                                                                  │  │
│   │    Eligibility Requirements:                                                    │  │
│   │    ─────────────────────────                                                    │  │
│   │    • Minimum stake: 10,000 MBO                                                 │  │
│   │    • Registered in validator set                                               │  │
│   │    • Not currently slashed                                                     │  │
│   │    • Online and responsive                                                     │  │
│   │                                                                                  │  │
│   │    Selection Formula:                                                           │  │
│   │    ─────────────────                                                            │  │
│   │    selection_score = (α × normalized_stake)                                    │  │
│   │                    + (β × normalized_pouw_score)                               │  │
│   │                    + VRF(slot_number, validator_key)                           │  │
│   │                                                                                  │  │
│   │    Where: α = 0.6, β = 0.4 (configurable)                                      │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           BLOCK SCORING                                          │  │
│   │                                                                                  │  │
│   │    block_weight = base_difficulty                                              │  │
│   │                 + Σ(attestation_weights)                                       │  │
│   │                 + Σ(pouw_receipt.score) × POUW_MULTIPLIER                      │  │
│   │                                                                                  │  │
│   │    Fork Choice: Select chain with highest cumulative weight                    │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Invalid Block Rejection

| Condition | Action | Consequence |
|-----------|--------|-------------|
| Invalid signature | Reject immediately | None |
| Wrong slot proposer | Reject block | None |
| Invalid PoUW receipts | Reject block | Proposer reputation penalty |
| State root mismatch | Reject block | Proposer potentially slashed |
| Double proposal | Reject + slash | Stake reduction |

### 6.3 Security Assumptions

```
┌────────────────────────────────────────────────────────────────┐
│                  SECURITY ASSUMPTIONS                          │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  1. HONEST MAJORITY                                           │
│     • >2/3 of stake controlled by honest validators           │
│     • Threshold required for finality                         │
│                                                                │
│  2. NETWORK SYNCHRONY                                         │
│     • Messages delivered within bounded time                  │
│     • Partial synchrony sufficient for liveness               │
│                                                                │
│  3. CRYPTOGRAPHIC HARDNESS                                    │
│     • Signature schemes unforgeable                           │
│     • Hash functions collision-resistant                      │
│     • VRF outputs unpredictable                               │
│                                                                │
│  4. ECONOMIC RATIONALITY                                      │
│     • Validators maximize expected returns                    │
│     • Slashing makes attacks unprofitable                     │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 6.4 GPU Compute Incentives

| Incentive | Mechanism |
|-----------|-----------|
| **Direct Rewards** | Fee payment for completed compute tasks |
| **PoUW Score** | Higher score increases block production probability |
| **Reputation** | Good track record leads to more task assignments |
| **Staking Boost** | Guardian nodes earn enhanced staking rewards |

---

## 7. Execution Pipeline Summary

### 7.1 Runtime Validation Flow

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         RUNTIME VALIDATION                                              │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  Transaction
       │
       ▼
  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
  │   FORMAT    │────▶│  SIGNATURE  │────▶│    NONCE    │────▶│   BALANCE   │
  │   CHECK     │     │   VERIFY    │     │    CHECK    │     │    CHECK    │
  └─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │                   │
       ▼                   ▼                   ▼                   ▼
    Reject if           Reject if          Reject if          Reject if
    malformed         sig invalid       nonce wrong      balance < fee
                                                                  │
                                                                  ▼
                                                          ┌─────────────┐
                                                          │   EXECUTE   │
                                                          │   (STATE)   │
                                                          └─────────────┘
```

### 7.2 State Machine Transitions

```rust
// State Transition Function
fn execute(state: State, tx: Transaction) -> (State, Receipt) {
    // 1. Deduct gas upfront
    state.deduct_balance(tx.sender, tx.max_fee);
    
    // 2. Increment nonce
    state.increment_nonce(tx.sender);
    
    // 3. Execute based on type
    let result = match tx.type {
        Transfer => transfer(state, tx.to, tx.amount),
        Stake => stake(state, tx.validator, tx.amount),
        Unstake => unstake(state, tx.validator, tx.amount),
        ComputeSubmit => submit_compute(state, tx.task_id, tx.result),
    };
    
    // 4. Refund unused gas
    state.add_balance(tx.sender, tx.max_fee - result.gas_used * tx.gas_price);
    
    // 5. Return new state and receipt
    (state, Receipt::from(result))
}
```

### 7.3 Determinism Constraints

| Allowed | Forbidden |
|---------|-----------|
| Fixed-point arithmetic | Floating-point |
| Ordered collections (BTreeMap) | Unordered (HashMap iteration) |
| Deterministic hashing | System random |
| Block-provided timestamp | System time |
| Explicit seeding | Implicit randomness |

### 7.4 Safety Rules

| Rule | Enforcement |
|------|-------------|
| Non-negative balances | Pre-execution check |
| Monotonic nonces | Strict validation |
| Gas limits | Runtime metering |
| State root consistency | Post-execution verification |
| No external I/O | Sandboxed execution |

### 7.5 Future: GPU-Accelerated Execution

```
Current Pipeline (CPU):
═══════════════════════════════════════════════════▶ Time
  Parse → Validate → Execute → Commit

Future Pipeline (CPU + GPU):
═════════════════════════════════▶ Time (faster)
  CPU: Parse → Dispatch ──────────────▶ Collect → Commit
                 │                           ▲
  GPU:           └── Batch Verify ───────────┘
                     Merkle Compute
                     Hash Operations
```

---

## 8. Storage Layer Summary

### 8.1 Storage Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              STORAGE ARCHITECTURE                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           LOGICAL STORES                                         │  │
│   │                                                                                  │  │
│   │   BLOCK STORE              STATE TRIE              INDEXES                      │  │
│   │   ───────────              ──────────              ───────                      │  │
│   │   header:{hash} → Header   account:{addr} → Acct   txhash:{h} → (block, idx)   │  │
│   │   body:{hash} → Body       storage:{addr}:{k} → V  addr:{a} → [tx_refs]        │  │
│   │   receipt:{hash} → Rcpt    code:{hash} → Bytes     height:{n} → block_hash     │  │
│   │   height:{n} → hash                                                             │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                          │                                              │
│                                          ▼                                              │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           KEY-VALUE ENGINE                                       │  │
│   │                                                                                  │  │
│   │   Implementation: RocksDB (default) / LevelDB / Custom                         │  │
│   │                                                                                  │  │
│   │   Features:                                                                     │  │
│   │   • LSM-tree structure for write optimization                                  │  │
│   │   • Bloom filters for read acceleration                                        │  │
│   │   • Compression (LZ4/Snappy)                                                   │  │
│   │   • Atomic batch writes                                                        │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 State Trie

| Property | Description |
|----------|-------------|
| **Structure** | Merkle Patricia Trie |
| **Key** | Account address (20 bytes) |
| **Value** | Account state (nonce, balance, code_hash, storage_root) |
| **Proof** | O(log n) inclusion/exclusion proofs |
| **Root** | 32-byte commitment to entire state |

### 8.3 Receipts

```rust
Receipt {
    tx_hash: Hash,
    status: Success | Failed,
    gas_used: u64,
    logs: Vec<Log>,
    state_root_after: Hash,  // Post-execution state root
}
```

### 8.4 Indexes

| Index | Purpose | Query Pattern |
|-------|---------|---------------|
| **TxHash** | Find transaction by hash | `GET tx:{hash}` |
| **Address** | Find transactions for address | `SCAN addr:{address}:*` |
| **BlockHeight** | Find block at height | `GET height:{number}` |
| **Validator** | Find validator info | `GET validator:{pubkey}` |

### 8.5 Checkpoints

```
Checkpoint Frequency: Every 1000 blocks

Contents:
├── state_snapshot.bin     # Full state trie
├── header.json            # Block header at checkpoint
├── validators.json        # Validator set snapshot
├── pouw_scores.json       # PoUW scores snapshot
└── proof.bin              # Merkle proof of validity

Use Cases:
• Fast sync for new nodes
• Disaster recovery
• State pruning anchor
• Archive verification
```

### 8.6 Persistence Rules

| Data Type | Retention | Prunable |
|-----------|-----------|----------|
| Block headers | Forever | No |
| Block bodies | Configurable | Yes (after checkpoint) |
| State trie | Latest + checkpoints | Yes (old versions) |
| Receipts | Configurable | Yes |
| Indexes | Forever | No |

---

## 9. Node Roles Summary

### 9.1 Node Type Comparison

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              NODE ROLES COMPARISON                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                              FULL NODE                                           │  │
│   │                                                                                  │  │
│   │   Capabilities:           Responsibilities:         Hardware:                   │  │
│   │   • Full validation       • Validate all blocks     • 4+ CPU cores             │  │
│   │   • Transaction relay     • Relay transactions      • 16 GB RAM                │  │
│   │   • State queries         • Serve RPC requests      • 500 GB SSD               │  │
│   │   • Sync serving          • Help peers sync         • 100 Mbps network         │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                            VALIDATOR NODE                                        │  │
│   │                                                                                  │  │
│   │   Capabilities:           Responsibilities:         Hardware:                   │  │
│   │   • All Full Node +       • Produce blocks          • 8+ CPU cores             │  │
│   │   • Block production      • Sign attestations       • 32 GB RAM                │  │
│   │   • Attestation signing   • Maintain uptime         • 1 TB NVMe SSD            │  │
│   │   • Staking               • Secure stake keys       • 1 Gbps network           │  │
│   │                                                                                  │  │
│   │   Requirements: Minimum stake of 10,000 MBO                                     │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                            GUARDIAN NODE                                         │  │
│   │                                                                                  │  │
│   │   Capabilities:           Responsibilities:         Hardware:                   │  │
│   │   • All Validator +       • Coordinate GPU jobs     • 16+ CPU cores            │  │
│   │   • GPU coordination      • Verify compute results  • 64 GB RAM                │  │
│   │   • PoUW verification     • Distribute tasks        • 2 TB NVMe SSD            │  │
│   │   • Compute scoring       • Submit PoUW proofs      • 10 Gbps network          │  │
│   │                                                     • GPU: A100/H100/RTX4090   │  │
│   │                                                     • 24+ GB VRAM              │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                          LIGHT NODE [FUTURE]                                     │  │
│   │                                                                                  │  │
│   │   Capabilities:           Responsibilities:         Hardware:                   │  │
│   │   • Header verification   • Verify headers          • 2 CPU cores              │  │
│   │   • Merkle proofs         • Request proofs          • 4 GB RAM                 │  │
│   │   • Minimal storage       • Minimal bandwidth       • 10 GB SSD                │  │
│   │   • Quick sync            • Mobile-friendly         • 10 Mbps network          │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Capability Matrix

| Capability | Full | Validator | Guardian | Light |
|------------|------|-----------|----------|-------|
| Full validation | ✓ | ✓ | ✓ | ✗ |
| Block production | ✗ | ✓ | ✓ | ✗ |
| Staking | ✗ | ✓ | ✓ | ✗ |
| GPU compute | ✗ | ✗ | ✓ | ✗ |
| PoUW scoring | ✗ | ✗ | ✓ | ✗ |
| Header proofs | ✓ | ✓ | ✓ | ✓ |
| Serve sync | ✓ | ✓ | ✓ | ✗ |

---

## 10. Network Model Summary

### 10.1 Message Types

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              MESSAGE TYPES                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CONTROL MESSAGES              DATA MESSAGES               SYNC MESSAGES              │
│   ────────────────              ─────────────               ─────────────              │
│   • Handshake                   • NewBlock                  • GetCheckpoint            │
│   • Ping/Pong                   • NewTransaction            • Checkpoint               │
│   • GetPeers/Peers              • GetBlocks/Blocks          • GetBlockBodies           │
│   • Disconnect                  • GetHeaders/Headers        • BlockBodies              │
│   • Status                      • GetReceipts/Receipts      • GetStateRange            │
│                                 • GetState/State            • StateRange               │
│                                                                                         │
│   CONSENSUS MESSAGES            COMPUTE MESSAGES                                       │
│   ──────────────────            ────────────────                                       │
│   • BlockProposal               • ComputeTaskAssign                                    │
│   • Attestation                 • ComputeResult                                        │
│   • AggregateAttestation        • PoUWProof                                           │
│   • ValidatorRegistration       • TaskVerification                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 10.2 Sync Flows

```
FULL SYNC (New Node):
─────────────────────
Peer A ◀──── GetHeaders(0, 1000) ────▶ Peer B
       ◀──── Headers[0..999] ─────────
       ◀──── GetBlocks(hashes) ──────▶
       ◀──── Blocks[...] ─────────────
       [Execute each block, verify state roots]

FAST SYNC (Checkpoint):
───────────────────────
Peer A ◀──── GetCheckpoint(latest) ──▶ Peer B
       ◀──── Checkpoint + Proof ──────
       [Verify proof, load state snapshot]
       ◀──── GetBlocks(checkpoint+1..) ▶
       ◀──── Blocks[...] ─────────────
       [Execute recent blocks only]

SNAP SYNC (State Download):
───────────────────────────
Peer A ◀──── GetStateRange(root, start, end) ──▶ Peer B
       ◀──── StateRange[accounts...] ──────────
       [Download state trie in parallel chunks]
       [Verify against state root]
```

### 10.3 Gossip Broadcasting

```
Transaction Gossip:
───────────────────
Node A receives new Tx
  │
  ├──▶ Validate locally
  │
  ├──▶ Add to mempool
  │
  └──▶ Gossip to N random peers (fanout)
         │
         ├──▶ Peer B ──▶ Gossip to peers...
         ├──▶ Peer C ──▶ Gossip to peers...
         └──▶ Peer D ──▶ Gossip to peers...

Block Gossip:
─────────────
Proposer creates block
  │
  └──▶ Broadcast to all validators (priority)
         │
         └──▶ Validators gossip to full nodes
                │
                └──▶ Full nodes gossip to peers
```

---

## 11. Security Architecture Summary

### 11.1 Threat Categories

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           SECURITY THREAT MATRIX                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   LAYER              THREATS                           MITIGATIONS                     │
│   ─────              ───────                           ───────────                     │
│                                                                                         │
│   CONSENSUS          • 51% attack                      • Stake distribution           │
│                      • Long-range attacks              • Checkpoints                  │
│                      • Nothing-at-stake               • Slashing                      │
│                      • Validator collusion             • VRF randomness              │
│                                                                                         │
│   EXECUTION          • Invalid state transitions      • Deterministic execution      │
│                      • Gas manipulation               • Metering                      │
│                      • Re-entrancy                    • State isolation              │
│                      • Overflow/underflow             • Safe math                     │
│                                                                                         │
│   NETWORKING         • Eclipse attacks                • Peer diversity               │
│                      • Sybil attacks                  • Peer scoring                 │
│                      • DoS attacks                    • Rate limiting                │
│                      • Man-in-middle                  • Encryption                   │
│                                                                                         │
│   STORAGE            • Data corruption                • Checksums                     │
│                      • State inconsistency            • Merkle proofs                │
│                      • Disk failure                   • Replication                  │
│                      • Malicious pruning              • Checkpoint verification      │
│                                                                                         │
│   COMPUTE (PoUW)     • Invalid results                • Replicated verification      │
│                      • Collusion                      • Random assignment            │
│                      • Resource exhaustion            • Task limits                  │
│                      • Score manipulation             • Fraud proofs                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 11.2 Consensus Security

| Threat | Description | Mitigation |
|--------|-------------|------------|
| **51% Attack** | Majority controls chain | High capital requirement, slashing |
| **Long-Range** | Rewrite old history | Finality checkpoints |
| **Nothing-at-Stake** | Validate multiple forks | Slashing for equivocation |
| **Grinding** | Manipulate leader selection | VRF-based randomness |

### 11.3 Execution Security

| Threat | Description | Mitigation |
|--------|-------------|------------|
| **Invalid State** | Incorrect transitions | Deterministic STF, verification |
| **Gas Attacks** | Resource exhaustion | Strict gas metering |
| **Replay** | Transaction reuse | Nonce + chain ID |
| **Overflow** | Arithmetic errors | Checked arithmetic |

### 11.4 Network Security

| Threat | Description | Mitigation |
|--------|-------------|------------|
| **Eclipse** | Isolate node from network | Diverse peer selection |
| **Sybil** | Fake node identities | Peer scoring, stake requirements |
| **DoS** | Overwhelm with requests | Rate limiting, banning |
| **MITM** | Intercept communications | libp2p encryption |

### 11.5 Storage Security

| Threat | Description | Mitigation |
|--------|-------------|------------|
| **Corruption** | Data integrity loss | Checksums, Merkle proofs |
| **Inconsistency** | State divergence | Atomic writes, verification |
| **Failure** | Hardware failure | Checkpoints, backups |

---

## 12. Future Roadmap

### 12.1 Development Timeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              DEVELOPMENT ROADMAP                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   2025 Q4 - 2026 Q1                    2026 Q2 - Q3                                    │
│   ─────────────────                    ───────────────                                 │
│   PHASE 1: FOUNDATION                  PHASE 2: OPTIMIZATION                          │
│   ☑ Core protocol                      ☐ GPU-accelerated execution                    │
│   ☑ Basic consensus                    ☐ Parallel transaction processing              │
│   ☑ Storage layer                      ☐ Advanced mempool                             │
│   ☐ Testnet launch                     ☐ Performance tuning                           │
│                                                                                         │
│   2025 Q4 - 2026 Q1                    2026+                                          │
│   ─────────────────                    ─────                                          │
│   PHASE 3: SMART CONTRACTS             PHASE 4: ECOSYSTEM                             │
│   ☐ WASM execution engine              ☐ Cross-chain bridges                          │
│   ☐ Contract deployment                ☐ Compute marketplace                          │
│   ☐ Standard library                   ☐ Developer ecosystem                          │
│   ☐ Developer tooling                  ☐ Enterprise integrations                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 12.2 GPU Compute Expansion

| Initiative | Description | Timeline |
|------------|-------------|----------|
| **Multi-GPU Farms** | Support coordinated GPU clusters | Q2 2025 |
| **Task Sharding** | Distribute large tasks across providers | Q3 2025 |
| **Model Training** | Enable distributed ML training | Q4 2025 |
| **Inference Marketplace** | Public AI inference API | 2026 |

### 12.3 Parallel Block Execution

```
Goal: 10x throughput improvement

Approach:
1. Dependency analysis at block creation
2. Transaction grouping by state access
3. Parallel execution of independent groups
4. Deterministic merge and commitment

Challenges:
• Conflict detection
• Deterministic ordering
• State isolation
```

### 12.4 ZK Transition

| Component | Status | Description |
|-----------|--------|-------------|
| **zkVM Integration** | Research | RISC Zero / SP1 evaluation |
| **State Proofs** | Design | Succinct state transition proofs |
| **Light Client** | Planned | ZK-verified sync |
| **Bridges** | Future | Cross-chain ZK proofs |

### 12.5 Smart Contract Layer

```
WASM Execution Engine:
──────────────────────
• Deterministic WASM runtime
• Gas metering at instruction level
• Sandboxed execution environment
• Host functions for state access

Contract Features:
──────────────────
• Arbitrary logic deployment
• Contract-to-contract calls
• Event emission
• Compute task requests (PoUW integration)
```

### 12.6 Cross-Chain Compute Marketplace

```
Vision:
───────
Global compute marketplace where any chain can submit tasks
and Mbongo validators provide verified computation results.

Components:
───────────
• Bridge contracts on partner chains
• Task relay network
• Cross-chain result verification
• Settlement and payment channels
• Reputation across ecosystems
```

---

## Quick Reference

### Key Metrics (Targets)

| Metric | Target |
|--------|--------|
| Block time | 2 seconds |
| Finality | ~12 seconds (6 blocks) |
| TPS (transfers) | 1,000+ |
| State size | Manageable pruning |
| Sync time (fast) | < 1 hour |

### Module Locations

| Module | Rust Crate |
|--------|------------|
| Networking | `network/` |
| Consensus | `pow/` |
| Execution | `runtime/` |
| Crypto | `crypto/` |
| Node | `node/` |
| CLI | `cli/` |

### Related Documentation

| Document | Purpose |
|----------|---------|
| `architecture_master_overview.md` | Detailed layer breakdown |
| `compute_engine_overview.md` | PoUW and GPU coordination |
| `execution_engine_overview.md` | Runtime and state machine |

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | November 2025 | Core Team | Initial document |

---

*This document is the primary entry point for understanding Mbongo Chain. For detailed specifications, refer to the linked documentation. For questions, open an issue in the main repository.*

