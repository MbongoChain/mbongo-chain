# PROTOCOL LOCK v0.4 — Compute Task Commitment Devnet Stable

**Status:** FROZEN
**Git tag:** `v0.4-devnet-stable` @ `fcec8ddc7b06247460e04db987de08232992e2fc` (the release commit; see [Release](#release))
**Supersedes:** [PROTOCOL_LOCK_v0.3.md](./PROTOCOL_LOCK_v0.3.md)
**Authorizing RFC:** [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md)
**Locked at:** `26beb2d872cd95de6777d9dd00222b98b7f1f968` (`dev`, 2026-09-07)
**Last updated:** 2026-09-07

---

## Purpose

This document locks the protocol surfaces of Mbongo Chain protocol v0.4,
which adds the compute task commitment (RFC 0005) to the v0.3 receipt
anchoring devnet. It locks what the repository **implements** at the commit
above, audited independently against RFC 0005, the consensus source, the
cross-language fixtures and the tests; it redesigns nothing and introduces
no field, rule, method or value of its own. Any change to a locked surface
requires a new RFC and a protocol version bump, per
[RFC_PROCESS.md](../RFC_PROCESS.md).

v0.4 is a **devnet** release. Nothing in this document claims mainnet
readiness, and nothing in it claims that an anchored receipt proves the
correctness of any computation (RFC 0005 §9.2).

---

## Canonical References

| Document | Path | Status |
|----------|------|--------|
| Authorizing RFC | [0005-compute-task-commitment-v1.md](../rfcs/0005-compute-task-commitment-v1.md) | Accepted, implemented (#129, #130); Released at the tag |
| Receipt anchoring RFC | [0002-receipt-anchoring-v0.3.md](../rfcs/0002-receipt-anchoring-v0.3.md) | Accepted, implemented; rules (a)–(j) unchanged |
| Receipt Specification | [RECEIPT_SPEC_v0.1.md](./RECEIPT_SPEC_v0.1.md) | EXPERIMENTAL by its own terms; its encoding, hash and validation **as consumed by consensus** are frozen (§7) |
| RPC contract | [rpc_v0.3.md](./rpc_v0.3.md) | FROZEN by this gate's independent audit (§8) |
| RPC contract before RFC 0005 | [rpc_v0.2.md](./rpc_v0.2.md) | SUPERSEDED, FROZEN, kept intact |
| Previous lock | [PROTOCOL_LOCK_v0.3.md](./PROTOCOL_LOCK_v0.3.md) | SUPERSEDED (historical record) |
| Storage Invariants | [../architecture/storage_invariants.md](../architecture/storage_invariants.md) | FROZEN, amended to schema 3 by this lock |
| Protocol vectors | [`test-vectors/`](../../test-vectors/) | FROZEN (§11) |

Non-consensus reference material this lock cites as compatibility evidence
and does **not** freeze: the compute architecture
([privacy](../architecture/compute-privacy-data-plane.md),
[data plane](../architecture/compute-private-data-plane-interface.md),
[control plane](../architecture/compute-control-plane-worker-interface.md)),
the [reference worker](../development/reference-worker.md) and
[Mbongo Compute Conformance](../development/compute-conformance.md). See
[Non-consensus reference material](#non-consensus-reference-material).

---

## v0.3 → v0.4 change inventory

Every protocol-relevant change since `v0.3-devnet-stable`, classified. Only
the first four classes are locked here.

| Change | Class | Where | Locked |
|---|---|---|---|
| `ComputeTask` six-field envelope, canonical SCALE, `task_id` derivation, `MAX_EXECUTION_SPEC_BYTES` | WIRE_ENCODING | `crates/mbongo-core/src/compute_task.rs` | §1, §3 |
| `TransactionPayload::ComputeTask`, codec index 2 | WIRE_ENCODING | `crates/mbongo-core/src/primitives.rs` | §2 |
| `(ComputeTask, None)` no longer executes as a transfer; rule (k) | VALIDITY_RULE / CONSENSUS | `crates/mbongo-node/src/backend.rs` | §5 |
| Rules (l)–(p) for `ComputeTask` transactions | CONSENSUS | `backend.rs` `apply_block`, `check_task_envelope` | §5 |
| Rules (q)–(s) for `AnchorReceipt` transactions | CONSENSUS (stricter validity; no wire change) | `backend.rs` `check_receipt_binding` | §5, §7 |
| `tasks` column family, `BatchOp::PutTask`, `Storage::{has_task, get_task}`, schema version 3 | STATE_SCHEMA | `crates/mbongo-storage` | §6 |
| Payload union widened in JSON: `{"ComputeTask": {…}}` | RPC | serde over the protocol types; [rpc_v0.3.md](./rpc_v0.3.md) | §8 |
| `@mbongo/sdk` source: `ComputeTask` construction, signing, decoding, bound receipts | SDK_COMPATIBILITY | `sdk/typescript/src/compute-task.ts`, `types.ts`, `client.ts` | [SDK compatibility](#sdk-compatibility) |
| Privacy architecture, E and F contracts | OFFCHAIN_ARCHITECTURE | `docs/architecture/` | not locked |
| Reference worker, control plane, data plane, `compute-conformance-v1` | TEST/CONFORMANCE | `crates/mbongo-compute` | not locked |
| Fixtures `compute-task-v1`, `anchor-binding-v1`, `compute-task-rpc-v1` | TEST (protocol vectors) | `test-vectors/` | §11 |
| Mempool admission mirrors of (k)–(s), pending-nonce chains, per-sender bounds | NON_PROTOCOL (node policy) | `backend.rs` `submit_transaction` | not locked |
| RFC 0003 / RFC 0004 monetary documents | NON_PROTOCOL for this lock (no consensus change) | `docs/rfcs/` | not locked |

Not changed since v0.3, and therefore carried forward unchanged: the
`Transaction` field order and signing rule, `TransactionType` indexes, the
`Receipt` encoding and hash, rules (a)–(j), `MAX_RECEIPT_METADATA_BYTES`,
atomic persistence, the P2P protocol identifiers and message shapes, and the
code-defined genesis.

---

## Locked Surfaces

Everything frozen at v0.3 remains frozen unless explicitly restated below.
The following are **immutable** at this lock.

### 1. `ComputeTask` Envelope and Canonical Encoding

Six fields, in this canonical order, exactly as RFC 0005 §2.1 and
`mbongo_core::ComputeTask` define them. `task_id` is **not** a field.

| # | Field | Type | Canonical bytes |
|---|---|---|---|
| 1 | `version` | `u8` | 1 byte; must be `1` (rule m) |
| 2 | `submitter` | `Address` | 32 transparent bytes |
| 3 | `executor` | `Address` | 32 transparent bytes |
| 4 | `salt` | `[u8; 32]` | 32 transparent bytes; client-chosen, opaque; zero is legal; **not** the transaction nonce |
| 5 | `input_commitment` | `[u8; 32]` | 32 transparent bytes; opaque (§4) |
| 6 | `execution_spec` | `Vec<u8>` | SCALE compact length prefix, then the raw bytes; `len <= 1024` (rule n) |

- `MAX_EXECUTION_SPEC_BYTES = 1024`. Raising it is a protocol version
  bump; it is never lowered retroactively. It is a consensus bound (rule
  n), not an encoding limit: a 1025-byte spec encodes and hashes like any
  other and is rejected by validation.
- The maximal canonical envelope is **1155** bytes (two-byte compact prefix
  at 1024); the maximal `task_id` preimage is **1177** bytes (RFC 0005
  §2.10). Both constants exist in `mbongo-core` and are pinned by
  `test-vectors/compute-task/compute-task-v1.json`.
- The envelope carries **no signature of its own**. It is authenticated
  only by the carrying transaction's signature (RFC 0005 §2.5).
- No field may be added, removed or reordered. `deadline`, `max_fee`,
  `task_type`, `model_id` and every economic field are deliberately absent
  (RFC 0005 §2.1, §2.9, §6).

### 2. Payload and Type Discriminants

`TransactionType` codec indexes — **unchanged from v0.3**, pinned by test and
by every transaction fixture:

| Variant | Index |
|---------|-------|
| `Transfer` | 0 |
| `ComputeTask` | 1 |
| `Stake` | 2 |
| `AnchorReceipt` | 3 |

`TransactionPayload` codec indexes — v0.3's two members unchanged, one
member added:

| Variant | Index | Wire form |
|---------|-------|-----------|
| `None` | 0 | `0x00` |
| `AnchorReceipt(Receipt)` | 1 | `0x01` ‖ canonical receipt bytes |
| `ComputeTask(ComputeTask)` | 2 | `0x02` ‖ canonical task bytes (**new**, RFC 0005 §2.7) |

- `Box<T>` encodes as `T`; no length prefix precedes the nested object.
- The `Transaction` field order `tx_type, sender, receiver, amount, nonce,
  payload, signature`, the signing payload (every field except `signature`)
  and the transaction hash (`BLAKE3(SCALE(tx))` including the signature)
  are unchanged from v0.3 §1. For a `ComputeTask` transaction the signing
  payload is therefore `0x01 ‖ sender[32] ‖ receiver[32] ‖ amount_u128_le[16]
  ‖ nonce_u64_le[8] ‖ 0x02 ‖ task bytes`, the task beginning at offset 90.
- Existing v0.3 encodings are preserved byte for byte:
  `test-vectors/transaction/anchor-receipt-v1.json` and
  `test-vectors/receipt/receipt-v1.json` are unchanged since v0.3 and still
  pass.

### 3. Task Identity

```text
task_id = BLAKE3( DOMAIN_TASK || SCALE(ComputeTask) )
DOMAIN_TASK = 6d 62 6f 6e 67 6f 3a 63 6f 6d 70 75 74 65 2d 74 61 73 6b 3a 76 31
            = "mbongo:compute-task:v1"  (22 ASCII bytes)
```

- The tag is prepended **raw**: no NUL terminator, no SCALE length prefix,
  not encoded as a `Vec<u8>` (RFC 0005 §2.2). The hash is over bytes, never
  over a hexadecimal rendering.
- The preimage is the six fields of §1 and nothing else: it contains **no
  transaction nonce, no transaction signature, no `task_id`** and no
  `tx_type`. A resubmission after a nonce race keeps its identity; a
  different executor is a different task (RFC 0005 §2.6).
- The fixture's `wrong_tag_diagnostics` (tag SCALE-encoded, tag
  NUL-terminated, no tag, hex rendering hashed) pin what each mistake
  produces so an implementation cannot pass by accident.
- The asymmetry with `receipt_hash`, which carries no domain tag, is
  deliberate and permanent (RFC 0005 §2.3).

### 4. `input_commitment` and `execution_spec` Semantics

- **`input_commitment` is opaque protocol data.** Consensus checks
  **equality** between a receipt's `input_commitment` and the registered
  task's (rule r) and nothing else. Consensus never fetches input, never
  derives a commitment, and never enforces a plain, blinded or any other
  derivation. The `mbongo:compute-input:v1` / `mbongo:compute-output:v1`
  conventions of RFC 0005 §2.4 are interoperability conventions, **not
  consensus**, and this lock does not promote them.
- **`execution_spec` is opaque, bounded public bytes.** The chain commits
  to it, stores it, hashes it into `task_id` and never interprets it (RFC
  0005 §2.12). The protocol permits any public bytes within the bound; it
  defines no format, no version byte and no execution profile. That the
  conformant private-data workflow never uses it as an input transport is
  an architecture requirement (P5, P15), tested by `compute-conformance-v1`,
  and **not a protocol rule**.
- `output_commitment` remains opaque to consensus, as in v0.3.

### 5. Validation Rules and Error Precedence

`apply_block` validates every transaction in body order. For each
transaction the checks run in the exact sequence below; the first failure
rejects the whole block with the listed deterministic error. Rule letters
are RFC 0002's (a)–(j) and RFC 0005's (k)–(s). RFC 0005 orders its own
rules; their placement relative to the generic signature, stored-transaction
and nonce checks is **the v0.3 pipeline's**, preserved unchanged, and is
locked here so that two nodes cannot disagree on which error names a block.

**`ComputeTask` transaction** (`tx_type == ComputeTask`):

| Rule | Check | Error |
|------|-------|-------|
| k | `payload` is `ComputeTask(task)`; no other type carries that payload and this type carries no other payload | `TypePayloadMismatch` |
| l | `amount == 0` and `receiver == zero address` | `TaskFieldConstraint` |
| c | transaction signature (Ed25519 by `sender` over the signing payload) | `InvalidSignature` |
| — | stored-transaction handling: a stored `ComputeTask` transaction is rejected (its task is already registered; same verdict as rule p) | `TaskAlreadyRegistered` |
| d | sender account exists; nonce matches and is consumed; **no balance moves** | `SenderAccountMissing` / `InvalidNonce` |
| m | `task.version == 1` | `TaskVersionUnsupported` |
| n | `task.execution_spec.len() <= 1024` | `ExecutionSpecTooLarge` |
| o | `task.submitter == tx.sender` | `SenderSubmitterMismatch` |
| p | no task with this `task_id` in prior chain state | `TaskAlreadyRegistered` |
| p | no task with this `task_id` earlier in the same block | `TaskRepeatedInBlock` |

`task_id` is derived only after rule (n) holds, so no node hashes an
out-of-bound preimage.

**`AnchorReceipt` transaction** (`tx_type == AnchorReceipt`) — (a)–(j) exactly
as v0.3 §3, then (q)–(s):

| Rule | Check | Error |
|------|-------|-------|
| a | `payload` is `AnchorReceipt(receipt)` | `TypePayloadMismatch` |
| b | `amount == 0` and `receiver == zero address` | `AnchorFieldConstraint` |
| c | transaction signature | `InvalidSignature` |
| — | stored anchor transaction is rejected (never idempotently skipped) | `TaskIdAlreadyAnchored` |
| d | sender account exists; nonce matches and is consumed | `SenderAccountMissing` / `InvalidNonce` |
| e | `metadata.len() <= 4096` | `ReceiptMetadataTooLarge` |
| f | `receipt.version == 1` | `ReceiptVersionUnsupported` |
| g | `tx.sender == receipt.executor` | `SenderExecutorMismatch` |
| h | receipt signature (Ed25519 by executor over the raw 32-byte hash) | `InvalidReceiptSignature` |
| i | `task_id` not anchored in prior chain state | `TaskIdAlreadyAnchored` |
| j | `task_id` not anchored earlier in the same block | `TaskIdRepeatedInBlock` |
| **q** | a task with `receipt.task_id` exists in prior chain state **or earlier in the same block** | `TaskNotRegistered` |
| **r** | `receipt.input_commitment == task.input_commitment` | `ReceiptInputCommitmentMismatch` |
| **s** | `receipt.executor == task.executor` | `ReceiptExecutorNotAuthorised` |

**`Transfer` and `Stake` transactions**: the v0.2 path, unchanged — (a)
requires `payload == None`, then (c), stored-transaction idempotent skip,
(d) with balance transfer. `Stake` still falls through to transfer
semantics (legacy, unvalidated, as at v0.3); **`ComputeTask` no longer
does** (below).

Locked consequences:

- **Legacy `(ComputeTask, None)` is rejected** by rule (k) with
  `TypePayloadMismatch` (RFC 0005 §5.1). At v0.3 it executed as a plain
  transfer; that behaviour is gone and cannot be re-enabled without an RFC.
  Likewise `(Transfer | Stake | AnchorReceipt, ComputeTask(_))` and
  `(Transfer | Stake | ComputeTask, AnchorReceipt(_))` are all `TypePayloadMismatch`.
- **Rule (g) and rule (s) are distinct and both apply.** (g) authenticates
  whoever anchors; (s) authorises them. Under v0.4 the anchoring sender is
  therefore always the executor the submitter named.
- **Duplicate receipts are unchanged from v0.3**: first-anchored-wins on
  `task_id` across the chain (rule i) and within a block (rule j).
  Precedence with the binding rules: (i) and (j) are evaluated **before**
  (q)–(s), so a second receipt for an anchored task — from any executor,
  with any commitment — is `TaskIdAlreadyAnchored`, never a binding error.
  Within the binding, (q) precedes (r) precedes (s).
- **Same-block ordering is consensus-observable and locked**: a
  `ComputeTask` earlier in a block followed by its bound receipt later in
  the same block is **accepted** (rule q's "earlier in the same block");
  the receipt ahead of its task is **rejected** with `TaskNotRegistered` at
  the receipt's index, and the block with it. A task and its receipt may
  share one `task_id` within a block only in that order.
- **Atomicity**: any failing check rejects the whole block. A block that
  registers a valid task and then carries a receipt failing (r) persists
  **nothing** — no task, no receipt, no account, nonce, sequence or height
  change — through the same single atomic `WriteBatch` v0.3 §4 established.
  No rollback code was added; none was needed.
- **Mempool admission is not consensus.** `submit_transaction` mirrors
  (k)–(s) (and reads pending tasks for the same-block half of q) so that a
  producing node does not build an invalid block; its rejection messages
  are not contract, and block application remains the sole authority on
  validity.

Pinned by `crates/mbongo-node/src/backend.rs`: `compute_task_type_payload_pairings_rejected`,
`compute_task_field_constraints_rejected`, `compute_task_invalid_signature_rejected`,
`compute_task_nonce_rules`, `compute_task_version_rejected`,
`compute_task_execution_spec_bound`, `compute_task_submitter_mismatch_rejected`,
`compute_task_duplicate_in_prior_state_rejected`, `compute_task_duplicate_in_same_block_rejected`,
`stored_compute_task_tx_in_later_block_rejected`, `compute_task_rule_precedence_is_deterministic`,
`compute_task_then_matching_receipt_in_one_block`, `same_block_anchor_before_task_rejected`,
`invalid_anchor_block_is_atomic`, `mixed_block_commits_atomically`,
`anchor_unknown_task_rejected`, `anchor_wrong_input_commitment_rejected`,
`anchor_wrong_executor_rejected`, `binding_precedence_is_deterministic`,
`first_failure_precedence_pinned`, `anchor_binding_vectors_drive_consensus`,
`admission_and_block_validation_agree_on_binding`.

### 6. Task Storage and State (Schema v3)

- On-disk schema version **3**: the v0.3 column families plus the `tasks`
  column family — key: the raw 32-byte `task_id`; value: the canonical
  SCALE `ComputeTask` bytes, **opaque to the storage layer**, which never
  decodes, validates or inspects them (RFC 0005 §4).
- Tasks are written only through `BatchOp::PutTask` in the **same atomic
  `WriteBatch`** as the block, transactions, accounts, indexes and
  receipts. There is no standalone task write API.
- **Protocol-visible task state is derived, never stored** (RFC 0005 §4.1):
  *submitted* ⟺ `task_id` present in `tasks`; *completed* ⟺ `task_id`
  present in `receipts`. No status field exists. The `tasks` and
  `receipts` column families are fully reconstructable by replay from
  genesis.
- Bytes under a `task_id` that do not decode are a **storage failure**,
  never an "unknown task": consensus does not turn corruption into a
  protocol verdict.
- The open/migration sequence is the one
  [storage_invariants.md](../architecture/storage_invariants.md) states:
  list column families → reject unknown → open exactly what exists → reject
  a `schema_version` greater than 3 → create `receipts` and `tasks` if
  absent → stamp `schema_version = 3` only after creation succeeded. The
  v1→v3 and v2→v3 migrations are additive, create the column families
  **empty**, transform no data, and are idempotent across a crash between
  creation and stamping (`rocksdb_v1_database_migrates_to_v3`,
  `rocksdb_v2_database_migrates_to_v3`, `rocksdb_interrupted_migration_recovers`,
  `rocksdb_interrupted_v3_migration_recovers`, `rocksdb_v3_database_reopens`,
  `rocksdb_newer_schema_rejected`, `rocksdb_unknown_column_family_rejected`).
- **Downgrade is not supported.** A v0.3 binary cannot open a schema-3
  directory. A v0.4 binary *can* open a v0.3 directory — the migration is a
  side effect of the open and crosses the downgrade boundary — but doing so
  is **not an activation path**: the blocks in it were validated under
  v0.3 rules, and any unbound receipt they contain is invalid under rule
  (q). Activation is a fresh genesis (§10). Migration on open is tested for
  the storage layer only; **no production or mainnet migration is claimed**.

Off-chain coordination state — leases, sessions, attempts, capabilities,
private objects — is **not** chain state and is not stored by the node
(see [Non-consensus reference material](#non-consensus-reference-material)).

### 7. Receipt and AnchorReceipt: Wire Unchanged, Validity Stricter

- The canonical `Receipt` field order, encoding, `receipt_hash`
  (`BLAKE3(SCALE(fields 1–6))`, untagged) and signature domain (Ed25519
  over the **raw 32-byte hash**) are **unchanged from v0.3 §2**. The
  canonical vector `0x56510bc6…a0f1` still pins them.
- The `AnchorReceipt` transaction wire form (`0x03 … 0x01 ‖ receipt bytes`)
  is **unchanged**. `test-vectors/transaction/anchor-receipt-v1.json` and
  `test-vectors/receipt/receipt-v1.json` are byte-identical to v0.3.
- What changed is **validity**, not shape: under rules (q)–(s) an
  `AnchorReceipt` is valid only if it answers a registered task, over that
  task's `input_commitment`, from the executor that task named. An
  **unbound receipt — valid at v0.3 — is invalid at v0.4** (RFC 0005 §5.2).
  This is a consensus break for anyone anchoring without a prior
  `ComputeTask`, and a compatibility break for the v0.1 SDK anchoring flow;
  it is not a wire-format break, and no receipt field or byte was added.
- No output-correctness semantics were added. Rules (q)–(s) are three
  field equalities; the chain still checks that a receipt corresponds to a
  committed task, not that the output is right (RFC 0005 §6, §9.2).

### 8. RPC v0.3

- The RPC contract at v0.4 activation is **[rpc_v0.3.md](./rpc_v0.3.md)**:
  one `/rpc` endpoint, no version routing, the six v0.2 methods (`ping`,
  `get_block_height`, `submit_transaction`, `produce_block`,
  `get_latest_block_hash`, `get_block_by_height`) with unchanged params,
  results and error codes; `-32601` for every other name, the five reserved
  compute names included (RFC 0005 §7 activates none).
- The only change from rpc_v0.2 is the public payload union:
  `None | AnchorReceipt(Receipt) | ComputeTask(ComputeTask)`, externally
  tagged, the `ComputeTask` object carrying exactly the six fields of §1 in
  the v0.2 byte-encoding convention (addresses hex, byte arrays as number
  arrays). `task_id` is **not** a wire field. Undecodable or undefined
  payload variants are `-32602` before any backend code; admission
  rejections are `-32603`; messages are not contract.
- rpc_v0.3 is **FROZEN** by the independent audit this gate performed (its
  §9 records it). Breaking changes require a new RPC version.
- **rpc_v0.2 stays FROZEN and intact** as the record of what a v0.2 client
  was promised. Server side, every v0.2 request still works. Client side, a
  v0.2 typed client cannot decode a block that carries a task, and the
  v0.2 anchoring flow (no prior task) is consensus-invalid (§7).

### 9. P2P Protocol Identifiers and Network Compatibility

The identifiers are **unchanged from v0.3** — this is a fact this lock
records, not a decision it makes:

| Identifier | String at v0.4 | Role |
|------------|----------------|------|
| Sync negotiation | `/mbongo-sync/2` | negotiation gate (unchanged) |
| Block notify negotiation | `/mbongo/block_notify/0.2.0` | negotiation gate (unchanged) |
| Identify metadata | `/mbongo/0.3.0` | informational only — never a gate |

Message shapes, framing (`u32` LE length prefix, 16 MiB max frame) and
`MAX_RANGE = 256` are unchanged and remain frozen.

**Consequence, stated plainly.** Unlike v0.2 → v0.3, where RFC 0002 bumped
both negotiation strings so that incompatible nodes fail at negotiation,
RFC 0005 specifies no identifier change and none was made. A v0.3 node and
a v0.4 node therefore **negotiate successfully** and are separated only by
what happens next (matrix below): the v0.3 node cannot decode a block that
carries payload index 2, and the v0.4 node rejects a block that is valid
only under v0.3 rules. Both failures are deterministic and stop the
receiving node at that block; neither is silent acceptance. **Mixed-version
operation is forbidden by the activation procedure (§10), not prevented by
negotiation.** Bumping the negotiation strings so that it *is* prevented,
and updating the identify string, is a change to a locked surface
(RFC_PROCESS lists protocol negotiation strings) and is recorded as a
follow-up requiring its own RFC or spec addendum; it is not performed here.

**Mixed-version matrix** (derived from the v0.3 and v0.4 codecs and rules;
the v0.3 side is derived from the `v0.3-devnet-stable` source, not run):

| Situation | Behaviour |
|---|---|
| v0.3 node receives a v0.4 block containing a `ComputeTask` | SCALE decode fails on payload index 2 (the v0.3 union has indexes 0 and 1 only); the block cannot be decoded or applied; the node stops following at that height |
| v0.3 node receives a v0.4 block containing only transfers, or only anchors whose tasks lie in earlier blocks | decodes and validates under (a)–(j); it would accept, but it can never have applied the earlier task blocks, so in practice it has already stopped |
| v0.4 node receives a v0.3 block containing only transfers | accepted — the transfer path is unchanged |
| v0.4 node receives a v0.3 block containing `(ComputeTask, None)` | rejected, `TypePayloadMismatch` (rule k) |
| v0.4 node receives a v0.3 block containing an unbound `AnchorReceipt` | rejected, `TaskNotRegistered` (rule q) |
| v0.2 RPC client (typed, e.g. `@mbongo/sdk` 0.1.0) → v0.4 node | requests work; `getBlockByHeight` throws on any block carrying a task; anchoring without a task is `-32603` |
| v0.4 SDK source → v0.3 node | `getBlockByHeight` works (no tasks exist); submitting a `ComputeTask` object is refused by the v0.3 server's closed union (`-32602`); a bound receipt is accepted under (a)–(j) since v0.3 has no rule (q) |
| two producers of different versions on one network | not a supported topology at any version; the devnet has one producer |

### 10. Genesis and Activation

- **Activation model: a clean version boundary** (RFC 0005 §5.3, §13.1).
  There is **no activation height, no version flag and no wall-clock
  activation**; blocks are validated under v0.4 rules throughout. This is
  the same model v0.2 → v0.3 used.
- **Fresh genesis.** A v0.4 chain starts from a new, empty data directory.
  The genesis block itself is the unchanged code-defined deterministic
  genesis (empty body, funded public dev account, computed identically by
  every node on first start of an empty directory); "fresh" means a new
  chain, not a new genesis definition.
- **v0.3 history is not carried forward.** A v0.3 chain that ever anchored
  a receipt cannot be replayed under v0.4 (§9 matrix). Historical blocks
  are not rewritten and not reinterpreted; they are abandoned with the
  directory, exactly as v0.2 history was at v0.3. RFC 0005 §5.3 authorises
  this: no mainnet exists and devnet state is disposable.
- **v0.4 history replays deterministically under v0.4.** The replay harness
  (`cargo run -p mbongo-node --bin replay_harness`) re-applies a chain
  containing a `ComputeTask` and its bound receipt on a fresh follower and
  reaches the identical height and tip hash; CI runs it on every change.
- **Devnet reset is required** and is tested by both devnet paths: the
  Docker devnet declares no volumes, so every `make devnet-down` /
  `make devnet-up` starts from fresh genesis; the PowerShell deployment
  (`scripts/devnet/reset-devnet.ps1 -ConfirmReset`) wipes to fresh genesis
  only after a verified backup and two confirmations, never automatically.
  The PowerShell runbook is pinned to `v0.3-devnet-stable` and schema 2 and
  needs its v0.4 counterpart when the tag exists (Workstream H).
- **No production or mainnet migration is claimed.** The reset requirement
  is a devnet requirement under RFC 0005 §5.3; extending it, or any
  in-place migration, to a network with persistent history needs its own
  authority.

### 11. Protocol Vectors

The v0.4 vector set is sufficient for an independent implementation and is
frozen with this lock. Expected values were derived from the protocol rules
by an independent script (hand-laid SCALE, `@noble` BLAKE3 and Ed25519) —
never by encoding with production Rust or the SDK — and re-derived at this
lock: all three RFC 0005 fixtures are **JSON-identical** to the independent
derivation, and Rust and TypeScript both consume them.

| Fixture | Blob at lock | Pins |
|---|---|---|
| `test-vectors/compute-task/compute-task-v1.json` | `f44d5da6` | 4 task vectors (`canonical`, `empty-spec`, `spec-64`, `spec-max-1024`): canonical bytes, preimage, `task_id`; `spec-1025` rejected; 5 identity variants (each field changes `task_id`, the nonce does not); 4 wrong-tag diagnostics; 3 signed transactions (signing payload, signature, encoding, hash); serialized JSON; discriminants; the 1155/1177 maxima |
| `test-vectors/compute-task/anchor-binding-v1.json` | `5cd8a265` | `bound-named-executor`, `bound-maximal-task` (accepted); `unknown-task` (q), `input-commitment-mismatch` (r), `executor-not-named` (s) |
| `test-vectors/rpc/compute-task-rpc-v1.json` | `d455f196` | `minimal`, `canonical`, `maximal` wire objects; a block with all three payload variants; 3 undefined-variant forms; the recorded v0.2-client behaviour |
| `test-vectors/receipt/receipt-v1.json` | `acc079af` | unchanged since v0.3: 5 valid, 3 invalid receipts |
| `test-vectors/transaction/anchor-receipt-v1.json` | `3a21537e` | unchanged since v0.3: 2 valid, 1 encoding-only, 2 invalid anchoring transactions |

Consumers: `crates/mbongo-core/tests/{compute_task_vectors,transaction_vectors,receipt_vectors}.rs`,
`crates/mbongo-node/src/backend.rs` (`anchor_binding_vectors_drive_consensus`),
`crates/mbongo-network/tests/jsonrpc_tests.rs`, and
`sdk/typescript/test/{compute-task,bound-receipt,anchor,receipt}.test.mjs`.
Changing an expected value is a protocol change; adding vectors is not.

---

## SDK Compatibility

Two different facts, recorded separately:

| | State at this lock |
|---|---|
| **Repository SDK source** (`sdk/typescript/src` at `26beb2d8`) | **v0.4-compatible**: `ComputeTask` type and canonical encoder, `computeTaskId` (22-byte raw tag), `signComputeTaskTransaction` (type `0x01`, payload `0x02`, task at offset 90, submitter key), payload discriminants, `getBlockByHeight` decoding of all three variants, `computeTasksInBlock`, `signBoundReceipt` (derives `task_id`, `input_commitment`, `executor` from the task; refuses any key that does not derive `task.executor`), executor-signed anchoring. Reproduces every RFC 0005 fixture and the unchanged v0.3 fixtures; 140 tests on Node 20.19.0 and 24. |
| **Published package** (`@mbongo/sdk@0.1.0`, tag `sdk-typescript-v0.1.0` at `72b8441e`) | **not v0.4-compatible**: the tag predates the ComputeTask work (#132) and its source tree has no `compute-task.ts`; it is an rpc_v0.2 client with the §9 v0.2-client behaviour. |

Minimum compatible SDK behaviour, for any client: construct the six-field
envelope, encode it canonically, derive `task_id` with the raw tag, sign the
transaction with the submitter key, decode the three-member payload union,
and anchor only receipts bound to a registered task with the executor's key.

**Release prerequisite.** Publishing a v0.4-capable `@mbongo/sdk` requires a
version bump beyond `0.1.0` and the full [RELEASE.md](../runbooks/RELEASE.md)
procedure. It is a separate gate; no publish, tag or version change is part
of this lock.

---

## Non-consensus Reference Material

The following exist in the repository, are cited above as compatibility
evidence, and are **not protocol**. The lock names them so that nobody
mistakes them for it:

| Item | Status | Where |
|---|---|---|
| Reference worker, control plane, in-memory data plane | REFERENCE IMPLEMENTATION, NON-CONSENSUS; passes `compute-conformance-v1` 38/38 | `crates/mbongo-compute`, [reference-worker.md](../development/reference-worker.md) |
| Execution leases, sessions, attempt ids, data capabilities, private objects | OFF-CHAIN; never in a `ComputeTask`, a `Receipt` or chain state | E and F contracts |
| Reverse-bytes profile, `mbongo-ref:reverse-bytes:v1` | REFERENCE / TEST EXECUTION PROFILE; the protocol defines no execution profile | `crates/mbongo-compute/src/execution.rs` |
| `mbongo:compute-input:v1` / `mbongo:compute-output:v1` commitment derivations | interoperability convention (RFC 0005 §2.4), not consensus | — |
| `compute-conformance-v1` | engineering conformance contract; tests the architecture, not the protocol | [compute-conformance.md](../development/compute-conformance.md) |
| Privacy, data-plane and control-plane architecture | NORMATIVE for off-chain behaviour; RFC 0005 wins on protocol | `docs/architecture/` |
| Confidential execution, TEE, attestation, conditional key release | FUTURE; not required, not locked | — |
| GPU or AI execution, metering, pricing, worker rewards, MBO compute settlement, staking/slashing | FUTURE; no field, rule or value exists | — |
| Output correctness, verification (redundant execution, ZK, fraud proofs) | FUTURE; an anchored receipt is a bound claim, not a proof | #52 |

---

## Devnet Migration Procedure (v0.3 → v0.4)

1. Stop every v0.3 node.
2. Back up data directories if their contents are needed for reference
   (they cannot serve as a v0.4 chain).
3. Wipe the chain data directories (Docker: `make devnet-down`; PowerShell
   deployment: `reset-devnet.ps1 -ConfirmReset`).
4. Deploy the same v0.4 binary and configuration to every node.
5. Start all nodes from the fresh genesis.
6. Verify task commitment and bound anchoring, deterministic replay and
   multi-node convergence: `replay_harness`, `compute_harness`,
   `compute_conformance` and `devnet_harness` must pass.
7. **Never mix protocol versions on one network.** v0.3 and v0.4 peers
   negotiate (§9); the procedure, not the protocol, keeps them apart.

Rollback to v0.3 is wiping every touched data directory and starting a
v0.3 binary from a fresh genesis; tasks and receipts committed on the
abandoned v0.4 chain are lost with it. No in-place downgrade exists.

---

## Release

What this lock does and does not do, per [RFC_PROCESS.md](../RFC_PROCESS.md)
("Released: protocol lock document updated, new git tag created; RFC status
set to Released"):

| Item | State |
|---|---|
| Lock document | this file, FROZEN |
| Git tag `v0.4-devnet-stable` | **created** on `fcec8ddc7b06247460e04db987de08232992e2fc`, the release commit (RFC 0005 Released, `@mbongo/sdk` 0.2.0); annotated, immutable |
| RFC 0005 status | **Released** (2026-09-07) |
| Protocol version constant | none exists in code; the version is the lock and its tag, as at v0.3 |
| Crate versions | workspace `0.1.0`, unchanged, as at v0.3 |
| `@mbongo/sdk` release | `0.2.0`, tag `sdk-typescript-v0.2.0` on the same commit, published through the release workflow ([RELEASE.md](../runbooks/RELEASE.md)); the first v0.4-capable SDK |
| Release notes | the lock, RFC 0005 §13 and this table |

---

## Experimental and Deferred Surfaces

Not frozen by this lock; activating any of them requires its own RFC (or
spec addendum where CONTRIBUTION_TIERS permits):

- Receipt economics: fees, rewards, slashing, challenges, dispute resolution
- Compute economics: worker payment, metering, pricing, MBO compute settlement
- Verification strategies: redundant execution, TEE attestation, ZK proofs, PoUW
- Task assignment, discovery, expiry, and any widening of rule (s) (RFC 0005 §2.13)
- Dedicated compute and receipt RPC methods (all reserved names return `-32601`)
- Task and receipt pruning/archival policy
- Relayed/delegated submission (`sender != executor`, `sender != submitter`)
- `Stake` transaction semantics (still legacy fall-through; unvalidated)
- P2P identifier bump to gate v0.3/v0.4 negotiation (§9 follow-up)
- [RECEIPT_SPEC_v0.1.md](./RECEIPT_SPEC_v0.1.md) remains EXPERIMENTAL by its
  own terms, with the v0.3 qualification carried forward: its encoding,
  hash and validation rules **as consumed by consensus** are frozen here

---

## Lock Inventory

| # | Surface | Classification | Where |
|---|---|---|---|
| B1 | `ComputeTask` six-field envelope | LOCKED_V04 | §1 |
| B2 | `TransactionPayload::ComputeTask` = index 2 | LOCKED_V04 | §2 |
| B3 | `TransactionType::ComputeTask` = index 1 | UNCHANGED_FROM_V03 (restated) | §2 |
| B4 | `ComputeTask` canonical SCALE encoding | LOCKED_V04 | §1 |
| B5 | `task_id` domain bytes and derivation | LOCKED_V04 | §3 |
| B6 | `input_commitment` protocol semantics (equality only; opaque) | LOCKED_V04 | §4 |
| B7 | `execution_spec` bound (1024) and opacity | LOCKED_V04 | §1, §4 |
| B8 | validation rules (k)–(p) | LOCKED_V04 | §5 |
| B9 | validation rules (q)–(s) | LOCKED_V04 | §5 |
| B10 | validation precedence, incl. placement of (c), stored-tx and (d) | LOCKED_V04 | §5 |
| B11 | task storage key/value semantics; derived state | LOCKED_V04 | §6 |
| B12 | storage schema 3, migration, no downgrade | LOCKED_V04 | §6 |
| B13 | same-block task-then-receipt accepted; receipt-then-task rejected | LOCKED_V04 | §5 |
| B14 | block atomicity across tasks and receipts | UNCHANGED_FROM_V03 (extended to tasks) | §5, §6 |
| B15 | `Receipt` wire format | UNCHANGED_FROM_V03 | §7 |
| B16 | `AnchorReceipt` wire format | UNCHANGED_FROM_V03 | §7 |
| B17 | `AnchorReceipt` stricter validity | LOCKED_V04 | §7 |
| B18 | duplicate receipt behaviour, and its precedence over (q)–(s) | UNCHANGED_FROM_V03 (interaction locked) | §5 |
| B19 | rpc_v0.3 public contract | LOCKED_V04 (FROZEN) | §8 |
| B20 | rpc_v0.2 supersession / freeze | UNCHANGED_FROM_V03 (SUPERSEDED, intact) | §8 |
| B21 | repository SDK v0.4 compatibility behaviour | NON_CONSENSUS (recorded) | SDK compatibility |
| B22 | published SDK release status | RELEASE_FOLLOWUP | SDK compatibility |
| B23 | protocol vector set | LOCKED_V04 | §11 |
| B24 | activation rule: clean boundary, fresh genesis, no height gating | LOCKED_V04 | §10 |
| B25 | mixed-version behaviour; identifiers unchanged | LOCKED_V04 (documented) + RELEASE_FOLLOWUP (identifier bump needs its own RFC) | §9 |
| B26 | reference worker is non-consensus | NON_CONSENSUS | Non-consensus reference material |
| B27 | `compute-conformance-v1` is non-consensus | NON_CONSENSUS | Non-consensus reference material |
| B28 | TEE | FUTURE | Experimental and deferred |
| B29 | GPU/AI metering | FUTURE | Experimental and deferred |
| B30 | MBO compute settlement | FUTURE | Experimental and deferred |

Conflicts between repository reality and accepted authority found by the
audit: **none**. Every locked value above was read from code, fixtures and
tests and matched RFC 0005 and RFC 0002.

---

## How to Propose a Locked Change

Identical to the v0.3 process: file an RFC under `docs/rfcs/` per
[RFC_PROCESS.md](../RFC_PROCESS.md), identify the affected locked surface,
specify the new version number, obtain Core Maintainer approval, then bump,
update the lock, and tag.
