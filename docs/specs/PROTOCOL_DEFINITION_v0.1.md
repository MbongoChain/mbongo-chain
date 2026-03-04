# Mbongo Chain Protocol Definition v0.1

**Version:** 0.1  
**Status:** DRAFT  
**Last Updated:** February 2025

---

## Scope

This document is the **canonical protocol definition** for Mbongo Chain. It reconciles implemented behavior (Phase 1) with the frozen Phase 2 scope for 2025. It does not duplicate vision or roadmap content; it defines what the protocol *is* and *does* today, and what is in scope for the 2025 devnet milestone.

**Audience:** Protocol implementers, contributors, auditors.

---

## Definition of Mbongo

**Mbongo Chain** is a specialized, compute-native Layer 1 blockchain designed for verifiable AI inference and GPU compute execution. It is **not** a general-purpose smart contract platform.

| Attribute | Value |
|-----------|-------|
| **Classification** | Specialized compute-native L1 for AI inference execution |
| **Scope** | Transfer execution today; compute marketplace and PoUW planned for future phases |
| **Geographic** | Global, neutral. No region-specific protocol behavior. |
| **Not** | General smart contracts, settlement-only layer, modular execution engine |

---

## Protocol Decision Matrix

| Component | IMPLEMENTED (Phase 1) | PLANNED (Phase 2, 2025) | OUT OF SCOPE (2025) |
|-----------|------------------------|-------------------------|---------------------|
| **Block structure** | parent_hash, state_root, transactions_root, timestamp, height | — | compute_receipts_root, proposer, PoUW fields |
| **Transaction types** | Transfer (executed); Stake/ComputeTask (structural only) | — | Stake/ComputeTask execution |
| **Consensus** | None. Single producer via RPC. | — | PoX, PoS, PoUW, AIDA, multi-validator |
| **Block production** | Manual `produce_block` RPC | Timer-triggered (optional), still single producer | Leader election, attestations |
| **Mempool** | Minimal in-memory (by_hash, by_sender_nonce, order) | — | Eviction by fee, size limits, fee ordering |
| **Networking** | HTTP JSON-RPC, REST | libp2p transport, block request/response sync | Transaction gossip, DHT, mDNS |
| **Sync** | None (single node) | Block request/response, bootstrap from genesis, height-based | Fork choice, reorg, longest-chain |
| **Node roles** | Single node type | Full node, Producer node, Follower node | Validator, Guardian, Light node |
| **Economics** | None | — | Block rewards, fees, slashing, AIDA |
| **Compute marketplace** | — | — | Task submission, PoUW verification, GPU coordination |

---

## Implemented (Phase 1) Summary

Phase 1 is **CLOSED** and **FROZEN**. Reference: [docs/PHASE_1_COMPLETE.md](../PHASE_1_COMPLETE.md).

### Delivered

- Block and transaction data structures (SCALE-encoded)
- Account model (balance, nonce)
- Transaction validation: signature, nonce, balance, replay (hash uniqueness)
- Transfer execution
- Persistent storage (RocksDB)
- Deterministic hashing (BLAKE3)
- RPC: `submit_transaction`, `produce_block`, `get_block_height`, `ping`
- REST: `GET /blocks`, `GET /blocks/{hash}`, `GET /transactions/{hash}`, `GET /accounts/{address}`, `GET /validators`
- Minimal in-memory mempool (insertion order, dedup by hash and (sender, nonce))
- Idempotent `submit_transaction` (same tx hash returns same hash)

### Explicit Non-Goals (Phase 1)

- Consensus protocol
- P2P block propagation
- Mempool eviction policy
- Fork choice rules
- Timed block production
- Gas metering
- Smart contracts

---

## Phase 2 (2025) Scope Freeze

Phase 2 deliverable for 2025: **Multi-node devnet minimal**.

### In Scope

1. **libp2p transport** — Node-to-node connections over libp2p (TCP, noise, yamux).
2. **Block request/response sync** — Request blocks by height or hash; respond with block data.
3. **Bootstrap from genesis** — New nodes fetch from height 0.
4. **Height-based sync** — Follower nodes request blocks sequentially by height.
5. **Single producer model** — One designated Producer node. Block production remains manual or timer-triggered; no consensus, no leader election.
6. **Node roles (Phase 2 minimal)**:
   - **Full node (devnet)** — Validates blocks, serves RPC/REST, may sync from peers.
   - **Producer node** — Single node that produces blocks (via `produce_block` RPC or timer).
   - **Follower node** — Sync-only; fetches blocks from Producer or Full nodes, does not produce.

### Explicitly Excluded from Phase 2 (2025)

- Consensus protocol (PoX, PoS, PoUW)
- Economics (block rewards, fees, slashing)
- Fork choice (longest-chain, reorg)
- Compute marketplace
- Transaction gossip
- Peer discovery beyond bootnodes (mDNS, DHT deferred)
- Mempool eviction, fee ordering, size limits
- TEE integration
- AIDA regulator

---

## Out of Scope (2025)

The following are **not** in scope for 2025. They are documented in vision/whitepaper/architecture docs but deferred.

- PoX consensus (PoS + PoUW + PoC)
- AIDA economic regulator
- Guardian nodes (GPU coordination)
- Compute marketplace (task submission, verification)
- TEE attestation
- ZK-ML proofs
- Fork handling (reorg, state revert)
- Finality gadget
- Gas model
- Smart contracts (WASM)
- Validator set management
- Slashing / economic penalties

---

## Canonical Interfaces

### RPC

**Canonical spec:** [docs/specs/rpc_v0.1.md](./rpc_v0.1.md)

- JSON-RPC 2.0 over HTTP POST
- Endpoint: `/rpc`
- Methods: `submit_transaction`, `produce_block`, `get_block_height`, `ping`
- Method names are **not** prefixed (e.g. `submit_transaction`, not `mbg_submitTransaction`)

### jsonrpc_v0.1.md

[docs/specs/jsonrpc_v0.1.md](./jsonrpc_v0.1.md) uses `mbg_`-prefixed method names (e.g. `mbg_getBlockNumber`, `mbg_sendTransaction`). It is **not** implemented. Status: **future/legacy**. Do not use for v0.1 implementation. May be adopted in a future protocol version.

### REST

REST API is implemented. Reference: [docs/openapi_reference.md](../openapi_reference.md). Endpoints: `GET /blocks`, `GET /blocks/{hash}`, `GET /transactions/{hash}`, `GET /accounts/{address}`, `GET /validators`.

---

## Data Types Summary

Only fields that exist today. No future fields.

### BlockHeader

| Field | Type | Description |
|-------|------|-------------|
| `parent_hash` | Hash (32 bytes) | Hash of parent block |
| `state_root` | Hash (32 bytes) | State root after block execution |
| `transactions_root` | Hash (32 bytes) | BLAKE3 commitment to body transactions |
| `timestamp` | u64 | Unix timestamp (seconds) |
| `height` | u64 | Block height (genesis = 0) |

### BlockBody

| Field | Type | Description |
|-------|------|-------------|
| `transactions` | Vec\<Transaction\> | Ordered list of transactions |

### Block

| Field | Type |
|-------|------|
| `header` | BlockHeader |
| `body` | BlockBody |

### Transaction

| Field | Type | Description |
|-------|------|-------------|
| `tx_type` | TransactionType | Transfer, ComputeTask, or Stake (Transfer only executed) |
| `sender` | Address (32 bytes) | Ed25519 public key |
| `receiver` | Address (32 bytes) | Destination |
| `amount` | u128 | Transfer amount |
| `nonce` | u64 | Replay protection |
| `signature` | [u8; 64] | Ed25519 over signing payload |

### Account

| Field | Type | Description |
|-------|------|-------------|
| `address` | Address (32 bytes) | Account identifier |
| `balance` | u128 | Non-negative balance |
| `nonce` | u64 | Monotonically increasing |

### Hash and Address

- **Hash:** 32-byte array. Display: `0x` + 64 hex chars.
- **Address:** 32-byte array (Ed25519 public key). Same display format.

---

## Block Production Model (Phase 2 Minimal)

- **Single producer.** One designated node produces blocks.
- **Trigger:** Manual `produce_block` RPC, or (Phase 2) configurable timer.
- **Flow:** 1) Ensure genesis exists; 2) Drain mempool (up to MAX_TX_PER_BLOCK); 3) Validate and execute each tx; 4) Persist block and state; 5) Update height index.
- **No consensus.** No attestations, no leader election, no finality gadget.
- **Deterministic.** Same mempool drain order yields same block. SCALE encoding, BLAKE3 hashing.

In Phase 2, the single producer is a configuration role, not a protocol rule. No election, no VRF, no randomness. Producer = config param (e.g. --producer).

---

## Sync Model (Phase 2)

- **Block request/response.** Follower requests blocks by height range or hash; peer responds with block data.
- **Bootstrap from genesis.** New node starts at height 0, fetches blocks sequentially.
- **Height-based sync.** Request blocks from `local_height + 1` to `peer_height`.
- **Single chain.** No fork choice. Producer is authoritative. Followers sync from Producer or Full nodes that have synced from Producer.
- **Validation.** Follower validates each received block: parent exists, height = parent.height + 1, transactions_root matches, transactions valid. Applies block to storage.

---

## Block Validity Rules

(Phase 1 rules; unchanged for Phase 2.)

1. **Parent linkage:** `block.header.parent_hash` equals the hash of the block at `height - 1`.
2. **Height monotonic:** `block.header.height == parent_height + 1`. No gaps.
3. **Deterministic SCALE hash:** Block hash = BLAKE3(SCALE_encode(block)). Same bytes → same hash.
4. **Transactions root:** `block.header.transactions_root` equals `compute_transactions_root(&block.body.transactions)`.
5. **Transaction validity:** Each tx in body passes validation (signature, nonce, balance, not already included).

---

## Transaction Validity Rules

1. **Signature:** Ed25519 verification over signing payload (tx_type, sender, receiver, amount, nonce).
2. **Nonce:** `tx.nonce == account.nonce` (next expected).
3. **Balance:** `account.balance >= tx.amount` for transfers.
4. **Idempotence:** Re-submitting same tx (by hash) returns same hash; no double enqueue.
5. **Inclusion at most once:** Transaction hash must not exist in storage (replay protection).

---

## Invariants

Storage and protocol invariants are defined in [docs/architecture/storage_invariants.md](../architecture/storage_invariants.md). Summary:

- **Account:** balance ≥ 0; nonce monotonically increasing.
- **Transaction:** Hash includes signature; included at most once; nonce ordering enforced.
- **Block:** Height strictly increasing; parent linkage required; deterministic SCALE hash.
- **Atomicity:** Block application is all-or-nothing (see storage_invariants.md).

For Phase 2, block application MUST be atomic. RocksDB implementation MUST use WriteBatch before multi-node sync is introduced.

---

## Compatibility / Versioning Rules

1. **Protocol version** is bumped when block format, transaction format, or wire protocol changes in a breaking way.
2. **RPC spec** ([rpc_v0.1.md](./rpc_v0.1.md)) is versioned. Breaking changes require a new spec file (e.g. rpc_v0.2.md).
3. **Doc change rules:** PRs touching storage must reference storage_invariants.md. PRs adding RPC methods must update the RPC spec.
4. **This PDD:** Status moves from DRAFT to FROZEN when accepted. Amendments require explicit approval and version bump.

---

## Open Questions

1. **RPC return format:** `produce_block` — Implementation returns block hash string only. Canonical spec [rpc_v0.1.md](./rpc_v0.1.md) says `{ block_hash: string, height: u64 }`. Resolve: update spec to match implementation, or extend implementation to return object.
2. **Sync protocol wire format:** Exact message types and encoding for block request/response over libp2p (to be defined in Phase 2 implementation).

---

## Docs Index Update Plan

The following documents should link to this PDD once it is FROZEN. **Do not edit them in this task**; this is a plan only.

| Document | Action |
|----------|--------|
| [README.md](../../README.md) | Add link to PDD in Documentation / Core Documentation section |
| [docs/PHASE_2_PLAN.md](../PHASE_2_PLAN.md) | Add "Canonical protocol definition: docs/specs/PROTOCOL_DEFINITION_v0.1.md" at top |
| [docs/ARCHITECTURE_GUARDRAILS.md](../ARCHITECTURE_GUARDRAILS.md) | Add reference to PDD for protocol scope |
| [docs/architecture/phase1_architecture.md](../architecture/phase1_architecture.md) | Add "See PROTOCOL_DEFINITION_v0.1.md for canonical protocol scope" |
| [docs/PHASE_1_COMPLETE.md](../PHASE_1_COMPLETE.md) | Add "Canonical protocol definition: docs/specs/PROTOCOL_DEFINITION_v0.1.md" |
| [docs/CONTRIBUTING.md](../CONTRIBUTING.md) | Add PDD to "Before contributing" / architecture reading list |
