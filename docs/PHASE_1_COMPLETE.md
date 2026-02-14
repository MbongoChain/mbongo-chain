# Phase 1 — Foundation Complete

**Status:** CLOSED  
**Date:** December 2025

---

## Scope of Phase 1

Phase 1 delivered the foundational blockchain primitives required for a single-node, manually-driven chain:

- Block and transaction data structures
- Account model with balance and nonce
- Transaction validation and execution
- Persistent storage with RocksDB
- Deterministic serialization (SCALE) and hashing (BLAKE3)
- RPC and REST API endpoints

---

## Execution Engine Summary

The execution engine applies transactions to account state in order. For each transaction:

1. Validate signature
2. Check nonce matches account nonce
3. Check balance sufficient for transfer amount
4. Apply state transitions atomically
5. Increment sender nonce

Transfer transactions move balance from sender to receiver. Stake and ComputeTask types are structurally supported; execution logic is simplified for Phase 1.

---

## Account Model

Each account is identified by a 32-byte address (Ed25519 public key). State includes:

- **balance:** Non-negative integer (base units)
- **nonce:** Monotonically increasing, used for replay protection

Validator accounts may have additional `validator_data` (stake, active status). Non-validator accounts have `validator_data: null`.

---

## Transaction Validation Flow

1. **Structural:** Valid SCALE decode, required fields present
2. **Signature:** Ed25519 verification over signing payload (tx_type, sender, receiver, amount, nonce)
3. **Nonce:** Must equal account nonce
4. **Balance:** Sender balance must be >= amount for transfers
5. **Replay:** Transaction hash must not exist in storage (included at most once)

Invalid transactions are rejected before execution.

---

## Storage Model

- **Blocks:** Keyed by height and hash. Header and body stored.
- **Transactions:** Indexed by hash. Links to block height.
- **Accounts:** Keyed by address. Balance and nonce.

Column families separate blocks, transactions, and accounts. All writes use `write_batch` for atomicity.

---

## Manual Block Production

Phase 1 does not include automated block production. A block producer (orchestrator) must:

1. Collect transactions from mempool (or direct submission)
2. Build block template with valid transactions
3. Execute transactions to compute state root
4. Sign block header
5. Submit via `produce_block` RPC

No consensus protocol. Single producer assumed.

---

## Deterministic Hashing

- **Serialization:** SCALE (parity-scale-codec) for all on-chain data
- **Hashing:** BLAKE3 for transaction roots, block hashes, state roots
- **Signing payload:** SCALE-encoded (tx_type, sender, receiver, amount, nonce)

Determinism is required for reproducible state roots and cross-node verification.

---

## Atomic write_batch Guarantee

All state changes from a block are applied in a single RocksDB `WriteBatch`. Either all writes succeed or none do. No partial state after a block.

---

## Idempotent submit_transaction

`submit_transaction` accepts a signed transaction and enqueues it for inclusion. If the same transaction (by hash) is submitted again, it is deduplicated. No double-enqueue. Returns existing tx hash if already present.

---

## RPC + REST Endpoints Implemented

**RPC (JSON-RPC 2.0 over HTTP POST /rpc):**

- `submit_transaction` — Enqueue signed transaction
- `produce_block` — Build and persist block from mempool
- `get_block_height` — Latest finalized height
- `ping` — Liveness check

**REST:**

- `GET /blocks` — List recent blocks
- `GET /blocks/{hash}` — Block by hash
- `GET /transactions/{hash}` — Transaction by hash
- `GET /accounts/{address}` — Account state
- `GET /validators` — Validator set

---

## Test Count

106 tests across mbongo-core, mbongo-network, mbongo-api, and integration suites.

---

## Explicit Non-Goals

Phase 1 does **not** include:

- Consensus protocol (multi-validator agreement)
- P2P block propagation
- Mempool with eviction policy
- Fork choice rules
- Timed block production
- Gas metering
- Smart contracts

---

## Formal Statement

**Phase 1 is CLOSED.**

No further Phase 1 scope changes. The foundation is frozen. Phase 2 development targets tooling, mempool, timed production, and P2P propagation.
