# Working with compute receipts

> **Document type:** Developer guide
> **Audience:** anyone building against the TypeScript SDK or the node's RPC
> **Architecture:** [compute-receipts.md](../architecture/compute-receipts.md)

How to build, sign, anchor and verify a compute receipt with the shipped
tooling. Everything here uses the API on `dev` today; nothing is aspirational.

For *why* the pieces are shaped this way — the two signature domains, the
validation layers, the storage rules — read the architecture document first.
This guide assumes it.

---

## 1. Install

```bash
cd sdk/typescript
npm ci
npm run build
```

The package has two runtime dependencies, `@noble/hashes` and `@noble/curves`,
both pinned exactly. BLAKE3 is in neither Node's `crypto` nor WebCrypto, so a
dependency was unavoidable.

---

## 2. The API

### Receipt primitives — offline, synchronous, pure

| Function | Returns |
|---|---|
| `encodeReceiptSigningPayload(receipt)` | SCALE of fields 1–6, signature excluded |
| `encodeReceipt(receipt)` | signing payload followed by the 64-byte signature |
| `receiptHash(receipt)` | `BLAKE3` of the signing payload, 32 bytes |
| `verifyReceiptSignature(receipt)` | `boolean` — the executor signature over the raw hash |

### Anchoring

| Function | Returns |
|---|---|
| `anchorReceiptSigningPayload(receipt, nonce)` | the bytes that get signed |
| `signAnchorReceiptTransaction(receipt, nonce, secretKey)` | a signed transaction |
| `anchorReceiptTransactionHash(tx)` | `BLAKE3` of the full signed encoding |
| `anchorReceiptTransactionToWire(tx)` | the JSON object the node expects |
| `submitAnchorReceipt(client, tx)` | the transaction hash the node reports |

Plus `MbongoAnchorError`, `MAX_RECEIPT_METADATA_BYTES`, `RECEIPT_VERSION` and
`ANCHOR_RECEIPT_PAYLOAD_PREFIX_BYTES`.

Receipt fields are `Uint8Array`, not hex. These values are hashed and signed,
and carrying them as text invites signing the text instead of the bytes.

---

## 3. Building a receipt

```typescript
import { receiptHash, verifyReceiptSignature } from "@mbongo/sdk";

const receipt = {
  version: 1,
  taskId,             // Uint8Array(32)
  inputCommitment,    // Uint8Array(32)
  outputCommitment,   // Uint8Array(32)
  executor,           // Uint8Array(32) — the executor's Ed25519 public key
  metadata,           // Uint8Array, at most 4096 bytes
  signature,          // Uint8Array(64)
};
```

The `signature` is the executor's, over **the raw 32 bytes of
`receiptHash(receipt)`** — not over its hex text, and not over the encoding.

The SDK deliberately exposes **no receipt-signing function** and no private-key
API for receipts. Producing that signature is the executor's job, in whatever
environment holds the key.

Verify one you were given before doing anything else:

```typescript
if (!verifyReceiptSignature(receipt)) throw new Error("bad receipt");
```

A `true` result means the receipt is canonical and the key in `executor` signed
this exact receipt. It says nothing about whether the computation happened.

---

## 4. Getting a nonce

`nonce` must equal the sender account's current nonce. **The SDK does not fetch
it**, and `0` is not a safe default — a freshly generated key has no account at
all and cannot anchor.

JSON-RPC v0.2 exposes no account method. The node's REST surface has one:

```
GET /accounts/:address  ->  { address, balance, nonce }
```

Read it immediately before constructing the transaction. On the Docker devnet
the REST port is not published to the host, so query it from inside a
container:

```bash
docker exec mbongo-devnet-producer curl -s http://127.0.0.1:8080/accounts/0x<address>
```

---

## 5. Signing and submitting

```typescript
import {
  MbongoClient,
  signAnchorReceiptTransaction,
  anchorReceiptTransactionHash,
  submitAnchorReceipt,
  MbongoAnchorError,
} from "@mbongo/sdk";

const client = new MbongoClient("http://127.0.0.1:9944/rpc");
const tx = signAnchorReceiptTransaction(receipt, nonce, executorSecretKey);

try {
  const txHash = await submitAnchorReceipt(client, tx);
  // The node returns the same hash the SDK computes locally:
  //   txHash === "0x" + hex(anchorReceiptTransactionHash(tx))
} catch (err) {
  if (err instanceof MbongoAnchorError) {
    err.reason;             // see §6
    err.isDuplicateTaskId;
  }
}
```

`secretKey` is a 32-byte Ed25519 seed. It is used once and discarded: nothing
is cached, stored or derived from it, and your array is not mutated. If it does
not derive `receipt.executor`, signing throws immediately rather than producing
a transaction consensus would refuse.

**You do not choose** `sender`, `receiver` or `amount`. `sender` is derived from
`receipt.executor`, `receiver` is the zero address and `amount` is `0` —
consensus requires all three. `nonce` is the only field you supply, and an
unsafe one (negative, fractional, `NaN`, at or above 2^53) throws **before**
anything is signed.

`submitAnchorReceipt` composes with the client's existing `submitTransaction`.
It opens no connection of its own and adds no RPC method.

### Success means mempool admission

A returned hash means the node accepted the transaction into its mempool. It
does not mean the transaction is in a block, and it certainly does not mean the
computation was correct.

---

## 6. Errors

`MbongoAnchorError` carries a `reason`:

| `reason` | Node rejected because |
|---|---|
| `duplicate-task-id` | the `task_id` is already anchored |
| `task-id-pending` | another transaction for that `task_id` is in the mempool |
| `metadata-too-large` | metadata exceeds 4096 bytes |
| `unsupported-receipt-version` | receipt version is not 1 |
| `sender-executor-mismatch` | `sender != receipt.executor` |
| `invalid-receipt-signature` | the executor signature does not verify |
| `invalid-transaction-signature` | the transaction signature does not verify |
| `invalid-nonce` | the nonce does not match account state |
| `invalid-anchor-fields` | non-zero amount or receiver, or a payload/type mismatch |
| `sender-account-unusable` | the sender account does not exist or lacks balance |
| `unknown` | a rejection this package does not recognise |

**Stability caveat.** The node answers `-32603` with a message for every
anchoring rule; there are no structured per-rule error codes. `reason` is
therefore derived by matching the message text. It works, and the devnet
harness classifies the same way, but it is only as stable as those strings.
Treat `unknown` as a rejection, never as success.

Errors that are not anchoring rejections pass through unchanged as
`MbongoRpcError` or `MbongoTransportError`.

---

## 7. Retrying

Before the `task_id` is anchored, re-submitting the **identical signed
transaction** is safe: the same receipt and nonce produce the same bytes, and
the node treats an unanchored duplicate as idempotent.

Once it is anchored, any further submission is rejected as
`duplicate-task-id` — and that reason **cannot tell you whether you anchored it
or someone else did**. Nothing in the response distinguishes the two, and no
public query API exists. If you need to know, record the transaction hash and
the block height at submission time.

---

## 8. Reading a receipt back

There is no `getReceipt(taskId)`, and there will not be a naive one: no
`task_id → height` index exists. Retrieval from a **known** height is
[#86](https://github.com/MbongoChain/mbongo-chain/issues/86) and is not
implemented.

Today you scan a block you already identified:

```typescript
const block = await client.getBlockByHeight(height);
for (const tx of block.body.transactions) {
  if (tx.payload !== "None" && tx.payload.AnchorReceipt) {
    tx.payload.AnchorReceipt.task_id;   // number[], not hex
    tx.payload.AnchorReceipt.executor;  // "0x…" string
  }
}
```

That mixed representation is not a mistake in your code. Within one receipt
object, `task_id`, `input_commitment`, `output_commitment` and `metadata` are
**arrays of numbers**, while `executor` and `signature` are `0x` hex strings.
The `WireReceipt` type models it exactly. See
[architecture §7.2](../architecture/compute-receipts.md#72-json-representation-of-a-nested-receipt).

---

## 9. Testing

### Against the shared fixtures

Both languages read the same two files, and no expected value is duplicated in
either:

```bash
cargo test -p mbongo-core --test receipt_vectors
cargo test -p mbongo-core --test transaction_vectors
```

```bash
cd sdk/typescript && npm test
```

If you change encoding, hashing or signing on either side, the fixtures are
what catch it. Do not "fix" a fixture to match new output — the values were
derived from the protocol rules, not from an implementation, and that is the
whole point.

### Against a live devnet

```bash
make devnet-up      # or: ./scripts/devnet/docker-devnet.sh up
make devnet-down
```

Three nodes, deterministic, with the dev account pre-funded at genesis. The
RPC endpoint is published on `127.0.0.1:29944` by default
(`MBONGO_HOST_RPC_PORT` in `.env.base`); override it in an untracked
`.env.local`.

This path is verified end to end: a transaction constructed and signed by the
TypeScript SDK was accepted by a real node, the node returned the same
transaction hash the SDK computes, the receipt appeared in a block, and a
duplicate submission came back as a typed `duplicate-task-id`.

### A Rust reference

To see the exact JSON-RPC request body for a signed anchoring transaction:

```bash
cargo run -p mbongo-wallet --example submit_receipt -- --nonce 0 --task-id <64 hex chars>
```

It uses the public devnet key and prints a warning saying so. Never a
production key.

---

## 10. What this package will not do for you

- discover a nonce
- store, derive or manage keys — no mnemonics, no HD derivation, no keystore
- sign a receipt
- sign a non-`AnchorReceipt` transaction — full-range `u128` support is
  [#91](https://github.com/MbongoChain/mbongo-chain/issues/91)
- look up a receipt by `task_id` or by `receipt_hash`
- tell you the computation was correct

---

## See also

- [Compute receipts and receipt anchoring](../architecture/compute-receipts.md)
- [`sdk/typescript/README.md`](../../sdk/typescript/README.md) — package reference
- [`rpc_v0.2.md`](../specs/rpc_v0.2.md) — the RPC surface (FROZEN)
- [devnet.md](devnet.md) — devnet operations
