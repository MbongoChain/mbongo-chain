# Receipt Anchoring v0.3 — Implementation Truth Audit

**Audit date:** 2026-08-28
**Audit type:** IMPLEMENTATION AUDIT — DESCRIPTIVE — NON-NORMATIVE
**Audited commit:** `261dcdff54d9e5da7953912bc169eaa989970d7c` (dev)
**Audited against:** [Issue #58](https://github.com/MbongoChain/mbongo-chain/issues/58), [RECEIPT_SPEC_v0.1.md](specs/RECEIPT_SPEC_v0.1.md), [RFC 0002](rfcs/0002-receipt-anchoring-v0.3.md), [PROTOCOL_LOCK_v0.3.md](specs/PROTOCOL_LOCK_v0.3.md)
**Protocol status:** v0.3 **FROZEN** — this audit changes nothing it describes

> This document is descriptive. It records what protocol v0.3 already
> implements, compares it to what Issue #58 asks for, and classifies the
> difference. It proposes no change to receipt semantics, encoding, hashing,
> validation or storage, and it is not a design proposal. Where it identifies
> a gap or a tension, it classifies it and stops.
>
> Filed at `docs/` root following the precedent of
> [ALIGNMENT_AUDIT_2026-02.md](ALIGNMENT_AUDIT_2026-02.md), the repository's
> existing documentation-versus-implementation audit.

---

## 1. Executive summary

**Receipt anchoring is implemented, merged on `dev`, and frozen.** Issue #58
is titled *Implement Receipt Anchoring (v1 Minimal)* and lists as its first
prerequisite an "RFC defining receipt anchoring rules (filed in
`docs/rfcs/`)". That RFC exists ([RFC 0002](rfcs/0002-receipt-anchoring-v0.3.md)),
it was approved, it authorized a protocol version bump, and the
implementation it governs shipped across seven commits, all ancestors of
`origin/dev`. [PROTOCOL_LOCK_v0.3.md](specs/PROTOCOL_LOCK_v0.3.md) is
**FROZEN** and names RFC 0002 as its authorizing RFC.

Eight of Issue #58's nine acceptance criteria are satisfied by shipped code.
The ninth — "Indexed by `task_id` and `receipt_hash`" — is **not a shortfall
against the specification**. [RECEIPT_SPEC_v0.1 §6](specs/RECEIPT_SPEC_v0.1.md)
requires storing `(task_id, receipt_hash)` "**or equivalent**" and a lookup
"by `task_id`". The implementation stores `task_id → canonical receipt bytes`,
from which `receipt_hash` is recomputable, and indexes by `task_id`. That is
the "or equivalent" the spec allows. The issue's wording is tighter than the
document it cites.

Two things genuinely warrant a decision, and neither is a defect in shipped
code:

1. **`task_id` uniqueness is global and first-anchored-wins**, so v0.3 admits
   exactly one anchored receipt per `task_id`. Any future redundant-execution
   design must therefore work through a different abstraction — not because
   v0.3 is wrong, but because the constraint is now frozen.
2. **`MAX_RECEIPT_METADATA_BYTES = 4096` is normatively documented and frozen**
   — in RFC 0002 §3 and PROTOCOL_LOCK_v0.3, **but not in RECEIPT_SPEC_v0.1**.
   Since Issues #58 and #60 both cite RECEIPT_SPEC_v0.1 as *the* reference, an
   independent implementer reading only that document would not learn the
   bound.

Nothing in this audit requires a protocol change, and none is proposed.

---

## 2. Audit scope

**In scope:** what v0.3 implements; how it differs from Issue #58; the
apparent acceptance-criteria gap; two protocol-design tensions; what requires
no action; what could change without touching consensus; what would need a
future RFC and version bump.

**Out of scope, deliberately:** any change to receipt fields, field order,
SCALE encoding, BLAKE3 rule, signing payload, receipt version, transaction
representation, uniqueness key, or metadata limit. Any RPC work (Issue #59).
Any SDK work (Issue #60). Any edit to Issue #58. Any redesign of receipt
semantics — v0.3 is evidence here, not a design sandbox.

---

## 3. Implementation timeline

All seven commits verified as ancestors of `origin/dev` via
`git merge-base --is-ancestor`.

| Commit | Date | Role | Evidence |
|---|---|---|---|
| `f9a53c0` | 2026-02-21 | **Specification.** Introduces the anchoring primitive. | `docs/specs/RECEIPT_SPEC_v0.1.md` (+154) |
| `5d58cd1` | 2026-07-19 | **Validation library.** Standalone receipt validation, no chain state. | `crates/mbongo-verification/src/receipt.rs` (+376) |
| `1470a92` | 2026-07-19 | **RFC.** The complete v0.3 design, filed before any consensus change. | `docs/rfcs/0002-receipt-anchoring-v0.3.md` (+370) |
| `60324e1` | 2026-07-19 | **Phase 1 — storage.** `receipts` CF, schema v1→v2 migration, batch op. | `mbongo-storage/{lib,memory,rocksdb,storage}.rs`, `docs/architecture/storage_invariants.md` |
| `d4264ff` | 2026-07-20 | **Phase 2 — core.** Typed transaction payload; canonical receipt relocated into `mbongo-core`. | `mbongo-core/{receipt,primitives,lib}.rs`, plus 8 dependent files |
| `7924166` | 2026-07-20 | **Phase 3 — consensus activation.** `apply_block` anchoring rules, mempool admission, harness coverage. | `mbongo-node/src/backend.rs` (+1229), `mempool.rs`, both harnesses |
| `751034a` | 2026-07-20 | **Protocol freeze.** v0.3 identifiers, negotiation tests, lock document. | `docs/specs/PROTOCOL_LOCK_v0.3.md` (+230), `mbongo-network/` |

The ordering is worth noting: specification, then validation as a pure
library, then the RFC, then storage, then the data type, then consensus, then
the freeze. Consensus rules were activated only after the RFC that authorized
them, and the lock was written last.

---

## 4. Authority map

| Document | Status | Role |
|---|---|---|
| [PROTOCOL_LOCK_v0.3.md](specs/PROTOCOL_LOCK_v0.3.md) | **FROZEN**, tag `v0.3-devnet-stable` | `CURRENT_PROTOCOL_LOCK` — authority for consensus surfaces |
| [PROTOCOL_LOCK_v0.2.md](specs/PROTOCOL_LOCK_v0.2.md) | **SUPERSEDED** by v0.3 | historical |
| [RFC 0002](rfcs/0002-receipt-anchoring-v0.3.md) | Draft (status line), but **the authorizing RFC of a frozen lock** | design rationale and decision record |
| [RECEIPT_SPEC_v0.1.md](specs/RECEIPT_SPEC_v0.1.md) | **EXPERIMENTAL** until v1.0-mainnet, by its own terms | receipt structure/encoding/hash |
| Issue #58 | OPEN | requirements, with stale references |

Two qualifications matter and are easy to get wrong:

**RFC 0002 still reads `Status: Draft`** while the lock it authorizes reads
`FROZEN`. The lock is the operative document for consensus surfaces; the RFC
status line has not been updated to reflect that its design shipped. Recorded
as an observation, not a defect to fix here.

**RECEIPT_SPEC_v0.1 remains EXPERIMENTAL, but not freely so.** The lock states
that the receipt encoding, hash and validation rules *as consumed by v0.3
consensus* are frozen regardless: changing them requires an RFC and a version
bump. The spec's own "breaking changes allowed" clause does not override the
lock.

**`ISSUE58_REFERENCE_STATUS`: stale.** Issue #58 references
`PROTOCOL_LOCK_v0.2.md`, which v0.3 supersedes, and lists as an unmet
prerequisite the RFC that has already authorized the frozen lock.

---

## 5. Runtime architecture

Responsibility is split cleanly across four crates.

**`mbongo-core`** — [`src/receipt.rs`](../crates/mbongo-core/src/receipt.rs)
owns the data definition only: the `Receipt` struct, its canonical SCALE
encoding, `signing_payload()` (all fields except `signature`) and
`receipt_hash()` = `BLAKE3(signing_payload)`. It contains no judgment about
whether a receipt is acceptable.
[`src/primitives.rs`](../crates/mbongo-core/src/primitives.rs) carries
`TransactionType::AnchorReceipt` (SCALE index 3) and
`TransactionPayload::AnchorReceipt(Box<Receipt>)` (index 1).

**`mbongo-verification`** —
[`src/receipt.rs`](../crates/mbongo-verification/src/receipt.rs) owns
validation judgment: `RECEIPT_VERSION`, Ed25519 signature verification over
the raw 32-byte receipt hash, and `validate_receipt` (version, signature,
duplicate). It is a pure library: no I/O, no chain state. Duplicate detection
is reached through the read-only `ReceiptIndex` port; validation never mutates
an index.

**`mbongo-storage`** — `BatchOp::PutReceipt([u8; 32], Vec<u8>)`,
`has_receipt(task_id)`, `get_receipt(task_id)`. RocksDB uses a dedicated
`receipts` column family with a v1→v2 migration; the in-memory backend mirrors
it. Storage treats receipt bytes as opaque and never decodes them. Per RFC
0002 §169 there is deliberately **no check-then-insert API**: outside the
block batch it could not be made concurrency-safe or consensus-meaningful, and
inside consensus it is unnecessary.

**`mbongo-node`** — [`src/backend.rs`](../crates/mbongo-node/src/backend.rs)
orchestrates. `CompositeReceiptIndex` unions prior persistent state with
receipts anchored earlier in the current block, distinguishing
`DuplicateSource::PriorState` from `DuplicateSource::CurrentBlock`.
`apply_block` runs the anchoring rules in a normative order and commits
effects in a single atomic batch.

---

## 6. Issue #58 acceptance-criteria matrix

Re-derived from the current issue body against code, not carried over.

| # | Criterion | Verdict | Evidence |
|---|---|---|---|
| 1 | Receipt struct matches RECEIPT_SPEC_v0.1 | **SATISFIED** | `mbongo-core/src/receipt.rs:23-39` — 7 fields, spec order |
| 2 | Canonical SCALE encoding implemented | **SATISFIED** | `#[derive(Encode, Decode)]`; `signing_payload()` re-encodes all fields except `signature`; test `scale_roundtrip` |
| 3 | BLAKE3 hash rule implemented exactly as spec | **SATISFIED** | `receipt.rs:72` — `Hash(blake3_hash(&self.signing_payload()))`; test `signature_excluded_from_receipt_hash` |
| 4 | Signature verification implemented | **SATISFIED** | `mbongo-verification/src/receipt.rs:28`; consensus rule (h) at `backend.rs:336` |
| 5 | `task_id` uniqueness enforced | **SATISFIED** | `CompositeReceiptIndex`; rules (i)/(j) at `backend.rs:340-357` |
| 6 | Receipt stored immutably | **SATISFIED** | Duplicate rejection precedes every `PutReceipt`; no overwrite path exists |
| 7 | Indexed by `task_id` and `receipt_hash` | **PARTIAL — see §7** | `task_id`: `receipts` CF + migration. `receipt_hash`: no index, no lookup API |
| 8 | Unit tests included | **SATISFIED** | core, verification, storage (memory + RocksDB + crash/migration), backend, harnesses |
| 9 | Deterministic replay unaffected | **SATISFIED** | `replay_harness` extended in `7924166`; green in CI at the audited commit |

`ISSUE58_SATISFIED = 8` · `ISSUE58_PARTIAL = 1` · `ISSUE58_UNSATISFIED = 0`

---

## 7. The `receipt_hash` index gap

The distinction that settles this: **`receipt_hash` is computable** — any
holder of receipt bytes calls `receipt_hash()` — **but it is not indexed or
queryable**. Those are not the same claim, and only the second is absent.

| Question | Answer |
|---|---|
| Is `receipt_hash` persisted? | No, not as a key. Canonical receipt bytes are stored under `task_id`. |
| Is it derivable from stored data? | **Yes** — `Receipt::decode(bytes).receipt_hash()`, deterministically. |
| Is there an index `receipt_hash → receipt`? | **No.** Repository-wide search finds zero lookup or index by receipt hash. |
| Is there a lookup API? | No. Storage exposes `has_receipt`/`get_receipt`, both keyed by `task_id`. |
| Is there an RPC lookup? | No — and dedicated receipt RPC is explicitly deferred by the lock. |
| Does any **specification** require it? | **No.** RECEIPT_SPEC_v0.1 §6 requires `(task_id, receipt_hash)` "or equivalent" for insert, and lookup **by `task_id`**. Its stated indexing requirement is O(1)/O(log n) **by `task_id`, for duplicate detection**. |
| Does RFC 0002 require it? | No. §15 describes indexing "anchored `task_id`s". PROTOCOL_LOCK_v0.3 freezes the `receipts` CF schema, which is `task_id`-keyed. |
| Would adding one alter consensus? | **No.** Duplicate detection — the only consensus use of the index — is decided by `task_id` in rules (i)/(j). |
| Would it alter the DB schema? | Yes, additively (a new column family or key prefix), requiring a schema-version step. |
| Is it deterministically reconstructible? | **Yes** — by decoding stored receipts and recomputing hashes. It is derived data, not consensus state. |

**Classification: `ISSUE_WORDING_AMBIGUITY`.** Criterion 7 restates the spec's
`(task_id, receipt_hash)` insert example as though both were required lookup
keys. The spec's own words are "or equivalent", and its only stated indexing
requirement is by `task_id`. The implementation satisfies the specification.

Should a lookup by receipt hash later be wanted — for example by Issue #60's
SDK — it would be a **`NON_CONSENSUS_OPTIMIZATION`**: a derived, reconstructible
index, not consensus state, and not a change to any locked surface. This audit
neither proposes nor rules it out.

---

## 8. `task_id` uniqueness analysis

### The rule as implemented

Global, chain-wide, **first-anchored-wins** (RFC 0002 §139). Two consensus
rules make it deterministic:

- **(i)** a block anchoring a `task_id` present in prior state is invalid
  (`TaskIdAlreadyAnchored`);
- **(j)** a block containing two receipts with the same `task_id` is invalid
  regardless of order (`TaskIdRepeatedInBlock`).

The uniqueness key is `task_id` **alone** — not `(task_id, executor)`, not
`(task_id, output_commitment)`. It follows directly that v0.3 admits exactly
one anchored receipt per `task_id`:

| Second receipt | v0.3 outcome |
|---|---|
| same `task_id`, same executor, same output | **rejected** |
| same `task_id`, same executor, different output | **rejected** |
| same `task_id`, different executor, same output | **rejected** |
| same `task_id`, different executor, different output | **rejected** |

Rejection is unconditional on everything except `task_id`.

### Relationship to redundant verification

`DOES_V03_TASK_ID_UNIQUENESS_BLOCK_MULTI_WORKER_RECEIPTS?` **YES** — for
multiple receipts sharing one `task_id`.

The status of redundant execution must be stated carefully.
PROTOCOL_LOCK_v0.3 lists "Verification strategies: redundant execution, TEE
attestation, ZK proofs, PoUW" under **Experimental and Deferred Surfaces**;
RFC 0002 §45 excludes verification of computational correctness from v0.3.
**No 2/3 agreement mechanism exists, and none is currently normative.** This
audit does not assert that redundant execution will be adopted.

### Is the obstacle irreversible?

**No.** It is an obstacle to one particular shape only. Distinguishable future
models, none selected or designed here:

- one task → many receipts (**directly blocked** by the current key);
- one parent task → worker-specific execution identifiers → one receipt each,
  each with its own globally unique `task_id` (**compatible** with the current
  key);
- one task → off-chain worker attestations aggregated into a single on-chain
  receipt (**compatible**).

So v0.3 does not foreclose future verification; it constrains the abstraction
through which it would be expressed. The observation worth acting on is one of
sequencing: the constraint is **frozen**, so if the first model is the one
eventually wanted, the cost of changing course grows as implementations settle
on v0.3.

Enabling multiple receipts per `task_id` would touch consensus validity (rules
i/j), the state key, the storage schema, duplicate-detection semantics, query
semantics, and replay determinism. That makes it a **`CONSENSUS_CHANGE`**
requiring an RFC and a protocol version bump. It is not proposed here.

---

## 9. Metadata bound analysis

`metadata: Vec<u8>` is the only variable-length field in a receipt. All others
are fixed-size, so bounding it bounds the whole receipt: a maximal receipt
encodes to `1 + 32 + 32 + 32 + 32 + (compact_len ≤ 2) + 4096 + 64 = 4291`
bytes (RFC 0002 §147).

**`METADATA_BOUND_VALUE = 4096` (4 KiB)**, defined at
`crates/mbongo-node/src/backend.rs:32`.

### Enforcement path

| Layer | Enforced? | Evidence |
|---|---|---|
| Block validation / `apply_block` | **YES** — consensus rule (e) | `backend.rs:323` → `ApplyBlockError::ReceiptMetadataTooLarge` |
| Mempool admission | YES | `backend.rs:661`, mirroring `apply_block`'s normative order |
| Wallet example (client side) | YES, advisory | `mbongo-wallet/examples/submit_receipt.rs:89` |
| `mbongo-verification` library | No — by design; this is an anchoring rule, not intrinsic receipt validity |
| Storage | No — bytes are opaque |

**A malicious block that bypasses RPC still meets the bound**, because
enforcement is in `apply_block`, which every node runs on every block. Mempool
and wallet checks are early rejections, not the authority.

`IS_METADATA_BOUND_CONSENSUS_CRITICAL?` **YES.**

### Documentation status

`IS_THE_EXACT_BOUND_NORMATIVELY_DOCUMENTED?` **YES.**

- RFC 0002 §147 — value, rationale, maximal encoded size, and the statement
  that raising it is a protocol version bump and it is never lowered
  retroactively;
- RFC 0002 decision record — "**APPROVED by maintainer decision
  (2026-07-19)**";
- PROTOCOL_LOCK_v0.3 §98 — rule (e) in the locked `apply_block` rules table;
- PROTOCOL_LOCK_v0.3 §105 — "`MAX_RECEIPT_METADATA_BYTES = 4096`. Raising it
  is a protocol version [bump]."

`COULD_AN_INDEPENDENT_IMPLEMENTATION_DERIVE_THE_BOUND_FROM_THE_PROTOCOL_DOCUMENTS_ALONE?`
**YES, from PROTOCOL_LOCK_v0.3 — which is the authority for consensus
surfaces. NO, from RECEIPT_SPEC_v0.1 alone**, which does not mention any
bound.

### The narrow, real risk

This is **not** a protocol documentation defect: the operative document
carries the bound. The risk is one of **reference routing**. Issue #58 and
Issue #60 both point to RECEIPT_SPEC_v0.1 as *the* reference, and Issue #60's
SDK is scoped to "Define Receipt Type aligned with RECEIPT_SPEC_v0.1". An SDK
built from that spec alone would encode receipts correctly and compute
`receipt_hash` correctly, but would not pre-validate metadata length — so
oversized submissions would be accepted by the SDK and rejected at consensus.

`INDEPENDENT_IMPLEMENTATION_DIVERGENCE_RISK`: **low for consensus correctness**
(no node would accept an oversized receipt), **moderate for client
ergonomics** (a client can construct a receipt it can never anchor). The
smallest sufficient remedy would be a pointer from RECEIPT_SPEC_v0.1 to
PROTOCOL_LOCK_v0.3 for consensus-level bounds — `DOC_ONLY`, no protocol
change. Not performed here: RECEIPT_SPEC_v0.1 is cited evidence in this audit,
and editing it is a separate decision.

---

## 10. Consensus vs non-consensus classification

More useful than any solution proposal: what class of change each possible
future action falls into.

| Action | Classification | Note |
|---|---|---|
| A. Documentation alignment (stale #58 refs, spec→lock pointer) | `DOC_ONLY` | Touches no code, no locked surface |
| B. `receipt_hash` derived index | `NON_CONSENSUS_IMPLEMENTATION` | Reconstructible from stored receipts; additive schema step; duplicate detection stays `task_id`-keyed |
| C. `receipt_hash` RPC lookup | `NON_CONSENSUS_IMPLEMENTATION` | Lock lists dedicated receipt RPC as deferred, not frozen — but it is Issue #59's surface, not this audit's |
| D. Metadata bound documentation | `DOC_ONLY` | The value is already normative in RFC 0002 and the lock |
| E. Changing the metadata bound | `CONSENSUS_CHANGE` | Lock §105 states this explicitly |
| F. Allowing multiple receipts per `task_id` | `CONSENSUS_CHANGE` | Alters rules (i)/(j), state key, schema, replay |
| G. Changing receipt identity (fields, order, encoding, hash) | `CONSENSUS_CHANGE` | Frozen by the lock regardless of the spec's EXPERIMENTAL status |
| H. Adding verification status to receipts | `CONSENSUS_CHANGE` | Verification strategies are a deferred surface requiring their own RFC |

---

## 11. Protocol lock implications

PROTOCOL_LOCK_v0.3 states that any change to a locked surface "requires a new
RFC and a protocol version bump", by the same process as v0.2: file under
`docs/rfcs/`, identify the affected surface, specify the new version, obtain
Core Maintainer approval, bump, update the lock, tag.

**Frozen** (non-exhaustive, as relevant here): the v0.3 transaction encoding
including `TransactionPayload`; the extended `apply_block` rules and their
normative order; `MAX_RECEIPT_METADATA_BYTES`; the `receipts` CF schema and
`schema_version` semantics; the receipt encoding, hash and validation rules as
consumed by v0.3 consensus.

**Explicitly not frozen** (deferred; each needs its own RFC or spec addendum
to *activate*): receipt economics (fees, rewards, slashing, challenges,
disputes); verification strategies (redundant execution, TEE, ZK, PoUW);
dedicated receipt RPC methods, with `submit_receipt`/`get_receipt` reserved
and returning `-32601`; receipt pruning/archival policy; relayed or delegated
submission where `sender != executor`; `ComputeTask` and `Stake` semantics.

`UNKNOWN`: the lock does not state whether adding a purely derived,
non-consensus database index (action B) counts as touching the frozen
`receipts` CF schema. The schema is frozen; whether a *separate additive*
index is inside or outside that freeze is not addressed by the lock's text,
and is not resolved here.

---

## 12. Issue #59 dependency analysis

Issue #59 is *Stub Reserved Compute RPC Methods (return -32601)*, Tier 1,
"No locked surfaces changed. Methods return errors only." It adds five method
names from COMPUTE_INTERFACE_v0.1 — `submit_compute_task`, `get_compute_task`,
`get_compute_receipt`, `list_compute_tasks`, `get_compute_node_status` — each
returning JSON-RPC `-32601`.

| Dependency | Verdict | Reason |
|---|---|---|
| `task_id` uniqueness | **DOES_NOT_BLOCK_59** | Stubs return an error and read no receipt state |
| Metadata documentation | **DOES_NOT_BLOCK_59** | No receipt is constructed or validated |
| `receipt_hash` index | **DOES_NOT_BLOCK_59** | No lookup is performed |

`ISSUE59_BLOCKED = NO.` #59 can proceed against v0.3 as it stands. The
dependency would become real only if #59's scope grew from reserving method
names to serving receipt data — at which point the `receipt_hash` lookup
question in §7 becomes `DEPENDS_ON_59_SCOPE`. On its stated scope, it does
not.

---

## 13. Issue #60 dependency analysis

Issue #60 is the TypeScript SDK v0.1: a Receipt type aligned with
RECEIPT_SPEC_v0.1, `submitReceipt()`, `getReceipt(taskId)`,
`verifyReceipt(receipt)`, and wrappers for the five reserved methods
including typed handling of `-32601`.

| SDK need | v0.3 primitive | Sufficient? |
|---|---|---|
| Canonical receipt bytes | RECEIPT_SPEC_v0.1 §2–3 — fixed field order, SCALE | **Yes** — independently reproducible |
| `receipt_hash` | RECEIPT_SPEC_v0.1 §4 — BLAKE3 over the signing payload | **Yes** |
| `verifyReceipt` | Ed25519 over the raw 32-byte hash | **Yes** |
| Submission | `TransactionType::AnchorReceipt` + `TransactionPayload::AnchorReceipt` | **Yes**, but the transaction encoding is in RFC 0002 §1 / the lock, not in RECEIPT_SPEC_v0.1 |
| `getReceipt(taskId)` | Storage is `task_id`-keyed | **Yes at the state layer**; no RPC exposes it — that is #59's deferred surface |
| Lookup by `receipt_hash` | — | **No index** (§7). Matters only if the SDK later needs hash-keyed retrieval |
| Metadata pre-validation | 4 KiB bound | **Not in RECEIPT_SPEC_v0.1** (§9) — an SDK built from that spec alone would omit it |

`ISSUE60_IMPACT`: the two gaps that would eventually matter are the absent
`receipt_hash` lookup and the bound's absence from the cited spec. Neither
blocks starting the SDK; both would surface as client-side defects rather than
consensus failures.

---

## 14. Future redundancy compatibility

Covered in §8. Summarised: v0.3 does not irreversibly prevent redundant
verification — it requires it to be expressed through an abstraction other
than "many receipts, one `task_id`". Two compatible shapes are named there;
neither is selected, designed, or endorsed, and no schema is proposed.

---

## 15. Issue #58 disposition options

| Option | Pro | Con | Protocol risk | Historical accuracy |
|---|---|---|---|---|
| **A. Close as already implemented** | Matches reality; the tracker stops claiming unbuilt work | Criterion 7 disappears without an explicit verdict | None | Good, **if** the closing note records why criterion 7 is satisfied-as-specified |
| **B. Keep open for the `receipt_hash` index** | Preserves a visible task | Keeps a `complexity-XL`, `priority-critical`, `tier-0`, `rfc-required` issue open for what is at most a small derived index — and one no specification requires | None | **Poor** — implies the anchoring protocol is unbuilt |
| **C. Rewrite to match the remaining gap** | One issue, accurate scope | Rewriting an issue's body erases the record that the original scope shipped | None | **Poor** — destroys evidence |
| **D. Split the gap to a new issue, close #58** | #58 closes truthfully; any residual work gets correct labels and tier | Two tracker actions instead of one | None | **Best** — preserves both facts separately |
| **E. Reopen protocol design** (uniqueness key) | Addresses §8 before v0.3 hardens further | Reopens a frozen surface on a hypothesis; no redundant-execution design is currently normative | **High** — RFC + version bump | Would misrepresent a deferred question as a defect |

**Recommendation: D**, with a qualification that matters.

Close #58 as implemented, recording that eight criteria are met by shipped
code and that criterion 7 is satisfied against RECEIPT_SPEC_v0.1 §6's "or
equivalent" wording. Open a separate, correctly-tiered issue **only if** a
`receipt_hash` lookup is actually wanted — driven by #59 or #60 needing it,
not by this audit. Nothing in the specification requires it, so creating that
issue speculatively would repeat the error this audit documents: a tracker
entry asserting work that no authority requires.

Option E is **not** recommended. §8's tension is real but its trigger is a
verification architecture that does not exist and is not currently normative.
Reopening a frozen surface on that basis would be premature.

**This audit does not edit, close, relabel, or comment on Issue #58.**

---

## 16. Recommendation

1. **Treat receipt anchoring as done.** v0.3 implements it, the lock freezes
   it, CI is green at the audited commit.
2. **Route #58 per option D** — a maintainer decision, not performed here.
3. **#59 may proceed** on its stated scope; it depends on none of the gaps.
4. **Before #60 hardens**, decide whether RECEIPT_SPEC_v0.1 should point to
   PROTOCOL_LOCK_v0.3 for consensus bounds (`DOC_ONLY`).
5. **Revisit §8's uniqueness tension when, and only when, a redundant-execution
   design is actually proposed.** It is a sequencing question, not a defect.

---

## 17. Open questions

1. Should RFC 0002's `Status: Draft` line be updated, given it authorizes a
   frozen lock? (`DOC_ONLY`; not resolved here.)
2. Does the lock's freeze on the `receipts` CF schema extend to *additional*
   derived indexes? The text does not say (`UNKNOWN`, §11).
3. Is a `receipt_hash` lookup actually wanted by any consumer? No
   specification requires it (§7).
4. If redundant execution is adopted, which abstraction carries it —
   worker-specific execution ids, or off-chain aggregation? (§8; not selected
   here.)
5. Should RECEIPT_SPEC_v0.1 carry consensus-level bounds, or continue to
   describe only the receipt object with the lock as bounds authority? (§9.)

---

## 18. Evidence index

| Claim | Evidence |
|---|---|
| Seven commits are ancestors of `dev` | `git merge-base --is-ancestor <sha> origin/dev`, all seven |
| Receipt data type and hash | `crates/mbongo-core/src/receipt.rs:23-75` |
| Transaction carrier | `crates/mbongo-core/src/primitives.rs:101-136` |
| Validation library | `crates/mbongo-verification/src/receipt.rs:20-101` |
| Storage ops and CF | `crates/mbongo-storage/src/{storage,memory,rocksdb}.rs`; CF `receipts`; v1→v2 migration |
| Consensus anchoring rules | `crates/mbongo-node/src/backend.rs:268-360` |
| Metadata bound | `backend.rs:32` (`4096`), enforced `backend.rs:323` (block) and `:661` (mempool) |
| Bound is normative | RFC 0002 §147, decision record; PROTOCOL_LOCK_v0.3 §98, §105 |
| Uniqueness rule | RFC 0002 §139; `backend.rs:340-357` |
| No `receipt_hash` index | Repository-wide search: zero index or lookup by receipt hash |
| Spec requires `task_id` lookup only | RECEIPT_SPEC_v0.1 §6 |
| Deferred surfaces | PROTOCOL_LOCK_v0.3, "Experimental and Deferred Surfaces" |
| CI green at audited commit | `Mbongo CI` → success on `261dcdf` |

```
CURRENT_PROTOCOL_LOCK=v0.3_FROZEN
AUTHORIZING_RFC=0002
RECEIPT_SPEC=RECEIPT_SPEC_v0.1_EXPERIMENTAL (encoding/hash/validation frozen as consumed by v0.3)
V03_PROTOCOL_CHANGE_PROPOSED=NO
RUNTIME_PROTOCOL_CHANGES=0
```
