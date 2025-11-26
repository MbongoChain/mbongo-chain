# Mbongo Chain — Node Architecture

This document describes the internal architecture of a Mbongo Chain node, including its responsibilities, module interactions, and processing pipelines.

---

## 1. Overview

### Purpose of the Node

The **node** is the core software component that participates in the Mbongo Chain network. It maintains a local copy of the blockchain state, validates transactions and blocks, and communicates with other nodes to achieve consensus.

### Role of Full Nodes

Full nodes are essential to the network's security and decentralization:

| Role | Description |
|------|-------------|
| **State Validation** | Independently verify all transactions and blocks |
| **Data Availability** | Store and serve blockchain data to peers |
| **Network Relay** | Propagate transactions and blocks across the network |
| **Consensus Participation** | Validators produce and attest to blocks |

### Node Responsibilities

A Mbongo Chain node performs five core functions:

1. **Networking** — Connect to peers, exchange messages, maintain topology
2. **Mempool** — Receive, validate, and queue pending transactions
3. **Block Production** — Assemble transactions into blocks (validators only)
4. **Consensus Participation** — Vote on blocks and finalize the chain
5. **Execution** — Process transactions and update chain state

---

## 2. High-Level Node Architecture

```
                           ┌─────────────────────────────────────┐
                           │            MBONGO NODE              │
                           └─────────────────────────────────────┘
                                            │
           ┌────────────────────────────────┼────────────────────────────────┐
           │                                │                                │
           ▼                                ▼                                ▼
┌─────────────────────┐          ┌─────────────────────┐          ┌─────────────────────┐
│     NETWORKING      │          │       MEMPOOL       │          │      CONSENSUS      │
│                     │◀────────▶│                     │◀────────▶│                     │
│  - Peer Discovery   │          │  - Tx Ingestion     │          │  - PoS Voting       │
│  - Message Router   │          │  - Tx Validation    │          │  - PoUW Integration │
│  - Gossip Protocol  │          │  - Priority Queue   │          │  - Finality         │
└─────────┬───────────┘          └─────────┬───────────┘          └─────────┬───────────┘
          │                                │                                │
          │                                ▼                                │
          │                    ┌─────────────────────┐                      │
          │                    │    BLOCK BUILDER    │                      │
          │                    │                     │                      │
          │                    │  - Tx Selection     │                      │
          │                    │  - Block Assembly   │                      │
          │                    │  - Header Creation  │                      │
          │                    └─────────┬───────────┘                      │
          │                              │                                  │
          │                              ▼                                  │
          │                    ┌─────────────────────┐                      │
          │                    │       RUNTIME       │                      │
          │                    │                     │                      │
          │                    │  - State Machine    │                      │
          │                    │  - Tx Execution     │                      │
          │                    │  - State Transition │                      │
          │                    └─────────┬───────────┘                      │
          │                              │                                  │
          │                              ▼                                  │
          │                    ┌─────────────────────┐                      │
          │                    │       STORAGE       │                      │
          │                    │                     │                      │
          │                    │  - Block Store      │◀─────────────────────┘
          │                    │  - State Store      │
          │                    │  - Receipt Store    │
          └───────────────────▶└─────────────────────┘
                                         │
                                         ▼
                               ┌─────────────────────┐
                               │    GOSSIP OUTPUT    │
                               │                     │
                               │  - Block Broadcast  │
                               │  - Tx Propagation   │
                               └─────────────────────┘
```

---

## 3. Node Responsibilities (Detailed)

### 3.1 Peer Discovery

The node maintains connections to other nodes in the network.

| Function | Description |
|----------|-------------|
| Bootstrap | Connect to seed nodes on startup |
| Discovery | Find new peers via DHT or gossip |
| Connection Management | Maintain target peer count (e.g., 25-50 peers) |
| Peer Rotation | Periodically refresh connections for network health |

### 3.2 Chain Syncing

New or restarted nodes must synchronize with the network.

**Sync Modes:**
- **Full Sync** — Download and verify all blocks from genesis
- **Fast Sync** — Download state snapshot + recent blocks (planned)
- **Checkpoint Sync** — Start from trusted checkpoint (planned)

**Sync Process:**
1. Request block headers from peers
2. Validate header chain (PoS signatures)
3. Download block bodies in parallel
4. Execute blocks to reconstruct state
5. Verify state root matches header

### 3.3 Block Validation

Every received block undergoes validation:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Header    │────▶│   Parent    │────▶│    Body     │────▶│  Execution  │
│  Validation │     │  Reference  │     │  Validation │     │ Verification│
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
```

**Validation Checks:**
- Header format and field validity
- Parent block exists and is valid
- Timestamp within acceptable bounds
- Transaction signatures valid
- State root matches execution result

### 3.4 State Execution

The node executes transactions to update chain state.

**Execution Guarantees:**
- **Deterministic** — Same inputs always produce same outputs
- **Isolated** — Transactions cannot interfere with each other
- **Metered** — Compute costs are tracked and limited

### 3.5 Block Production (Validators)

Validators produce blocks when selected by consensus.

**Production Steps:**
1. Receive slot assignment from consensus
2. Select transactions from mempool
3. Execute transactions in order
4. Compute new state root
5. Assemble block header
6. Sign and broadcast block

### 3.6 Block and Transaction Broadcasting

The node propagates data to maintain network consistency.

| Data Type | Propagation Method |
|-----------|-------------------|
| New Blocks | Immediate gossip to all peers |
| Transactions | Gossip to subset of peers |
| Attestations | Aggregate and broadcast |

---

## 4. Module Interactions

The `node` crate orchestrates interactions between workspace modules.

### Dependency Graph

```
                    ┌──────────────┐
                    │     node     │
                    └──────┬───────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│   runtime    │  │   network    │  │     pow      │
└──────┬───────┘  └──────────────┘  └──────┬───────┘
       │                                   │
       └───────────────┬───────────────────┘
                       │
                       ▼
               ┌──────────────┐
               │    crypto    │
               └──────────────┘
```

### Crate Responsibilities

| Crate | Responsibility | Used By |
|-------|---------------|---------|
| **crypto** | Hashing, signatures, keypairs | All modules |
| **network** | P2P communication, gossip | node |
| **runtime** | State machine, execution | node |
| **pow** | Compute proof verification | node, runtime |
| **node** | Orchestration, lifecycle | cli |
| **cli** | User interface | End users |

### Inter-Module Communication

```rust
// Node startup sequence (conceptual)
impl Node {
    pub fn start(&mut self) {
        // 1. Initialize crypto subsystem
        self.crypto.init();
        
        // 2. Load state from storage
        self.runtime.load_state();
        
        // 3. Start networking
        self.network.start();
        
        // 4. Begin sync if needed
        self.sync_manager.sync();
        
        // 5. Start consensus participation
        self.consensus.start();
        
        // 6. Enter main event loop
        self.run_event_loop();
    }
}
```

---

## 5. Block Processing Pipeline

### Complete Pipeline Flow

```
┌───────────────────────────────────────────────────────────────────────────┐
│                         BLOCK PROCESSING PIPELINE                         │
└───────────────────────────────────────────────────────────────────────────┘

  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
  │   Tx    │───▶│ Mempool │───▶│  Block  │───▶│ Runtime │───▶│  State  │
  │ Admit   │    │  Queue  │    │ Builder │    │  Exec   │    │ Commit  │
  └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
       │              │              │              │              │
       ▼              ▼              ▼              ▼              ▼
  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
  │Signature│    │Priority │    │  Header │    │  Apply  │    │ Merkle  │
  │  Check  │    │  Sort   │    │ Create  │    │   Txs   │    │  Root   │
  └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
                                     │              │              │
                                     ▼              ▼              ▼
                                ┌─────────┐    ┌─────────┐    ┌─────────┐
                                │  Seal   │◀───│ Verify  │◀───│ Persist │
                                │  Block  │    │ Results │    │  State  │
                                └────┬────┘    └─────────┘    └─────────┘
                                     │
                                     ▼
                                ┌─────────┐
                                │ Gossip  │
                                │Broadcast│
                                └─────────┘
```

### Stage Details

#### 5.1 Transaction Admission

**Input:** Raw transaction from user or peer

**Process:**
1. Decode transaction format
2. Verify signature against sender
3. Check nonce ordering
4. Validate basic constraints (size, gas limit)
5. Add to mempool if valid

**Output:** Transaction in mempool or rejection

#### 5.2 Pre-Validation

**Input:** Transaction from mempool

**Process:**
1. Re-verify signature (may have changed)
2. Check sender balance for fees
3. Verify nonce against current state
4. Estimate execution cost

**Output:** Validated transaction ready for inclusion

#### 5.3 Runtime Execution

**Input:** Ordered list of transactions

**Process:**
1. Begin state transaction
2. For each transaction:
   - Load sender account
   - Deduct fees
   - Execute operation
   - Update receiver state
   - Generate receipt
3. Compute state diff

**Output:** State changes and receipts

#### 5.4 State Transition

**Input:** Execution results

**Process:**
1. Apply state changes to trie
2. Compute new state root
3. Verify deterministic execution

**Output:** New state root hash

#### 5.5 Block Sealing

**Input:** Transactions, state root, metadata

**Process:**
1. Assemble block header
2. Include consensus data (PoS signature)
3. Attach PoUW proofs if applicable
4. Compute block hash

**Output:** Complete, signed block

#### 5.6 Gossip Propagation

**Input:** Sealed block

**Process:**
1. Announce block hash to peers
2. Serve block on request
3. Track propagation status

**Output:** Block available network-wide

---

## 6. Mempool Overview

The mempool manages pending transactions awaiting inclusion in blocks.

### Mempool Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                           MEMPOOL                               │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │  Pending Queue  │  │  Ready Queue    │  │  Future Queue   │  │
│  │                 │  │                 │  │                 │  │
│  │  (Validating)   │  │  (Executable)   │  │  (Nonce Gap)    │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│  Priority: Fee per Gas × Urgency Factor                         │
│  Capacity: 10,000 transactions (configurable)                   │
│  Expiry: 30 minutes (configurable)                              │
└─────────────────────────────────────────────────────────────────┘
```

### Transaction Lifecycle

| Stage | Description |
|-------|-------------|
| **Received** | Transaction arrives from user or peer |
| **Pending** | Undergoing validation |
| **Ready** | Valid and executable, awaiting inclusion |
| **Future** | Valid but blocked by nonce gap |
| **Included** | Added to a block |
| **Dropped** | Expired, replaced, or invalid |

### Prioritization

Transactions are ordered by:

1. **Fee Priority** — Higher fee per gas = higher priority
2. **Arrival Time** — Earlier arrivals preferred (tie-breaker)
3. **Sender Nonce** — Maintain per-sender ordering

### Broadcasting

- New transactions are broadcast to a random subset of peers
- Re-broadcast occurs if not included within timeout
- Duplicate transactions are filtered by hash

---

## 7. Networking Layer Overview

### Message Types

| Category | Messages |
|----------|----------|
| **Handshake** | `Hello`, `Status`, `Disconnect` |
| **Sync** | `GetHeaders`, `Headers`, `GetBodies`, `Bodies` |
| **Gossip** | `NewBlock`, `NewTransaction`, `Attestation` |
| **Request** | `GetState`, `GetReceipts`, `GetProofs` |

### Message Flow

```
┌──────────┐                              ┌──────────┐
│  Node A  │                              │  Node B  │
└────┬─────┘                              └────┬─────┘
     │                                         │
     │──────── Hello (version, chain) ────────▶│
     │◀─────── Hello (version, chain) ─────────│
     │                                         │
     │──────── Status (head, height) ─────────▶│
     │◀─────── Status (head, height) ──────────│
     │                                         │
     │◀─────── NewBlock (hash, block) ─────────│
     │──────── GetBlock (hash) ───────────────▶│
     │◀─────── Block (data) ───────────────────│
     │                                         │
```

### Gossip Mechanism

**Block Gossip:**
1. Producer broadcasts `NewBlock` announcement
2. Peers request full block if not seen
3. Validated blocks are re-announced

**Transaction Gossip:**
1. Originator broadcasts `NewTransaction`
2. Peers validate and add to mempool
3. Validated transactions re-broadcast to subset

### Peer Scoring

*Status: Placeholder for future implementation*

Peers will be scored based on:
- Response latency
- Data validity
- Protocol compliance
- Uptime and availability

Low-scoring peers will be deprioritized or disconnected.

---

## 8. Storage Layer

### Data Categories

| Category | Contents | Access Pattern |
|----------|----------|----------------|
| **Blocks** | Full block data (header + body) | Sequential + random |
| **Headers** | Block headers only | Sequential + random |
| **State** | Account balances, nonces, data | Random access |
| **Receipts** | Transaction execution results | Random access |
| **Indices** | Block number → hash, tx hash → block | Lookup |

### Storage Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        STORAGE LAYER                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │   Block     │  │   State     │  │   Index     │              │
│  │   Store     │  │   Store     │  │   Store     │              │
│  │             │  │             │  │             │              │
│  │  - Headers  │  │  - Accounts │  │  - Height   │              │
│  │  - Bodies   │  │  - Storage  │  │  - TxHash   │              │
│  │  - Receipts │  │  - Code     │  │  - Address  │              │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘              │
│         │                │                │                     │
│         └────────────────┼────────────────┘                     │
│                          │                                      │
│                    ┌─────▼─────┐                                │
│                    │  Backend  │                                │
│                    │           │                                │
│                    │ In-Memory │  ← Current implementation      │
│                    │ RocksDB   │  ← Production (planned)        │
│                    └───────────┘                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Data Access Patterns

```rust
// Block retrieval (conceptual)
impl BlockStore {
    fn get_block_by_hash(&self, hash: &Hash) -> Option<Block>;
    fn get_block_by_number(&self, number: u64) -> Option<Block>;
    fn get_header(&self, hash: &Hash) -> Option<BlockHeader>;
    fn store_block(&mut self, block: &Block) -> Result<()>;
}

// State access (conceptual)
impl StateStore {
    fn get_account(&self, address: &Address) -> Option<Account>;
    fn get_storage(&self, address: &Address, key: &Key) -> Option<Value>;
    fn apply_changes(&mut self, changes: StateChanges) -> Result<Hash>;
}
```

---

## 9. Consensus Role

### PoS Participation

The node participates in Proof of Stake consensus:

| Role | Responsibility |
|------|---------------|
| **Validator** | Produce blocks, sign attestations |
| **Full Node** | Verify blocks, maintain state |
| **Light Client** | Verify headers only (planned) |

### Validator Duties

```
Epoch Timeline
──────────────────────────────────────────────────────────────────▶
│ Slot 0  │ Slot 1  │ Slot 2  │ ... │ Slot N  │
├─────────┼─────────┼─────────┼─────┼─────────┤
│ Produce │ Attest  │ Attest  │ ... │ Produce │
│  Block  │         │         │     │  Block  │
```

**Block Production:**
1. Check if assigned to current slot
2. Build block from mempool
3. Execute and seal block
4. Broadcast to network

**Attestation:**
1. Receive block for slot
2. Validate block
3. Sign attestation vote
4. Broadcast attestation

### PoUW Integration

Proof of Useful Work results are integrated into consensus:

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Compute Task   │────▶│   Proof Gen     │────▶│  Block Include  │
│  (Off-chain)    │     │   (Provider)    │     │   (On-chain)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
                                                        ▼
                                               ┌─────────────────┐
                                               │ Proof Verified  │
                                               │ Reward Issued   │
                                               └─────────────────┘
```

**Integration Points:**
- Compute proofs submitted as special transactions
- Runtime verifies proofs during execution
- Valid proofs trigger reward distribution
- Invalid proofs are rejected (no penalty to submitter)

---

## 10. Future Extensions

### Light Clients

*Status: Planned*

Light clients verify the chain without storing full state:

- Download block headers only
- Verify PoS signatures on headers
- Request state proofs on-demand
- Suitable for mobile and browser environments

### Snap Sync

*Status: Planned*

Fast synchronization for new nodes:

- Download recent state snapshot
- Verify snapshot against known checkpoint
- Sync only recent blocks
- Reduces sync time from hours to minutes

### Multi-Engine Parallel Execution

*Status: Research*

Parallel transaction execution for higher throughput:

- Detect independent transactions
- Execute in parallel across CPU cores
- Merge results deterministically
- Potential 4-8x throughput improvement

### Additional Roadmap

- [ ] Archive node mode (full historical state)
- [ ] State pruning (reduce storage requirements)
- [ ] WebSocket API for real-time updates
- [ ] Metrics and monitoring integration
- [ ] Remote procedure call (RPC) server

---

## Summary

The Mbongo Chain node is a modular system that coordinates networking, mempool management, block production, consensus participation, and state execution. Its architecture enables scalable, secure, and efficient blockchain operation.

For implementation details, see the source code in `/node/src/`.

For the overall architecture, see [Architecture Overview](architecture_overview.md).

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

