# Storage Invariants

This document defines invariants that must hold for the Phase 1 storage layer.

---

## Account Invariants

- **balance non-negative:** `account.balance >= 0` at all times. No overdraft.
- **nonce monotonic:** For each account, `nonce` never decreases. Each transaction increments sender nonce by exactly 1.

---

## Transaction Invariants

- **hash includes signature:** Transaction hash is computed over the full SCALE-encoded transaction, including the signature field.
- **tx_seq monotonic:** Within a block, transactions are ordered. Nonce ordering is enforced at validation time.
- **included at most once:** A transaction hash appears in at most one block. Replay protection.

---

## Block Invariants

- **height strictly increasing:** Block height N+1 is only valid if block N exists. No gaps.
- **parent linkage required:** `block.header.parent_hash` must equal the hash of the block at `height - 1`.
- **deterministic SCALE hash:** Block hash is derived from SCALE-encoded header. Same bytes produce same hash.

---

## Atomicity Guarantees

- **write_batch usage:** All state changes for a block are applied in a single RocksDB `WriteBatch`. Commit or rollback as a unit.
- **no partial state writes:** If any write in the batch fails, the entire batch is aborted. No partial application of a block.
