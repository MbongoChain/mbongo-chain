# RFC 0005 — Compute Task Commitment (Protocol v0.4)

**Status:** Draft
**Author:** Gilbert Kalombo
**Created:** 2026-08-29
**Protocol version:** v0.3 → v0.4
**Locked surfaces affected:** `TransactionPayload` SCALE encoding, `apply_block` validity rules, Storage trait semantics (additive), RocksDB schema version. See [Scope](#scope).

---

## Motivation

Protocol v0.3 gave the chain a receipt. It did not give the receipt a
question to answer.

Today an executor can anchor a `Receipt` carrying **any** `task_id` and
**any** `input_commitment`. Consensus checks that the receipt is canonical,
that the executor signed it, and that no receipt for that `task_id` was
anchored earlier — and nothing else. No client ever attested to the input, and
nothing on chain says which task the receipt answers. `Receipt.input_commitment`
is a 32-byte field that currently binds to nothing.

That is the gap this RFC closes, and only that gap.

After this RFC, a client commits a task on chain; an executor answers it; and
consensus checks that the receipt's `input_commitment` is the one the client
committed to. The receipt stops being an unattributed claim and becomes an
answer to an authorised question.

**This RFC does not make the chain verify computation.** It never will under
this design. What it establishes is *correspondence*: this executor answered
*this* committed task with *this* output commitment, first. Whether the output
is correct is [#52](https://github.com/MbongoChain/mbongo-chain/issues/52) and
a later RFC.

### Why the protocol, and not a worker

[VISION_v1.md](../VISION_v1.md) is explicit: Mbongo "does not execute AI
models", "does not schedule, route, or manage GPU hardware", and execution
"happens off-chain on infrastructure the executor controls."

Every part of the client → worker → receipt loop except the commitment can
therefore be built with no protocol change at all, using primitives that
already exist. The commitment is the one piece that cannot: it needs consensus
authority. This RFC adds that piece and nothing else.

---

## Scope

**In scope**

- A canonical `ComputeTask` envelope and its identity derivation
- A new `TransactionPayload` variant carrying it
- Task validation, storage and uniqueness
- A binding rule tying `AnchorReceipt` to a registered task
- The disposition of the legacy `TransactionType::ComputeTask` fall-through
- The compatibility relationship with `COMPUTE_INTERFACE_v0.1`

**Explicitly out of scope**

Marketplace, worker scheduling or assignment, task discovery, payment,
rewards, staking, slashing, fee markets, PoUW, fraud proofs, TEE attestation,
ZK proofs, GPU management, AI inference APIs, reputation, and any form of
computation verification.

No field is reserved "for later" for any of the above. Adding one when it is
designed is a protocol change either way, and an unused field is a liability
in the meantime.

---

## 1. Authority: one receipt model

`COMPUTE_INTERFACE_v0.1` §2 defines a `ComputeReceipt` that predates the
implemented one and conflicts with it:

| | `COMPUTE_INTERFACE_v0.1` `ComputeReceipt` | Implemented `Receipt` (v0.3) |
|---|---|---|
| binds the input | **no** | `input_commitment` |
| signature message | `SCALE(all fields except signature)` | the **raw 32 bytes** of the BLAKE3 hash |
| version field | none | `version: u8` |
| self-reported fields | `compute_time_ms`, `hardware_id`, `proof_blob` | none; `metadata` is opaque |

**`Receipt` as frozen by `PROTOCOL_LOCK_v0.3` is authoritative.**
`ComputeReceipt` is **superseded** and must not be implemented. Its
`compute_time_ms` and `hardware_id` are executor self-declarations that
consensus cannot check, and `proof_blob` presupposes a verification strategy
that has not been chosen; anything of that shape belongs in the receipt's
opaque `metadata`, or in the later verification RFC that actually defines it.

`COMPUTE_INTERFACE_v0.1` is not rewritten by this RFC. It remains as historical
design evidence, and §11 records the disposition of each of its concepts.

Its §7 versioning plan is also now historical: it predicted v0.3 would carry
compute types and activate the reserved RPC. v0.3 shipped receipt anchoring
instead ([RFC 0002](0002-receipt-anchoring-v0.3.md)).

---

## 2. Design

### 2.1 The canonical task envelope

```rust
struct ComputeTask {
    /// Protocol version of this envelope. Must be 1.
    version: u8,
    /// The account committing the task. Must equal the carrying
    /// transaction's sender.
    submitter: Address,
    /// Client-chosen opaque uniqueness value.
    salt: [u8; 32],
    /// Commitment to the input data. The data itself is off-chain.
    input_commitment: [u8; 32],
    /// Opaque, bounded description of what was requested. The chain never
    /// interprets it.
    execution_spec: Vec<u8>,
}
```

Five fields, in this canonical order. `task_id` is **not** a field — it is
derived (§2.2). Adding, removing or reordering fields is a protocol change.

Every field justified:

- **`version`** — mirrors `Receipt.version`. Lets a future envelope change
  fail closed rather than be misread.
- **`submitter`** — makes task identity per-client (§2.5). Consensus requires
  it to equal `tx.sender`, so it carries no independent authority; it is in the
  envelope so that identical work requested by two clients yields two tasks.
- **`salt`** — lets a client deliberately repeat the same computation (§2.6).
  Deliberately **not** the transaction nonce: coupling task identity to replay
  protection would change the `task_id` whenever a transaction is resubmitted
  after a nonce race.
- **`input_commitment`** — the entire point. Without it the receipt binds to
  nothing.
- **`execution_spec`** — without it the task says what input, but not what to
  do with it. Opaque bytes rather than an enum of task kinds: an
  `AIInference | ZKProof | Rendering | Generic` enum bakes today's product
  guesses into consensus, and the chain cannot act on the distinction anyway.

Deliberately absent: `deadline` / expiry (§2.9), `max_fee` and every other
economic field (§6), `task_type`, `model_id`.

### 2.2 Task identity

```
task_id = BLAKE3( DOMAIN_TASK || SCALE(ComputeTask) )

DOMAIN_TASK = b"mbongo:compute-task:v1"   (22 bytes, ASCII, no terminator)
```

`SCALE(ComputeTask)` is the five fields above in canonical order:
`version` as one byte, `submitter` as 32 transparent bytes, `salt` as 32
transparent bytes, `input_commitment` as 32 transparent bytes, then
`execution_spec` as a SCALE compact length prefix followed by its raw bytes.

**No circularity.** `task_id` is not a field of the envelope, so the preimage
never contains it.

The hash is over raw bytes, never over a hexadecimal rendering of them.

### 2.3 Domain separation, and an honest asymmetry

Nothing in the chain currently uses domain separators. `receipt_hash` is
`BLAKE3(SCALE(receipt fields 1–6))`, the transaction hash is
`BLAKE3(SCALE(transaction))`, and `compute_transactions_root` distinguishes
its inputs by length-prefixing rather than by tagging.

This RFC introduces `DOMAIN_TASK` anyway, for one reason: `task_id`,
`input_commitment` and `receipt_hash` are all 32-byte BLAKE3 outputs that this
design compares and stores side by side. Resting their distinctness on "the
preimages happen to have different shapes" is weaker than making it
structural, and the tag costs 22 bytes of preimage.

**The asymmetry with `receipt_hash` is deliberate and permanent.** Receipt
hashing is frozen by `PROTOCOL_LOCK_v0.3`, is already pinned by
cross-language vectors, and adding a tag to it would invalidate every anchored
receipt. It stays as it is. New hash domains defined from here on carry tags;
the frozen ones do not.

### 2.4 Commitment conventions

Consensus checks **equality** between a receipt's `input_commitment` and the
committed task's. It does not, and cannot, check how either was derived.

So the following are **non-normative interoperability conventions**, not
consensus rules. Implementations should follow them so that independent
clients agree on what a commitment means:

```
input_commitment  = BLAKE3( DOMAIN_INPUT  || input_bytes )
output_commitment = BLAKE3( DOMAIN_OUTPUT || output_bytes )

DOMAIN_INPUT  = b"mbongo:compute-input:v1"
DOMAIN_OUTPUT = b"mbongo:compute-output:v1"
```

`output_commitment` is opaque to consensus in both v0.3 and this RFC. The
chain has never seen the output and never will. Stating the convention lets
two parties disagree about a result in a way that is checkable *between them*;
it gives consensus nothing, and this RFC does not pretend otherwise.

### 2.5 Submitter authentication

A `ComputeTask` is carried by an ordinary signed `Transaction`. The
transaction signature already authenticates the submission, so **the envelope
carries no second signature**.

Consensus requires `task.submitter == tx.sender`, exactly mirroring the
`sender == receipt.executor` rule (g) that v0.3 established for anchoring. One
key, one signature, one authority.

This is a deliberate refusal to add a third Ed25519 signature domain. The
chain already has two — the receipt's over a hash, the transaction's over raw
bytes — and confusing them was the single most expensive mistake of the v0.3
SDK work. A third would be worse.

### 2.6 Duplicate and repeated tasks

`task_id` is content-derived and includes `submitter` and `salt`. Therefore:

- Two clients requesting identical work get **different** task ids. Neither
  can occupy the other's identity.
- One client repeating identical work with a **different `salt`** gets a
  different task id, so legitimate repetition is possible.
- One client resubmitting the **same** envelope gets the same `task_id`. The
  second registration is **rejected**: first-registered-wins, mirroring
  first-anchored-wins for receipts.
- Resubmitting the carrying transaction with a different transaction nonce
  leaves `task_id` unchanged, because the transaction nonce is not in the
  envelope.

### 2.7 Transaction representation

`TransactionType::ComputeTask` keeps codec index **1**. It is already frozen by
`PROTOCOL_LOCK_v0.3`, and repurposing a frozen discriminant would be worse than
leaving it in place.

A new payload variant carries the envelope:

```rust
enum TransactionPayload {
    #[codec(index = 0)] None,
    #[codec(index = 1)] AnchorReceipt(Box<Receipt>),
    #[codec(index = 2)] ComputeTask(Box<ComputeTask>),   // new
}
```

Index **2**, explicit, following the existing convention. `Box<T>` encodes as
`T`, so the payload is `0x02` followed directly by the canonical task bytes —
the same shape `AnchorReceipt` already has.

The transaction signing payload rule is unchanged:
`SCALE(tx_type, sender, receiver, amount, nonce, payload)`, signed raw, no
prehash.

### 2.8 Field constraints on the carrying transaction

A `ComputeTask` transaction is not a transfer:

- `tx_type` must be `ComputeTask`
- `payload` must be `ComputeTask(task)`
- `receiver` must be the zero address
- `amount` must be `0`
- `task.submitter` must equal `tx.sender`

`amount == 0` and the zero receiver are **consensus rules**, not conventions,
for the same reason they are for anchoring: without them the legacy transfer
behaviour survives by accident and a task submission silently moves money.

### 2.9 No expiry

The first envelope has no deadline. Nothing would enforce one: there is no
reward to reclaim, no assignment to time out, and no state that expiry would
release. Adding a `deadline: u64` now would put a field into consensus that no
rule reads.

Expiry becomes meaningful when assignment or payment exists, and belongs to
whichever RFC introduces those.

### 2.10 Bounds

```
MAX_EXECUTION_SPEC_BYTES = 1024
```

`execution_spec` is the only variable-length field, so this bounds the whole
envelope: a maximal task encodes to `1 + 32 + 32 + 32 + 2 + 1024 = 1123` bytes.

**Not 4096.** The receipt's metadata bound was sized for an application-layer
commitment pointer. An execution specification is a short identifier or
parameter blob; anything larger belongs off-chain behind `input_commitment`,
which is the same argument that produced the receipt's cap. Choosing 1 KiB
deliberately rather than copying 4 KiB keeps the two bounds independently
justified.

Every task is committed to permanently, by every node, and is never pruned.
The bound is what stops task submission from being a cheap way to write
arbitrary data into every full node's storage.

---

## 3. Consensus rules

Lettering continues from RFC 0002, which used (a)–(j).

For a `ComputeTask` transaction, in this order:

- **(k) Type/form.** `payload` is `ComputeTask(task)` when `tx_type` is
  `ComputeTask`, and no other type carries that payload.
- **(l) Field constraints.** `amount == 0`, `receiver == 0`.
- **(m) Envelope version.** `task.version == 1`.
- **(n) Bound.** `task.execution_spec.len() <= MAX_EXECUTION_SPEC_BYTES`.
- **(o) Submitter identity.** `task.submitter == tx.sender`.
- **(p) Uniqueness.** No task with this `task_id` exists in prior chain state
  or earlier in the same block.

For an `AnchorReceipt` transaction, rules (a)–(j) are unchanged, plus:

- **(q) Task existence.** A task with `receipt.task_id` exists in prior chain
  state or earlier in the same block.
- **(r) Input binding.** `receipt.input_commitment` equals that task's
  `input_commitment`.

Rule (r) is the whole point of this RFC. Everything else exists to make it
mean something.

Note what (q) and (r) deliberately do **not** say: nothing constrains *who*
may answer a task. Any executor may anchor a receipt for any registered task,
and first-anchored-wins decides. See §9 for the consequence.

---

## 4. Storage

A new column family, mirroring `receipts` exactly:

| | |
|---|---|
| name | `tasks` |
| key | the raw 32-byte `task_id` |
| value | canonical SCALE `ComputeTask` bytes, opaque to the storage layer |
| writes | batch-only, through `BatchOp::PutTask`, inside the same atomic `write_batch` as block state |
| derivation | fully reconstructable by replay from genesis |

Schema version goes from **2 to 3**. As with the v2 migration, downgrade is
not supported.

The storage layer never decodes, validates or inspects a task. All validation
lives above it, exactly as RFC 0002 §6.1 established for receipts.

### 4.1 State model

Two states, both **derived**, neither stored:

- **submitted** — a task with this `task_id` is in the `tasks` column family.
- **completed** — a receipt with this `task_id` is in the `receipts` column
  family.

No status field is written. Storing a status that is derivable from two
existing indexes creates a second source of truth that can drift.

`COMPUTE_INTERFACE_v0.1`'s seven states (`Pending`, `Assigned`, `Executing`,
`Completed`, `Failed`, `Verified`, `Slashed`) presuppose assignment,
verification and slashing. None exist. `Assigned` and `Executing` are
off-chain facts the chain cannot observe; `Verified` and `Slashed` are the
verification and economic layers this RFC excludes.

---

## 5. Compatibility and activation

### 5.1 The legacy `ComputeTask` fall-through

Today `(TransactionType::ComputeTask, TransactionPayload::None)` is accepted
and **executes as a plain transfer** — `PROTOCOL_LOCK_v0.3` records it as
"still legacy fall-through; unvalidated types."

After activation that combination is **rejected** by rule (k).

### 5.2 Unbound receipts

Today an `AnchorReceipt` needs no task. After activation, rule (q) requires
one. **This is a breaking change**, and it is the intended one: an unbound
receipt is exactly the unattributed claim this RFC exists to eliminate.

It breaks the anchoring flow shipped in the v0.1 SDK, which submits a receipt
with no prior task. That flow gains a step rather than disappearing.

### 5.3 A clean version boundary

Both changes above are consensus changes with no height gating. Blocks
validated under v0.4 rules are validated under v0.4 rules throughout.

This is viable because the same precedent already applies: the v0.3 schema
migration states downgrade is unsupported and "rollback requires wiping the
data directory." No mainnet exists, and devnet state is disposable. Activation
is a new protocol version, a new genesis, and a fresh data directory.

Designing height-gated dual-rule validation for a devnet with no persistent
history would add permanent consensus complexity to solve a problem nobody
has.

---

## 6. What this RFC refuses to add

**Economics.** No worker payment, no reward, no staking, no slashing, no
compute fee market. `max_fee` from `COMPUTE_INTERFACE_v0.1` is deliberately
absent: a fee field with no fee rule is a field consensus does not read.

**Verification.** The chain checks that a receipt corresponds to a committed
task. It does not check that the output is right, and no field here helps it
try. That is [#52](https://github.com/MbongoChain/mbongo-chain/issues/52).

**Assignment and discovery.** No scheduler, no matching, no reservation. Tasks
are visible in blocks; how an executor learns of one is off-protocol.

**The worker.** A reference executor will exist to demonstrate the loop. It is
an external process with no consensus role, and nothing in this RFC constrains
its behaviour beyond the commitments it must produce.

---

## 7. RPC

**This RFC activates no RPC method.** All five reserved names stay reserved
and keep returning `-32601`.

| Method | Disposition |
|---|---|
| `submit_compute_task` | **KEEP_RESERVED** |
| `get_compute_task` | KEEP_RESERVED |
| `get_compute_receipt` | REDESIGN_LATER — its reserved shape returns the superseded `ComputeReceipt` |
| `list_compute_tasks` | KEEP_RESERVED |
| `get_compute_node_status` | KEEP_RESERVED |

`submit_compute_task` deserves its own justification, because activating it
looks obvious and is wrong. Clients sign their own transactions; the node holds
no client keys. A `submit_compute_task` that accepted an unsigned task would
require the node to sign on the client's behalf, creating a second signing
authority inside the node. One that accepted a signed transaction would be
`submit_transaction` with a narrower type.

A `ComputeTask` transaction is an ordinary transaction. `submit_transaction`
already carries it, and `get_block_by_height` already returns it. Reserving a
name is not a reason to implement it.

---

## 8. Security invariants

1. `task_id` is deterministic under the canonical encoding, and its preimage
   is domain-separated and unambiguous.
2. The submitter is authenticated by the transaction signature; the envelope
   cannot claim a submitter other than `tx.sender`.
3. A registered task is immutable — the `task_id` commits to every field, so
   mutation produces a different task.
4. A receipt cannot claim an input the submitter did not commit to (rule r).
5. After activation, a receipt cannot complete a task that does not exist
   (rule q).
6. At most one task per `task_id` and at most one anchored receipt per
   `task_id`.
7. Executor identity remains authenticated by rule (g).
8. Task payloads are bounded (§2.10).
9. **No correctness claim is made or implied.**

---

## 9. Threat model

Addressed:

| Threat | Disposition |
|---|---|
| task spoofing | rule (o) — the envelope's submitter must be the signer |
| task mutation | `task_id` commits to every field |
| input substitution | rule (r) |
| task replay | rule (p), first-registered-wins |
| duplicate registration | rule (p) |
| receipt for a nonexistent task | rule (q) |
| receipt/task mismatch | rule (r) |
| duplicate receipt | rule (i)/(j), unchanged |
| malformed task | rules (k)–(n) |
| resource exhaustion | §2.10 |

Explicitly deferred: a malicious-but-valid wrong computation, colluding
executors, economic attacks, and every advanced verification strategy.

### 9.1 Task squatting — an accepted limitation

Rules (q) and (r) do not constrain **who** may answer a task, and
first-anchored-wins is global. A third party who sees a registered task can
therefore compute any output, anchor a receipt for that `task_id` first, and
permanently occupy it. The legitimate executor's receipt is then rejected as a
duplicate.

The squatter gains nothing — no payment exists, and their receipt carries their
own executor identity, so the submitter can see it is not the answer they
wanted. But the task is consumed and the submitter must re-register with a new
`salt`.

This is **not solved here**, and pretending otherwise would be worse than
naming it. Solving it means constraining who may answer, which is assignment —
the marketplace question this RFC exists to avoid. It is recorded as the
principal known limitation of the first vertical and belongs to whichever RFC
introduces assignment.

---

## 10. End-to-end sequence

Normative for the first vertical:

1. Client canonicalises the task envelope.
2. Client computes `task_id` per §2.2.
3. Client signs and submits a `ComputeTask` transaction.
4. Chain validates (k)–(p) and stores the task atomically with the block.
5. Executor obtains the task and, off-protocol, the input data.
6. Executor runs the computation off-chain.
7. Executor builds a `Receipt` with the task's `task_id` and
   `input_commitment`, and its own `output_commitment`.
8. Executor signs the receipt over the raw 32-byte `receipt_hash`.
9. Executor builds and signs an `AnchorReceipt` transaction.
10. Chain validates (a)–(j) plus (q) and (r).
11. Chain stores the receipt atomically with the block.
12. Client reads the receipt back from the height it recorded.

Step 5 is the only step with no protocol content, and that is the design.

---

## 11. `COMPUTE_INTERFACE_v0.1` disposition

| Concept | Disposition |
|---|---|
| `ComputeTask` | **REDEFINED** — §2.1 replaces it. `task_type`, `model_id`, `max_fee` and `deadline` are dropped with reasons above; `task_id` derivation is retained in spirit and specified exactly |
| `ComputeReceipt` | **SUPERSEDED** by the implemented `Receipt` (§1). Must not be implemented |
| `ComputeStatus` | **SUPERSEDED** by the two derived states of §4.1 |
| §3 RPC reservations | **RETAINED as reservations**; none activated (§7) |
| §4 event model | **DEFERRED** — no events in this RFC |
| §5 economic placeholders | **DEFERRED** (§6) |
| §7 versioning plan | **HISTORICAL** — it predicted compute in v0.3; v0.3 shipped receipt anchoring |

---

## 12. Test plan

Following the precedent set by
[`test-vectors/receipt/receipt-v1.json`](../../test-vectors/receipt/receipt-v1.json)
and
[`test-vectors/transaction/anchor-receipt-v1.json`](../../test-vectors/transaction/anchor-receipt-v1.json),
implementation must ship a neutral cross-language fixture pinning:

- the canonical `ComputeTask` SCALE encoding
- `task_id`, including the domain tag in the preimage
- the `ComputeTask` transaction signing payload, signature, full encoding and
  transaction hash
- the `AnchorReceipt` binding: a receipt whose `input_commitment` matches its
  task, and one whose does not
- boundary vectors: empty `execution_spec`, the 1024-byte maximum, and 1025
  rejected

**Anti-circularity is mandatory**, as in #83 and #94: expected values are
derived from the protocol rules, not by encoding with production Rust. Both
Rust and TypeScript are consumers that must agree with values neither produced.

Vectors are not generated in this RFC.

---

## 13. Protocol version

Per [RFC_PROCESS.md](../RFC_PROCESS.md) and the v0.2→v0.3 precedent, a change
to locked surfaces requires a protocol version bump and a new lock document.

This RFC **proposes** v0.3 → **v0.4**, with a new `PROTOCOL_LOCK_v0.4.md`
naming this RFC as its authority, superseding `PROTOCOL_LOCK_v0.3.md`.

The proposal is not the activation. No lock document is created or amended by
this RFC while its status is Draft.

---

## 14. Decisions

| Question | Decision |
|---|---|
| Canonical task representation | five-field envelope, §2.1 |
| `task_id` derivation | `BLAKE3(DOMAIN_TASK ‖ SCALE(envelope))`, envelope excludes `task_id` |
| Input commitment | consensus checks **equality** with the task's; derivation is convention |
| Output commitment | unchanged, opaque to consensus |
| Submitter authentication | transaction signature only; no second envelope signature |
| `amount` / `receiver` | must be `0` and the zero address, as consensus rules |
| Legacy `ComputeTask` + `None` | rejected after activation |
| Task storage | `tasks` column family, `task_id` → canonical bytes, schema 3 |
| Receipt binding | rules (q) and (r) |
| Backward compatibility | clean version boundary; no height gating |
| RPC activation | **none** |
| Worker | external, no consensus role |
| Verification | out of scope; no correctness claim |
| Economics | out of scope; no reserved fields |

---

## 15. Unresolved design questions

Recorded rather than hidden. None blocks review; each needs a decision before
acceptance.

1. **Task squatting** (§9.1). Accepted as a limitation of the first vertical.
   Confirm that is acceptable, or accept that assignment must land in the same
   protocol version.
2. **`MAX_EXECUTION_SPEC_BYTES = 1024`** is a judgement, not a derivation. It
   bounds the envelope at 1123 bytes. If a real execution specification format
   is chosen later and does not fit, the bound is a protocol change.
3. **`execution_spec` as opaque bytes** gives consensus no way to reject a
   nonsensical specification, and no two clients need agree on its meaning. A
   versioned specification hash would be stricter and less flexible. This RFC
   chooses flexibility because no specification format exists yet to be strict
   about.
4. **Whether `submitter` belongs in the envelope at all.** Consensus requires
   it to equal `tx.sender`, so it is redundant *within a transaction* — its
   only load-bearing role is making `task_id` per-client (§2.6). An alternative
   is dropping the field and folding `tx.sender` into the `task_id` preimage,
   which is smaller but makes the envelope non-self-describing.

---

## References

- [RFC 0002 — Receipt Anchoring](0002-receipt-anchoring-v0.3.md)
- [`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) — FROZEN
- [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md)
- [`COMPUTE_INTERFACE_v0.1.md`](../specs/COMPUTE_INTERFACE_v0.1.md) — spec only
- [`VISION_v1.md`](../VISION_v1.md)
- [`RFC_PROCESS.md`](../RFC_PROCESS.md)
- [Compute receipts architecture](../architecture/compute-receipts.md)
