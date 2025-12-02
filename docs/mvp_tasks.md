# Mbongo Chain — MVP Development Tasks

> **Document Version:** 1.0.0
> **Last Updated:** December 2025
> **Status:** Planning Phase

---

## Table of Contents

1. [Overview](#1-overview)
2. [mbongo-core Tasks](#2-mbongo-core-tasks)
3. [mbongo-consensus Tasks](#3-mbongo-consensus-tasks)
4. [mbongo-verification Tasks](#4-mbongo-verification-tasks)
5. [mbongo-compute Tasks](#5-mbongo-compute-tasks)
6. [mbongo-network Tasks](#6-mbongo-network-tasks)
7. [mbongo-runtime Tasks](#7-mbongo-runtime-tasks)
8. [mbongo-api Tasks](#8-mbongo-api-tasks)
9. [mbongo-wallet Tasks](#9-mbongo-wallet-tasks)
10. [mbongo-node Tasks](#10-mbongo-node-tasks)
11. [Testing & Documentation](#11-testing--documentation)
12. [Summary & Roadmap](#12-summary--roadmap)

---

## 1. Overview

### MVP Goals

The Mbongo Chain MVP (Minimum Viable Product) aims to deliver:

1. **Functional Blockchain**: Basic block production, validation, and finality
2. **PoX Consensus**: Hybrid PoS + PoUW with AIDA regulation (simplified)
3. **Compute Marketplace**: Submit, execute, and verify compute tasks
4. **Developer Tools**: CLI, RPC API, and basic wallet
5. **Testnet Launch**: Public testnet with 10-20 validators

### MVP Scope

**In Scope:**
- Core blockchain primitives (blocks, transactions, accounts)
- Basic PoX consensus (PoS + simple PoC scoring)
- Redundant execution verification (3 validators)
- Simple compute task submission and execution
- P2P networking with libp2p
- JSON-RPC API
- CLI wallet and node management

**Out of Scope (Post-MVP):**
- Smart contracts / WASM runtime
- TEE integration (Phase 2)
- ZK-ML proofs (Phase 3)
- Advanced AIDA regulation
- Governance module
- Full tokenomics (simplified for testnet)

### Bounty Budget

Total MVP bounty allocation: **500,000 MBO** (from Bounty Program allocation)

---

## 2. mbongo-core Tasks

### 2.1 Core Data Structures

**User Story:**
**As a** blockchain developer
**I want** fundamental data structures (Block, Transaction, Account)
**So that** I can build higher-level protocol features

**Tasks:**

#### Task 2.1.1: Implement Block Structure
- **Complexity:** M (Medium)
- **Estimated Effort:** 2-3 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Block struct with header and body
- Header includes: parent_hash, state_root, transactions_root, timestamp, height
- Body includes: Vec<Transaction>
- Serialization/deserialization with serde
- Unit tests with 90%+ coverage

#### Task 2.1.2: Implement Transaction Structure
- **Complexity:** M (Medium)
- **Estimated Effort:** 2-3 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Transaction struct with sender, receiver, amount, nonce, signature
- Support for different transaction types (Transfer, ComputeTask, Stake)
- SCALE encoding/decoding
- Signature verification with ed25519
- Unit tests for all transaction types

#### Task 2.1.3: Implement Account Model
- **Complexity:** M (Medium)
- **Estimated Effort:** 2 days
- **Bounty:** 4,000 MBO

**Acceptance Criteria:**
- Account struct with balance, nonce, validator_data
- Account state transitions
- Balance transfer logic
- Nonce increment logic
- Unit tests for state transitions

---

### 2.2 Cryptography Module

**User Story:**
**As a** protocol engineer
**I want** cryptographic primitives (hashing, signing, verification)
**So that** I can secure transactions and blocks

**Tasks:**

#### Task 2.2.1: Implement Hashing Functions
- **Complexity:** S (Simple)
- **Estimated Effort:** 1 day
- **Bounty:** 2,000 MBO

**Acceptance Criteria:**
- BLAKE3 hashing wrapper
- SHA256 for compatibility
- Merkle tree construction
- Unit tests with known test vectors

#### Task 2.2.2: Implement Signature Scheme
- **Complexity:** M (Medium)
- **Estimated Effort:** 2 days
- **Bounty:** 4,000 MBO

**Acceptance Criteria:**
- Ed25519 key generation
- Transaction signing
- Signature verification
- Batch verification optimization
- Unit tests for all operations

---

### 2.3 Storage Layer

**User Story:**
**As a** node operator
**I want** persistent storage for blockchain data
**So that** the node can recover state after restart

**Tasks:**

#### Task 2.3.1: Implement RocksDB Integration
- **Complexity:** L (Large)
- **Estimated Effort:** 4-5 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- RocksDB wrapper with CRUD operations
- Column families for blocks, transactions, accounts
- Batch write support
- Snapshot support for state rollback
- Error handling and recovery
- Integration tests

#### Task 2.3.2: Implement State Trie
- **Complexity:** XL (Very Large)
- **Estimated Effort:** 1-2 weeks
- **Bounty:** 20,000 MBO

**Acceptance Criteria:**
- Merkle Patricia Trie implementation
- Insert, get, delete operations
- State root calculation
- Proof generation for state queries
- Benchmarks showing O(log n) operations
- Unit and integration tests

---

**Total mbongo-core Bounty: 50,000 MBO**

---

## 3. mbongo-consensus Tasks

### 3.1 PoS Consensus Engine

**User Story:**
**As a** validator
**I want** a Proof of Stake consensus mechanism
**So that** I can participate in block production based on my stake

**Tasks:**

#### Task 3.1.1: Implement Validator Registry
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 6,000 MBO

**Acceptance Criteria:**
- ValidatorSet struct managing active validators
- Register validator with stake (minimum 1,000 MBO)
- Unregister validator with cooldown period
- Update validator stake
- Validator state (Active, Inactive, Slashed)
- Unit tests

#### Task 3.1.2: Implement Weighted Random Selection
- **Complexity:** M (Medium)
- **Estimated Effort:** 2-3 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Weighted random selection algorithm
- Verifiable randomness from previous block hash
- Deterministic for same input
- Unit tests with distribution analysis

#### Task 3.1.3: Implement Block Production
- **Complexity:** L (Large)
- **Estimated Effort:** 5 days
- **Bounty:** 12,000 MBO

**Acceptance Criteria:**
- Block proposer selection
- Block template creation
- Transaction inclusion from mempool
- Block signing
- Block broadcast
- Integration tests

---

### 3.2 PoC (Proof of Compute) Scoring

**User Story:**
**As a** compute provider
**I want** my compute contributions to be scored
**So that** I earn higher validator weight

**Tasks:**

#### Task 3.2.1: Implement PoC Score Calculation
- **Complexity:** L (Large)
- **Estimated Effort:** 4-5 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- PoC score formula implementation
- Compute units (CU) tracking
- Reliability score based on task completion
- Validity score from verification
- Time decay function (30-day half-life)
- Unit tests with example scenarios

#### Task 3.2.2: Implement AIDA Regulator (Simplified)
- **Complexity:** L (Large)
- **Estimated Effort:** 5 days
- **Bounty:** 12,000 MBO

**Acceptance Criteria:**
- AIDA coefficient adjustment (C_SR, C_NL)
- Stake-to-work ratio monitoring
- Coefficient bounds (0.8-1.2)
- Coefficient sum = 2.0 constraint
- Adjustment every 1000 blocks
- Unit tests with various scenarios

#### Task 3.2.3: Implement PoX Weight Calculation
- **Complexity:** M (Medium)
- **Estimated Effort:** 2 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Formula implementation
- Stake weight with time multiplier
- Square root on PoC score
- Integration with validator selection
- Unit tests

---

**Total mbongo-consensus Bounty: 50,000 MBO**

---

## 4. mbongo-verification Tasks

### 4.1 Redundant Execution (Phase 1)

**User Story:**
**As a** protocol security engineer
**I want** redundant execution verification
**So that** compute results are trustworthy

**Tasks:**

#### Task 4.1.1: Implement Validator Assignment
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 6,000 MBO

**Acceptance Criteria:**
- Assign 3 random validators per compute task
- VRF-based selection for unpredictability
- No validator can verify own tasks
- Re-assignment on validator failure
- Unit tests

#### Task 4.1.2: Implement Result Aggregation
- **Complexity:** M (Medium)
- **Estimated Effort:** 2-3 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Collect results from 3 validators
- 2/3 majority consensus required
- Result hash comparison
- Slashing for divergent results
- Unit tests

---

### 4.2 Fraud Proof System

**User Story:**
**As a** network participant
**I want** fraud proof submission
**So that** I can challenge incorrect compute results

**Tasks:**

#### Task 4.2.1: Implement Fraud Proof Submission
- **Complexity:** L (Large)
- **Estimated Effort:** 4 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- Submit fraud proof transaction
- 100-block challenge period
- Include disputed result and proof data
- Challenger stake (collateral)
- Unit tests

#### Task 4.2.2: Implement Fraud Proof Arbitration
- **Complexity:** XL (Very Large)
- **Estimated Effort:** 1 week
- **Bounty:** 18,000 MBO

**Acceptance Criteria:**
- Re-execute disputed task
- Compare with fraud proof claim
- Slash malicious validator (20% stake)
- Reward whistleblower (10% of slashed amount)
- Refund if fraud proof invalid
- Integration tests

---

**Total mbongo-verification Bounty: 39,000 MBO**

---

## 5. mbongo-compute Tasks

### 5.1 Compute Task Execution

**User Story:**
**As a** compute provider
**I want** to execute compute tasks
**So that** I earn rewards and improve my PoC score

**Tasks:**

#### Task 5.1.1: Implement Task Submission API
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 6,000 MBO

**Acceptance Criteria:**
- ComputeTask struct with task_type, input_data, max_compute_units
- Task validation (size limits, format)
- Task queuing
- Task ID generation
- Unit tests

#### Task 5.1.2: Implement Simple Executor (Docker-based)
- **Complexity:** XL (Very Large)
- **Estimated Effort:** 1-2 weeks
- **Bounty:** 22,000 MBO

**Acceptance Criteria:**
- Docker container isolation
- Execute Python/Rust compute jobs
- Resource limits (CPU, RAM, time)
- Capture stdout/stderr
- Return result hash + output
- Error handling and cleanup
- Integration tests

#### Task 5.1.3: Implement Resource Metering
- **Complexity:** L (Large)
- **Estimated Effort:** 4 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- Track CPU time
- Track memory usage
- Track GPU usage (basic)
- Calculate compute units (CU)
- Enforce resource limits
- Unit tests

---

**Total mbongo-compute Bounty: 38,000 MBO**

---

## 6. mbongo-network Tasks

### 6.1 P2P Networking

**User Story:**
**As a** node operator
**I want** P2P networking with other nodes
**So that** I can sync blocks and propagate transactions

**Tasks:**

#### Task 6.1.1: Implement libp2p Integration
- **Complexity:** L (Large)
- **Estimated Effort:** 5-6 days
- **Bounty:** 12,000 MBO

**Acceptance Criteria:**
- libp2p transport (TCP)
- Noise protocol for encryption
- Yamux for multiplexing
- Peer discovery (mDNS for local, DHT for global)
- Connection management
- Integration tests

#### Task 6.1.2: Implement Gossipsub for Block Propagation
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 7,000 MBO

**Acceptance Criteria:**
- Gossipsub protocol setup
- Block topic subscription
- Block broadcast
- Block receipt and validation
- Duplicate detection
- Unit tests

#### Task 6.1.3: Implement Transaction Propagation
- **Complexity:** M (Medium)
- **Estimated Effort:** 2 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Transaction gossip topic
- Transaction broadcast
- Transaction receipt and validation
- Mempool integration
- Unit tests

---

### 6.2 RPC API

**User Story:**
**As a** developer
**I want** a JSON-RPC API
**So that** I can interact with the blockchain programmatically

**Tasks:**

#### Task 6.2.1: Implement JSON-RPC Server
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 7,000 MBO

**Acceptance Criteria:**
- HTTP server with Axum
- JSON-RPC 2.0 protocol
- Error handling
- CORS support
- Unit tests

#### Task 6.2.2: Implement Core RPC Methods
- **Complexity:** L (Large)
- **Estimated Effort:** 4-5 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- chain_getBlock, chain_getBlockByHeight methods
- chain_getTransaction method
- account_getBalance, account_getNonce methods
- tx_submit method
- Unit and integration tests

---

**Total mbongo-network Bounty: 41,000 MBO**

---

## 7. mbongo-runtime Tasks

### 7.1 Transaction Execution

**User Story:**
**As a** protocol engineer
**I want** transaction execution logic
**So that** state transitions are applied correctly

**Tasks:**

#### Task 7.1.1: Implement Transaction Executor
- **Complexity:** L (Large)
- **Estimated Effort:** 5 days
- **Bounty:** 12,000 MBO

**Acceptance Criteria:**
- Execute transfer transactions
- Execute stake transactions
- Execute compute task transactions
- State transition implementation
- Nonce validation
- Balance checks
- Integration tests

#### Task 7.1.2: Implement Gas Metering (Simplified)
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 6,000 MBO

**Acceptance Criteria:**
- Gas cost per transaction type
- Gas limit enforcement
- Gas fee calculation
- Refund unused gas
- Unit tests

---

**Total mbongo-runtime Bounty: 18,000 MBO**

---

## 8. mbongo-api Tasks

### 8.1 REST API

**User Story:**
**As a** frontend developer
**I want** a REST API
**So that** I can build web interfaces easily

**Tasks:**

#### Task 8.1.1: Implement REST Endpoints
- **Complexity:** M (Medium)
- **Estimated Effort:** 3-4 days
- **Bounty:** 7,000 MBO

**Acceptance Criteria:**
- GET /blocks, GET /blocks/{hash} endpoints
- GET /transactions/{hash} endpoint
- GET /accounts/{address}, GET /validators endpoints
- OpenAPI/Swagger documentation
- Unit tests

---

### 8.2 WebSocket Subscriptions

**User Story:**
**As a** developer
**I want** real-time updates via WebSocket
**So that** my app can react to chain events

**Tasks:**

#### Task 8.2.1: Implement WebSocket Server
- **Complexity:** L (Large)
- **Estimated Effort:** 4 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- WebSocket connection handling
- Subscribe to newBlocks, newTransactions
- Subscribe to accountUpdates(address)
- Unsubscribe mechanism
- Integration tests

---

**Total mbongo-api Bounty: 17,000 MBO**

---

## 9. mbongo-wallet Tasks

### 9.1 Key Management

**User Story:**
**As a** user
**I want** secure key storage
**So that** I can manage my MBO tokens

**Tasks:**

#### Task 9.1.1: Implement Keystore
- **Complexity:** L (Large)
- **Estimated Effort:** 4-5 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- Generate Ed25519 keypair
- Encrypt private key with password (AES-256-GCM)
- Store keystore to disk (JSON format)
- Load and decrypt keystore
- Mnemonic phrase generation (BIP39)
- Unit tests

#### Task 9.1.2: Implement Transaction Signing
- **Complexity:** M (Medium)
- **Estimated Effort:** 2 days
- **Bounty:** 5,000 MBO

**Acceptance Criteria:**
- Sign transfer transaction
- Sign stake transaction
- Sign compute task transaction
- Signature verification
- Unit tests

---

### 9.2 CLI Wallet

**User Story:**
**As a** user
**I want** a CLI wallet
**So that** I can interact with the blockchain from terminal

**Tasks:**

#### Task 9.2.1: Implement CLI Commands
- **Complexity:** L (Large)
- **Estimated Effort:** 5 days
- **Bounty:** 12,000 MBO

**Acceptance Criteria:**
- wallet create, wallet import commands
- wallet list, wallet balance commands
- wallet transfer, wallet stake commands
- Help text and examples
- Integration tests

---

**Total mbongo-wallet Bounty: 27,000 MBO**

---

## 10. mbongo-node Tasks

### 10.1 Full Node

**User Story:**
**As a** node operator
**I want** a full node binary
**So that** I can run a Mbongo Chain node

**Tasks:**

#### Task 10.1.1: Implement Node Orchestrator
- **Complexity:** XL (Very Large)
- **Estimated Effort:** 1-2 weeks
- **Bounty:** 25,000 MBO

**Acceptance Criteria:**
- Node startup and initialization
- Component lifecycle management
- Configuration loading from file
- Graceful shutdown
- Error handling and recovery
- Integration tests

#### Task 10.1.2: Implement Sync Engine
- **Complexity:** XL (Very Large)
- **Estimated Effort:** 1-2 weeks
- **Bounty:** 25,000 MBO

**Acceptance Criteria:**
- Request blocks from peers
- Validate and import blocks
- Handle chain reorgs
- Sync from genesis to current height
- Fast sync with state snapshots (optional)
- Integration tests

#### Task 10.1.3: Implement CLI Node Commands
- **Complexity:** M (Medium)
- **Estimated Effort:** 3 days
- **Bounty:** 7,000 MBO

**Acceptance Criteria:**
- node start, node stop commands
- node status, node logs commands
- Configuration via CLI flags
- Integration tests

---

**Total mbongo-node Bounty: 57,000 MBO**

---

## 11. Testing & Documentation

### 11.1 Integration Tests

**User Story:**
**As a** QA engineer
**I want** comprehensive integration tests
**So that** the system works end-to-end

**Tasks:**

#### Task 11.1.1: E2E Test Suite
- **Complexity:** XL (Very Large)
- **Estimated Effort:** 2 weeks
- **Bounty:** 25,000 MBO

**Acceptance Criteria:**
- Multi-node testnet setup
- Test block production and finality
- Test transaction submission and execution
- Test compute task lifecycle
- Test validator registration and rewards
- Test fraud proof submission
- CI/CD integration

---

### 11.2 Documentation

**User Story:**
**As a** developer
**I want** comprehensive documentation
**So that** I can understand and contribute to the project

**Tasks:**

#### Task 11.2.1: API Documentation
- **Complexity:** L (Large)
- **Estimated Effort:** 5 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- Complete rustdoc for all public APIs
- JSON-RPC API reference
- REST API OpenAPI spec
- Code examples for common operations
- Published to docs site

#### Task 11.2.2: Developer Guides
- **Complexity:** L (Large)
- **Estimated Effort:** 5 days
- **Bounty:** 10,000 MBO

**Acceptance Criteria:**
- Getting started guide
- Running a validator guide
- Running a compute provider guide
- Contributing guide updates
- Troubleshooting guide

---

**Total Testing & Documentation Bounty: 45,000 MBO**

---

## 12. Summary & Roadmap

### Total MVP Bounty Allocation

| Crate | Tasks | Estimated Bounty |
|-------|-------|------------------|
| **mbongo-core** | 7 tasks | 50,000 MBO |
| **mbongo-consensus** | 6 tasks | 50,000 MBO |
| **mbongo-verification** | 4 tasks | 39,000 MBO |
| **mbongo-compute** | 3 tasks | 38,000 MBO |
| **mbongo-network** | 5 tasks | 41,000 MBO |
| **mbongo-runtime** | 2 tasks | 18,000 MBO |
| **mbongo-api** | 2 tasks | 17,000 MBO |
| **mbongo-wallet** | 3 tasks | 27,000 MBO |
| **mbongo-node** | 3 tasks | 57,000 MBO |
| **Testing & Docs** | 3 tasks | 45,000 MBO |
| **TOTAL** | **38 tasks** | **382,000 MBO** |

### Complexity Distribution

| Complexity | Count | Total Bounty |
|------------|-------|--------------|
| **S** (Simple) | 1 | 2,000 MBO |
| **M** (Medium) | 18 | 111,000 MBO |
| **L** (Large) | 13 | 157,000 MBO |
| **XL** (Very Large) | 6 | 112,000 MBO |

### Development Phases

#### Phase 1: Foundation (Weeks 1-4)
**Goal:** Core data structures and storage

**Tasks:** mbongo-core (50,000 MBO), mbongo-runtime (18,000 MBO)

**Deliverable:** Basic blockchain with blocks, transactions, state

---

#### Phase 2: Consensus (Weeks 5-8)
**Goal:** PoX consensus implementation

**Tasks:** mbongo-consensus (50,000 MBO)

**Deliverable:** Working PoS + PoC consensus

---

#### Phase 3: Compute & Verification (Weeks 9-12)
**Goal:** Compute task execution and verification

**Tasks:** mbongo-compute (38,000 MBO), mbongo-verification (39,000 MBO)

**Deliverable:** Compute marketplace with redundant execution

---

#### Phase 4: Networking & APIs (Weeks 13-16)
**Goal:** P2P networking and developer APIs

**Tasks:** mbongo-network (41,000 MBO), mbongo-api (17,000 MBO)

**Deliverable:** Full node with RPC/REST APIs

---

#### Phase 5: Tooling & Launch (Weeks 17-20)
**Goal:** Wallet, CLI, and testnet launch

**Tasks:** mbongo-wallet (27,000 MBO), mbongo-node (57,000 MBO), Testing & Documentation (45,000 MBO)

**Deliverable:** Public testnet with 10-20 validators

---

### Critical Path

Priority tasks on the critical path:

1. **Core Data Structures** - Blocks everything
2. **Storage Layer** - Required for persistence
3. **PoS Consensus** - Required for block production
4. **Node Orchestrator** - Integrates all components
5. **Sync Engine** - Required for multi-node testnet
6. **Compute Executor** - Core differentiator

---

### Next Steps

1. **Create GitHub Issues**: Convert each task to GitHub issue with task and bounty labels
2. **Assign Bounties**: Committee reviews and approves bounty amounts
3. **Open for Contributors**: Announce in Discord and GitHub Discussions
4. **Track Progress**: Weekly check-ins and progress updates
5. **Iterate**: Adjust estimates and priorities based on feedback

---

**For more information:**
- Contributor Compensation: [contributor_compensation.md](contributor_compensation.md)
- Contributing Guide: [CONTRIBUTING.md](../CONTRIBUTING.md)
- Bounty Portal: https://github.com/mbongo-chain/mbongo-chain/issues

---

**Last Updated:** December 2025
**Next Review:** January 2026
**Document Owner:** Core Development Team
