# Architecture Guardrails

Strict layer rules and invariant protections for Phase 2 development.

---

## Layer Rules

### mbongo-core

- Pure logic only. No I/O, no network, no file system.
- No dependencies on mbongo-storage, mbongo-network, or mbongo-node.
- All types must be serializable (SCALE + serde where needed).
- Cryptographic operations use approved crates (ed25519-dalek, blake3).

### mbongo-storage

- Persistence only. No business logic.
- All writes use RocksDB `WriteBatch` for atomicity.
- Column families: blocks, transactions, accounts, mempool.
- No direct network or RPC handling.

### mbongo-network

- Transport only. HTTP, JSON-RPC, REST.
- No state mutation. Forwards to storage/execution layer.
- No consensus logic.

### mbongo-node

- Orchestration only. Wires components. Manages lifecycle.
- No business logic. No direct storage access.
- Config loading, CLI parsing, server startup.

---

## Invariant Protections

### Account Invariants

- `balance >= 0` at all times.
- `nonce` monotonically increasing per account.

### Transaction Invariants

- Hash includes signature.
- Included at most once per chain.
- Nonce ordering enforced at validation.

### Block Invariants

- Height strictly increasing.
- Parent linkage required.
- Deterministic SCALE hash.

### Atomicity

- Block application is all-or-nothing.
- No partial state writes.
- Rollback on any write failure.

---

## Enforcement

- CI runs fmt, clippy, tests.
- PRs touching storage must reference `docs/architecture/storage_invariants.md`.
- PRs adding RPC methods must update spec and follow versioning.
- Layer violations block merge.
