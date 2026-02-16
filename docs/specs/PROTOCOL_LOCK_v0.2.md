# PROTOCOL LOCK v0.2 — Devnet Stable

**Status:** FROZEN
**Git tag:** `v0.2-devnet-stable`
**Last updated:** 2026-02-16

---

## Purpose

This document locks the protocol surfaces that shipped with the `v0.2-devnet-stable` tag. Any change to a locked surface requires an RFC (see [How to propose a locked change](#how-to-propose-a-locked-change)) and a protocol version bump.

---

## Canonical References

| Document | Path | Status |
|----------|------|--------|
| Protocol Definition | [PROTOCOL_DEFINITION_v0.1.md](./PROTOCOL_DEFINITION_v0.1.md) | Canonical PDD |
| RPC Specification | [rpc_v0.1.md](./rpc_v0.1.md) | FROZEN |
| Storage Invariants | [../architecture/storage_invariants.md](../architecture/storage_invariants.md) | FROZEN |

---

## Locked Surfaces

The following are **immutable** at this tag. Breaking changes require an RFC, review, and a version bump (e.g. v0.3).

### 1. Block and Transaction SCALE Encoding

- All on-disk and on-wire serialisation uses `parity-scale-codec` (SCALE).
- `BlockHeader` fields and order: `parent_hash`, `state_root`, `transactions_root`, `timestamp`, `height`.
- `Transaction` fields and order: `tx_type`, `sender`, `receiver`, `amount`, `nonce`, `signature`.
- Adding, removing, or reordering fields is a breaking change.

### 2. Hashing Rules

- Block hash: `BLAKE3(SCALE_encode(header))`.
- Transactions root: `BLAKE3` Merkle commitment over SCALE-encoded transactions.
- Transaction hash: `BLAKE3(SCALE_encode(transaction))` (includes signature).
- Hash display: `0x` + 64 lowercase hex characters (32 bytes).

### 3. `apply_block` Validity Rules

As defined in [PROTOCOL_DEFINITION_v0.1.md](./PROTOCOL_DEFINITION_v0.1.md), Section "Block Validity Rules":

1. Parent linkage (`parent_hash` equals hash of block at `height - 1`).
2. Height monotonic (`height == parent_height + 1`).
3. Deterministic SCALE hash.
4. Transactions root matches recomputed commitment.
5. Each transaction passes validation (signature, nonce, balance, uniqueness).

All state changes for a block MUST be applied in a single atomic `write_batch`. Partial application is forbidden.

### 4. P2P Wire Formats

#### Sync protocol (`/mbongo-sync/1`)

Framing: `[u32 LE length][SCALE payload]`. Max frame: 16 MiB.

| Message | Type | Fields |
|---------|------|--------|
| `SyncRequest::GetHeight` | request | (unit) |
| `SyncRequest::GetBlocks` | request | `start_height: u64`, `end_height: u64` (half-open) |
| `SyncResponse::Height` | response | `u64` |
| `SyncResponse::Blocks` | response | `Vec<(Hash, Block)>` |
| `SyncResponse::Error` | response | `String` |

`MAX_RANGE = 256` blocks per request.

#### Block notification (`/mbongo/block_notify/0.1.0`)

Same framing. Request: `SyncNotification::NewBlock { block: Block }`. Response: `BlockNotifyAck` (empty struct).

### 5. RPC Interface (v0.1)

All method names, parameter shapes, and return types defined in [rpc_v0.1.md](./rpc_v0.1.md) are locked:

- `submit_transaction`, `produce_block`, `get_block_height`, `get_latest_block_hash`, `ping`
- JSON-RPC 2.0 over HTTP POST at `/rpc`
- Error codes as specified

---

## Allowed Changes (no version bump required)

The following may change freely without an RFC or version bump:

- Documentation, diagrams, READMEs
- Developer tooling, scripts, CI pipelines
- SDK libraries and explorer front-ends
- Logging output, log levels, log format
- Metrics, telemetry, observability
- Test harnesses and benchmarks
- CLI flag additions that do not alter protocol behaviour
- Internal refactors that preserve all locked surface semantics

---

## Forbidden Changes

The following MUST NOT change without an RFC and version bump:

| Surface | Rationale |
|---------|-----------|
| Block header/body field set or order | Breaks SCALE encoding and hash continuity |
| Transaction field set or order | Breaks SCALE encoding and signature verification |
| BLAKE3 hashing inputs or algorithm | Breaks hash chain and Merkle root verification |
| `apply_block` validation rules | Breaks consensus on block validity |
| Atomic `write_batch` requirement | Breaks storage consistency guarantees |
| Storage trait semantics (`get_block_by_height`, `get_latest_height`, `write_batch` meaning) | Breaks invariants defined in [storage_invariants.md](../architecture/storage_invariants.md) |
| `SyncRequest` / `SyncResponse` enum variants or field types | Breaks P2P interoperability |
| `SyncNotification` / `BlockNotifyAck` wire format | Breaks block announcement protocol |
| Protocol negotiation strings (`/mbongo-sync/1`, `/mbongo/block_notify/0.1.0`) | Breaks protocol handshake |
| RPC method names, parameter types, or return types in rpc_v0.1 | Breaks RPC client compatibility |
| Frame encoding (u32 LE length prefix) | Breaks all wire communication |

---

## How to Propose a Locked Change

1. Open an RFC document in `docs/rfcs/` following the process defined in [RFC_PROCESS.md](../RFC_PROCESS.md).
2. The RFC MUST identify which locked surface is affected and justify the break.
3. The RFC MUST specify the new version number (e.g. protocol v0.3, rpc v0.2).
4. The RFC requires at least one reviewer approval before merge.
5. On merge: bump the version, update this lock document, and create a new git tag.
