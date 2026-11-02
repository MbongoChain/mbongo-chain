# Mbongo Chain RPC Specification v0.3

**Status:** FROZEN — the independent audit of §9.1 found no divergence; frozen with [PROTOCOL_LOCK_v0.4.md](./PROTOCOL_LOCK_v0.4.md)
**Supersedes:** [rpc_v0.2.md](./rpc_v0.2.md) as the description of current node RPC behaviour
**Authorising protocol change:** [RFC 0005 — Compute Task Commitment](../rfcs/0005-compute-task-commitment-v1.md), Accepted; implemented in Workstreams A and A2 of the Compute vertical epic (#126)
**Derived from:** executable code and tests at `f6aa3972fe6fbde77a61bdf0bbdb5e2b5f864906` plus the tests this document names in §5

> This document describes what the node **does**. It is rpc_v0.2 plus
> **one widening**: the transaction payload union has a third variant,
> `ComputeTask`, because the protocol has one. Every method, parameter form,
> result form, error code and byte-encoding rule of v0.2 is carried over
> unchanged, and nothing here changes consensus, which is decided by RFC 0005
> and implemented in `crates/mbongo-node`.
>
> [rpc_v0.2.md](./rpc_v0.2.md) stays intact as a FROZEN artifact. It is the
> evidence of what a v0.2 client was promised, and §7 below states exactly
> what such a client sees now.

---

## 0. Why a new version, and what it is not

rpc_v0.2 §4.1 pins the `payload` field of a `Transaction` as

```
None | AnchorReceipt(Receipt)
```

and §8 freezes the document with the rule that "anything that would alter a
canonical parameter form, a result shape, or the public method set" requires
a new RPC version. `TransactionPayload::ComputeTask` (RFC 0005 §2.7, codec
index 2) is a new member of that union. It appears in the `get_block_by_height`
result whenever a block carries a task, and it is accepted by
`submit_transaction`. That alters a result shape and a parameter form, so
it is a versioned change by v0.2's own rule. This is the whole reason v0.3
exists.

What v0.3 is **not**:

- **Not a runtime change.** The node's JSON is `serde` over the protocol
  types. When RFC 0005 added the variant to `mbongo-core`, the wire form
  followed. v0.3 documents that form; no handler, route, parameter or result
  was modified to produce it, and the six methods are dispatched exactly as
  before.
- **Not a new method.** RFC 0005 §7 activates no RPC method. The five
  reserved compute names and `submit_receipt` / `get_receipt` still return
  `-32601` (§2.7).
- **Not a change to the Receipt or AnchorReceipt shapes.** RFC 0005 rules
  (q)–(s) are validity rules; they added no receipt field and no byte.
  §4.1's `AnchorReceipt(Receipt)` member is unchanged (§4.5).
- **Not a consensus validator.** RPC deserialises and forwards. Rules (k)–(s)
  run in the node's admission and block application, and their verdicts
  surface through the existing `-32603` path (§2.3).

---

## 1. Overview and byte encoding

Unchanged from v0.2 §1: JSON-RPC 2.0 over HTTP POST at `/rpc`, batch
supported, `id` echoed, six methods, `-32601` for everything else.

The byte-encoding rule is also unchanged, and is now reached in **two**
places rather than one:

| Type | JSON form | Source |
|---|---|---|
| `Address` (32 bytes) | `"0x" + 64 hex` | `impl Display for Address` |
| `Hash` (32 bytes) | `"0x" + 64 hex` | `impl Display for Hash` |
| `signature` (64 bytes) | `"0x" + 128 hex` | `serde_arr64` |
| unannotated `[u8; N]` / `Vec<u8>` | array of numbers, each `0..=255` | serde default |
| `u128` / `u64` / `u8` | JSON number | serde default |

The unannotated row applies to the `Receipt` nested in an `AnchorReceipt`
payload (v0.2 §1) and, from this version, to the `ComputeTask` nested in a
`ComputeTask` payload: its `salt`, `input_commitment` and `execution_spec`
are arrays of numbers, while its `submitter` and `executor` are `Address`
and therefore hex. No new encoding convention is introduced.

**Request body limit.** The node accepts request bodies up to 2 MiB
(axum's default). A maximal `ComputeTask` transaction — 1024-byte
`execution_spec`, RFC 0005 §2.10 — is under 4 KiB as JSON. This is an
implementation limit, not contract, recorded so a client knows the
protocol-maximal task is submittable.

---

## 2. Methods

All six methods keep their v0.2 params, results, backends and error codes.
Only the two that carry a `Transaction` are affected by the widening, and
only in what the `Transaction` may contain.

### 2.1 `ping`, 2.2 `get_block_height`, 2.4 `produce_block`, 2.5 `get_latest_block_hash`

Unchanged from v0.2. `produce_block` remains parameterless, and a block it
produces may now contain `ComputeTask` transactions drawn from the mempool.

### 2.3 `submit_transaction`

| Field | Value |
|---|---|
| Params | a JSON **object** deserialising to `Transaction` (§4), now including `payload: {"ComputeTask": {…}}` (§4.4) |
| Returns | a JSON **string**: the hex-encoded transaction hash — unchanged |
| Errors | `-32602` on missing params or a payload that does not deserialise, including any payload variant the protocol does not define (§4.1); `-32603` on backend rejection — unchanged codes |

A `ComputeTask` transaction is an ordinary signed transaction (RFC 0005
§2.5): the client signs the SCALE signing payload (§4.3) and submits the
object. The node's admission checks mirror consensus rules (k)–(p), and for
an `AnchorReceipt` rules (a)–(j) and (q)–(s); a rejection is `-32603` with
a human-readable message. **Messages are not contract; codes are**, as in
v0.2 §5.

Consensus decides validity. The typed submission shape is complete for
every payload the protocol defines; there is no separate "submit task"
method and, per RFC 0005 §7, there will not be one under this design.

### 2.6 `get_block_by_height`

Params, result nesting, `-32602` / `-32603` behaviour and the bare-number
tolerance are unchanged from v0.2 §2.6. The result's
`body.transactions[].payload` may now be `{"ComputeTask": {…}}` (§4.4). A
block containing a task is returned whole; nothing is dropped, relabelled or
transformed.

### 2.7 Everything else

Unchanged. `-32601` for every other name, including the reserved compute
names. `-32601` means the method is unavailable, never that a resource was
not found.

---

## 3. Method status classification

Unchanged from v0.2 §3: all six dispatched methods are intentional public
contract. No method is added or removed by this version.

---

## 4. Data shapes

### 4.1 `Transaction`

| Field | JSON type | Notes |
|---|---|---|
| `tx_type` | enum | `Transfer` \| `ComputeTask` \| `Stake` \| `AnchorReceipt` — unchanged |
| `sender` | `"0x…"` 32 bytes | |
| `receiver` | `"0x…"` 32 bytes | zero address for `AnchorReceipt` **and `ComputeTask`** (RFC 0005 §2.8) |
| `amount` | number (`u128`) | `0` for `AnchorReceipt` **and `ComputeTask`** |
| `nonce` | number (`u64`) | |
| `payload` | `None` \| `AnchorReceipt(Receipt)` \| **`ComputeTask(ComputeTask)`** | **the v0.3 widening**, RFC 0005 §2.7 |
| `signature` | `"0x…"` 64 bytes | over the signing payload, §4.3 |

The payload is **externally tagged**: `None` is the bare JSON string
`"None"`; each other variant is a single-key object whose key is the variant
name — `{"AnchorReceipt": {…}}`, `{"ComputeTask": {…}}`.

**Type/payload pairing is consensus, not representation.** `tx_type` and
`payload` are separate fields on the wire; RFC 0002 rule (a) and RFC 0005
rule (k) require them to agree (`AnchorReceipt` ⟺ anchor payload,
`ComputeTask` ⟺ task payload, `Transfer` and `Stake` ⟺ `"None"`), and a
mismatch is rejected by the node with `-32603`. The legacy
`(ComputeTask, "None")` pairing that v0.3-protocol nodes executed as a
transfer is rejected the same way (RFC 0005 §5.1).

**Unknown variants.** The union is closed. A payload object whose key is not
`AnchorReceipt` or `ComputeTask`, a string other than `"None"`, or a variant
object missing a field does not deserialise and is `-32602` before any
backend code runs. The server never emits a variant it does not define.

### 4.2 `Block`

Unchanged from v0.2 §4.2. A block response contains its `ComputeTask`
transactions in full, exactly as it contains anchored receipts.

### 4.3 Signing

Unchanged rule, one more instance of it. Both signatures are Ed25519:

| Signature | Signed bytes |
|---|---|
| `transaction.signature` | `SCALE(tx_type, sender, receiver, amount, nonce, payload)` — the `signature` field excluded |
| `receipt.signature` | the **raw 32 bytes** of `receipt_hash`, never its hex string |

For a `ComputeTask` transaction the signing payload is therefore

```
0x01 || sender[32] || receiver[32] || amount_u128_le[16] || nonce_u64_le[8]
     || 0x02 || <canonical task bytes>
```

with the task bytes beginning at offset 90, exactly as a receipt does under
`0x03 … 0x01`. The envelope carries **no signature of its own** (RFC 0005
§2.5); the transaction signature is the only authentication, and consensus
requires `sender == task.submitter` (rule o). The canonical task bytes and
the whole formula are pinned by
[`test-vectors/compute-task/compute-task-v1.json`](../../test-vectors/compute-task/compute-task-v1.json).

A client still needs SCALE to **sign**, not to **transport**.

### 4.4 `ComputeTask`

The value under the `ComputeTask` payload key. Six fields, all of RFC 0005
§2.1, none added:

| Field | JSON type | Protocol type | Notes |
|---|---|---|---|
| `version` | number | `u8` | must be `1` (rule m) |
| `submitter` | `"0x…"` 32 bytes | `Address` | must equal `sender` (rule o) |
| `executor` | `"0x…"` 32 bytes | `Address` | the one executor authorised to answer; immutable, and a different executor is a different task (RFC 0005 §2.6) |
| `salt` | array of 32 numbers | `[u8; 32]` | client-chosen, opaque; zero is legal; **not** the transaction nonce and not input blinding |
| `input_commitment` | array of 32 numbers | `[u8; 32]` | opaque commitment to off-chain input; consensus compares it for equality with a receipt's (rule r) and never learns how it was derived — plain or blinded (RFC 0005 §2.4) |
| `execution_spec` | array of `0..=1024` numbers | `Vec<u8>` | opaque bytes; carried and returned exactly as signed, never decoded, normalised or interpreted (RFC 0005 §2.12); bound by rule (n) |

**What RPC does with these bytes: nothing.** It does not read the input the
commitment refers to, does not fetch anything from any data plane, and
forms no opinion about `execution_spec`. If a submitter chooses to publish
private bytes in `execution_spec`, the chain holds them and RPC returns them;
RPC redacts nothing, because nothing on chain is private. The architecture's
guidance not to do that
([compute-privacy-data-plane.md](../architecture/compute-privacy-data-plane.md))
is guidance for clients, not a consensus or RPC rule.

**Not on the wire, by design:** `task_id`, and every off-chain notion —
lease, attempt, worker, provider, status, expiry, cancellation, capability,
locator, attestation. None is a consensus field, so none is a transaction
field.

#### `task_id`

`task_id` is **not a field of the transaction**. It is derived:

```
task_id = BLAKE3( "mbongo:compute-task:v1" || SCALE(ComputeTask) )
```

per RFC 0005 §2.2, with the 22-byte tag prepended raw. A client computes it
from the canonical task bytes it already has to sign; the node computes it
the same way in consensus. RPC exposes no derived value and defines no
second derivation. On the wire, a task's identity appears only inside a
receipt, as the `task_id` array of an anchored `Receipt` (§4.5), and
consensus requires that array to equal the derived identity of a registered
task (rule q).

### 4.5 `Receipt` and `AnchorReceipt` — unchanged

The `Receipt` object nested under `{"AnchorReceipt": …}` is exactly v0.2's:
`version` (number), `task_id` / `input_commitment` / `output_commitment`
(arrays of 32 numbers), `executor` (hex), `metadata` (array of numbers),
`signature` (hex). Pinned by
[`test-vectors/transaction/anchor-receipt-v1.json`](../../test-vectors/transaction/anchor-receipt-v1.json),
which is unchanged.

What **did** change is validity, not shape: under RFC 0005 rules (q)–(s) an
`AnchorReceipt` is accepted only if a `ComputeTask` with its `task_id` is
already committed (in prior state or earlier in the same block), its
`input_commitment` equals that task's, and its `executor` is the executor
that task named. An unbound receipt — the flow the rpc_v0.2 SDK ships — is
rejected with `-32603` at submission and would invalidate a block. That is
a consequence of RFC 0005 §5.2, visible through this RPC, and not an RPC
change.

The operational order is therefore: **commit the task, observe it in a
block, then anchor**. Same-block ordering (task first, receipt second) is
also valid.

---

## 5. Contract coverage

Every v0.2 test in `crates/mbongo-network/tests/jsonrpc_tests.rs` still
passes unchanged. The widening is pinned by these additional boundary
tests, all consuming the neutral fixture
[`test-vectors/rpc/compute-task-rpc-v1.json`](../../test-vectors/rpc/compute-task-rpc-v1.json):

| Surface | Test | What it pins |
|---|---|---|
| `submit_transaction` with `ComputeTask` | `submit_transaction_accepts_compute_task_objects_from_minimal_to_maximal` | the minimal, canonical and maximal objects deserialise to the exact protocol transactions the compute-task fixture pins (bytes, hash, `task_id`, signature); addresses are the canonical hex form; `execution_spec` bytes survive exactly; no `task_id` field on the wire |
| request size | `maximal_compute_task_request_is_far_below_the_body_limit` | the protocol-maximal task is a few kilobytes against a 2 MiB limit |
| `get_block_by_height` with `ComputeTask` | `get_block_by_height_returns_compute_task_payloads_intact` | a block with all three variants is served as the pinned object and every transaction round-trips to the protocol type; `transactions_root` is the real commitment |
| closed union | `unknown_payload_variants_are_rejected_before_the_backend` | three undefined payload forms yield `-32602` and never reach the backend |
| v0.2 shapes | `v02_transaction_shapes_are_unchanged_under_v03` | the Transfer and AnchorReceipt objects submit and round-trip unchanged; the receipt keeps its mixed byte form |
| identity | `receipt_task_id_on_the_wire_is_the_derived_task_identity` | the receipt's `task_id` array equals the identity derived from the committed task |

Admission and consensus verdicts for tasks and bound receipts are covered in
`crates/mbongo-node/src/backend.rs` (RFC 0005 rules k–s, positive and
negative, and the neutral binding vectors), not here.

---

## 6. Decisions

| | Decision |
|---|---|
| version | a new spec file, v0.3, because v0.2 §8 makes a result-shape change a versioned change; v0.2 left intact and FROZEN |
| runtime | unchanged; the JSON is serde over the protocol types, and no DTO layer is introduced |
| `ComputeTask` representation | the serde form: six fields, hex addresses, byte arrays, externally tagged |
| `task_id` | not a wire field; derived per RFC 0005 §2.2; appears only inside receipts |
| submission | the existing structured `submit_transaction`; no `submit_compute_task` (RFC 0005 §7) |
| retrieval | the existing `get_block_by_height`; no task lookup method (RFC 0005 §7, architecture: discovery is block observation) |
| errors | existing codes; `-32602` for undecodable payloads, `-32603` for admission rejection; messages not contract |
| Receipt / AnchorReceipt | shapes unchanged; validity stricter under RFC 0005 (q)–(s) |
| body limit | 2 MiB implementation limit recorded, not promised |

### 6.1 Why no DTO

A separate RPC-side task model would be a second definition of a consensus
object, with its own drift. The serde derivation on the protocol type is the
representation, its byte-encoding rule is the one v0.2 §1 already states,
and the fixture pins it. If a future version needs a different JSON form
(hex arrays, say), that is a versioned change with a DTO and conversion
tests, not a reinterpretation of this one.

### 6.2 Why no task lookup

Nothing in RFC 0005 or the Compute architecture needs one for the first
vertical: a task is discovered by observing the block that carries it
(`get_block_by_height`), and its `task_id` is derivable by anyone holding
the bytes. Adding an index method is a separate, additive RPC decision with
its own coverage; it is not required to expose the payload.

---

## 7. What an rpc_v0.2 client sees

Server and client compatibility are different questions.

**Server.** A node serving this version returns a block that contains a
`ComputeTask` normally — no panic, no dropped block, no relabelling — and
accepts a v0.2-shaped Transfer or AnchorReceipt object exactly as before.
A v0.2 client's *requests* all still work.

**Client.** The published TypeScript SDK (`@mbongo/sdk` 0.1.0, an rpc_v0.2
client) was measured against a block carrying a `ComputeTask`, using its
built distribution and the objects in the RPC fixture:

| Operation | Behaviour | Consequence |
|---|---|---|
| `submitTransaction(tx)` with a `ComputeTask` object | passes it through byte-exact; the client does not inspect `payload` | a v0.2 client **can submit** a task it constructed elsewhere |
| `receiptsInBlock(block)` | skips the `ComputeTask` payload and returns the anchored receipts | unaffected |
| `getBlockByHeight(h)` for a block containing a `ComputeTask` | **throws** `block.body.transactions[i].payload is not a known payload variant` | a v0.2 typed client **cannot read any block that contains a task** |
| anchoring a receipt with no prior task | accepted by the client, **rejected by the node** (`-32603`, rule q) | the v0.2 anchoring flow no longer completes end to end (RFC 0005 §5.2) |

So: `CURRENT_SDK_COMPUTETASK_SUPPORT = NO`,
`CURRENT_SDK_ANCHOR_FLOW_COMPATIBLE = NO`, and the break is a **validity**
and **typed-decoding** break, not a wire-format break. Workstream D widens
the payload type and the block normaliser, adds task construction and
signing, and makes anchoring carry the committed task's `task_id` and
`input_commitment`. The fixture's `old_client` block records these
observations for D.

---

## 8. Relationship to other documents

| Document | Status | Relationship |
|---|---|---|
| [rpc_v0.2.md](./rpc_v0.2.md) | FROZEN | Superseded as a description of current behaviour. Kept intact; the whole of it except §4.1's union carries over. |
| [rpc_v0.1.md](./rpc_v0.1.md) | FROZEN | Historical, per v0.2 §7. |
| [RFC 0005](../rfcs/0005-compute-task-commitment-v1.md) | Accepted | Defines the object this version represents and every rule the node applies to it. §7: no RPC method activated. |
| [PROTOCOL_LOCK_v0.3.md](./PROTOCOL_LOCK_v0.3.md) | FROZEN | Does not freeze the general RPC surface. Unchanged. The v0.4 lock will name this RPC version (§9). |
| [COMPUTE_INTERFACE_v0.1.md](./COMPUTE_INTERFACE_v0.1.md) | — | Its five reserved names stay reserved and unavailable. |
| Compute architecture ([privacy](../architecture/compute-privacy-data-plane.md), [data plane](../architecture/compute-private-data-plane-interface.md), [control plane](../architecture/compute-control-plane-worker-interface.md)) | NORMATIVE (non-consensus) | Rely only on `submit_transaction` and `get_block_by_height`, both unchanged in form. |

---

## 9. Path to FROZEN, and what the v0.4 lock will record

This document follows v0.2's own discipline: the contract is complete and
every claim has an executable test (§5), and it stays DRAFT through the
change that introduced it so the freeze cannot be hidden inside an edit
about something else. It becomes FROZEN after an independent audit derives
the contract afresh from `server.rs`, the `RpcBackend` trait, the serde
types and the tests, and finds no divergence — the same audit v0.2 §8
records.

Facts `PROTOCOL_LOCK_v0.4` will need from this version, recorded here so B
does not have to rediscover them:

- RPC version at activation: **v0.3**, one `/rpc` endpoint, no version
  routing, six methods unchanged.
- Public payload union: `None | AnchorReceipt(Receipt) | ComputeTask(ComputeTask)`,
  externally tagged; `ComputeTask` object per §4.4; byte encoding per §1.
- Compatibility: server-compatible with v0.2 requests; v0.2 typed clients
  cannot decode blocks containing tasks; the v0.2 anchoring flow is
  consensus-invalid (RFC 0005 §5.2).
- Activation implication: a v0.4 node writes storage schema 3 and serves
  blocks that v0.2 typed clients cannot read; clients upgrade with the
  node, per the fresh-genesis activation RFC 0005 §5.3 already requires.

### 9.1 The independent audit, and the freeze

The audit was run by the v0.4 lock gate (Workstream B of #126) at
`26beb2d872cd95de6777d9dd00222b98b7f1f968`, deriving the contract afresh
from `crates/mbongo-network/src/server.rs`, the `RpcBackend` trait, the
serde derivations on `mbongo-core` types, `jsonrpc_tests.rs` and the neutral
fixture, and comparing that derivation against this text:

- **Method set**: the dispatcher matches exactly `ping`, `get_block_height`,
  `submit_transaction`, `produce_block`, `get_latest_block_hash`,
  `get_block_by_height`; every other name is `-32601`. Six methods, none
  added, none removed — as §2 and §3 state.
- **Params, results, errors**: unchanged from v0.2 for all six; `-32602`
  for undecodable params and payloads, `-32603` for backend rejection, as
  §2.3 and §4.1 state.
- **Widening**: exactly one — the third payload member,
  `{"ComputeTask": {…}}`, carrying the six RFC 0005 fields under the v0.2
  byte-encoding rule (§1, §4.4); no `task_id` on the wire; the union is
  closed (§4.1). The `Receipt` and `AnchorReceipt` shapes are unchanged
  (§4.5), and `test-vectors/transaction/anchor-receipt-v1.json` is
  byte-identical to its v0.3 blob.
- **Body limit**: `server.rs` sets no explicit limit, so axum's 2 MiB
  default applies, as §1 records; the maximal task request test holds.
- **Old clients**: the §7 observations were re-checked against the
  published `@mbongo/sdk@0.1.0` source tree at `sdk-typescript-v0.1.0`,
  which has no `ComputeTask` support.
- **Coverage**: all 19 tests in `jsonrpc_tests.rs` pass, including the six
  §5 names; the three RFC 0005 fixtures were re-derived independently and
  are JSON-identical to the committed files.

No divergence between runtime, tests and this text. This document is
therefore **FROZEN**. Breaking changes — anything that would alter a
canonical parameter form, a result shape, the payload union or the public
method set — require a new RPC version.
