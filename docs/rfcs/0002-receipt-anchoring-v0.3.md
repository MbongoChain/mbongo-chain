# RFC 0002 — Receipt Anchoring (Protocol v0.3)

**Status:** Draft
**Author:** Gilbert Kalombo
**Created:** 2026-07-19
**Protocol version:** v0.2 → v0.3
**Locked surfaces affected:** Transaction SCALE encoding, `apply_block` validity rules, Storage trait semantics (additive), P2P protocol negotiation strings. See [Scope](#scope).

---

## Motivation

Mbongo Chain's product thesis ([VISION_v1.md](../VISION_v1.md)) is a deterministic verification layer for off-chain AI inference receipts. Today the chain cannot record a receipt at all: v0.2 is a transfer-only devnet, and the receipt primitive exists only as a pure library (`mbongo_verification::receipt`, implementing [RECEIPT_SPEC_v0.1.md](../specs/RECEIPT_SPEC_v0.1.md)) with no consensus, storage, or API integration.

Bridging that gap touches several locked v0.2 surfaces at once: the transaction format (to carry a receipt), `apply_block` (to validate it deterministically), the storage layer (to index anchored `task_id`s atomically with block application), and the physical RocksDB schema (a change that makes data directories unreadable by older v0.2 binaries). Per [PROTOCOL_LOCK_v0.2.md](../specs/PROTOCOL_LOCK_v0.2.md) and [RFC_PROCESS.md](../RFC_PROCESS.md), none of these changes may land piecemeal. This RFC defines the complete, minimal receipt-anchoring design for protocol v0.3 so the storage schema is changed exactly once, in the same protocol version that gives the change consensus meaning.

If we do nothing, the receipt library stays inert and the roadmap's v0.3 milestone ("receipt verification prototype") cannot ship.

---

## Scope

Minimal receipt anchoring only: a new transaction type that carries one receipt, validated deterministically during block application and persisted atomically with block state. The chain records that a well-formed, executor-signed receipt with a globally unique `task_id` was anchored at a given position in the chain. Nothing more.

Locked surfaces modified (from the [PROTOCOL_LOCK_v0.2.md](../specs/PROTOCOL_LOCK_v0.2.md) tables):

- [x] Block/transaction SCALE encoding — `Transaction` gains a trailing typed `payload` field; `TransactionType` gains an appended `AnchorReceipt` variant
- [ ] Hashing rules — algorithm and display format unchanged (hash *inputs* change only as a consequence of the encoding change above)
- [x] `apply_block` validity rules — per-transaction rule 5 gains type dispatch and receipt-specific checks in a normative order
- [x] Atomic `write_batch` requirement — requirement itself unchanged and reaffirmed; the operation set is extended with `PutReceipt`
- [x] Storage trait semantics — additive only: new `has_receipt`/`get_receipt` methods and `PutReceipt` batch op; existing method semantics, key schemas, and atomicity guarantees are untouched
- [x] P2P wire formats — message *shapes* unchanged, but `Block`/`Transaction` payloads inside them re-encode; protocol negotiation strings are bumped so v0.2 and v0.3 nodes fail cleanly at negotiation
- [ ] Frame encoding — unchanged (`u32` LE length prefix)
- [ ] RPC method names, params, or return types — unchanged; accepted *content* of `submit_transaction` changes (see [API Scope](#api-scope))

---

## Non-Goals

Explicitly out of scope, deferred to future RFCs:

- Proof of Useful Work, or any consensus change
- Rewards, fees, or any economic parameter (the [COMPUTE_INTERFACE_v0.1.md](../specs/COMPUTE_INTERFACE_v0.1.md) economic placeholders remain inactive)
- Challenge mechanism, dispute resolution, fraud proofs, or slashing
- Zero-knowledge proofs, TEE attestation, or any verification of the *correctness* of the underlying computation — the chain validates structure, signature, and uniqueness only
- On-chain execution of any workload
- Task submission lifecycle (`ComputeTask` transactions, scheduling, executor registration, reputation)
- **Relayed, delegated, or proxy submission of receipts.** v0.3 requires the transaction sender to *be* the receipt executor (see Design §2). Third-party relayers, meta-transactions, fee sponsorship, and executor-delegates-to-submitter models are all deferred; supporting them later is an additive relaxation gated by its own RFC.
- Dedicated receipt RPC methods (`submit_receipt`, `get_receipt`) — reserved here, activated in a separate step (see [API Scope](#api-scope))
- A global block-size limit. v0.2 has no explicit block-byte limit (the de facto bounds are `MAX_TX_PER_BLOCK = 1000` and the 16 MiB P2P frame cap); introducing an explicit limit is a separate protocol concern and is NOT smuggled in via this RFC's per-receipt cap (see Design §3)
- Tightening the currently unvalidated `Stake` and `ComputeTask` transaction types (v0.2 executes every transaction as a transfer regardless of `tx_type`; this RFC dispatches only `Transfer` and `AnchorReceipt` and leaves the legacy behavior of the other two variants as-is)

---

## Design

### 1. Transaction format (v0.3)

`TransactionType` gains one appended variant (existing variant indices are unchanged):

```rust
enum TransactionType {
    Transfer,      // index 0 (unchanged)
    ComputeTask,   // index 1 (unchanged, still inert)
    Stake,         // index 2 (unchanged, still inert)
    AnchorReceipt, // index 3 (NEW)
}
```

`Transaction` gains one trailing field, placed after `nonce` and before `signature`, using a **SCALE-typed payload enum** (see §1.1 for the alternative considered):

```rust
/// Typed transaction payload. Appending variants is the extension point
/// for future transaction kinds.
enum TransactionPayload {
    None,                   // index 0 — required for Transfer/ComputeTask/Stake
    AnchorReceipt(Receipt), // index 1 — required for AnchorReceipt
}

struct Transaction {
    tx_type: TransactionType,
    sender: Address,
    receiver: Address,
    amount: u128,
    nonce: u64,
    payload: TransactionPayload, // NEW
    signature: [u8; 64],
}
```

- `signing_payload()` covers all fields except `signature`, now including `payload`. The transaction hash remains `BLAKE3(SCALE_encode(transaction))` including the signature — the hashing *rule* is unchanged; its input changes because the encoding changes.
- Structural consistency is a validity rule: `tx_type == AnchorReceipt` ⟺ `payload` is the `AnchorReceipt` variant. Every other type MUST carry `TransactionPayload::None` (which encodes as a single `0x00` byte, so transfers grow by exactly one byte relative to v0.2).
- **Submitter identity (v0.3 minimal): `transaction.sender == receipt.executor` is REQUIRED.** The account that anchors a receipt is the executor itself: it pays the nonce (no fees exist in v0.3) and signs the transaction, and the same key signs the receipt hash inside the payload. The two signatures remain independent verifications over different messages, but in v0.3 they MUST verify against the same public key. Relayers/delegation are out of scope (see Non-Goals); relaxing this later (allowing `sender != executor`) is additive and needs no encoding change.
- For `AnchorReceipt`: `amount` MUST be `0`; `receiver` MUST be the zero address. Unused fields are pinned to a single canonical value so two encodings of the same anchoring cannot differ.

The receipt structure, field order, `receipt_hash` computation (BLAKE3 over the SCALE signing payload), and signature semantics (Ed25519 over the raw 32-byte hash) are exactly those specified in [RECEIPT_SPEC_v0.1.md](../specs/RECEIPT_SPEC_v0.1.md) and already implemented and pinned by test vector in `mbongo_verification::receipt`. This RFC changes nothing in that spec, and the existing fixed hash vector remains valid and untouched.

Because the payload is typed, a malformed receipt is not a decodable transaction at all: SCALE decoding of the `Receipt` happens wherever the transaction is decoded (RPC hex decode, mempool admission, block decode at the sync boundary), not as a separate validity rule inside `apply_block`. An undecodable payload is rejected at the decode boundary with the same error class as any other malformed transaction.

#### 1.1 Payload representation: `Vec<u8>` vs typed enum

Two candidate representations were compared:

| | `payload: Vec<u8>` (opaque bytes) | `payload: TransactionPayload` (typed enum) — **RECOMMENDED** |
|---|---|---|
| Decode point | Deferred: bytes decode as `Receipt` inside `apply_block` (extra validity rule, second decode pass) | At transaction decode: malformed receipts never construct a `Transaction` |
| Type safety | None: any byte string is representable; consistency between `tx_type` and payload content is convention | Structural: the variant carries a real `Receipt`; illegal states are mostly unrepresentable |
| Double encoding | Yes: receipt SCALE-encoded, then wrapped as `Vec<u8>` (length prefix + bytes) inside the tx encoding | No: receipt fields encode directly inside the enum variant |
| Witness stuffing | Needs an explicit "payload empty unless AnchorReceipt" rule on raw bytes | `None` variant is a single byte; nothing arbitrary to stuff |
| Future extension | New payload kinds are re-interpretations of bytes (error-prone) | Append enum variants (SCALE-clean, same pattern as `TransactionType`) |
| Dependency cost | None: `mbongo-core` never sees the `Receipt` type | `Receipt` type must be visible to `mbongo-core` (see below) |
| Malformed-payload failure mode | Inside consensus (`apply_block` rejects block) | At decode boundary (bad tx/block never enters validation) |

The only argument for opaque bytes is the dependency cost, and it dissolves on inspection: once receipts are consensus objects, the `Receipt` **data type** belongs in `mbongo-core` next to `Transaction` and `Block` — exactly where every other consensus-encoded type lives. Only the *type and its canonical byte identity* move; all receipt validation logic stays in `mbongo-verification`. The precise ownership split and migration requirements are normative in Design §6.1–6.2.

**Decision: the typed enum (APPROVED).** The current architecture provides no strong reason to prefer opaque bytes, and the typed representation removes an entire class of validity rules and failure modes.

### 2. Consensus integration: `apply_block` (v0.3)

The five v0.2 validity rules are retained. Rule 5 (per-transaction validation) gains type dispatch. For each transaction at index `i`, in body order, the checks run in this **normative** sequence — cheap structural checks first, signature verifications after, state-dependent checks last. Every node accepts or rejects identically and reports the same first failure:

> a. **Type/form:** `tx_type` is a known variant; payload variant is structurally consistent with `tx_type` (`AnchorReceipt` ⟺ `AnchorReceipt(receipt)`, otherwise `None`).
> b. **Field constraints:** for `AnchorReceipt`: `amount == 0` and `receiver == Address::zero()`.
> c. **Transaction signature:** Ed25519 by `sender` over `signing_payload()`.
> d. **Transaction nonce:** `validate_and_increment_nonce` against the in-memory account view (no persistent mutation; see §4).
> e. **Payload size:** `receipt.metadata.len() <= MAX_RECEIPT_METADATA_BYTES` (see §3). *(SCALE decoding of the receipt already happened at the transaction decode boundary — typed payload, §1 — so no decode step occurs here.)*
> f. **Receipt version:** `receipt.version == 1`.
> g. **Submitter identity:** `transaction.sender == receipt.executor`.
> h. **Receipt signature:** Ed25519 by `receipt.executor` over the raw 32-byte `receipt_hash` (never the hex display string).
> i. **Duplicate vs prior state:** `receipt.task_id` is not already anchored in the chain state as of the parent block.
> j. **Duplicate vs this block:** `receipt.task_id` does not appear in an earlier `AnchorReceipt` transaction of the same block.

Rule (g) is a **transaction-level anchoring rule, not an intrinsic property of a receipt**: it relates two objects (the enclosing transaction and the embedded receipt) and is therefore orchestrated by `mbongo-node` during block application, alongside the ordering of all the rules above. A standalone `Receipt` is cryptographically valid on its own terms — structure, version, executor signature — before and independent of being placed in any `AnchorReceipt` transaction; the pure verification entry point in `mbongo-verification` (§6.1) checks exactly that and knows nothing about transactions.

Checks (e)–(j) apply only to `AnchorReceipt` transactions; `Transfer` (and, unchanged from v0.2, `Stake`/`ComputeTask`) proceed from (d) to the existing balance rules. Ordering rationale: (a)–(d) are cheap or already-required work shared with all transaction types; the second signature verification (h) — the most expensive receipt-specific check — runs only after every structural and identity check has passed; state lookups (i)–(j) come last so a block full of garbage receipts is rejected before touching the index.

**Any failing check invalidates the entire block** — consistent with existing v0.2 behavior, where one bad signature rejects the whole block. There is no partial acceptance and no skip-and-continue for receipt transactions.

Uniqueness semantics: `task_id` is globally unique across the whole chain, first-anchored-wins. Rules (i) and (j) together make the duplicate decision a pure function of (prior state, block contents, body order) — deterministic on every node. A block containing two receipts with the same `task_id` is invalid regardless of order (rule j fires on the second, invalidating the block). A block anchoring a `task_id` that exists in prior state is invalid (rule i).

Checks (f)–(j) reuse `mbongo_verification` logic; the duplicate index for (i)+(j) is supplied through the existing read-only `ReceiptIndex` port — see §4 and §5.

The mempool applies the same checks on submission as a best-effort filter (including a mempool-local duplicate `task_id` guard); mempool acceptance is advisory, consensus validation in `apply_block` is authoritative.

### 3. Size limits

**`MAX_RECEIPT_METADATA_BYTES = 4096` (4 KiB) — proposed value, requires maintainer approval** (see [Decision record](#decision-record)). The cap applies to the `metadata` field, the only variable-length component of a receipt. All other fields are fixed-size, so the cap bounds the entire receipt: a maximal receipt encodes to `1 + 32 + 32 + 32 + 32 + (compact_len ≤ 2) + 4096 + 64 = 4291` bytes. Rationale for 4 KiB: metadata is an opaque application-layer commitment pointer (spec §7 of RECEIPT_SPEC_v0.1 — the chain never interprets it); anything larger belongs off-chain behind a hash, which is the entire design philosophy of receipt anchoring. The cap is a consensus validity rule (e): raising it is a protocol version bump; it is never lowered retroactively.

**A per-receipt limit is not a block-size limit.** With `MAX_TX_PER_BLOCK = 1000`, a pathological all-receipt block is bounded at roughly 4.5 MB — within the existing 16 MiB P2P frame cap, which together with the transaction-count cap remains the only global bound, exactly as in v0.2. An explicit block-byte limit is a separate protocol concern (listed in Non-Goals) and should get its own RFC; this RFC neither introduces nor depends on one.

### 4. Atomicity and in-block duplicate implementation

Receipt persistence is part of block application, inside the **same** `write_batch` as the block, its transactions, and account updates:

```rust
enum BatchOp {
    // ... all seven existing variants, unchanged ...
    PutReceipt([u8; 32], Vec<u8>), // NEW: task_id → SCALE-encoded receipt bytes
}
```

The implementation contract for `apply_block`, mirroring the existing `account_cache` pattern:

1. **Immutable prior-state index.** Duplicate check (i) reads the persistent `receipts` column family through the read-only `Storage::has_receipt` — a view of state as of the parent block. Nothing is written during validation.
2. **Temporary `pending_task_ids` set.** A transient in-memory `HashSet<[u8; 32]>`, empty at the start of each block validation, accumulates the `task_id` of every validated `AnchorReceipt` in body order. Check (j) consults this set. It is discarded when validation ends (success or failure); it never touches storage.
3. **No persistent mutation until the entire block is valid.** All effects — receipts (`PutReceipt`), account updates, the block itself, indexes — accumulate as `BatchOp`s in a `Vec` while validation walks the transactions. If any transaction fails any check, the batch is dropped and the block is rejected with no trace.
4. **One final atomic `WriteBatch`.** On full success, the accumulated ops commit in a single `Storage::write_batch` call. The v0.2 atomicity invariant ([storage_invariants.md](../architecture/storage_invariants.md)) extends to receipts unchanged: a block is applied all-or-nothing.

**There is no standalone check-then-insert receipt API.** An independent `insert_receipt`-if-absent method is rejected: outside the block batch it cannot be made concurrency-safe or consensus-meaningful, and inside consensus it is unnecessary — uniqueness is decided by rules (i)/(j) *before* the batch is built, and the batch write itself is atomic. Storage exposes only reads (`has_receipt`, `get_receipt`) and a batch op (`PutReceipt`); the decision logic lives in consensus.

Duplicate within one block: block invalid (rule j). `task_id` already in prior state: block invalid (rule i). In both cases nothing is written.

### 5. Storage schema

**Layout.** One new RocksDB column family in the existing database:

| CF | Key | Value |
|----|-----|-------|
| `receipts` (NEW) | `task_id` — raw 32 bytes | SCALE-encoded `Receipt` — opaque bytes to the storage layer |

The six existing column families (`accounts`, `blocks`, `transactions`, `meta`, `height_index`, `tx_seq_index`) and their key/value encodings are byte-for-byte unchanged. Storage never decodes receipt bytes ("persistence only, no business logic" per [ARCHITECTURE_GUARDRAILS.md](../ARCHITECTURE_GUARDRAILS.md)); duplicate detection needs only key existence, which RocksDB provides in O(log n).

**Storage trait additions (additive only):**

```rust
fn has_receipt(&self, task_id: &[u8; 32]) -> Result<bool, StorageError>;
fn get_receipt(&self, task_id: &[u8; 32]) -> Result<Option<Vec<u8>>, StorageError>;
// plus BatchOp::PutReceipt handled by write_batch
```

The locked semantics of `get_block_by_height`, `get_latest_height`, and `write_batch` atomicity are unchanged.

**Schema versioning and open/migration sequence.** A new `meta` key `schema_version` (`u32`, big-endian; absent means 1, the v0.2 layout) is introduced. A v0.3 binary opens a database with this normative sequence:

1. **List** existing column families via `DB::list_cf` on the path (for a fresh directory, skip to step 5).
2. **Reject unknown.** If the listing contains any column family not in the v0.3 known set (`default`, the six v0.2 CFs, `receipts`), refuse to open: the directory was written by a newer or foreign binary. Error message names the unknown CF.
3. **Open** the database with exactly the intersection of listed and known column families (never `create_missing_column_families` blindly).
4. **Check version.** Read `schema_version` from `meta`. If greater than 2, refuse to open with an error naming both the found and the supported version. (Steps 2 and 4 are complementary guards: step 2 catches structural drift, step 4 catches versioned schemas that reuse known CF names.)
5. **Migrate v1 → v2 if needed.** If `schema_version` is absent/1: create the `receipts` CF (via `create_cf` on the open handle) if and only if it does not already exist, **then** stamp `schema_version = 2`. The `receipts` CF is created *only* through this migration path (or fresh-directory initialization, which creates all CFs and stamps 2 immediately).
6. **Steady state.** If `schema_version == 2`, verify `receipts` is present (create-if-missing as self-healing is permitted since the CF is derived state) and proceed.

**Crash behavior during migration.** The migration has exactly one intermediate state: `receipts` CF created but `schema_version` not yet stamped. A crash there is harmless — on next open, step 5 finds version 1 with the CF already present, skips creation, and stamps the version. The migration is idempotent and involves no data transformation, so there is no partially-migrated data to corrupt. The stamp is written only after successful CF creation, never before.

**Downgrade/rollback.** Not supported on a touched data directory. A v0.2 binary cannot open a database containing the `receipts` CF (RocksDB refuses databases with column families it was not asked to open, and v0.2 predates the `schema_version` guard so the error is RocksDB's, not ours). Rolling back to v0.2 therefore requires wiping the data directory and resyncing — acceptable for disposable devnet directories, and stated in [Rollout](#rollout) as an explicit limitation.

**Recovery and replay.** The `receipts` CF is fully derived state: every anchored receipt is contained in a block, so replaying the chain from genesis deterministically reconstructs the CF byte-for-byte. Crash recovery needs nothing new — `WriteBatch` atomicity guarantees the CF is always consistent with the persisted tip. The deterministic replay harness gains a check that the replayed `receipts` CF content matches the original ([Testing](#testing)).

### 6. Dependency architecture

Edges after Phase 2: `mbongo-verification → mbongo-core`; `mbongo-storage → mbongo-core`; `mbongo-node → {storage, verification, core, network}` — identical to today. No new inter-crate edges anywhere in this RFC.

#### 6.1 Crate ownership (normative)

| Crate | Owns | Explicitly does NOT own |
|-------|------|-------------------------|
| `mbongo-core` | Canonical `Receipt` data structure; SCALE `Encode`/`Decode` with the fixed field order; `signing_payload()`; `receipt_hash()`; `TransactionPayload::None`; `TransactionPayload::AnchorReceipt(Receipt)` | Any validation judgment — core defines *what a receipt is*, never whether one is acceptable |
| `mbongo-verification` | `ReceiptIndex` trait; `ReceiptError`; version validation; Ed25519 receipt-signature verification (over the raw 32-byte hash); duplicate validation against a supplied index; `validate_receipt` (or equivalent) — the pure verification entry point | Storage access; transaction-level rules (it never sees a `Transaction`); the ordering of consensus checks |
| `mbongo-node` | Composition of the prior persistent receipt state and `pending_task_ids` into the read-only index (§4); the deterministic validation order (a)–(j) of §2; state-transition orchestration; construction of the final atomic `WriteBatch` | Receipt cryptography or encoding (delegates to core/verification) |
| `mbongo-storage` | Opaque receipt persistence and lookup: `has_receipt`, `get_receipt`, `BatchOp::PutReceipt` over raw `task_id` keys and opaque value bytes | Signature, version, or any receipt business validation; the `Receipt` type; the `ReceiptIndex` trait |

Constraints restated as rules:

- **`mbongo-core` stays pure logic** (no I/O) per [ARCHITECTURE_GUARDRAILS.md](../ARCHITECTURE_GUARDRAILS.md); it gains data definitions, not policy.
- **`mbongo-verification` stays pure.** No I/O, no storage dependency, ever. A `verification → storage` edge is forbidden (and would invite a cycle).
- **`mbongo-storage` does NOT depend on `mbongo-verification`.** An earlier draft placed an `impl ReceiptIndex for RocksDbStorage` in `mbongo-storage`, requiring a `storage → verification` edge. That edge is acyclic and points the defensible ports-and-adapters direction, but it is **unnecessary**: no consumer outside `mbongo-node` would use it, and "a consumer needs it" is the only justification that adds inter-crate edges. Rejected.
- **The adapter lives in `mbongo-node`**, which already depends on both crates. During `apply_block`, the node builds the composite read-only `ReceiptIndex` over (`Storage::has_receipt` ∪ `pending_task_ids`, §4) and passes it to `validate_receipt`. Errors map `StorageError → ReceiptError::Index`.

#### 6.2 Migration requirements for the existing receipt library

The receipt library shipped in `mbongo-verification` (commit `5d58cd1`) is relocated in Phase 2 under these normative requirements:

1. **Byte identity preserved.** The `Receipt` struct moves to `mbongo-core` without any change to its SCALE encoding: same fields, same order, same derives. Any byte emitted or accepted before the move is emitted and accepted identically after it.
2. **Hash vector preserved.** The fixed receipt hash vector (`0x56510b…a0f1`) passes byte-for-byte after the move. The vector test relocates with the type (or is duplicated in verification's suite) but its expected value MUST NOT change; a changed vector fails the migration.
3. **No duplicate definitions.** Exactly one `Receipt` definition exists in the workspace at every commit — the move and the deletion of the old definition land in the same commit. Transitional copies, even private ones, are forbidden (two definitions with independent derives is how encodings silently fork).
4. **Re-export for compatibility (optional but recommended).** `mbongo-verification` MAY `pub use mbongo_core::Receipt` (and related types it previously exported) so internal call sites and any external consumers of the crate keep compiling without path changes.
5. **Logic split enforced.** `verify_signature` (currently a method on `Receipt` in `mbongo-verification`) does not move to core with the type: signature verification is validation, owned by `mbongo-verification` per §6.1 — it becomes a function (or extension) in the verification crate operating on the core type. Likewise `validate`, `ReceiptIndex`, and `ReceiptError` stay put.

### 7. Protocol changes and compatibility impact

**Version bump:** protocol v0.2 → v0.3 (breaking). Required by [RFC_PROCESS.md](../RFC_PROCESS.md) versioning rules because old and new nodes cannot interoperate.

**Why interoperability breaks:** SCALE has no optional or self-describing fields. A v0.2 node cannot decode a v0.3 `Transaction` (unexpected payload field before the signature bytes) and therefore cannot decode any v0.3 block that contains transactions. The aspiration in [COMPUTE_INTERFACE_v0.1.md](../specs/COMPUTE_INTERFACE_v0.1.md) §7 that "v0.2 nodes can still sync blocks but ignore compute fields" is **not achievable** with the v0.2 encoding, which has no envelope versioning; this RFC supersedes that wording for the v0.3 transition.

**Old nodes reject, they do not ignore — and they must reject cleanly.** To prevent v0.2 and v0.3 nodes from half-connecting and exchanging undecodable payloads, the P2P protocol negotiation strings are bumped:

| Protocol | v0.2 | v0.3 |
|----------|------|------|
| Sync | `/mbongo-sync/1` | `/mbongo-sync/2` |
| Block notify | `/mbongo/block_notify/0.1.0` | `/mbongo/block_notify/0.2.0` |

Message shapes (`SyncRequest`, `SyncResponse`, `SyncNotification`, `BlockNotifyAck`), framing, and `MAX_RANGE` are unchanged; only the `Block`/`Transaction` bytes carried inside re-encode. Mixed-version nodes fail protocol negotiation and simply never sync — the correct outcome for a hard break.

**Chain data:** existing v0.2 chain history cannot be migrated. Historical transactions are signed over the v0.2 signing payload; re-encoding them under v0.3 would invalidate every signature, and signatures cannot be re-created. v0.3 devnets start from a fresh genesis. (The genesis block itself is encoding-identical in both versions — an empty transaction vector encodes the same — but this is inconsequential given the reset.)

**Block header:** untouched. Field set, order, hashing, and `transactions_root` scheme are all unchanged — the root automatically commits to the payload because it hashes whole SCALE-encoded transactions.

---

## API Scope

**Method surface unchanged; accepted content changes.** No JSON-RPC or REST method is added, renamed, or removed, and no parameter or return *shape* changes. But this RFC does not claim "zero API change":

- **`submit_transaction` behavior changes.** Its parameter remains `[signed_tx: string]` (hex-encoded SCALE transaction) and its return remains `{ tx_hash }`, but the accepted SCALE encoding becomes the v0.3 transaction format. Consequences: (1) transactions encoded by v0.2 clients no longer decode and are rejected with the existing `-32602` invalid-params error; (2) a well-formed `AnchorReceipt` transaction is now accepted content; (3) the receipt-specific validation failures (rules b, e–j of Design §2) surface through the existing `-32000` application-error code with descriptive messages — no new error *codes*. This is the same client-breaking encoding boundary described in [Protocol changes](#7-protocol-changes-and-compatibility-impact), stated here explicitly for API consumers.
- **How a receipt is submitted in v0.3:** the executor builds a `Receipt`, signs its hash, wraps it in an `AnchorReceipt` transaction (sender = executor, amount 0, zero receiver, correct nonce), signs the transaction, SCALE-encodes it, hex-encodes that, and calls the existing `submit_transaction`. The `mbongo-wallet` SDK gains a helper for this flow (Tier 2, Phase 2).
- **Dedicated receipt methods remain reserved, not implemented.** `submit_receipt` and `get_receipt` are hereby reserved (returning JSON-RPC `-32601` until activated), joining the five reserved names from [COMPUTE_INTERFACE_v0.1.md](../specs/COMPUTE_INTERFACE_v0.1.md) §3. Their activation — parameter shapes, error codes, an rpc_v0.2 spec addendum — is a separate implementation step (and spec change) after the consensus layer of this RFC is proven on devnet; whether it needs a full RFC or a spec addendum follows the CONTRIBUTION_TIERS rule for new non-conflicting endpoints.

---

## Compatibility

- **Existing nodes:** v0.2 and v0.3 nodes do not interoperate. Negotiation-string bump makes the failure clean (no connection) rather than byzantine (undecodable payloads). Devnet upgrades are coordinated: all nodes restart on v0.3 together.
- **Existing data:** v0.2 data directories are not carried forward. Forward path: wipe and restart from the v0.3 genesis (devnet data is disposable by policy). A v0.3-touched directory cannot be opened by v0.2 (Design §5); rollback requires a wipe.
- **Existing clients:** rpc_v0.1 method surface is stable, but clients that hex-encode transactions must move to the v0.3 encoding (see [API Scope](#api-scope)). `mbongo-wallet` and the SDK update in lockstep; they are Tier 2 and version-pinned to the node.

Migration steps are enumerated in [Rollout](#rollout).

---

## Security

- **Attack surface:** transaction decoding now includes receipt fields. The typed payload (§1.1) means malformed receipts are rejected at the decode boundary — before validation, mempool admission, or block application — with decode work bounded by the message size and the `MAX_RECEIPT_METADATA_BYTES` cap bounding legitimate receipt size.
- **DoS:** duplicate-`task_id` spam is rejected at mempool admission (persistent-index lookup + mempool-local set) before reaching a block. Receipt signature verification adds at most one extra Ed25519 verify per receipt transaction — same cost class as existing transaction verification, bounded by `MAX_TX_PER_BLOCK`, and ordered after all cheap checks (Design §2) so structurally invalid receipts never reach it.
- **Block validation integrity:** all new rules are pure functions of (prior state, block bytes, body order); no wall-clock, randomness, or node-local configuration enters validation. Determinism is preserved by construction and re-proven by the replay harness.
- **Signatures and keys:** two Ed25519 verifications per anchoring — the transaction signature (over the tx signing payload) and the receipt signature (over the raw 32-byte receipt hash) — both against the same key in v0.3 (`sender == executor`). The signed messages have different fixed structures, so no cross-protocol signature reuse is possible; requiring the same key does not weaken this (an attacker without the executor key can forge neither).
- **Replay protection:** transaction-level replay is covered by the existing nonce rule; receipt-level replay by global `task_id` uniqueness (first-anchored-wins).

---

## Testing

Acceptance criteria: every item below implemented and green, plus the full v0.2 regression suite unchanged.

- [ ] **Receipt validation vectors** — the existing `mbongo-verification` fixed vector (`receipt_hash` = `0x56510b…a0f1`) still passes **unchanged** (it moves crates in Phase 2 but stays byte-identical); new vectors for a full `AnchorReceipt` transaction: fixed encoded bytes, fixed tx hash, valid/invalid transaction signature, valid/invalid receipt signature, tampered payload, `sender != executor` rejection.
- [ ] **Form rules** — non-`None` payload on `Transfer`/`Stake`/`ComputeTask` rejected; `AnchorReceipt` with `None` payload rejected; `amount != 0` or non-zero `receiver` rejected; metadata over `MAX_RECEIPT_METADATA_BYTES` rejected at rule (e); malformed receipt bytes rejected at the decode boundary (RPC `-32602`, block decode failure at sync).
- [ ] **Normative order** — targeted tests pinning first-failure reporting: a transaction failing both (c) and (h) reports (c); failing both (e) and (i) reports (e).
- [ ] **Duplicate handling** — same `task_id` twice in one block → block invalid, nothing persisted; `task_id` in prior state → block invalid; distinct `task_id`s in one block → all anchored; first-anchored-wins across blocks; `pending_task_ids` demonstrably discarded on rejection (a rejected block leaves the next block's validation unaffected).
- [ ] **Atomic failure** — a block whose *last* transaction fails any receipt rule leaves *no* trace: no receipt keys, no account changes, no block, height unchanged (both backends).
- [ ] **Storage schema** — `has_receipt`/`get_receipt`/`PutReceipt` round-trip on both backends; open-sequence tests: fresh directory initializes all CFs and stamps 2; v0.2-layout directory migrates (CF created, then stamped) and re-opens cleanly; directory with an unknown CF refused with the naming error; `schema_version > 2` refused with the versions in the error; simulated crash between CF creation and stamping recovers idempotently on next open.
- [ ] **Deterministic replay** — replay harness extended: after replaying a receipt-bearing chain, tip hash AND full `receipts` CF contents match the original byte-for-byte.
- [ ] **Multi-node convergence** — devnet harness extended with `AnchorReceipt` traffic: producer + followers converge on identical height, tip hash, and receipt state across producer/follower restarts.
- [ ] **Mixed-version behavior** — a v0.2 peer and a v0.3 node fail protocol negotiation and exchange no blocks (test via protocol-string mismatch); documented as the expected outcome rather than graceful degradation.

---

## Rollout

Implementation phases (each lands as a separately reviewable PR on `dev`, in order; the protocol is "v0.3" only when all are merged):

1. **Phase 1 — Storage schema.** `receipts` CF, `schema_version` guard and open/migration sequence (Design §5), `has_receipt`/`get_receipt`, `BatchOp::PutReceipt`, both backends, schema tests. No node wiring; behavior of v0.2 surfaces unchanged. (Tier 0, this RFC as authority.)
2. **Phase 2 — Transaction format.** `TransactionPayload` enum, `AnchorReceipt` variant, `Receipt` data-type relocation to `mbongo-core` under the ownership split and migration requirements of Design §6.1–6.2 (validation logic stays in `mbongo-verification`; hash vector byte-identical), signing-payload update, wallet/SDK helper, encoding vectors. Chain restart boundary: from this phase on, fresh genesis required.
3. **Phase 3 — Consensus rules.** `apply_block` dispatch and normative rules (a)–(j), `pending_task_ids` mechanics, node-side composite `ReceiptIndex` adapter, mempool checks, atomicity and duplicate tests, extended replay/devnet harnesses.
4. **Phase 4 — Network cutover.** Protocol string bump, mixed-version test, devnet migration, docs.

**Activation:** by version, not by feature flag. A binary built from post-Phase-4 `dev` speaks only v0.3. No runtime toggle — a flag that switches consensus rules would make validation outcome depend on node configuration, which violates determinism.

**Devnet migration procedure:** announce cutover; stop all nodes; wipe data directories; start all nodes on the v0.3 binary (fresh genesis); run devnet + replay harnesses as the smoke test.

**Lock document update:** on release, create `PROTOCOL_LOCK_v0.3.md` freezing the v0.3 transaction encoding (including `TransactionPayload`), the extended `apply_block` rules and their normative order, `MAX_RECEIPT_METADATA_BYTES`, the `receipts` CF schema and `schema_version` semantics, and the new protocol strings. `RECEIPT_SPEC_v0.1.md` remains EXPERIMENTAL until v1.0-mainnet per its own upgrade-path section.

**Git tag:** `v0.3-devnet-stable` at completion.

**Rollback limitations (explicit):** rolling back to v0.2 after cutover requires wiping every touched data directory — v0.2 binaries cannot open the v0.3 schema, and v0.3 chain data has no backward migration (signatures cannot be re-created under the old encoding). Any receipts anchored on the abandoned v0.3 devnet chain are lost with it. This is acceptable pre-mainnet and is the strongest argument for keeping this RFC minimal.

---

## Decision record

**Accepted** (settled by existing specs, prior review, or forced by invariants — implementing them needs no further sign-off beyond this RFC's approval):

- Receipt structure, encoding, hash, and signature semantics per RECEIPT_SPEC_v0.1 (already implemented and vector-pinned; unchanged here).
- Receipt persistence atomic with block application via the shared `WriteBatch`; no standalone check-then-insert API (Design §4).
- Whole-block invalidation on any failing check (consistency with v0.2 behavior).
- Global first-anchored-wins `task_id` uniqueness; duplicate-in-block invalidates the block.
- New `receipts` column family over the alternatives (see [Alternatives Considered](#alternatives-considered)).
- **Typed `TransactionPayload` enum with the canonical `Receipt` data type relocated to `mbongo-core` — APPROVED by maintainer decision (2026-07-19)**, under the normative crate-ownership split of Design §6.1 (core owns the data type, encoding, `signing_payload`, `receipt_hash`; verification owns all validation including receipt-signature verification) and the migration requirements of Design §6.2 (byte identity, hash vector, no duplicate definitions, optional re-export).
- **`transaction.sender == receipt.executor` required for v0.3 minimal — APPROVED by maintainer decision (2026-07-19)**, explicitly deferring relayers/delegation. It is a transaction-level anchoring rule orchestrated by `mbongo-node`, not an intrinsic receipt validity rule — a standalone receipt is cryptographically valid independent of any enclosing transaction (Design §1, §2).
- **`MAX_RECEIPT_METADATA_BYTES = 4096` — APPROVED by maintainer decision (2026-07-19)** (Design §3).
- **The normative validation order (a)–(j) as the locked v0.3 ordering — APPROVED by maintainer decision (2026-07-19)** (Design §2).
- **Protocol string values `/mbongo-sync/2` and `/mbongo/block_notify/0.2.0` — APPROVED by maintainer decision (2026-07-19)** (Design §7).
- Storage/verification dependency rules: verification pure, storage byte-oriented, adapter in the node, no new inter-crate edges (Design §6.1).
- Old nodes reject rather than ignore; fresh genesis at v0.3; no history migration (forced by SCALE, Design §7).

**Proposals requiring maintainer approval:** none outstanding — all design decisions in this RFC are accepted as of 2026-07-19.

**Deferred** (out of this RFC; each needs its own RFC or spec addendum when taken up):

- Activation of `submit_receipt` / `get_receipt` (and the rpc_v0.2 addendum defining their shapes).
- Relayed/delegated submission (`sender != executor`), fee sponsorship, meta-transactions.
- Fees, rewards, slashing, challenges — all economics.
- An explicit global block-size limit.
- Tightening or activating `Stake` and `ComputeTask` transaction types.
- Receipt pruning/archival policy (the `receipts` CF grows monotonically; irrelevant at devnet scale).

---

## Alternatives Considered

**A. New RocksDB column family (`receipts`) — RECOMMENDED.**
Pros: clean key space; O(log n) existence checks with no prefix scans; participates in the existing shared `WriteBatch` (atomicity with block application); iteration over all receipts stays trivially possible for future RPC. Cons: physical schema change → v0.2 binaries cannot open touched directories. That cost is unavoidable *somewhere* in this RFC anyway (the transaction encoding change already forces a chain reset), so paying it in the same version transition is strictly better than paying it twice.

**B. Prefixed keys in an existing column family (e.g. `meta` or `transactions` with a `receipt:` prefix).**
Avoids the CF-open incompatibility — a v0.2 binary could still open the directory. Rejected: it pollutes a locked key schema (prefix collisions with future keys in a CF whose layout is frozen), makes existence checks and future iteration depend on prefix conventions rather than CF isolation, and buys nothing real — the directory a v0.2 binary could "still open" contains a v0.3 chain it cannot decode anyway. Compatibility of the container without compatibility of the contents is not compatibility.

**C. Separate receipt database (second RocksDB instance).**
Rejected outright: receipts could no longer be written in the same `WriteBatch` as block state, so block application would span two non-atomic commits. That violates the atomicity invariant this RFC is required to preserve (it is the check-then-insert flaw at the architecture level). Crash between the two commits desynchronizes receipt state from chain state, breaking deterministic replay.

**D. Storage-only implementation before consensus wiring (the previously evaluated step).**
Rejected — this rejection is the origin of this RFC. It ships the downgrade-breaking schema change *without* the consensus feature that justifies it; it requires a standalone insert API whose check-then-insert semantics are not concurrency-safe and would be deleted in the consensus step; and it risks the schema being subtly wrong for atomic integration (e.g. an insert API instead of a batch op). The correct order is: agree on the full design (this RFC), then implement storage as Phase 1 *of* it.

**Recommendation:** Alternative A, implemented via the four phases in [Rollout](#rollout). It is the only option that keeps receipt persistence atomic with block application, keeps key schemas clean, and takes the unavoidable compatibility break exactly once, at a declared protocol version boundary.
