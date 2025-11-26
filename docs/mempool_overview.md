# Mbongo Chain — Mempool Overview

This document describes the mempool design in Mbongo Chain, covering transaction lifecycle, storage, validation, and eviction policies.

---

## 1. Introduction

### What is the Mempool?

The **mempool** (memory pool) is a buffer that holds pending transactions before they are included in blocks. It serves as the gateway between users submitting transactions and validators producing blocks.

### Purpose

| Function | Description |
|----------|-------------|
| **Queuing** | Hold transactions waiting for block inclusion |
| **Validation** | Filter invalid transactions before propagation |
| **Prioritization** | Order transactions by fee and other criteria |
| **Propagation** | Share pending transactions with peers |

---

## 2. High-Level Mempool Design

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            MEMPOOL                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐     │
│  │    PENDING      │    │     READY       │    │     FUTURE      │     │
│  │     QUEUE       │    │     QUEUE       │    │     QUEUE       │     │
│  │                 │    │                 │    │                 │     │
│  │  Transactions   │    │  Transactions   │    │  Transactions   │     │
│  │  being          │───▶│  ready for      │    │  with nonce     │     │
│  │  validated      │    │  inclusion      │    │  gaps           │     │
│  └─────────────────┘    └─────────────────┘    └─────────────────┘     │
│                                │                       │               │
│                                │                       │               │
│                                ▼                       │               │
│                         ┌─────────────┐                │               │
│                         │   BLOCK     │◀───────────────┘               │
│                         │  BUILDER    │   (when gap fills)             │
│                         └─────────────┘                                │
│                                                                         │
├─────────────────────────────────────────────────────────────────────────┤
│  Indexes:                                                               │
│  • By Hash (O(1) lookup)                                               │
│  • By Sender (per-account ordering)                                    │
│  • By Fee (priority sorting)                                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### Design Principles

- **Bounded Size** — Fixed memory limits prevent resource exhaustion
- **Priority-Based** — Higher fees receive preferential treatment
- **Per-Sender Ordering** — Transactions ordered by nonce within sender
- **Fast Lookups** — Multiple indexes for efficient queries

---

## 3. Transaction Lifecycle

### Complete Lifecycle Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    TRANSACTION LIFECYCLE                                │
└─────────────────────────────────────────────────────────────────────────┘

  ┌──────────┐
  │  User    │
  │  Submit  │
  └────┬─────┘
       │
       ▼
  ┌──────────┐     ┌──────────┐     ┌──────────┐
  │ Receive  │────▶│ Validate │────▶│  Accept  │
  │          │     │          │     │          │
  └──────────┘     └────┬─────┘     └────┬─────┘
                        │                │
                   ┌────▼────┐           │
                   │ REJECT  │           │
                   │ Invalid │           │
                   └─────────┘           │
                                         ▼
                                    ┌──────────┐
                                    │  Queue   │
                                    │ (Ready/  │
                                    │  Future) │
                                    └────┬─────┘
                                         │
       ┌─────────────────────────────────┼─────────────────────────────────┐
       │                                 │                                 │
       ▼                                 ▼                                 ▼
  ┌──────────┐                     ┌──────────┐                     ┌──────────┐
  │ Gossip   │                     │ Include  │                     │  Evict   │
  │ to Peers │                     │ in Block │                     │ (Expire/ │
  │          │                     │          │                     │  Replace)│
  └──────────┘                     └────┬─────┘                     └──────────┘
                                        │
                                        ▼
                                   ┌──────────┐
                                   │ FINALIZE │
                                   │ On-Chain │
                                   └──────────┘
```

### Lifecycle Stages

| Stage | Description | Duration |
|-------|-------------|----------|
| **Received** | Transaction arrives from user or peer | Instant |
| **Validating** | Signature, nonce, balance checks | < 10ms |
| **Pending** | Awaiting validation completion | < 100ms |
| **Ready** | Valid, executable, awaiting inclusion | Variable |
| **Future** | Valid but blocked by nonce gap | Variable |
| **Included** | Added to proposed block | — |
| **Finalized** | Block containing tx is finalized | — |
| **Evicted** | Removed due to expiry/replacement | — |

---

## 4. Broadcast and Gossip Behavior

### Transaction Announcement

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   TRANSACTION GOSSIP                                    │
└─────────────────────────────────────────────────────────────────────────┘

  Node A (Originator)           Node B                    Node C
       │                          │                          │
       │                          │                          │
       │──── NewTxHashes ────────▶│                          │
       │     [hash1, hash2]       │                          │
       │                          │                          │
       │◀─── GetTxs ──────────────│                          │
       │     [hash1]              │                          │
       │                          │                          │
       │──── Transactions ───────▶│                          │
       │     [tx1]                │                          │
       │                          │                          │
       │                          │──── NewTxHashes ────────▶│
       │                          │     [hash1]              │
       │                          │                          │
       │                          │◀─── GetTxs ──────────────│
       │                          │     [hash1]              │
       │                          │                          │
       │                          │──── Transactions ───────▶│
       │                          │     [tx1]                │
       │                          │                          │
```

### Gossip Rules

| Rule | Description |
|------|-------------|
| **Announce First** | Send hash before full transaction |
| **Request on Demand** | Fetch full tx only if not seen |
| **Fan-Out Limit** | Gossip to √n peers (not all) |
| **Deduplication** | Track seen hashes, skip duplicates |
| **Rate Limit** | Max announcements per peer per second |

### Re-Broadcast Policy

Transactions are re-broadcast when:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   RE-BROADCAST CONDITIONS                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. New Peer Connection                                                │
│     └── Share recent high-priority transactions                        │
│                                                                         │
│  2. Timeout Without Inclusion                                          │
│     └── Re-announce after N blocks without inclusion                   │
│                                                                         │
│  3. Peer Request                                                       │
│     └── Respond to explicit GetTxs requests                            │
│                                                                         │
│  Limits:                                                                │
│  • Max 3 re-broadcasts per transaction                                 │
│  • Minimum 30 seconds between re-broadcasts                            │
│  • Stop after transaction age > 30 minutes                             │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Mempool Storage Structure

### Data Structures

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   MEMPOOL STORAGE                                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                     PRIMARY STORE                               │   │
│  │                                                                 │   │
│  │  HashMap<TxHash, PooledTransaction>                            │   │
│  │                                                                 │   │
│  │  • O(1) lookup by hash                                         │   │
│  │  • Stores full transaction data                                │   │
│  │  • Includes metadata (arrival time, fee, etc.)                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                     SENDER INDEX                                │   │
│  │                                                                 │   │
│  │  HashMap<Address, BTreeMap<Nonce, TxHash>>                     │   │
│  │                                                                 │   │
│  │  • O(1) lookup by sender                                       │   │
│  │  • Ordered by nonce within sender                              │   │
│  │  • Enables nonce gap detection                                 │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                     PRIORITY QUEUE                              │   │
│  │                                                                 │   │
│  │  BTreeSet<(EffectiveFee, TxHash)>                              │   │
│  │                                                                 │   │
│  │  • Sorted by effective fee (descending)                        │   │
│  │  • O(log n) insertion and removal                              │   │
│  │  • Enables efficient block building                            │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                     TIME INDEX                                  │   │
│  │                                                                 │   │
│  │  BTreeMap<Timestamp, Vec<TxHash>>                              │   │
│  │                                                                 │   │
│  │  • Sorted by arrival time                                      │   │
│  │  • Enables expiration scanning                                 │   │
│  │  • O(log n) range queries                                      │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### PooledTransaction Structure

```rust
// Conceptual structure
pub struct PooledTransaction {
    /// The transaction itself
    pub transaction: Transaction,
    
    /// Computed transaction hash
    pub hash: TxHash,
    
    /// When the transaction was received
    pub received_at: Timestamp,
    
    /// Effective fee (fee / gas_limit)
    pub effective_fee: u128,
    
    /// Current status
    pub status: TxStatus,
    
    /// Number of times gossiped
    pub gossip_count: u32,
    
    /// Peers that have seen this tx
    pub seen_by: HashSet<PeerId>,
}

pub enum TxStatus {
    Pending,    // Being validated
    Ready,      // Ready for inclusion
    Future,     // Nonce gap exists
}
```

### Memory Limits

| Parameter | Default | Description |
|-----------|---------|-------------|
| Max Transactions | 10,000 | Total transactions in pool |
| Max Per Sender | 100 | Transactions per address |
| Max Size | 64 MB | Total memory usage |
| Max Tx Size | 128 KB | Single transaction limit |

---

## 6. Validation Pipeline

### Validation Stages

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   VALIDATION PIPELINE                                   │
└─────────────────────────────────────────────────────────────────────────┘

  ┌──────────────┐
  │   Raw Tx     │
  │   Bytes      │
  └──────┬───────┘
         │
         ▼
  ┌──────────────┐     ┌─────────────────────────────────────────────┐
  │   DECODE     │────▶│  • Parse transaction format                 │
  │              │     │  • Reject malformed transactions            │
  └──────┬───────┘     └─────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────┐     ┌─────────────────────────────────────────────┐
  │  SIGNATURE   │────▶│  • Verify signature against sender          │
  │   CHECK      │     │  • Reject invalid signatures                │
  └──────┬───────┘     └─────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────┐     ┌─────────────────────────────────────────────┐
  │   NONCE      │────▶│  • Check nonce against account state        │
  │   CHECK      │     │  • Route to Future queue if gap exists      │
  └──────┬───────┘     └─────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────┐     ┌─────────────────────────────────────────────┐
  │  BALANCE     │────▶│  • Verify sender can pay max fee            │
  │   CHECK      │     │  • Reject if insufficient balance           │
  └──────┬───────┘     └─────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────┐     ┌─────────────────────────────────────────────┐
  │   GAS        │────▶│  • Verify gas limit within bounds           │
  │   CHECK      │     │  • Verify gas price meets minimum           │
  └──────┬───────┘     └─────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────┐     ┌─────────────────────────────────────────────┐
  │  DUPLICATE   │────▶│  • Check if tx hash already in pool         │
  │   CHECK      │     │  • Handle replacement if same nonce         │
  └──────┬───────┘     └─────────────────────────────────────────────┘
         │
         ▼
  ┌──────────────┐
  │   ACCEPT     │
  │   TO POOL    │
  └──────────────┘
```

### Validation Results

| Result | Action |
|--------|--------|
| **Valid + Ready** | Add to ready queue, gossip to peers |
| **Valid + Future** | Add to future queue, gossip to peers |
| **Invalid** | Reject, do not gossip, penalize sender peer |
| **Duplicate** | Ignore if identical, replace if higher fee |

---

## 7. Pool Eviction Rules

### Eviction Triggers

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   EVICTION RULES                                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  1. EXPIRATION                                                  │   │
│  │                                                                 │   │
│  │  Transaction Age > 30 minutes                                   │   │
│  │  └── Remove from all queues                                     │   │
│  │  └── Do not re-broadcast                                        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  2. REPLACEMENT                                                 │   │
│  │                                                                 │   │
│  │  New tx with same (sender, nonce) and higher fee               │   │
│  │  └── Remove old transaction                                     │   │
│  │  └── Insert new transaction                                     │   │
│  │  └── Fee increase must be >= 10%                                │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  3. CAPACITY OVERFLOW                                           │   │
│  │                                                                 │   │
│  │  Pool size exceeds limit                                        │   │
│  │  └── Evict lowest-fee transactions first                        │   │
│  │  └── Never evict if new tx has lower fee than minimum           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  4. SENDER LIMIT                                                │   │
│  │                                                                 │   │
│  │  Per-sender count exceeds limit (100)                          │   │
│  │  └── Evict oldest transactions from that sender                 │   │
│  │  └── Keep highest-fee transactions                              │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  5. INCLUSION                                                   │   │
│  │                                                                 │   │
│  │  Transaction included in finalized block                        │   │
│  │  └── Remove from pool                                           │   │
│  │  └── Promote dependent transactions from Future to Ready        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  6. INVALIDATION                                                │   │
│  │                                                                 │   │
│  │  State change invalidates transaction                           │   │
│  │  └── Sender balance now insufficient                            │   │
│  │  └── Nonce already used (different tx included)                 │   │
│  │  └── Remove immediately                                         │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Eviction Priority

When capacity is exceeded, evict in this order:

```
  Highest Priority to Evict
           │
           ▼
  ┌─────────────────┐
  │  Lowest Fee     │  ← Evict first
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  Oldest Age     │  ← Tie-breaker
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  Future Queue   │  ← Prefer evicting queued txs
  └────────┬────────┘
           │
           ▼
  ┌─────────────────┐
  │  Ready Queue    │  ← Last resort
  └─────────────────┘
           │
           ▼
  Lowest Priority to Evict
```

---

## 8. Block Building Integration

### Transaction Selection

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   BLOCK BUILDING                                        │
└─────────────────────────────────────────────────────────────────────────┘

  Block Builder                           Mempool
       │                                     │
       │─── Request Transactions ───────────▶│
       │    (gas_limit: 30M)                 │
       │                                     │
       │                         ┌───────────┴───────────┐
       │                         │  Select by priority   │
       │                         │  • Highest fee first  │
       │                         │  • Respect nonce order│
       │                         │  • Stay within limit  │
       │                         └───────────┬───────────┘
       │                                     │
       │◀── Ordered Transactions ────────────│
       │    [tx1, tx2, tx3, ...]             │
       │                                     │
       │                                     │
       │─── Mark as Pending ────────────────▶│
       │                                     │
       │                                     │
       │    (Block proposed)                 │
       │                                     │
       │─── Confirm Inclusion ──────────────▶│
       │    or                               │
       │─── Release (block failed) ─────────▶│
       │                                     │
```

### Selection Algorithm

```
1. Initialize: selected = [], gas_used = 0

2. For each sender in priority order:
   a. Get executable transactions (nonce-ordered)
   b. For each transaction:
      - If gas_used + tx.gas_limit > block_gas_limit: skip
      - If tx depends on unselected tx: skip
      - Add to selected, update gas_used

3. Return selected transactions in execution order
```

---

## Summary

The Mbongo Chain mempool provides efficient transaction queuing, validation, and prioritization. Its multi-index storage structure enables fast lookups while maintaining bounded resource usage. The eviction policy ensures fair access while protecting against spam and resource exhaustion.

For networking details, see [Networking Overview](networking_overview.md).

For runtime execution, see [Runtime Architecture](runtime_architecture.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

