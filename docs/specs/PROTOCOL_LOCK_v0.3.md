# PROTOCOL LOCK v0.3 — Receipt Anchoring Devnet Stable

**Status:** FROZEN
**Git tag:** `v0.3-devnet-stable`
**Supersedes:** [PROTOCOL_LOCK_v0.2.md](./PROTOCOL_LOCK_v0.2.md)
**Authorizing RFC:** [RFC 0002 — Receipt Anchoring](../rfcs/0002-receipt-anchoring-v0.3.md)
**Last updated:** 2026-07-20

---

## Purpose

This document locks the protocol surfaces of Mbongo Chain protocol v0.3,
which adds receipt anchoring (RFC 0002) to the v0.2 devnet foundation. Any
change to a locked surface requires a new RFC and a protocol version bump,
per [RFC_PROCESS.md](../RFC_PROCESS.md).

v0.3 is a **devnet** release. Nothing in this document claims mainnet
readiness.

---

## Canonical References

| Document | Path | Status |
|----------|------|--------|
| Authorizing RFC | [0002-receipt-anchoring-v0.3.md](../rfcs/0002-receipt-anchoring-v0.3.md) | Accepted, implemented |
| Receipt Specification | [RECEIPT_SPEC_v0.1.md](./RECEIPT_SPEC_v0.1.md) | EXPERIMENTAL (see [Experimental surfaces](#experimental-and-deferred-surfaces)) |
| Previous lock | [PROTOCOL_LOCK_v0.2.md](./PROTOCOL_LOCK_v0.2.md) | SUPERSEDED (historical record) |
| Storage Invariants | [../architecture/storage_invariants.md](../architecture/storage_invariants.md) | FROZEN |

---

## Locked Surfaces

Everything frozen at v0.2 remains frozen unless explicitly restated below.
The following are **immutable** at this tag.

### 1. Transaction SCALE Encoding

Canonical field order:

```text
tx_type, sender, receiver, amount, nonce, payload, signature
```

`TransactionType` codec indexes (explicit, pinned by test):

| Variant | Index |
|---------|-------|
| `Transfer` | 0 |
| `ComputeTask` | 1 |
| `Stake` | 2 |
| `AnchorReceipt` | 3 |

`TransactionPayload` codec indexes (explicit, pinned by test):

| Variant | Index |
|---------|-------|
| `None` | 0 |
| `AnchorReceipt(Receipt)` | 1 |

- The transaction signing payload covers every field except `signature`,
  including `payload`. The transaction hash is `BLAKE3(SCALE_encode(tx))`
  including the signature.
- v0.2 transaction bytes (no payload field) do not decode under v0.3 and
  are permanently incompatible; signed v0.2 history cannot be migrated.

### 2. Receipt Encoding and Hash

Canonical `Receipt` field order (per RECEIPT_SPEC_v0.1 §2–3):

```text
version, task_id, input_commitment, output_commitment, executor, metadata, signature
```

- `receipt_hash = BLAKE3(SCALE_encode(all fields except signature))`,
  32 bytes; display `0x` + 64 lowercase hex.
- The receipt signature is Ed25519 over the **raw 32-byte hash**, never
  the hex display string.
- The canonical fixed test vector
  `0x56510bc65a92b2655cbeba66b4c219705862d431181a244b0ce37ca04322a0f1`
  pins this encoding in `mbongo-core`.

### 3. AnchorReceipt Validation Order and Error Precedence

`apply_block` validates every transaction in body order. For each
transaction the checks run in this exact sequence; the first failure
rejects the whole block with the listed deterministic error:

| Rule | Check | Error |
|------|-------|-------|
| a | payload variant matches `tx_type` (`AnchorReceipt` ⟺ anchor payload, all others ⟺ `None`) | `TypePayloadMismatch` |
| b | anchor: `amount == 0` and `receiver == zero address` | `AnchorFieldConstraint` |
| c | transaction signature (Ed25519 by sender over the signing payload) | `InvalidSignature` |
| — | stored-transaction handling: a stored non-anchor tx is idempotently skipped; a stored **anchor** tx is rejected (see rule i note) | `TaskIdAlreadyAnchored` |
| d | sender account exists; nonce matches and is consumed | `SenderAccountMissing` / `InvalidNonce` |
| e | anchor: `metadata.len() <= MAX_RECEIPT_METADATA_BYTES` | `ReceiptMetadataTooLarge` |
| f | anchor: `receipt.version == 1` | `ReceiptVersionUnsupported` |
| g | anchor: `transaction.sender == receipt.executor` | `SenderExecutorMismatch` |
| h | anchor: receipt signature (Ed25519 by executor over raw hash) | `InvalidReceiptSignature` |
| i | anchor: `task_id` not anchored in prior chain state | `TaskIdAlreadyAnchored` |
| j | anchor: `task_id` not anchored earlier in the same block | `TaskIdRepeatedInBlock` |

- `MAX_RECEIPT_METADATA_BYTES = 4096`. Raising it is a protocol version
  bump; it is never lowered retroactively.
- `sender == executor` (rule g) is a transaction-level anchoring rule of
  v0.3; a standalone receipt is cryptographically valid independent of
  any enclosing transaction. Relaxing it (relayers/delegation) is
  additive but RFC-gated.
- **Global task_id uniqueness:** first-anchored-wins across the whole
  chain. Re-including or re-submitting an already anchored AnchorReceipt
  — including the byte-identical original transaction — is rejected as
  `TaskIdAlreadyAnchored`. The stored-transaction idempotence path
  applies only to non-anchor transaction types.
- **In-block duplicates:** a block containing two receipts with the same
  `task_id` is invalid in its entirety; rule (j) fires on the second
  occurrence, tracked by a transient per-block `pending_task_ids` set
  that never touches storage.
- Any failing check rejects the whole block atomically: no account,
  nonce, block, transaction, index, sequence, or receipt state is
  persisted.

### 4. Atomic Receipt Persistence

- Valid receipts are stored as the **canonical SCALE encoding** of the
  `Receipt`, keyed by the raw 32-byte `task_id`, via
  `BatchOp::PutReceipt` in the **same atomic WriteBatch** as the block,
  transactions, accounts, and indexes. There is no standalone receipt
  write API, and the storage layer never decodes or validates receipt
  bytes.
- **Transaction sequence allocation:** the sequence baseline is read once
  per block from the persisted last-included value and advanced locally;
  `SetTxSeq` and `SetLastIncludedTxSeq` are committed only in the final
  atomic batch. Rejected blocks cannot leak sequence increments or
  create divergent `tx_seq_index` state; sequence values are a pure
  function of accepted chain history.

### 5. Storage Schema (v2)

- On-disk schema version **2**: the v0.2 column families plus the
  `receipts` column family (key: raw 32-byte `task_id`; value: opaque
  canonical receipt bytes).
- The open/migration sequence, `schema_version` semantics, and
  idempotent v1→v2 migration are as specified in
  [storage_invariants.md](../architecture/storage_invariants.md) and
  RFC 0002 §5. A v0.2 binary cannot open a schema-v2 directory.

### 6. P2P Protocol Identifiers

| Identifier | v0.3 string | Role |
|------------|-------------|------|
| Sync negotiation | `/mbongo-sync/2` | **Authoritative compatibility gate** |
| Block notify negotiation | `/mbongo/block_notify/0.2.0` | **Authoritative compatibility gate** |
| Identify metadata | `/mbongo/0.3.0` | Informational only — never a gate |

- Message shapes (`SyncRequest`, `SyncResponse`, `SyncNotification`,
  `BlockNotifyAck`), framing (`u32` LE length prefix, 16 MiB max frame),
  and `MAX_RANGE = 256` are unchanged from v0.2 and remain frozen.
- **There is no fallback to the v0.2 strings.** v0.2 and v0.3 nodes are
  intentionally network-incompatible: negotiation fails deterministically
  at libp2p multistream-select (`UnsupportedProtocols`) before any
  payload byte is exchanged, on both the sync and block-notify channels.
  Peers do not silently downgrade.
- **Identify metadata alone is not a negotiation gate.** The libp2p
  Identify protocol-version string is advertised for observability;
  nothing admits or rejects peers based on it, and no Identify-based
  fallback or admission logic may be added. Compatibility is decided
  exclusively by the two request/response protocol negotiations above.

### 7. Genesis and Chain Compatibility

- v0.3 chains start from a **fresh genesis**. v0.2 chain history cannot
  be migrated (historical signatures cover the v0.2 encoding and cannot
  be re-created).
- v0.2 and v0.3 are incompatible at every layer: transaction encoding,
  block payloads, storage schema, and network negotiation.

---

## Devnet Migration Procedure (v0.2 → v0.3)

1. Stop every v0.2 node.
2. Back up data directories if their contents are needed for reference
   (they cannot be used by v0.3).
3. Wipe the incompatible chain data directories.
4. Deploy the same v0.3 binary and configuration to every node.
5. Start all nodes from the approved fresh genesis.
6. Verify receipt anchoring, deterministic replay, and multi-node
   convergence (`replay_harness` and `devnet_harness` must pass).
7. Never mix protocol versions on one network: v0.2 peers cannot
   negotiate with v0.3 peers, by design.

Rollback to v0.2 after cutover requires wiping every touched data
directory; receipts anchored on the abandoned v0.3 chain are lost with
it.

---

## Experimental and Deferred Surfaces

The following are **not** frozen by this lock and remain experimental or
deferred. Activating any of them requires its own RFC (or spec addendum
where CONTRIBUTION_TIERS permits):

- Receipt economics: fees, rewards, slashing, challenges, dispute
  resolution
- Verification strategies: redundant execution, TEE attestation, ZK
  proofs, PoUW
- Dedicated receipt RPC methods (`submit_receipt`, `get_receipt` remain
  reserved and return `-32601`)
- Receipt pruning/archival policy
- Relayed/delegated submission (`sender != executor`), fee sponsorship,
  meta-transactions
- `ComputeTask` and `Stake` transaction semantics (still legacy
  fall-through; unvalidated types)
- [RECEIPT_SPEC_v0.1.md](./RECEIPT_SPEC_v0.1.md) remains EXPERIMENTAL by
  its own terms until v1.0-mainnet — with the qualification that the
  receipt encoding, hash, and validation rules **as consumed by v0.3
  consensus** are frozen by this lock: changing them now requires an RFC
  and a protocol version bump in any case.

---

## How to Propose a Locked Change

Identical to the v0.2 process: file an RFC under `docs/rfcs/` per
[RFC_PROCESS.md](../RFC_PROCESS.md), identify the affected locked
surface, specify the new version number, obtain Core Maintainer
approval, then bump, update the lock, and tag.
