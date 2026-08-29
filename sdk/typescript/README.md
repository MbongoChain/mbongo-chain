# Mbongo Chain TypeScript SDK

`@mbongo/sdk` — a typed JSON-RPC client for the Mbongo Chain node.

## Status

**Unstable, pre-1.0.** Breaking changes are allowed until v1.0.

Aligned with [`docs/specs/rpc_v0.2.md`](../../docs/specs/rpc_v0.2.md)
(**FROZEN**), which describes the RPC surface the node actually serves.

Earlier versions of this package targeted `docs/specs/jsonrpc_v0.1.md`, an
aspirational `mbg_*` surface the node has never implemented — every call it
made returned `-32601`. Those methods and the types that went with them
(`ValidatorData`, `TransactionStatus`, `Account`, the flattened `Block`) are
gone.

## Install and build

```bash
npm install
npm run build      # tsc -> dist/
npm run typecheck  # tsc --noEmit
npm test           # builds, then runs the wire-contract tests
```

## Supported methods

All six RPC v0.2 methods, and only those:

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

## Signing: only `AnchorReceipt`

`submitTransaction` sends an **already-signed** transaction: the caller
supplies a complete `Transaction` object and the client serialises it as-is.
It signs nothing.

The one transaction this package can build and sign for you is
`AnchorReceipt` — see [Anchoring a receipt](#anchoring-a-receipt). There is
deliberately no general `signTransaction`: a generic signer would have to
encode arbitrary `u128` amounts, and this package refuses to vouch for values
outside the JavaScript safe-integer range. `AnchorReceipt` sidesteps that
entirely, because consensus pins its amount to `0`.

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

## Compute helpers: not included

There is no compute client and no receipt **query** here. The five reserved
compute RPC methods and `submit_receipt` / `get_receipt` are **unavailable on
the node** and return `-32601`; wrapping them would only wrap an error.

Offline receipt primitives — encoding, hashing and signature verification —
**are** included; see [Receipt primitives](#receipt-primitives). So is
anchoring a receipt through the generic `submit_transaction`; see
[Anchoring a receipt](#anchoring-a-receipt).

Blocks containing anchored receipts decode through the RPC types with the
receipt body typed `unknown`: those types model the JSON wire shape, while
the receipt primitives work in canonical bytes. The two are deliberately
separate.

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

## Numeric range: the SDK accepts only safe integers

`@mbongo/sdk` 0.1 supports integer values in **`0 .. 2^53 - 1`** for the RPC
fields carried as JSON numbers: `Transaction.amount`, `Transaction.nonce`,
`BlockHeader.height`, `BlockHeader.timestamp`, the `get_block_height` result
and the `getBlockByHeight` argument.

Values outside that range are **rejected**, with `MbongoNumericRangeError`
naming the field:

```typescript
await client.submitTransaction({ ...tx, amount: 9007199254740992 });
// MbongoNumericRangeError: transaction.amount: exceeds the JavaScript
// safe-integer range (max 9007199254740991)
```

Outbound values are checked **before any network call**. Inbound values are
checked before being returned, including every transaction inside a block
body.

### Why

`rpc_v0.2.md` represents these fields as JSON numbers, and the Rust types
behind them are wider than JavaScript can hold exactly — `amount` is a
`u128`, the rest are `u64`. JavaScript is integer-exact only through
`Number.MAX_SAFE_INTEGER`, so a larger literal is rounded **when JavaScript
parses it**, before this package ever sees the value. The original cannot be
recovered.

What can be detected is that the value in hand is not a safe integer. The SDK
fails closed on that: it will not transmit a value it cannot vouch for, and
will not hand one back as though it were trustworthy. A rounded `amount`
would otherwise be signed for and settled as a different number than
intended.

### What this is not

This is an **SDK restriction**, not a protocol rule. The node accepts the
full Rust domain, and `rpc_v0.2.md` is unchanged and remains FROZEN. Nothing
here narrows the protocol; it narrows what this client is willing to vouch
for.

Supporting the full range across languages would need a different wire
representation and therefore a versioned RPC decision. No such format has
been selected.

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

### Not included

No transaction construction, no signing, no submission, no receipt query. The
package exposes no private-key API at all.

## Anchoring a receipt

Anchoring puts a signed receipt inside a transaction that is itself signed,
and submits it through the ordinary `submit_transaction` method. No new RPC is
involved.

```typescript
import {
  signAnchorReceiptTransaction,
  submitAnchorReceipt,
  MbongoAnchorError,
} from "@mbongo/sdk";

const tx = signAnchorReceiptTransaction(receipt, nonce, executorSecretKey);

try {
  const txHash = await submitAnchorReceipt(client, tx);
} catch (err) {
  if (err instanceof MbongoAnchorError) {
    err.reason;             // "duplicate-task-id", "invalid-nonce", …
    err.isDuplicateTaskId;
  }
}
```

| Function | Returns |
|---|---|
| `anchorReceiptSigningPayload(receipt, nonce)` | the bytes that get signed |
| `signAnchorReceiptTransaction(receipt, nonce, secretKey)` | a signed transaction |
| `anchorReceiptTransactionHash(tx)` | `BLAKE3` of the full signed encoding |
| `anchorReceiptTransactionToWire(tx)` | the JSON object the node expects |
| `submitAnchorReceipt(client, tx)` | the transaction hash the node reports |

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
validates structure, signature and uniqueness — and nothing about the work.

Reading an anchored receipt back is not part of this package.

## Tests

`npm test` builds the package and runs Node's built-in test runner against
`dist/` — the same artifact a consumer installs. The tests assert the JSON
actually put on the wire, so a wrong method string, a stray parameter or a
reintroduced legacy form fails the suite. No test framework dependency is
added.
