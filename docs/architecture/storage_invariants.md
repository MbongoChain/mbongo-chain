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

---

## Receipt Invariants (RFC 0002 Phase 1)

- **opaque values:** The `receipts` column family maps a raw 32-byte `task_id` key to opaque receipt bytes. The storage layer never decodes, validates, hashes, or inspects them; all receipt validation lives above storage (see [RFC 0002](../rfcs/0002-receipt-anchoring-v0.3.md) §6.1).
- **batch-only writes:** Receipts are written exclusively through `BatchOp::PutReceipt` inside the shared atomic `write_batch`. There is no standalone insert API, and no check-then-insert semantics at the storage level; `task_id` uniqueness is a consensus rule enforced before the batch is built.
- **derived state:** The `receipts` column family is fully derived from chain blocks and is deterministically reconstructed by replay from genesis.

---

## Schema Versioning (RFC 0002 §5)

- **version key:** `meta` key `schema_version` (`u32`, big-endian). Absent means version 1 (the v0.2 layout). Current version is 2 (adds the `receipts` column family).
- **open sequence:** List existing column families → reject unknown ones → open exactly what is listed → reject `schema_version` greater than supported → create `receipts` if absent (the v1→v2 migration) → stamp `schema_version = 2` only after successful creation.
- **idempotent migration:** A crash between column-family creation and version stamping is recovered on next open: creation is skipped, the stamp is applied. No data transformation occurs in the migration.
- **migration on open:** The v1→v2 migration runs as a side effect of opening an existing v0.2 directory. Phase 1 activates no consensus-visible, block, transaction, RPC, network, or node state-transition behavior — but the open itself changes the physical schema and crosses the downgrade boundary below.
- **downgrade:** Not supported. A v0.2 binary cannot open a database containing the `receipts` column family; rollback requires wiping the data directory.
