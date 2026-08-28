# Mbongo Chain RPC Specification v0.2

**Status:** DRAFT
**Supersedes:** [rpc_v0.1.md](./rpc_v0.1.md) as the description of current node RPC behaviour
**Derived from:** executable code at `206e2c73868864165d0717c770b37ac581f53f25`
**Not frozen.** Three questions from the first draft are now decided (§6.1); five remain open (§6.2), and two methods still have no executable contract coverage.

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
| Coverage | **none for the wire shape** — see §5 |

**Differs from v0.1** on both sides. v0.1 specified params
`[signed_tx: string]` (a hex-encoded SCALE blob) and a `{ tx_hash: string }`
result object. The runtime takes a structured JSON transaction and returns a
bare string.

**Decided for the request: align to the runtime.** The canonical v0.2 request
is a structured `Transaction` JSON object matching the Rust serde wire
representation. The historical `[signed_tx_hex]` interface is **not**
restored: the structured form is the implemented path and matches both the
current transaction model and the wallet tooling.

**Response retained as-is for v0.2** — the bare transaction-hash string —
unless an object envelope is deliberately chosen before v0.2 is frozen.

Both shapes need executable contract tests before this document can be
frozen. See §5 and §6.

### 2.4 `produce_block`

| Field | Value |
|---|---|
| Params | **none** — the method is parameterless (decided, see below) |
| Returns | a JSON **string**: the hex-encoded block hash |
| Mutates state | yes — produces a block and applies it |
| Backend | `RpcBackend::produce_block` |
| Coverage | **none** — see §5 |

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
anywhere; whether it should be is a separate question from this method's
signature, and is recorded in §6.

Should a caller-supplied limit later be wanted, it is a deliberate runtime
change — trait signature, handler, and tests — not a documentation edit.

The result is a bare hash string rather than v0.1's
`{ block_hash, height }` object. Retained as-is for v0.2; see §6.

### 2.5 `get_latest_block_hash`

| Field | Value |
|---|---|
| Params | ignored |
| Returns | a JSON **string**: the hex-encoded tip block hash |
| Mutates state | no |
| Backend | `RpcBackend::get_latest_block_hash` |
| Coverage | tested — asserts a string result |

**Not documented by v0.1.** See §3.

### 2.6 `get_block_by_height`

| Field | Value |
|---|---|
| Params | `{"height": <u64>}` **or** a bare `<u64>` — both accepted |
| Returns | the serialised `Block` (§4.2) |
| Mutates state | no |
| Backend | `RpcBackend::get_block_by_height` |
| Errors | `-32602` on missing params or an unparseable height; `-32603` when no block exists at that height |
| Coverage | tested — asserts `result.header.height` |

**Not documented by v0.1.** See §3.

Note that a missing block is reported as `-32603` (internal error), not as a
null result or a dedicated not-found code.

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

Not every dispatched method carries the same weight.

| Method | Status |
|---|---|
| `ping`, `get_block_height`, `submit_transaction`, `produce_block` | documented by v0.1 — a public contract, however inaccurately described |
| `get_latest_block_hash`, `get_block_by_height` | **implemented but never documented** by a spec |

The last two are dispatched and tested, but no specification ever declared
them. This draft records their behaviour; it does not by itself promote them
to stable public API. That promotion is one of the decisions in §6.

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

| Method | Executable coverage |
|---|---|
| `ping` | result shape asserted |
| `get_block_height` | result shape asserted |
| `get_latest_block_hash` | result shape asserted |
| `get_block_by_height` | result shape asserted |
| `submit_transaction` | **TEST_GAP** — no wire-shape test |
| `produce_block` | **TEST_GAP** — no wire-shape test |

The two uncovered methods are the two that mutate state, and the two whose
v0.1 contract diverges most. That is the least comfortable place for a gap,
and it is why this document is DRAFT rather than FROZEN.

---

## 6. Decisions and remaining questions

### 6.1 Resolved

**`ping` result — align the spec to the runtime.** The canonical v0.2 result
is the JSON string `"pong"`. The behaviour is established and covered by an
executable test, and no consumer of v0.1's `{ ok: true }` is evidenced. No
runtime change. `INTENTIONAL_PUBLIC_CONTRACT`.

**`submit_transaction` request — align the spec to the runtime.** The
canonical v0.2 request is a structured `Transaction` JSON object matching the
Rust serde wire representation. The historical `[signed_tx_hex]` interface is
not restored. `INTENTIONAL_PUBLIC_CONTRACT`.

**`produce_block` is parameterless.** `max_txs` is not an intentional API
requirement: the backend trait takes no argument, no caller passes one, and
block size is already bounded node-side by `MAX_TX_PER_BLOCK = 1000`. The
method is defined as parameterless rather than as accepting-and-ignoring a
parameter, so v0.2 does not attribute semantics the code does not have.
`INTENTIONAL_PUBLIC_CONTRACT`. Adding a caller-supplied limit later is a
deliberate runtime change, not a documentation edit.

### 6.2 Still open

**Q-A — Should `get_latest_block_hash` and `get_block_by_height` become
public contract?**
They are implemented, dispatched and tested, but were never specified. Until
this is answered they must not be described as stable to SDK consumers.
`AMBIGUOUS_REQUIRES_MAINTAINER`.

**Q-B — Is the dual parameter form of `get_block_by_height` a contract?**
It accepts both `{"height": N}` and a bare `N`, via
`params.get("height").cloned().unwrap_or(params.clone())`. Leniency of this
kind is usually an implementation detail rather than a promise.
`AMBIGUOUS_REQUIRES_MAINTAINER`.

**Q-C — Should `submit_transaction` return an object envelope?**
The runtime returns a bare hash string. Retained for v0.2 unless an envelope
is deliberately chosen before freezing. The same question applies to
`produce_block` and `get_latest_block_hash`, whose bare-string results also
replace v0.1 objects. `CURRENT_IMPLEMENTATION_DETAIL` pending that choice.

**Q-D — Should a missing block have a distinct signal?**
`get_block_by_height` reports an absent block as `-32603` (internal error)
rather than a null result or a dedicated code.
`CURRENT_IMPLEMENTATION_DETAIL`.

**Q-E — Should `MAX_TX_PER_BLOCK = 1000` be a declared public contract?**
Blocks are bounded, but by a node-side constant that no specification
declares. Distinct from the `produce_block` signature question resolved
above. `AMBIGUOUS_REQUIRES_MAINTAINER`.

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

This document should not be frozen until:

1. Q-A through Q-E are answered;
2. `submit_transaction` and `produce_block` have executable wire-shape tests;
3. the implementation details recorded in §6.2 are either promoted to
   contract or documented as unstable.

Freezing it earlier would repeat the failure it exists to correct: a document
declared authoritative while the node does something else.
