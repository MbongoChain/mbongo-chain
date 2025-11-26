# Mbongo Chain — Architecture Overview

High-level system architecture for Mbongo Chain, a compute-first Layer 1 blockchain.

---

## 1. High-Level Summary

### What is Mbongo Chain?

**Mbongo Chain** is a next-generation Layer 1 blockchain designed from the ground up for verifiable compute, decentralized GPU coordination, and secure state execution. Built entirely in Rust, the protocol combines economic security with computational utility.

### Core Design Principles

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MBONGO CHAIN CORE PRINCIPLES                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  COMPUTE-FIRST ARCHITECTURE                                                 │
│  ──────────────────────────                                                 │
│  • Security computation produces real value (AI, ML, scientific)           │
│  • PoUW transforms waste energy into useful work                           │
│  • Native support for GPU-accelerated workloads                            │
│  • Deterministic execution for verifiable compute proofs                   │
│                                                                             │
│  HYBRID CONSENSUS (PoS + PoUW)                                              │
│  ─────────────────────────────                                              │
│  • PoS provides economic security and finality                             │
│  • PoUW validates useful computation                                       │
│  • Combined weight determines canonical chain                              │
│  • Sustainable security model                                              │
│                                                                             │
│  RUST-NATIVE FULL NODE                                                      │
│  ─────────────────────                                                      │
│  • No legacy code or compatibility layers                                  │
│  • Memory safety and performance by design                                 │
│  • Modern async/await patterns                                             │
│  • Modular crate architecture                                              │
│                                                                             │
│  SECURE EXECUTION PIPELINE                                                  │
│  ─────────────────────────                                                  │
│  • Secure mempool with prioritization                                      │
│  • Deterministic state machine                                             │
│  • Modular execution engine                                                │
│  • Verifiable state transitions                                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| **1-second block time** | Fast finality for responsive applications |
| **Hybrid consensus** | PoS for security, PoUW for compute utility |
| **Deterministic execution** | Bit-for-bit reproducible across nodes |
| **Modular architecture** | Clean separation of concerns |
| **GPU-ready** | Architecture supports GPU acceleration |
| **Developer-friendly** | Comprehensive docs, modern tooling |

---

## 2. System Diagram

### Full Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MBONGO CHAIN ARCHITECTURE                               │
└─────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────┐
                              │   EXTERNAL      │
                              │   CLIENTS       │
                              │  (RPC/WebSocket)│
                              └────────┬────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              NODE LAYER                                     │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         NETWORKING LAYER                            │   │
│  │                                                                     │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │   │
│  │  │  Peer    │  │  Gossip  │  │   Sync   │  │ Message  │            │   │
│  │  │ Discovery│  │ Protocol │  │  Engine  │  │  Router  │            │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                          MEMPOOL LAYER                              │   │
│  │                                                                     │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │   │
│  │  │    Tx    │  │ Priority │  │ Eviction │  │ Broadcast│            │   │
│  │  │Validation│  │  Queue   │  │  Policy  │  │  Logic   │            │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         CONSENSUS LAYER                             │   │
│  │                                                                     │   │
│  │  ┌────────────────────────┐  ┌────────────────────────┐            │   │
│  │  │       PoS ENGINE       │  │      PoUW ENGINE       │            │   │
│  │  │                        │  │                        │            │   │
│  │  │  • Stake Management    │  │  • Compute Tasks       │            │   │
│  │  │  • Leader Election     │  │  • Proof Verification  │            │   │
│  │  │  • Slashing Logic      │  │  • Scoring System      │            │   │
│  │  │  • Finality Gadget     │  │  • Receipt Validation  │            │   │
│  │  │                        │  │                        │            │   │
│  │  └───────────┬────────────┘  └────────────┬───────────┘            │   │
│  │              │                            │                         │   │
│  │              └──────────┬─────────────────┘                         │   │
│  │                         │                                           │   │
│  │              ┌──────────▼──────────┐                                │   │
│  │              │   FORK CHOICE RULE  │                                │   │
│  │              │                     │                                │   │
│  │              │  W = 0.7×Stake +    │                                │   │
│  │              │      0.3×Compute    │                                │   │
│  │              └─────────────────────┘                                │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                       STATE MACHINE LAYER                           │   │
│  │                                                                     │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │   │
│  │  │    Tx    │  │Execution │  │   Gas    │  │  Receipt │            │   │
│  │  │ Dispatch │  │  Engine  │  │ Metering │  │Generator │            │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │   │
│  │                        │                                            │   │
│  │              ┌─────────▼─────────┐                                  │   │
│  │              │  STATE TRANSITION │                                  │   │
│  │              │                   │                                  │   │
│  │              │  Pre → Exec → Post│                                  │   │
│  │              └───────────────────┘                                  │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                         STORAGE LAYER                               │   │
│  │                                                                     │   │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │   │
│  │  │  Block   │  │  State   │  │ Receipt  │  │Checkpoint│            │   │
│  │  │  Store   │  │  Trie    │  │  Store   │  │  Chain   │            │   │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Node Roles

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          NODE ROLES                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  FULL NODE                                                                  │
│  ─────────                                                                  │
│  • Validates all blocks and transactions                                   │
│  • Maintains complete state                                                │
│  • Relays transactions and blocks                                          │
│  • Does NOT produce blocks                                                 │
│                                                                             │
│  VALIDATOR NODE                                                             │
│  ──────────────                                                             │
│  • Full node + block production rights                                     │
│  • Stakes tokens for participation                                         │
│  • Proposes and signs blocks                                               │
│  • Subject to slashing for misbehavior                                     │
│                                                                             │
│  GUARDIAN NODE (Planned)                                                    │
│  ────────────────────────                                                   │
│  • Header-only validation                                                  │
│  • Checkpoint verification                                                 │
│  • Lightweight monitoring                                                  │
│  • No full state storage                                                   │
│                                                                             │
│  LIGHT NODE (Planned)                                                       │
│  ────────────────────                                                       │
│  • Minimal state (headers only)                                            │
│  • Relies on full nodes for proofs                                         │
│  • Suitable for mobile/constrained devices                                 │
│  • ZK proofs for verification (future)                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Network Topology

```
                              ┌─────────────┐
                              │  VALIDATOR  │
                              │    NODE     │
                              └──────┬──────┘
                                     │
         ┌───────────────────────────┼───────────────────────────┐
         │                           │                           │
         ▼                           ▼                           ▼
  ┌─────────────┐            ┌─────────────┐            ┌─────────────┐
  │  FULL NODE  │◀──────────▶│  FULL NODE  │◀──────────▶│  FULL NODE  │
  └──────┬──────┘            └──────┬──────┘            └──────┬──────┘
         │                          │                          │
         ▼                          ▼                          ▼
  ┌─────────────┐            ┌─────────────┐            ┌─────────────┐
  │  GUARDIAN   │            │   LIGHT     │            │  GUARDIAN   │
  │    NODE     │            │   NODE      │            │    NODE     │
  └─────────────┘            └─────────────┘            └─────────────┘
```

---

## 3. Execution Pipeline Overview

### Transaction Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     TRANSACTION LIFECYCLE                                   │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 1. TRANSACTION SUBMISSION                                                │
  │    User submits transaction via RPC                                     │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 2. TRANSACTION VALIDATION                                                │
  │    • Signature verification                                             │
  │    • Nonce check                                                        │
  │    • Balance check                                                      │
  │    • Gas limit validation                                               │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 3. GOSSIP PROPAGATION                                                    │
  │    • Broadcast to connected peers                                       │
  │    • Flood protection                                                   │
  │    • Deduplication                                                      │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 4. MEMPOOL PRIORITIZATION                                                │
  │    • Fee-based ordering                                                 │
  │    • Gas price sorting                                                  │
  │    • Account nonce ordering                                             │
  │    • Eviction of low-priority txs                                       │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 5. BLOCK BUILDING                                                        │
  │    • Validator selects transactions                                     │
  │    • Orders by priority                                                 │
  │    • Respects gas limit                                                 │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 6. BLOCK VALIDATION                                                      │
  │    • Header validation                                                  │
  │    • PoUW receipt verification                                          │
  │    • Transaction validation                                             │
  │    • State root comparison                                              │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 7. STATE TRANSITION                                                      │
  │    • Execute transactions                                               │
  │    • Update account states                                              │
  │    • Generate receipts                                                  │
  │    • Compute new state root                                             │
  └────────────────────────────────────────────────────────────┬─────────────┘
                                                               │
                                                               ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 8. FINALITY                                                              │
  │    • Block committed to chain                                           │
  │    • Checkpoint created (periodic)                                      │
  │    • State finalized                                                    │
  └──────────────────────────────────────────────────────────────────────────┘
```

### State Transition Flow

```
   Pre-State                    Execution                    Post-State
  ┌──────────┐              ┌──────────────┐              ┌──────────┐
  │          │              │              │              │          │
  │ Account  │──────────────│  Apply Tx    │──────────────│ Account  │
  │ Balances │              │              │              │ Balances │
  │          │              │  • Debit     │              │          │
  │ Nonces   │              │  • Credit    │              │ Nonces   │
  │          │              │  • Update    │              │          │
  │ Storage  │              │              │              │ Storage  │
  │          │              │              │              │          │
  └──────────┘              └──────────────┘              └──────────┘
       │                                                        │
       │                                                        │
       ▼                                                        ▼
  State Root A                                            State Root B
```

### Finality Model

| Stage | Description | Reversibility |
|-------|-------------|---------------|
| **Pending** | Transaction in mempool | Fully reversible |
| **Included** | Transaction in block | Reorganizable |
| **Confirmed** | Block has N confirmations | Unlikely reversal |
| **Finalized** | Checkpoint reached | Irreversible |

---

## 4. Consensus Overview

### Hybrid PoS + PoUW Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     HYBRID CONSENSUS MODEL                                  │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌───────────────────────────────┐    ┌───────────────────────────────┐
  │        PROOF OF STAKE         │    │    PROOF OF USEFUL WORK       │
  │           (70%)               │    │           (30%)               │
  ├───────────────────────────────┤    ├───────────────────────────────┤
  │                               │    │                               │
  │  • Economic security          │    │  • Compute verification       │
  │  • Validator selection        │    │  • Useful work scoring        │
  │  • Slashing penalties         │    │  • Receipt validation         │
  │  • Quick finality             │    │  • GPU market integration     │
  │                               │    │                               │
  └───────────────┬───────────────┘    └───────────────┬───────────────┘
                  │                                    │
                  └──────────────┬─────────────────────┘
                                 │
                                 ▼
                  ┌───────────────────────────────┐
                  │       FORK CHOICE RULE        │
                  │                               │
                  │   Weight = 0.7 × Stake +      │
                  │           0.3 × Compute       │
                  │                               │
                  │   Canonical = max(Weight)     │
                  └───────────────────────────────┘
```

### Chain Scoring (Placeholder)

```rust
/// Chain weight calculation (placeholder)
fn calculate_chain_weight(chain: &Chain) -> u128 {
    let stake_weight = chain.total_stake();           // PoS component
    let compute_weight = chain.total_compute_score(); // PoUW component
    
    // Weighted combination
    (stake_weight * 70 / 100) + (compute_weight * 30 / 100)
}
```

*Note: Exact scoring formula to be finalized in protocol specification.*

### Validator Responsibilities

| Responsibility | Description |
|----------------|-------------|
| **Stake tokens** | Lock tokens as collateral |
| **Propose blocks** | Create new blocks when selected |
| **Validate blocks** | Verify blocks from other validators |
| **Sign attestations** | Confirm block validity |
| **Maintain uptime** | Stay online and responsive |
| **Execute honestly** | Follow protocol rules |

### Guardian Responsibilities (Planned)

| Responsibility | Description |
|----------------|-------------|
| **Validate headers** | Verify block headers |
| **Check signatures** | Validate proposer signatures |
| **Track checkpoints** | Maintain finality checkpoints |
| **Report violations** | Flag invalid blocks |
| **Serve light clients** | Provide proofs to light nodes |

### Security Assumptions

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     SECURITY ASSUMPTIONS                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  HONEST MAJORITY                                                            │
│  • > 2/3 of stake is honest                                                │
│  • Honest validators follow protocol                                       │
│                                                                             │
│  NETWORK SYNCHRONY                                                          │
│  • Messages delivered within bounded time                                  │
│  • Network partitions are temporary                                        │
│                                                                             │
│  CRYPTOGRAPHIC HARDNESS                                                     │
│  • Hash functions are collision-resistant                                  │
│  • Signatures are unforgeable                                              │
│                                                                             │
│  COMPUTE VERIFIABILITY                                                      │
│  • PoUW tasks are deterministically verifiable                             │
│  • Compute proofs cannot be forged                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Networking Overview

### Peer Discovery

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     PEER DISCOVERY                                          │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 1. BOOTSTRAP                                                             │
  │    Node connects to hardcoded bootstrap nodes                           │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 2. PEER EXCHANGE (PEX)                                                   │
  │    Request peer lists from connected nodes                              │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 3. CONNECTION ESTABLISHMENT                                              │
  │    Connect to discovered peers, perform handshake                       │
  └──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
  ┌──────────────────────────────────────────────────────────────────────────┐
  │ 4. PEER SCORING                                                          │
  │    Track peer behavior, prioritize reliable peers                       │
  └──────────────────────────────────────────────────────────────────────────┘
```

### Peer Exchange (PEX) and Scoring

| Metric | Description | Weight |
|--------|-------------|--------|
| **Latency** | Response time | High |
| **Uptime** | Connection stability | Medium |
| **Data quality** | Valid blocks/txs provided | High |
| **Protocol compliance** | Follows message protocol | Critical |

### Transport Layer

| Feature | Current | Planned |
|---------|---------|---------|
| **Protocol** | TCP | QUIC |
| **Encryption** | TLS 1.3 | TLS 1.3 |
| **Multiplexing** | Single stream | Multi-stream |
| **0-RTT** | No | Yes |

### Message Types

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MESSAGE TYPES                                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  CONTROL MESSAGES                                                           │
│  • Ping/Pong           – Liveness check                                    │
│  • Handshake           – Protocol negotiation                              │
│  • PeerList            – Peer discovery                                    │
│                                                                             │
│  DATA MESSAGES                                                              │
│  • Transaction         – New transaction                                   │
│  • Block               – Full block                                        │
│  • BlockHeader         – Header only                                       │
│  • PoUWReceipt         – Compute proof                                     │
│                                                                             │
│  SYNC MESSAGES                                                              │
│  • GetHeaders          – Request headers                                   │
│  • GetBlocks           – Request block bodies                              │
│  • GetState            – Request state data                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Storage Overview

### Storage Components

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     STORAGE ARCHITECTURE                                    │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │                           BLOCK STORE                                    │
  │                                                                          │
  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐         │
  │  │  Headers   │  │   Bodies   │  │  Receipts  │  │   Index    │         │
  │  │            │  │            │  │            │  │            │         │
  │  │ hash→header│  │ hash→txs  │  │ hash→rcpts │  │height→hash │         │
  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘         │
  │                                                                          │
  └──────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │                           STATE TRIE                                     │
  │                                                                          │
  │  ┌────────────┐  ┌────────────┐  ┌────────────┐                         │
  │  │  Accounts  │  │  Balances  │  │  Storage   │                         │
  │  │            │  │            │  │            │                         │
  │  │ addr→acct  │  │ addr→bal   │  │ addr→data  │                         │
  │  └────────────┘  └────────────┘  └────────────┘                         │
  │                                                                          │
  │                    State Root: 0x...                                     │
  │                                                                          │
  └──────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │                         MEMPOOL STORAGE                                  │
  │                                                                          │
  │  ┌────────────┐  ┌────────────┐  ┌────────────┐                         │
  │  │  Pending   │  │ Priority Q │  │  By Sender │                         │
  │  │            │  │            │  │            │                         │
  │  │ hash→tx    │  │ fee→hash   │  │ addr→[tx]  │                         │
  │  └────────────┘  └────────────┘  └────────────┘                         │
  │                                                                          │
  └──────────────────────────────────────────────────────────────────────────┘

  ┌──────────────────────────────────────────────────────────────────────────┐
  │                        CHECKPOINT CHAIN                                  │
  │                                                                          │
  │  ┌────────────┐  ┌────────────┐  ┌────────────┐                         │
  │  │Checkpoint 1│──│Checkpoint 2│──│Checkpoint 3│──▶ ...                  │
  │  │ Height: 100│  │ Height: 200│  │ Height: 300│                         │
  │  │ Root: 0x.. │  │ Root: 0x.. │  │ Root: 0x.. │                         │
  │  └────────────┘  └────────────┘  └────────────┘                         │
  │                                                                          │
  └──────────────────────────────────────────────────────────────────────────┘
```

### Storage Summary

| Store | Content | Persistence | Size |
|-------|---------|-------------|------|
| **Block Store** | Headers, bodies, receipts | Permanent | ~100 GB/year |
| **State Trie** | Account states | Latest + archive | Variable |
| **Mempool** | Pending transactions | Ephemeral | ~100 MB |
| **Checkpoints** | Finality markers | Permanent | Small |

### Checkpoint Model

Checkpoints provide:
- **Finality guarantee** — Blocks before checkpoint are irreversible
- **Sync optimization** — New nodes can start from checkpoint
- **Light client support** — Minimal data for verification

---

## 7. Roadmap Links

### Core Documentation

| Document | Description |
|----------|-------------|
| [developer_introduction.md](developer_introduction.md) | Comprehensive introduction for new developers |
| [developer_environment.md](developer_environment.md) | Environment setup and tooling guide |
| [getting_started.md](getting_started.md) | Quick start guide (5 minutes) |
| [developer_workflow.md](developer_workflow.md) | Git workflow and contribution process |

### Architecture Deep Dives

| Document | Description |
|----------|-------------|
| [final_architecture_overview.md](final_architecture_overview.md) | Complete end-to-end architecture |
| [runtime_architecture.md](runtime_architecture.md) | Execution engine and state machine |
| [node_architecture.md](node_architecture.md) | Node internals and module interactions |
| [consensus_overview.md](consensus_overview.md) | PoS + PoUW consensus model |
| [networking_overview.md](networking_overview.md) | P2P networking layer |

### Validation & Sync

| Document | Description |
|----------|-------------|
| [block_validation_pipeline.md](block_validation_pipeline.md) | Block validation from proposal to commit |
| [consensus_validation.md](consensus_validation.md) | Consensus rules and validation |
| [state_machine_validation.md](state_machine_validation.md) | State transition validation |
| [sync_validation.md](sync_validation.md) | Chain synchronization mechanisms |

### Components

| Document | Description |
|----------|-------------|
| [mempool_overview.md](mempool_overview.md) | Transaction pool design and lifecycle |
| [guardian_status.md](guardian_status.md) | Guardian node role and architecture |

### Project Planning

| Document | Description |
|----------|-------------|
| [roadmap.md](roadmap.md) | Quarterly development roadmap |
| [setup_validation.md](setup_validation.md) | Environment validation checklist |

---

## Summary

Mbongo Chain is a compute-first Layer 1 blockchain with:

- **Hybrid PoS + PoUW consensus** for security and utility
- **Rust-native architecture** for performance and safety
- **Modular design** for maintainability and extensibility
- **Deterministic execution** for verifiable computation
- **Modern networking** with planned QUIC support
- **Comprehensive documentation** for developer onboarding

The architecture prioritizes:
1. Security through economic incentives and cryptographic proofs
2. Performance through efficient Rust implementation
3. Utility through Proof of Useful Work
4. Developer experience through comprehensive tooling and docs

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.
