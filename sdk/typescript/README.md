# Mbongo Chain TypeScript SDK

`@mbongo/sdk` — a typed JSON-RPC client for the Mbongo Chain node.

## Status

**Published.** `@mbongo/sdk` is on the public npm registry. `0.2.0` is the
first release with `ComputeTask` support — task construction, `task_id`,
signing and submission, task-bearing block decoding, task-bound receipts and
executor-authorised anchoring against protocol v0.4 (`rpc_v0.3`). `0.1.0`
predates all of that: it is an `rpc_v0.2` client that cannot submit a task or
decode a block that carries one, and it must not be used with a v0.4 node for
Compute.

What this package does **not** contain: any private-data-plane, control-plane
or worker API (those are the non-consensus reference implementation in
`crates/mbongo-compute`), and no GPU, AI, metering, pricing or settlement
functionality.

**Unstable, pre-1.0.** Breaking changes are allowed until v1.0. The SDK
carries its own version and does not track the node or protocol version.

Aligned with
[`docs/specs/rpc_v0.3.md`](https://github.com/MbongoChain/mbongo-chain/blob/dev/docs/specs/rpc_v0.3.md),
which describes the RPC surface the node actually serves: the six methods of
the frozen `rpc_v0.2.md`, with the transaction payload union widened by the
`ComputeTask` variant of RFC 0005. A client built against `rpc_v0.2.md` still
sends valid requests, but cannot decode a block that carries a task; this
version can.

Earlier versions of this package targeted `docs/specs/jsonrpc_v0.1.md`, an
aspirational `mbg_*` surface the node has never implemented — every call it
made returned `-32601`. Those methods and the types that went with them
(`ValidatorData`, `TransactionStatus`, `Account`, the flattened `Block`) are
gone.

**Verifying a receipt signature proves cryptographic attribution and
integrity — not that the computation the receipt describes was performed
correctly.** Nothing in this package, or in the chain behind it, checks the
work. That distinction is load-bearing and is spelled out under
[Receipt primitives](#what-verifyreceiptsignature-proves).

## Requirements

- **Node.js `>=20.19.0`.** This is the supported runtime floor, and it comes
  from the shipped dependencies: `@noble/hashes` and `@noble/curves` both
  declare it. CI exercises the floor itself — the SDK job runs on exactly
  `20.19.0` as well as on the current Node 24 line, so the published minimum
  is a tested one. It is a support boundary rather than a claim about the
  first version the code can execute on.
- **ESM only.** The package ships `"type": "module"` with a single `.` export
  and no `require` condition, so `require("@mbongo/sdk")` fails with
  `ERR_PACKAGE_PATH_NOT_EXPORTED` even on Node versions that can otherwise
  require an ESM module.
- **No ambient `fetch` type is required to compile against this package.**
  `MbongoClientOptions.fetch` is typed by the SDK's own `MbongoFetch`, so the
  declarations need neither a DOM lib nor `@types/node`: `lib: ["ES2022"]`
  with `types: []` is enough. The platform `globalThis.fetch` still satisfies
  the contract, and any implementation matching `MbongoFetch` can be injected.
  That is a statement about typing only — the SDK does not polyfill `fetch`,
  so with none injected a real global `fetch` must exist at runtime, which
  Node `>=20.19.0` provides.

Two consumer environments are proven, both exercised in CI against the packed
tarball: a Node ESM import, and TypeScript with `module` and
`moduleResolution` set to `NodeNext`. Browsers, Deno, Bun and React Native are
not tested and are not claimed.

## Install

```bash
npm install @mbongo/sdk
```

To work on the SDK itself rather than consume it, see
[Working on the SDK](#working-on-the-sdk).

## Supported methods

All six RPC v0.3 methods, and only those:

| Client method | RPC method | Params | Result |
|---|---|---|---|
| `ping()` | `ping` | none | `"pong"` |
| `getBlockHeight()` | `get_block_height` | none | number |
| `submitTransaction(tx)` | `submit_transaction` | `Transaction` object | hash string |
| `produceBlock()` | `produce_block` | none | hash string |
| `getLatestBlockHash()` | `get_latest_block_hash` | none | hash string |
| `getBlockByHeight(n)` | `get_block_by_height` | `{"height": n}` | `{header, body}` |

## Usage

```typescript
import { MbongoClient, MbongoRpcError } from "@mbongo/sdk";

const client = new MbongoClient("http://127.0.0.1:9944/rpc");

await client.ping();                  // "pong"
await client.getBlockHeight();        // 0
const block = await client.getBlockByHeight(0);
block.header.height;                  // 0
block.body.transactions;              // []
```

## Signing: `ComputeTask` and `AnchorReceipt`

`submitTransaction` sends an **already-signed** transaction: the caller
supplies a complete `Transaction` object and the client serialises it as-is.
It signs nothing.

The two transactions this package can build and sign for you are
`ComputeTask` — see [Committing a compute task](#committing-a-compute-task) —
and `AnchorReceipt` — see [Anchoring a receipt](#anchoring-a-receipt). There
is deliberately no general `signTransaction`: a generic signer would have to
encode arbitrary `u128` amounts, and this package caps `amount` at `u64::MAX`
for the read-path reason described under
[Numeric range](#numeric-range-exact-integers). Both supported transactions
sidestep the question entirely, because consensus pins their amount to `0`.

For any other transaction type the caller still supplies a signed object:

The node expects a structured JSON object, not the historical
`[signed_tx_hex]` form, and byte fields cross the wire as `0x` hex strings:

```typescript
await client.submitTransaction({
  tx_type: "Transfer",
  sender: "0xe734…",     // 32 bytes
  receiver: "0x2222…",   // 32 bytes
  amount: 100,
  nonce: 0,
  payload: "None",
  signature: "0x1c37…",  // 64 bytes, over the SCALE signing payload
});
```

To produce one today, see
`cargo run -p mbongo-wallet --example sign_tx`.

## Compute RPC methods: not wrapped

There is no compute client and no lookup by `task_id` here. The five reserved
compute RPC methods and `submit_receipt` / `get_receipt` are **unavailable on
the node** and return `-32601`; wrapping them would only wrap an error. A
compute task travels through the ordinary `submit_transaction` and is read
back out of a block with `getBlockByHeight` — see
[Committing a compute task](#committing-a-compute-task).

## Committing a compute task

A `ComputeTask` commits a question to the chain: who asks (`submitter`), who
alone may answer (`executor`), a commitment to the input, and an opaque
specification. The input, the work and the result stay off-chain. Consensus
rules are RFC 0005; the wire form is `rpc_v0.3` §4.4.

```typescript
import {
  computeTaskId,
  signComputeTaskTransaction,
  submitComputeTask,
  computeTasksInBlock,
  MbongoComputeTaskError,
} from "@mbongo/sdk";

const task = {
  version: 1,
  submitter: submitterPublicKey,      // Uint8Array(32)
  executor: executorPublicKey,        // Uint8Array(32): the one party allowed to answer
  salt,                               // Uint8Array(32), your choice; zero is legal
  inputCommitment,                    // Uint8Array(32), opaque to the chain
  executionSpec,                      // Uint8Array, at most 1024 bytes, never interpreted
};

const taskId = computeTaskId(task);                              // Uint8Array(32)
const tx = signComputeTaskTransaction(task, nonce, submitterSecretKey);
const txHash = await submitComputeTask(client, tx);

// Later, once the transaction is in a block you know the height of:
const block = await client.getBlockByHeight(height);
const committed = computeTasksInBlock(block);                    // ComputeTask[]
```

| Function | Returns |
|---|---|
| `encodeComputeTask(task)` | the canonical SCALE bytes of the six fields |
| `computeTaskIdPreimage(task)` | the raw 22-byte domain tag followed by those bytes |
| `computeTaskId(task)` | `BLAKE3` of the preimage, 32 bytes |
| `computeTaskSigningPayload(task, nonce)` | the bytes that get signed |
| `signComputeTaskTransaction(task, nonce, secretKey)` | a signed transaction |
| `computeTaskTransactionHash(tx)` | `BLAKE3` of the full signed encoding |
| `computeTaskToWire(task)`, `computeTaskTransactionToWire(tx)` | the JSON objects the node expects |
| `wireComputeTaskToComputeTask(wire)` | the canonical task from its wire form |
| `submitComputeTask(client, tx)` | the transaction hash the node reports |
| `computeTasksInBlock(block)` | the tasks committed in a block you already fetched |

### What is fixed, and what you choose

`sender` is the task's `submitter` (rule o), `receiver` is the zero address
and `amount` is `0` (rule l); consensus requires all three, so none is a
parameter. The secret key must derive `submitter`, or signing fails rather
than producing a transaction the node is guaranteed to refuse. **`nonce` is
the only transaction field you choose**, exactly as for anchoring.

`executor` is chosen by you, before the task is committed, and it is part of
the task's identity: the same work asked of a different executor is a
**different task**, and nothing on chain ever reassigns it. Only that
executor's receipt will be accepted.

### `task_id` is derived, never sent

`task_id = BLAKE3("mbongo:compute-task:v1" || SCALE(task))`, with the 22-byte
tag prepended raw. It is not a field of the task and does not appear in the
transaction; a receipt carries it. Changing any field — including `salt`,
which exists so you can ask the same executor the same thing twice — changes
it. The transaction nonce does not, so a resubmission after a nonce race is
the same task.

### Bytes are bytes

`salt`, `inputCommitment` and `executionSpec` cross the wire as arrays of
numbers and are preserved exactly: nothing is decoded as text, normalised or
truncated. `executionSpec` above 1024 bytes throws before anything is
encoded. Whatever you put in `executionSpec` is **public chain data**; the
chain does not interpret it, and this package does not redact it. Keep
private material off-chain, behind `inputCommitment`.

`inputCommitment` is opaque 32 bytes. How you derive it — a plain hash of the
input, or a blinded one — is between you and your executor; the chain only
compares it for equality with the receipt's. This package provides no input
hashing helper, on purpose: RFC 0005 defines no application canonicalisation,
and a helper here would look like one.

### Observing a task

There is no lookup by `task_id`. A committed task is visible in the block
that carries it, which is how an executor learns it was asked. Record the
height at submission time, or scan forward with `getBlockByHeight`.

### The receipt that answers it

Since RFC 0005 a receipt is accepted only if it answers a committed task:
its `taskId` is the task's derived identity, its `inputCommitment` is the
task's, and its `executor` is the executor the task named — and it must be
anchored by that executor. See
[Anchoring a receipt](#anchoring-a-receipt) for `signBoundReceipt`, which
derives all three from the task so they cannot be supplied wrong.

Offline receipt primitives — encoding, hashing and signature verification —
**are** included; see [Receipt primitives](#receipt-primitives). So is
anchoring a receipt through the generic `submit_transaction`; see
[Anchoring a receipt](#anchoring-a-receipt).

Blocks containing anchored receipts decode through the RPC types, whose
receipt body is typed `WireReceipt`: those types model the JSON wire shape,
while the receipt primitives work in canonical bytes. The two are deliberately
separate, and `anchorReceiptTransactionToWire` is the boundary between them.

## Errors

Two error classes, kept apart on purpose:

- **`MbongoRpcError`** — the node answered with a JSON-RPC error object.
  `code`, `message` and `data` are preserved.
- **`MbongoTransportError`** — no usable response: unreachable host,
  unsuccessful HTTP status with an unreadable body, or a body that is not a
  JSON-RPC 2.0 object.

`-32601` means **the method is unavailable**, never that a resource was not
found. `err.isMethodUnavailable` reads it correctly; do not translate it into
a domain-level absence.

```typescript
try {
  await client.getBlockByHeight(99999);
} catch (err) {
  if (err instanceof MbongoRpcError) {
    err.code;                 // -32603 when no block exists at that height
    err.isMethodUnavailable;  // false — the method exists
  }
}
```

## `getBlockByHeight` sends the canonical form

The client always sends `{"height": N}`. The node also tolerates a bare
number, but that is an implementation detail of the current runtime rather
than contract, so this client never emits it.

## Numeric range: exact integers

The four RPC fields whose Rust type is wider than a JavaScript number are
carried as `bigint`: `Transaction.amount`, `Transaction.nonce`,
`BlockHeader.height` and `BlockHeader.timestamp`, along with the
`get_block_height` result and the `getBlockByHeight` argument.

**Input** accepts either form. A `bigint` is taken as-is; a `number` is
accepted while it is still a safe non-negative integer, which converts
exactly. So existing code keeps working:

```typescript
await client.submitTransaction({ ...tx, amount: 100, nonce: 0 });
await client.submitTransaction({ ...tx, amount: 100n, nonce: 9007199254740993n });
```

**Output** is always `bigint`, including for small values — a type that
changed with the magnitude would need a check at every call site:

```typescript
const height = await client.getBlockHeight();   // bigint
const block = await client.getBlockByHeight(height);
block.header.timestamp;                          // bigint
block.body.transactions[0]?.amount;              // bigint
```

Smaller fields stay `number`, because their domains fit: `receipt.version`,
the byte-array elements, JSON-RPC error codes and request ids.

### An unsafe number is refused, not repaired

```typescript
await client.submitTransaction({ ...tx, amount: 9007199254740993 });
// MbongoNumericRangeError: transaction.amount: exceeds the JavaScript
// safe-integer range (max 9007199254740991)
```

That literal is already `9007199254740992` before this package sees it —
JavaScript rounded it while parsing the source. The intent cannot be
recovered, so the SDK rejects it rather than converting it to a `bigint` that
would merely look precise. Pass a `bigint` when you need the full range.

### `amount` currently stops at `u64::MAX`

Rust's `Transaction.amount` is a `u128` and the node accepts that whole
domain on submission, but `get_block_by_height` serialises blocks through
`serde_json::to_value`, which fails above `u64::MAX`. An amount past that
bound could be submitted and included, and the block holding it would then be
unreadable. The SDK refuses such a value rather than helping produce a block
the chain cannot serve back.

**Full `u128` amounts are not supported.** Lifting the cap is a node-side
change, tracked separately.

### How this works, and what it does not fix

`rpc_v0.2.md` represents these fields as JSON numbers and fixes no magnitude.
A JSON number token is lexically unbounded, so the node already emits and
accepts exact decimal digits — the precision was never lost on the wire. It
was lost in JavaScript: `JSON.parse` rounds every token to a double, and
`JSON.stringify` cannot serialise a `bigint` at all.

This package replaces both on the RPC path with an exact parser and
serialiser. Integers remain JSON numbers, never strings; method names,
parameter shapes and response shapes are unchanged. `rpc_v0.2.md` stays
FROZEN and needs no version bump.

That fixes this client, and only this client. **A JavaScript program calling
the node with `fetch` and native `JSON.parse` still loses integers above
2^53 − 1**, silently. The transport preserves the exact decimal token; the
native parser does not. Use `@mbongo/sdk`, or an exact-number JSON parser.

## Receipt primitives

Offline, synchronous, pure. Nothing here touches the network.

```typescript
import {
  encodeReceiptSigningPayload,
  encodeReceipt,
  receiptHash,
  verifyReceiptSignature,
} from "@mbongo/sdk";

const hash = receiptHash(receipt);          // Uint8Array(32)
const ok   = verifyReceiptSignature(receipt); // boolean
```

| Function | Returns |
|---|---|
| `encodeReceiptSigningPayload(r)` | SCALE of fields 1–6, signature excluded |
| `encodeReceipt(r)` | signing payload followed by the 64-byte signature |
| `receiptHash(r)` | `BLAKE3` of the signing payload, 32 bytes |
| `verifyReceiptSignature(r)` | executor signature over the **raw** hash |

### Fields are bytes, not hex

```typescript
interface Receipt {
  version: number;            // must be 1
  taskId: Uint8Array;         // 32
  inputCommitment: Uint8Array;  // 32
  outputCommitment: Uint8Array; // 32
  executor: Uint8Array;       // 32, Ed25519 public key
  metadata: Uint8Array;       // at most 4096
  signature: Uint8Array;      // 64
}
```

The RPC types carry hex because that is their wire form. Receipt fields are
`Uint8Array` because they are hashed and signed, and carrying them as text
invites signing the text instead of the bytes.

None of these functions mutate the arrays you pass them.

### What `verifyReceiptSignature` proves

That the receipt is structurally canonical, its version is supported, its
metadata is within bound, and the key in `executor` signed **this exact
receipt**.

It does **not** prove that the computation was performed correctly, that the
receipt is anchored on chain, that the task exists, that the executor was
authorised to run it, or that anything was settled. The chain itself
validates structure, signature and uniqueness — and nothing about the work.
The name is deliberately narrow for that reason.

### Fail closed

- **Version 1 only.** Any other version throws rather than being hashed as
  though understood.
- **Metadata over 4096 bytes throws**, before any encoding or hashing. The
  bound is normative through RFC 0002 §3 and frozen by `PROTOCOL_LOCK_v0.3`,
  though `RECEIPT_SPEC_v0.1` omits it. Producing a canonical-looking hash for
  a receipt consensus can never anchor would be the worst possible output,
  because it looks right.
- **Wrong field widths throw.** TypeScript types do not survive to runtime,
  so widths are checked there.

A structurally sound receipt whose signature simply does not match is not an
error: `verifyReceiptSignature` returns `false`. `MbongoReceiptError` is for
receipts that cannot be canonically encoded at all.

### Correctness

These primitives are checked against `test-vectors/receipt/receipt-v1.json`,
the shared fixture the Rust node also reads. No expected value is duplicated
in TypeScript — a copied constant would only prove the copy was faithful.

The fixture's five valid vectors sit on the SCALE compact-length boundaries
that matter: at 4096 bytes of metadata, the consensus maximum, the length
prefix is **two** bytes, not one.

### Not included here

These four functions are offline only. Building, signing and submitting an
anchoring transaction is a separate surface — see
[Anchoring a receipt](#anchoring-a-receipt), and reading receipts back out of
a block is [Reading receipts back](#reading-receipts-back).

## Anchoring a receipt

Anchoring puts a signed receipt inside a transaction that is itself signed,
and submits it through the ordinary `submit_transaction` method. No new RPC is
involved.

**A receipt must answer a committed task.** Under RFC 0005 rules (q)–(s) the
node rejects a receipt whose `taskId` is not a committed task's identity,
whose `inputCommitment` is not that task's, or whose `executor` is not the
executor that task named. The unbound flow earlier versions of this package
documented — a receipt with any `taskId` — is no longer accepted by the node.

The executor-side flow, once the task is visible in a block and the work is
done off-chain:

```typescript
import {
  signBoundReceipt,
  signAnchorReceiptTransaction,
  submitAnchorReceipt,
  MbongoAnchorError,
} from "@mbongo/sdk";

// task: the ComputeTask you read out of the block with computeTasksInBlock.
const receipt = signBoundReceipt(
  task,
  { outputCommitment, metadata },   // outputCommitment: Uint8Array(32), yours to assert
  executorSecretKey,                // must derive task.executor
);
const tx = signAnchorReceiptTransaction(receipt, nonce, executorSecretKey);

try {
  const txHash = await submitAnchorReceipt(client, tx);
} catch (err) {
  if (err instanceof MbongoAnchorError) {
    err.reason;             // "duplicate-task-id", "task-not-registered", "executor-not-authorised", …
    err.isDuplicateTaskId;
  }
}
```

`signBoundReceipt` derives `taskId`, `inputCommitment` and `executor` from
the task rather than accepting them, so a receipt it builds cannot fail the
binding rules by construction. It refuses any key that does not derive
`task.executor` — the submitter's included, when the submitter is not the
executor. `assertReceiptBoundToTask(receipt, task)` performs the same three
checks on a receipt built any other way; it throws a
`MbongoReceiptBindingError` naming the first binding that fails.

| Function | Returns |
|---|---|
| `signBoundReceipt(task, { outputCommitment, metadata? }, executorSecretKey)` | a signed receipt bound to the task |
| `assertReceiptBoundToTask(receipt, task)` | nothing; throws if any binding fails |
| `anchorReceiptSigningPayload(receipt, nonce)` | the bytes that get signed |
| `signAnchorReceiptTransaction(receipt, nonce, secretKey)` | a signed transaction |
| `anchorReceiptTransactionHash(tx)` | `BLAKE3` of the full signed encoding |
| `anchorReceiptTransactionToWire(tx)` | the JSON object the node expects |
| `submitAnchorReceipt(client, tx)` | the transaction hash the node reports |

The low-level receipt and anchoring functions are unchanged and still
describe any receipt: they are the encoding, not the policy. A receipt built
by hand can be anchored only if it happens to satisfy the binding; the node,
not this package, is the authority on that.

### Two signatures, one key

Consensus requires `tx.sender == receipt.executor`, so the same Ed25519 key
produces both signatures. They are **different signatures**, because the
messages differ:

| Signature | Key | Message |
|---|---|---|
| `receipt.signature` | executor | the raw 32 bytes of `receiptHash(receipt)` |
| transaction signature | sender | the **raw** transaction signing payload |

The transaction signature has **no prehash**. It is over the payload bytes
themselves, never over a digest of them — applying the receipt's
hash-then-sign pattern here produces a transaction the node rejects.

Three values are easy to confuse and are not interchangeable:

| | Covers | Hashed? |
|---|---|---|
| `receiptHash(receipt)` | the receipt, signature excluded | yes |
| the transaction signing payload | the whole transaction, signature excluded | **no** |
| `anchorReceiptTransactionHash(tx)` | the whole transaction, signature **included** | yes |

The last one is what `submit_transaction` returns, so you can check the node
answered about the transaction you actually signed.

### What is fixed, and what you choose

`sender` is derived from `receipt.executor` rather than accepted as an
argument, so the two cannot contradict each other. `receiver` is the zero
address and `amount` is `0`; consensus requires both, so neither is a
parameter. **`nonce` is the only field you choose.**

The secret key is a 32-byte Ed25519 seed, used once and discarded. If it does
not derive `receipt.executor`, signing fails immediately rather than producing
a transaction that could never be anchored. This package has no key storage,
no derivation, no mnemonics and no keystore.

### You supply the nonce

`nonce` must equal the sender account's current nonce. **This package does not
fetch it**, and does not assume `0`: JSON-RPC v0.2 exposes no account method.
The account lookup lives on the REST surface, which this client does not
model. A freshly generated key has no account at all and cannot anchor.

### Retrying

Before the task is anchored, re-submitting the **identical signed
transaction** is safe — same receipt, same nonce, therefore the same bytes,
and the node treats an unanchored duplicate as idempotent.

Once the `task_id` is anchored, any further submission is rejected as
`duplicate-task-id`. That reason **cannot tell you whether you anchored it or
someone else did**. Nothing in the response distinguishes the two, and there
is no public query API that would. Record the transaction hash and block
height at submission time if you need to know.

### What anchoring does not mean

A returned hash means the node accepted the transaction into its mempool. It
does not mean the transaction is in a block, that the receipt is anchored, or
that the computation the receipt describes was performed correctly. The chain
validates structure, signature, uniqueness and — since RFC 0005 — that the
receipt answers a committed task from the executor it named. It validates
nothing about the work: an anchored receipt is a **bound claim** by that
executor, not a verified result.

Reading anchored receipts back out of a block you already identified is
[below](#reading-receipts-back).

## Reading receipts back

Two pure, offline functions. Neither touches the network, and neither takes a
client.

```typescript
import { receiptsInBlock, verifyReceiptSignature } from "@mbongo/sdk";

const block    = await client.getBlockByHeight(knownHeight);  // 1 RPC call
const receipts = receiptsInBlock(block);                      // 0 calls
```

| Function | Returns |
|---|---|
| `receiptsInBlock(block)` | the canonical receipts anchored in that block, in transaction order |
| `wireReceiptToReceipt(wire)` | one wire receipt converted to canonical bytes |

A block may also carry `ComputeTask` transactions; `receiptsInBlock` ignores
them, and `computeTasksInBlock(block)` reads them the same way.

### Known height only

There is no `task_id` to height index anywhere in the chain, so this works
only when you already know the height — because you recorded it when you
anchored. Nothing here discovers a height, scans the chain, or looks a receipt
up by `task_id`.

`receiptsInBlock` takes a `Block` rather than a height on purpose: a function
accepting a bare `task_id` would read as a chain-side lookup, and there is
none. Filtering within a block you already have is one line and deliberately
not an API:

```typescript
const mine = receipts.find((r) => r.taskId.every((b, i) => b === taskId[i]));
```

### 0, 1 or many

A block may anchor any number of receipts — consensus only forbids repeating
one `task_id` within a block — so the result is always an array, in
transaction order. An empty array means the block anchored nothing, which is
ordinary and not an error.

### Decoding is not verification

Receipts come back decoded, not verified. The block passed the node's
consensus validation, which already checked each one — but this package
checked nothing:

```typescript
const verified = receipts.filter(verifyReceiptSignature);
```

### Fail closed

A transaction claiming to carry a receipt whose payload cannot be decoded
**throws** `MbongoReceiptError` naming the offending transaction, rather than
being skipped. Version, the four fixed widths, the 4096-byte metadata bound,
`0x` lowercase hex and every number-array element in `0..=255` are all
checked — a byte outside that range would be silently truncated into a receipt
whose hash no longer matches the chain's.

### Whole-block validation

`getBlockByHeight` normalises the **whole** block, including every
transaction's `amount` and `nonce`. Integers arrive exactly, as `bigint`; a
structurally malformed block is refused rather than partially returned.

## Working on the SDK

From a clone of the repository:

```bash
cd sdk/typescript
npm ci
npm run typecheck   # tsc --noEmit
npm test            # builds, then the wire-contract suite
npm run build       # tsc -> dist/
npm run test:consumer
```

`npm test` runs Node's built-in test runner against `dist/` — the same
artifact a consumer installs. The tests assert the JSON actually put on the
wire, so a wrong method string, a stray parameter or a reintroduced legacy
form fails the suite. No test framework dependency is added.

`npm run test:consumer` packs the package, installs the resulting tarball into
a throwaway project outside the repository, and imports `@mbongo/sdk` by name
from both JavaScript and TypeScript. It runs in CI on every change, so a
`files` or `exports` change that would make the package uninstallable fails
there rather than after a release. It proves the artifact is installable; it
is not publication provenance.
