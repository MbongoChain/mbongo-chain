# Mbongo Chain RPC Specification v0.2

**Status:** FROZEN
**Supersedes:** [rpc_v0.1.md](./rpc_v0.1.md) as the description of current node RPC behaviour
**Derived from:** executable code and tests at `1adf15e7d8c4f1877ffa895deef4c50093fe42b4`
**Frozen.** All contract questions are decided (§6), all six methods have executable coverage (§5), and an independent audit against the runtime and the tests found no divergence — see §8. Breaking changes require a new RPC version.

> This document describes what the node **does**, derived from
> `crates/mbongo-network/src/server.rs`, the `RpcBackend` trait, and the
> serde implementations in `mbongo-core`. It proposes no change to node
> behaviour and changes none.
>
> [rpc_v0.1.md](./rpc_v0.1.md) stays intact as a historical FROZEN artifact.
> It is preserved rather than corrected in place because it is the evidence
> that a frozen document and the shipped node disagree.

---

## 1. Overview

JSON-RPC 2.0 over HTTP POST. Endpoint: `/rpc`. Content-Type:
`application/json`. Batch requests are supported: an array of request objects
returns an array of response objects.

The request `id` is echoed on every response, success or error. Six methods
are dispatched; every other method name — including all reserved compute
names — returns `-32601`.

### Byte encoding in JSON

Byte arrays cross the wire as `0x`-prefixed lowercase hex strings, not as
number arrays:

| Type | JSON form | Source |
|---|---|---|
| `Address` (32 bytes) | `"0x" + 64 hex` | `impl Display for Address` |
| `Hash` (32 bytes) | `"0x" + 64 hex` | `impl Display for Hash` |
| `signature` (64 bytes) | `"0x" + 128 hex` | `serde_arr64` |
| `u128` / `u64` | JSON number | serde default |

This matters for clients: a transaction can be **submitted** as JSON without
any SCALE implementation. SCALE is required only to compute the bytes that
get signed (§4.3).

---

## 2. Methods

Six methods are dispatched. Four were documented by v0.1; two were not.

### 2.1 `ping`

| Field | Value |
|---|---|
| Params | ignored |
| Returns | the JSON string `"pong"` |
| Mutates state | no |
| Backend | `RpcBackend::ping`, default impl returns `"pong"` |
| Coverage | tested — `jsonrpc_tests.rs` asserts `result == "pong"` |

**Differs from v0.1**, which specified `{ ok: true }`. **Decided: align the
spec to the runtime.** The behaviour is established and covered by an
executable test, and no consumer of the `{ ok: true }` shape is evidenced.
No runtime change.

### 2.2 `get_block_height`

| Field | Value |
|---|---|
| Params | ignored |
| Returns | a JSON number (`u64`) |
| Mutates state | no |
| Backend | `RpcBackend::get_block_height` |
| Coverage | tested — asserts `result == 1234` |

Matches v0.1.

### 2.3 `submit_transaction`

| Field | Value |
|---|---|
| Params | a JSON **object** deserialising to `Transaction` (§4) |
| Returns | a JSON **string**: the hex-encoded transaction hash |
| Mutates state | yes — mempool admission |
| Backend | `RpcBackend::submit_transaction` |
| Errors | `-32602` on missing params (`"missing params"`) or on a payload that does not deserialise (`"invalid transaction: …"`); `-32603` on backend rejection |
| Coverage | tested — object params, string result, id, reaches backend; hex form rejected (§5) |

**Differs from v0.1** on both sides. v0.1 specified params
`[signed_tx: string]` (a hex-encoded SCALE blob) and a `{ tx_hash: string }`
result object. The runtime takes a structured JSON transaction and returns a
bare string.

**Decided for the request: align to the runtime.** The canonical v0.2 request
is a structured `Transaction` JSON object matching the Rust serde wire
representation. The historical `[signed_tx_hex]` interface is **not**
restored: the structured form is the implemented path and matches both the
current transaction model and the wallet tooling.

**Response retained as a bare transaction-hash string**, not wrapped in an
envelope (§6.2). Both the accepted object form and the rejection of the
historical hex form are pinned by tests (§5).

### 2.4 `produce_block`

| Field | Value |
|---|---|
| Params | **none** — the method is parameterless (decided, see below) |
| Returns | a JSON **string**: the hex-encoded block hash |
| Mutates state | yes — produces a block and applies it |
| Backend | `RpcBackend::produce_block` |
| Coverage | tested — no params, string result, id, mutating path exercised (§5) |

**Differs from v0.1**, which specified an optional `[max_txs: u32]` parameter
and a `{ block_hash, height }` result object.

`max_txs` is **not** an intentional API requirement, and `produce_block` is
therefore defined here as parameterless rather than as "accepts a parameter
and ignores it". Three facts settle this, and none of them is a preference:

1. `RpcBackend::produce_block(&self)` takes **no argument at all**. The
   parameter is not ignored at the JSON layer while being plumbed underneath
   — it is absent from the implementation entirely, and adding it would
   change the trait signature.
2. **No caller anywhere passes one.** Every call site — the block-producer
   loop in `main.rs`, the harnesses, and roughly forty tests — calls
   `produce_block()` with no argument.
3. The concern `max_txs` would address is **already handled**:
   `MAX_TX_PER_BLOCK = 1000` bounds every block at
   `backend.rs` via `pool.drain_for_block(MAX_TX_PER_BLOCK)`.

Blocks are therefore bounded, but bounded **node-side by a constant**, not by
the caller. That constant is not currently declared as a public contract
anywhere, and v0.2 does not promise caller control over it (§6.3).

Should a caller-supplied limit later be wanted, it is a deliberate runtime
change — trait signature, handler, and tests — not a documentation edit.

The result is a bare hash string rather than v0.1's
`{ block_hash, height }` object. Retained (§6.2).

### 2.5 `get_latest_block_hash`

| Field | Value |
|---|---|
| Params | none |
| Returns | a JSON **string**: the hex-encoded tip block hash |
| Mutates state | no |
| Backend | `RpcBackend::get_latest_block_hash` |
| Coverage | tested — asserts a string result and id preservation |

**Not documented by v0.1**, and **adopted as public contract by v0.2.** See
§3. The bare string result is retained rather than wrapped in an envelope.

### 2.6 `get_block_by_height`

| Field | Value |
|---|---|
| Params | **canonical:** `{"height": <u64>}` |
| Returns | the serialised `Block` (§4.2), nested `{header, body}` |
| Mutates state | no |
| Backend | `RpcBackend::get_block_by_height` |
| Errors | `-32602` on missing params or an unparseable height; `-32603` when no block exists at that height |
| Coverage | tested — asserts `result.header.height` and id preservation |

**Not documented by v0.1**, and **adopted as public contract by v0.2.** See
§3.

#### Canonical form versus runtime tolerance

The canonical v0.2 request form is the object `{"height": N}`, and that is
the only form clients should emit.

The runtime **also** accepts a bare numeric `N`, because the handler reads
`params.get("height").cloned().unwrap_or(params.clone())`. That is
**runtime compatibility tolerance, not a second canonical form.** It is not
promised, it is not removed here, and a client relying on it is relying on an
implementation detail.

The distinction is already reflected in the test suite: the existing contract
test emits `{"height": 5}`, and no test asserts the bare-number form.

#### Missing block

An absent block surfaces as `-32603` (internal error), not as a null result
or a dedicated not-found code. Documented here as **observed v0.2 behaviour**,
retained deliberately rather than redesigned in this revision. It is
versioned behaviour that a later RPC revision may improve; nothing in this
document argues it is the right long-term design.

### 2.7 Everything else

Any other method returns `-32601` with message `"Method not found: <name>"`
and the request id preserved. This includes the five names reserved by
[COMPUTE_INTERFACE_v0.1](./COMPUTE_INTERFACE_v0.1.md) §3 and the
`submit_receipt` / `get_receipt` names that
[PROTOCOL_LOCK_v0.3](./PROTOCOL_LOCK_v0.3.md) lists as deferred. Their
unavailability is pinned by tests.

`-32601` means **the method is unavailable**. It never means a resource was
not found.

---

## 3. Method status classification

All six dispatched methods are **intentional public v0.2 contract**. How they
got there differs, and the distinction is worth keeping:

| Method | Origin |
|---|---|
| `ping`, `get_block_height`, `submit_transaction`, `produce_block` | documented by v0.1 — a public contract, however inaccurately described |
| `get_latest_block_hash`, `get_block_by_height` | **implemented but never documented** by any spec; **adopted** by v0.2 |

Adoption is not a claim of history. The last two were never frozen and never
specified; v0.2 deliberately takes them into the public contract now that
each has executable coverage (§5). Nothing here retroactively asserts they
were stable before.

The reserved compute methods are **not** in this table and are not
implemented. See §2.7.

---

## 4. Data shapes

### 4.1 `Transaction`

Field order below is the SCALE order; JSON is an object, so wire order does
not matter for transport — only for signing (§4.3).

| Field | JSON type | Notes |
|---|---|---|
| `tx_type` | enum | `Transfer` \| `ComputeTask` \| `Stake` \| `AnchorReceipt` |
| `sender` | `"0x…"` 32 bytes | |
| `receiver` | `"0x…"` 32 bytes | zero address for `AnchorReceipt` |
| `amount` | number (`u128`) | `0` for `AnchorReceipt` |
| `nonce` | number (`u64`) | |
| `payload` | `None` \| `AnchorReceipt(Receipt)` | v0.3 addition, RFC 0002 §1 |
| `signature` | `"0x…"` 64 bytes | over the signing payload, §4.3 |

### 4.2 `Block`

Nested, not flat:

```
{ "header": { "parent_hash", "state_root", "transactions_root",
              "timestamp", "height" },
  "body":   { "transactions": [ Transaction, … ] } }
```

An `AnchorReceipt` transaction carries its receipt inside
`body.transactions[].payload`, so a block response contains anchored receipts
in full.

### 4.3 Signing

Two distinct signatures over two distinct payloads, both Ed25519:

| Signature | Signed bytes |
|---|---|
| `transaction.signature` | `SCALE(tx_type, sender, receiver, amount, nonce, payload)` — the `signature` field excluded |
| `receipt.signature` | the **raw 32 bytes** of `receipt_hash`, never its hex string |

`receipt_hash = BLAKE3(SCALE(receipt fields except signature))`, per
[RECEIPT_SPEC_v0.1](./RECEIPT_SPEC_v0.1.md) §4.

So a client needs SCALE to **sign**, not to **transport**.

---

## 5. Contract coverage

Every method in §2 has an executable contract test at the JSON-RPC boundary,
in `crates/mbongo-network/tests/jsonrpc_tests.rs`. Methods are mapped to the
tests that pin them rather than to a test count, which would go stale.

| Method | Test | What it pins |
|---|---|---|
| `ping` | `test_ping` | string result `"pong"`, id preserved |
| `get_block_height` | `test_get_block_height` | numeric result, id preserved |
| `submit_transaction` | `submit_transaction_accepts_a_structured_transaction_object` | object params, string result, id preserved, and the transaction reaches the backend with a still-valid signature |
| | `submit_transaction_does_not_accept_the_historical_hex_string_form` | the v0.1 `[signed_tx_hex]` form yields `-32602` and never reaches the backend |
| `produce_block` | `produce_block_takes_no_parameters_and_returns_a_hash_string` | no params, string result, id preserved, mutating backend path exercised once |
| `get_latest_block_hash` | `test_get_latest_block_hash` | string result, id preserved |
| `get_block_by_height` | `test_get_block_by_height` | canonical `{"height": N}` params, nested `{header, body}` result, id preserved |

What this coverage does **not** claim:

- It exercises the **wire boundary**, not consensus. Signature validity,
  mempool admission rules and block application are covered by their own
  suites elsewhere.
- The bare-number tolerance on `get_block_by_height` (§2.6) is asserted by no
  test, deliberately — it is tolerance, not contract.
- Error **messages** are not pinned anywhere. Only codes are.
- `-32601` for every reserved and unknown method is pinned separately, in the
  reserved-compute tests.

---

## 6. Decisions

Every question raised by earlier drafts of this document is now decided. The
decisions and their reasons are recorded here so that a later reader sees
what was chosen deliberately rather than inherited by accident.

| | Decision |
|---|---|
| `ping` result | the JSON string `"pong"` — align the spec to the runtime |
| `submit_transaction` request | a structured `Transaction` JSON object — align the spec to the runtime |
| `produce_block` params | none; the method is parameterless |
| `get_latest_block_hash`, `get_block_by_height` | adopted as public v0.2 contract |
| `get_block_by_height` params | `{"height": N}` canonical; bare `N` is tolerance |
| string results | retained as strings; no envelopes introduced |
| missing block | `-32603` retained as observed v0.2 behaviour |
| `MAX_TX_PER_BLOCK` | node implementation limit, **not** RPC contract |

### 6.1 Aligned to the runtime rather than to v0.1

**`ping` returns `"pong"`.** v0.1 described `{ ok: true }`. The runtime
behaviour is established and covered by a test, and no consumer of the object
form is evidenced. No runtime change.

**`submit_transaction` takes a structured `Transaction` object.** v0.1
described `[signed_tx: string]`, a hex-encoded SCALE blob. That form is not
restored: the structured representation is the implemented path, matches the
current transaction model and the wallet tooling, and its rejection is now
pinned by a test.

### 6.2 Results and errors retained as they are

**String results stay strings.** `submit_transaction`, `produce_block` and
`get_latest_block_hash` each return a bare hex hash string where v0.1 used an
object. No envelope is introduced for aesthetic consistency, and no runtime
change is made. A future envelope migration would require its own RPC
version.

**A missing block stays `-32603`.** Documented faithfully as observed
behaviour rather than redesigned here. No new error code is invented, and no
claim is made that this is the right long-term shape — it is versioned
behaviour a later revision may improve.

### 6.3 Boundaries held

**`get_latest_block_hash` and `get_block_by_height` are adopted, not
retroactively frozen.** They were implemented and dispatched but never
specified. v0.2 takes them into the public contract now that each has
executable coverage. See §3.

**The bare-number form of `get_block_by_height` is tolerance, not contract.**
See §2.6. It is not promised, not tested, and not removed from the runtime
here.

**`MAX_TX_PER_BLOCK = 1000` is not an RPC guarantee.** It is a node-side
block-production limit. It does not become a property of the `produce_block`
interface merely because the backend uses it, and this document promises no
caller control over it. `produce_block` remains parameterless, and
`MAX_TX_PER_BLOCK`, the `RpcBackend::produce_block` signature, the handler
and consensus behaviour are untouched. Whether that limit deserves normative
protocol documentation is a separate concern, and not an RPC one.

**Reserved compute methods are not implemented.** The five names from
COMPUTE_INTERFACE_v0.1 §3, and `submit_receipt` / `get_receipt`, return
`-32601` and are pinned there by tests. Nothing in this document presents
them as available.

---

## 7. Relationship to other documents

| Document | Status | Relationship |
|---|---|---|
| [rpc_v0.1.md](./rpc_v0.1.md) | FROZEN | Superseded as a description of current behaviour. Kept intact: it is the evidence of the divergence. |
| [jsonrpc_v0.1.md](./jsonrpc_v0.1.md) | no status line | Describes an `mbg_*` surface the node has never implemented. Recommended classification: **ASPIRATIONAL / NOT IMPLEMENTED**, to be marked as such rather than deleted. |
| [PROTOCOL_LOCK_v0.3.md](./PROTOCOL_LOCK_v0.3.md) | FROZEN | Does **not** freeze the general RPC surface. It defers dedicated receipt RPC and reserves `submit_receipt` / `get_receipt` at `-32601`. |
| [COMPUTE_INTERFACE_v0.1.md](./COMPUTE_INTERFACE_v0.1.md) | — | Reserves five compute method names, all unavailable. |

---

## 8. Path to FROZEN

The three preconditions this document set for itself are now met:

1. **Every contract question is decided** (§6) — nothing is left as
   `AMBIGUOUS_REQUIRES_MAINTAINER`.
2. **Every method has an executable wire-shape test** (§5), including
   `submit_transaction` and `produce_block`, which were the gap.
3. **The retained implementation details are documented as such** rather than
   silently promoted: bare string results (§6.2), `-32603` for a missing
   block (§6.2), and the bare-number tolerance on `get_block_by_height`
   (§2.6).

The document was deliberately kept DRAFT through the change that resolved
those questions, so the freeze could not be hidden inside an edit about
something else — close to how the v0.1 divergence went unnoticed in the first
place.

**The independent audit has since been run**, deriving the contract afresh
from `server.rs`, the `RpcBackend` trait, the serde types and
`jsonrpc_tests.rs`, and comparing that derivation against this text. All six
methods matched on runtime behaviour, on test coverage and on documentation:
no divergence. This document is therefore **FROZEN**.

Breaking changes now require a new RPC version. That includes anything that
would alter a canonical parameter form, a result shape, or the public method
set — and, specifically, promoting the `get_block_by_height` bare-number
tolerance (§2.6) to canonical, introducing result envelopes (§6.2), or
exposing `max_txs` (§6.3).

What is **not** frozen by this document: the reserved compute RPC surface,
which remains deferred and unavailable, and node-side implementation limits
such as `MAX_TX_PER_BLOCK`, which are not RPC contract.
