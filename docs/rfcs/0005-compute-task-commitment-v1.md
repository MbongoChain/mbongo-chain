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
    /// The one executor authorised to answer this task.
    executor: Address,
    /// Client-chosen opaque uniqueness value.
    salt: [u8; 32],
    /// Commitment to the input data. The data itself is off-chain.
    input_commitment: [u8; 32],
    /// Opaque, bounded description of what was requested. The chain never
    /// interprets it.
    execution_spec: Vec<u8>,
}
```

Six fields, in this canonical order. `task_id` is **not** a field — it is
derived (§2.2). Adding, removing or reordering fields is a protocol change.

Every field justified:

- **`version`** — mirrors `Receipt.version`. Lets a future envelope change
  fail closed rather than be misread.
- **`submitter`** — makes task identity per-client (§2.6), and makes the
  stored task self-describing (§2.11). Consensus requires it to equal
  `tx.sender`, so it carries no independent authority.
- **`executor`** — the client names who may answer. This is the authorisation
  model of §2.5, and it is what makes task squatting impossible (§9.1).
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
  See §2.12 for what "opaque" does and does not promise.

Deliberately absent: `deadline` / expiry (§2.9), `max_fee` and every other
economic field (§6), `task_type`, `model_id`.

### 2.2 Task identity

```
task_id = BLAKE3( DOMAIN_TASK || SCALE(ComputeTask) )

DOMAIN_TASK = b"mbongo:compute-task:v1"   (22 bytes, ASCII, no terminator)
```

`SCALE(ComputeTask)` is the six fields above in canonical order: `version` as
one byte, then `submitter`, `executor`, `salt` and `input_commitment` as 32
transparent bytes each, then `execution_spec` as a SCALE compact length prefix
followed by its raw bytes.

`DOMAIN_TASK` is the literal ASCII bytes
`6d 62 6f 6e 67 6f 3a 63 6f 6d 70 75 74 65 2d 74 61 73 6b 3a 76 31` — 22
bytes, prepended raw. **No NUL terminator and no SCALE string encoding:** the
tag is concatenated as bytes, not encoded as a `Vec<u8>` with a length prefix.

**No circularity.** `task_id` is not a field of the envelope, so the preimage
never contains it.

The hash is over raw bytes, never over a hexadecimal rendering of them.

Note what `task_id` therefore commits to: the submitter, the authorised
executor, the salt, the input commitment **and the execution specification**.
Changing what was asked changes the task identity even when the input bytes
are identical.

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

### 2.5 Authority: who submits, and who may answer

**Submission.** A `ComputeTask` is carried by an ordinary signed
`Transaction`. The transaction signature already authenticates the submission,
so **the envelope carries no second signature**. Consensus requires
`task.submitter == tx.sender`, mirroring the `sender == receipt.executor` rule
(g) that v0.3 established for anchoring.

This is a deliberate refusal to add a third Ed25519 signature domain. The
chain already has two — the receipt's over a hash, the transaction's over raw
bytes — and confusing them was the single most expensive mistake of the v0.3
SDK work. A third would be worse.

**Answering.** The task names exactly one authorised executor, and consensus
requires `receipt.executor == task.executor` (rule s). A receipt from anyone
else is rejected however well-formed it is.

Three models were considered:

| | Model | Verdict |
|---|---|---|
| A | permissionless — any executor may answer, first anchored wins | rejected |
| B | one authorised executor named in the task | **chosen** |
| C | a general authorisation policy commitment | deferred |

**A is rejected because it is not actually permissionless — it is
unguarded.** There is no discovery mechanism, no assignment, and no reward, so
nothing draws an honest stranger to a task. What it does allow is a third
party to consume a task by anchoring a worthless receipt first (§9.1). A
competition model with no competitors and an open denial-of-service is not a
model.

**B is chosen** because it matches how the first vertical actually works.
Input data reaches the executor off-protocol, which means the client already
knows who they asked. Naming that executor commits a fact the client already
holds, and costs one 32-byte field.

Naming an executor is **not** a marketplace. It authenticates "this client
asked *this* executor to perform *this* task." It defines no discovery, no
bidding, no pricing and no selection algorithm, and the chain performs no
matching.

**C is deferred** to whichever RFC introduces assignment. Widening
authorisation later — a policy commitment, or a sentinel meaning "anyone" — is
a change confined to `ComputeTask` semantics and leaves `Receipt` v1 untouched
(§2.13). Narrowing later would not be safe, so starting narrow is the
reversible direction.

### 2.6 Duplicate and repeated tasks

`task_id` is content-derived over all six fields. The full matrix:

| Varying | `task_id` | Outcome |
|---|---|---|
| nothing — identical envelope | same | second registration **rejected**, first-registered-wins |
| `salt` | different | allowed — this is how a client repeats the same computation |
| `submitter` | different | allowed — two clients cannot collide, whatever their salts |
| `executor` | different | allowed — a client may ask two executors the same work |
| `input_commitment` or `execution_spec` | different | a different task, as it should be |
| only the carrying transaction's nonce | **unchanged** | the tx nonce is not in the envelope, so a resubmission after a nonce race keeps its identity |

Replaying a receipt for an already-completed task is rejected by
first-anchored-wins, unchanged from v0.3. Replaying it under a *different*
executor is additionally rejected by rule (s).

`salt` is `[u8; 32]`, client-chosen and opaque. It need not be random — a
client may derive it from an internal job identifier — and **a zero salt is
legal**. Its only job is to let the same submitter ask the same executor for
the same work twice, deliberately.

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
envelope. At the maximum, with the two-byte SCALE compact prefix that 1024
requires:

```
version           1
submitter        32
executor         32
salt             32
input_commitment 32
compact(1024)     2
execution_spec 1024
                ----
canonical task  1155 bytes

DOMAIN_TASK      22
task_id preimage 1177 bytes
```

**This is a protocol safety bound, not an optimum.** No calculation produces
1024; it is a judgement, and the RFC says so rather than dressing it up.

**Not 4096.** The receipt's metadata bound was sized for an application-layer
commitment pointer. An execution specification is a short identifier or
parameter blob; anything larger belongs off-chain behind `input_commitment`,
which is the same argument that produced the receipt's cap. Choosing 1 KiB
deliberately rather than copying 4 KiB keeps the two bounds independently
justified.

Every task is committed to permanently, by every node, and is never pruned.
The bound is what stops task submission from being a cheap way to write
arbitrary data into every full node's storage.

### 2.11 The stored task is self-describing

Storage holds `task_id → canonical ComputeTask` and nothing else — no back
reference to the registering transaction. Keeping `submitter` in the envelope
means the stored task fully answers who asked, who may answer, what input was
committed and what was requested, without retrieving the block that carried
it.

That is why `submitter` stays in the envelope even though consensus requires
it to equal `tx.sender` and it is therefore redundant *within a transaction*.
The alternative — dropping the field and folding `tx.sender` into the
`task_id` preimage — is 32 bytes smaller and makes the task meaningless on its
own.

### 2.12 What "opaque" promises, and what it does not

The chain never interprets `execution_spec`. It commits to it, stores it, and
includes it in `task_id`; it forms no opinion about its contents.

This means the protocol **cannot** guarantee that two parties read a
specification the same way. Applications should therefore version their own
specification format inside `execution_spec` — a leading version byte or a
self-describing encoding — so that a reader can tell which convention applies.
That is a convention for interoperability, not a consensus rule, and the chain
enforces none of it.

The ambiguity has no consensus consequence under §2.5: exactly one executor is
authorised, so there is no second party whose differing interpretation could
produce a competing receipt.

An enum of task kinds would not have helped. It fixes today's product guesses
in consensus and the chain still could not act on the distinction.

### 2.13 Room to widen later

`Receipt` v1 is untouched by this RFC and should stay untouched by the RFCs
that follow it. Everything a future assignment or marketplace design needs to
change lives in `ComputeTask`: authorisation may widen from one named executor
to a policy commitment or an explicit "anyone" sentinel, and expiry, payment
or bidding fields may appear, all without altering the receipt, its hash, its
signature domain or its anchoring rules.

The one rule such an RFC must revisit is (s). That is the intended seam.

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
- **(s) Executor authorisation.** `receipt.executor` equals that task's
  `executor`.

Rules (r) and (s) are the point of this RFC. Everything else exists to make
them mean something: (r) ties the answer to the committed question, (s) ties
it to the party the client asked.

Rule (g) — `tx.sender == receipt.executor` — is unchanged and does different
work. It authenticates *whoever is anchoring*. Rule (s) authenticates that
they were *authorised to answer*. Neither implies the other: before this RFC a
receipt was authenticated but unauthorised.

"Task already completed" needs no rule of its own. First-anchored-wins on
`task_id` — rules (i) and (j), unchanged from RFC 0002 — already rejects a
second receipt for the same task.

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

## 5. Compatibility

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

## 6. Non-Goals

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

## 8. Security

1. `task_id` is deterministic under the canonical encoding, and its preimage
   is domain-separated and unambiguous.
2. The submitter is authenticated by the transaction signature; the envelope
   cannot claim a submitter other than `tx.sender`.
3. A registered task is immutable — the `task_id` commits to every field, so
   mutation produces a different task.
4. A receipt cannot claim an input the submitter did not commit to (rule r).
5. After activation, a receipt cannot complete a task that does not exist
   (rule q).
6. **Only the executor the submitter named can complete a task (rule s).**
7. At most one task per `task_id` and at most one anchored receipt per
   `task_id`.
8. Executor identity remains authenticated by rule (g), which is distinct from
   authorisation by rule (s).
9. Task payloads are bounded (§2.10).
10. **No correctness claim is made or implied** (§9.2).

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
| **task squatting / unauthorised completion** | **rule (s)** — §9.1 |
| duplicate receipt | rule (i)/(j), unchanged |
| malformed task | rules (k)–(n) |
| resource exhaustion | §2.10 |

Explicitly deferred: a malicious-but-valid wrong computation, colluding
executors, economic attacks, and every advanced verification strategy.

### 9.1 Task squatting — resolved by rule (s)

The first draft of this RFC left tasks answerable by anyone, and that was a
denial-of-service hole rather than a competition model.

The attack: a submitter registers task T; `task_id` and `input_commitment`
become public in a block; before the intended executor anchors, an attacker
builds a receipt with T's `task_id`, T's `input_commitment`, an arbitrary
`output_commitment` and their own executor key, signs it, and anchors it.
Every step satisfied the draft's rules. First-anchored-wins then consumed T,
and the legitimate executor's receipt was rejected as a duplicate.

Notably the attacker never had to compute anything: `output_commitment` is
opaque to consensus, so any 32 bytes would do. The attack cost one transaction
and denied the submitter their task permanently.

**Rule (s) eliminates it.** Only the executor the submitter named can produce
an acceptable receipt for that task; an unauthorised receipt fails validation
and never consumes the task. The submitter is no longer exposed to a stranger
burning their `task_id`.

What remains is not squatting: the **authorised** executor can still anchor a
wrong `output_commitment`, because nothing here verifies computation (§9.2).
That is a correctness question, not an availability one.

### 9.2 What an authorised executor can still do

Rule (s) fixes *who* may answer. It says nothing about *what* they answer
with. The named executor can sign a receipt committing to any output at all,
and consensus will accept it: the chain has never seen the output and cannot
check it.

So the properties this RFC delivers are, precisely:

- **task correspondence** — the receipt answers a committed task
- **input correspondence** — over the input the submitter committed to
- **executor authorisation** — by the party the submitter named
- **executor attribution** — signed by that party's key

and the property it does **not** deliver, in any form:

- **output correctness**

Saying "verified receipt" of anything this RFC produces would be false.

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
10. Chain validates (a)–(j) plus (q), (r) and (s).
11. Chain stores the receipt atomically with the block.
12. Client reads the receipt back from the height it recorded.

Step 5 is the only step with no protocol content, and that is the design. The
client already knows which executor they asked — that is how the input reached
them — which is precisely what step 3 commits and step 10 enforces.

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

## 12. Testing

Following the precedent set by
[`test-vectors/receipt/receipt-v1.json`](../../test-vectors/receipt/receipt-v1.json)
and
[`test-vectors/transaction/anchor-receipt-v1.json`](../../test-vectors/transaction/anchor-receipt-v1.json),
implementation must ship a neutral cross-language fixture pinning:

- the canonical `ComputeTask` SCALE encoding
- `task_id`, including the domain tag in the preimage
- the `ComputeTask` transaction signing payload, signature, full encoding and
  transaction hash
- the `AnchorReceipt` binding, one vector per outcome: a receipt whose
  `input_commitment` matches its task and whose executor is the authorised
  one; a receipt whose `input_commitment` does not match (rule r); and a
  receipt from an executor the task did not name (rule s)
- boundary vectors: empty `execution_spec`, the 1024-byte maximum with its
  two-byte compact prefix, and 1025 rejected
- the domain tag as literal bytes, so a fixture catches an implementation that
  SCALE-encodes it or appends a NUL

**Anti-circularity is mandatory**, as in #83 and #94: expected values are
derived from the protocol rules, not by encoding with production Rust. Both
Rust and TypeScript are consumers that must agree with values neither produced.

Vectors are not generated in this RFC.

---

## 13. Rollout

Per [RFC_PROCESS.md](../RFC_PROCESS.md) and the v0.2→v0.3 precedent, a change
to locked surfaces requires a protocol version bump and a new lock document.

This RFC **proposes** v0.3 → **v0.4**, with a new `PROTOCOL_LOCK_v0.4.md`
naming this RFC as its authority, superseding `PROTOCOL_LOCK_v0.3.md`.

The proposal is not the activation. No lock document is created or amended by
this RFC while its status is Draft.

### 13.1 Deployment sequence

Restating decisions already made above, in the order they apply:

1. **Acceptance precedes implementation.** No code lands while this RFC is in
   Draft or Review.
2. **Implement against v0.4 rules**: the new `TransactionPayload` variant and
   its codec index, rules (k)-(p) for tasks, rules (q)-(s) for anchoring, and
   the `tasks` column family at storage schema 3 (�2.7, �3, �4).
3. **Clean version boundary.** There is no height gating: blocks are validated
   under v0.4 rules throughout (�5.3).
4. **Fresh state.** Schema 3 is not downgradable, and activation requires a new
   genesis and a wiped data directory  the same coordination the v0.3
   migration already required (�5.3).
5. **Release.** A new `PROTOCOL_LOCK_v0.4.md` naming this RFC supersedes
   `PROTOCOL_LOCK_v0.3.md`, and the release is tagged. Per
   [RFC_PROCESS.md](../RFC_PROCESS.md) the lock is created at **Released**, not
   at Accepted.

**Rollback** is wiping the data directory and running a v0.3 binary against a
fresh genesis. No in-place downgrade exists, and none is proposed.

---

## 14. Decisions

| Question | Decision |
|---|---|
| Canonical task representation | six-field envelope, §2.1 |
| `task_id` derivation | `BLAKE3(DOMAIN_TASK ‖ SCALE(envelope))`, envelope excludes `task_id` |
| **Execution authorisation** | **Model B — the task names one authorised executor (§2.5)** |
| Submitter field | retained in the envelope; makes the stored task self-describing and identity per-client (§2.11) |
| `salt` | `[u8; 32]`, client-chosen, opaque, zero permitted (§2.6) |
| `execution_spec` | opaque bounded bytes, application-versioned by convention (§2.12) |
| `MAX_EXECUTION_SPEC_BYTES` | 1024 — a safety bound, not an optimum (§2.10) |
| Maximal canonical task / preimage | 1155 / 1177 bytes (§2.10) |
| Input commitment | consensus checks **equality** with the task's; derivation is convention |
| Output commitment | unchanged, opaque to consensus |
| Submitter authentication | transaction signature only; no second envelope signature |
| `amount` / `receiver` | must be `0` and the zero address, as consensus rules |
| Legacy `ComputeTask` + `None` | rejected after activation |
| Task storage | `tasks` column family, `task_id` → canonical bytes, schema 3 |
| Receipt binding | rules (q), (r) and (s) |
| Backward compatibility | clean version boundary; no height gating |
| RPC activation | **none** |
| Worker | external, no consensus role |
| Verification | out of scope; no correctness claim |
| Economics | out of scope; no reserved fields |

---

## 15. Design questions, resolved

The first draft carried four open questions. All four are now closed
normatively; none is left for the implementer to decide.

1. **Task squatting → RESOLVED by rule (s)** (§2.5, §9.1). Permissionless
   answering was rejected: with no discovery, assignment or reward, it offered
   no competition and a cheap denial of service. The task names one authorised
   executor.
2. **`MAX_EXECUTION_SPEC_BYTES` → RESOLVED at 1024**, with the maximal
   canonical task at 1155 bytes and the `task_id` preimage at 1177 (§2.10).
   The RFC states plainly that this is a safety bound and not an optimum. If a
   real specification format later does not fit, raising it is a protocol
   change — which is the correct cost.
3. **`execution_spec` → RESOLVED as opaque, application-versioned bytes**
   (§2.12). Consensus commits to it and never interprets it; applications
   version their own format. Under rule (s) the interpretive ambiguity has no
   consensus consequence, because only one party may answer.
4. **`submitter` → RESOLVED: retained in the envelope** (§2.11). It is
   redundant for authentication and load-bearing for two other reasons — the
   stored task is self-describing without its registering transaction, and
   task identity is separated per client structurally rather than by relying
   on salt hygiene.

### Remaining non-blocking questions

None that change consensus semantics. Two matters are deliberately left to
later RFCs and are named here so they are not mistaken for oversights:
widening authorisation beyond a single executor (§2.13), and verifying output
correctness (§9.2). Neither is required by the first implementation.

---

## References

- [RFC 0002 — Receipt Anchoring](0002-receipt-anchoring-v0.3.md)
- [`PROTOCOL_LOCK_v0.3.md`](../specs/PROTOCOL_LOCK_v0.3.md) — FROZEN
- [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md)
- [`COMPUTE_INTERFACE_v0.1.md`](../specs/COMPUTE_INTERFACE_v0.1.md) — spec only
- [`VISION_v1.md`](../VISION_v1.md)
- [`RFC_PROCESS.md`](../RFC_PROCESS.md)
- [Compute receipts architecture](../architecture/compute-receipts.md)
