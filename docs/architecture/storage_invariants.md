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

## Task Invariants (RFC 0005 §4)

- **opaque values:** The `tasks` column family maps a raw 32-byte `task_id` key to the canonical SCALE `ComputeTask` bytes. The storage layer never decodes, validates, hashes, or inspects them; rules (k)–(p) live above storage, exactly as receipt validation does.
- **batch-only writes:** Tasks are written exclusively through `BatchOp::PutTask` inside the shared atomic `write_batch`, in the same batch as the block, transactions, accounts, indexes and receipts. There is no standalone insert API; `task_id` uniqueness (rule p) is a consensus rule enforced before the batch is built.
- **derived state, no status:** *submitted* is the presence of a `task_id` in `tasks`; *completed* is its presence in `receipts`. No status field is stored (RFC 0005 §4.1). Both column families are fully derived from chain blocks and deterministically reconstructed by replay from genesis.

---

## Schema Versioning (RFC 0002 §5, RFC 0005 §4)

- **version key:** `meta` key `schema_version` (`u32`, big-endian). Absent means version 1 (the v0.2 layout). Version 2 added the `receipts` column family (v0.3). **Current version is 3**, which adds the `tasks` column family (v0.4, [PROTOCOL_LOCK_v0.4.md](../specs/PROTOCOL_LOCK_v0.4.md) §6).
- **open sequence:** List existing column families → reject unknown ones → open exactly what is listed → reject `schema_version` greater than supported → create `receipts` (v1→v2) and `tasks` (v2→v3) if absent → stamp `schema_version = 3` only after successful creation.
- **idempotent migration:** A crash between column-family creation and version stamping is recovered on next open: creation is skipped, the stamp is applied. No data transformation occurs in either migration; both column families are created empty.
- **migration on open:** The v1→v3 and v2→v3 migrations run as a side effect of opening an existing v0.2 or v0.3 directory. The open itself changes the physical schema and crosses the downgrade boundary below. It is a storage-layer fact, **not a protocol activation path**: a v0.3 chain's blocks were validated under v0.3 rules, and v0.4 activation is a fresh genesis (PROTOCOL_LOCK_v0.4 §10).
- **downgrade:** Not supported. A v0.2 binary cannot open a database containing the `receipts` column family; a v0.3 binary cannot open one containing `tasks`. Rollback requires wiping the data directory.
