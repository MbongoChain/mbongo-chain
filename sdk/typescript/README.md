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

## Signing: not included

`submitTransaction` sends an **already-signed** transaction. This package
does not sign anything — no SCALE encoding, no BLAKE3, no Ed25519. The caller
supplies a complete `Transaction` object and the client serialises it as-is.

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

## Receipt and compute helpers: not included

There is no `Receipt` type, no `receipt_hash`, no `verifyReceipt`, no
`AnchorReceipt` construction and no compute client here. The five reserved
compute RPC methods and `submit_receipt` / `get_receipt` are **unavailable on
the node** and return `-32601`; wrapping them would only wrap an error.

Blocks containing anchored receipts still decode: the receipt body inside
`TransactionPayload` is typed `unknown`, so the wire shape is modelled
without implementing receipt semantics.

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

## Numeric precision

`Transaction.amount` is a `u128` and `nonce`, `height` and `timestamp` are
`u64`, all specified as JSON numbers by the frozen RPC contract. JavaScript
numbers are exact only to `Number.MAX_SAFE_INTEGER` (2^53 − 1), so an
`amount` above that bound cannot round-trip through these types.

This is a property of the wire contract, and it is not papered over here: the
types describe the actual JSON. Heights and timestamps are unaffected in
practice; `amount` is the field to watch if large denominations are ever
used.

## Tests

`npm test` builds the package and runs Node's built-in test runner against
`dist/` — the same artifact a consumer installs. The tests assert the JSON
actually put on the wire, so a wrong method string, a stray parameter or a
reintroduced legacy form fails the suite. No test framework dependency is
added.
