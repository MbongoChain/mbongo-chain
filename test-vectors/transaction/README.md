# AnchorReceipt transaction vectors

Neutral, language-agnostic golden vectors for `AnchorReceipt` transactions.
Owned by no implementation: Rust reads them, and TypeScript will read the same
file in #85.

`anchor-receipt-v1.json` pins the SCALE signing bytes, the transaction
signature, the full signed encoding, the transaction hash, and the serialised
`Transaction` JSON object.

## Why this exists on top of the receipt vectors

`../receipt/receipt-v1.json` pins the receipt. It cannot pin anything at the
transaction layer, and four things live only there:

| Element | Encoding | Pinned before this fixture |
|---|---|---|
| `TransactionType::AnchorReceipt` | one `codec(index)` byte, `0x03` | no |
| `TransactionPayload::AnchorReceipt` | one `codec(index)` byte, `0x01` | no |
| `amount: u128` | 16 bytes little-endian | no |
| `nonce: u64` | 8 bytes little-endian | no |

All four sit inside the bytes an account signs. Nothing in the repository
pinned a SCALE integer encoding before this file.

## The signing formula

```
transaction signing payload =
    0x03                          TransactionType::AnchorReceipt
 || sender[32]
 || receiver[32]
 || amount   u128 little-endian, 16 bytes
 || nonce    u64  little-endian,  8 bytes
 || 0x01                          TransactionPayload::AnchorReceipt
 || <full canonical receipt bytes, receipt signature included>

full transaction = signing payload || transaction_signature[64]
transaction hash = BLAKE3(full transaction)
```

The transaction signature is over the **raw** signing payload. There is no
prehash, and no compact length prefix appears anywhere at this layer — the
receipt is a nested struct, not a length-prefixed `Vec<u8>`.

Everything before the receipt is fixed-width, so the receipt bytes always begin
at **offset 90** (`1 + 32 + 32 + 16 + 8 + 1`), whatever the metadata length.
Transaction-layer correctness is therefore independent of receipt size, which
is why one referenced receipt vector is enough.

## Three cryptographic values, none interchangeable

| | Content | Hashed first? |
|---|---|---|
| receipt hash | BLAKE3 over the receipt signing payload | yes |
| transaction signing message | the raw SCALE signing payload | **no** |
| transaction hash | BLAKE3 over the **full signed** transaction | yes |

Two of these are 32 bytes, so the tests compare actual values rather than
lengths.

## Two signature domains, one key

Anchoring requires `sender == receipt.executor`, so a single Ed25519 key
produces both signatures. They are still different signatures, because the
messages differ:

| Signature | Key | Message |
|---|---|---|
| `receipt.signature` | executor | the raw 32 bytes of `receipt_hash` |
| `transaction.signature` | sender | the raw transaction signing payload |

The fixture pins all four outcomes: each signature verifies over its own
message and fails over the other one. This is the mistake the fixture exists to
catch, and reusing one signature for the other is exactly what the two invalid
vectors encode.

## Receipt reference, not duplication

A vector names a receipt instead of restating one:

```json
"receipt_vector": "empty-metadata"
```

That name resolves in `../receipt/receipt-v1.json`, which stays the single
receipt source of truth. `sender` is written as `<receipt.executor>` and
resolved the same way, so the sender and the executor cannot drift apart.

The dependency points one way only: this fixture knows about the receipt
fixture, and the receipt fixture knows nothing about transactions.

## Vector kinds

**`valid`** — two consensus-valid `AnchorReceipt` transactions.
`canonical-diagnostic-nonce` carries a deliberately non-palindromic nonce
(`1108152157446` = `0x010203040506`, little-endian `0605040302010000`) so a
big-endian or compact encoding cannot produce the same signature. `nonce-zero`
covers the realistic first-anchor case and guards against a consumer
hard-coding the diagnostic nonce.

**`encoding_only`** — one case, and **not a valid transaction**. A
consensus-valid `AnchorReceipt` must carry `amount = 0`, so no valid vector can
ever exercise `u128` byte order. This case exists solely to pin that encoding.
Its amount is written as a decimal **string**, not a JSON number, because the
value exceeds the JavaScript safe-integer range and a number literal would be
silently rounded when parsed.

**`invalid`** — two domain-confusion mistakes, both differing from the
canonical vector only in the transaction signature:
`transaction-signature-over-receipt-hash` signs the receipt's message, and
`transaction-signature-over-prehashed-payload` applies the receipt's
hash-then-sign pattern to a transaction. Consensus rules such as duplicate
`task_id`, wrong nonce, non-zero receiver or sender mismatch are deliberately
absent — they are backend behaviour, already tested there, and would make this
a second copy of the node's test suite.

## The JSON wire object

`serialized_transaction` pins the exact serde output for the canonical vector.
The JSON-RPC envelope (`jsonrpc`, `id`, `method`, `params`) is **not** pinned
here; the SDK wire tests already own it.

Within one transaction object, three byte representations coexist:

| Field | Rust type | JSON |
|---|---|---|
| `sender`, `receiver` | `Address` | `"0x…"` |
| `signature` | `[u8; 64]` + `serde_arr64` | `"0x…"` |
| `receipt.executor` | `Address` | `"0x…"` |
| `receipt.signature` | `[u8; 64]` + `serde_arr64` | `"0x…"` |
| `receipt.task_id` | `[u8; 32]` | **array of numbers** |
| `receipt.input_commitment` | `[u8; 32]` | **array of numbers** |
| `receipt.output_commitment` | `[u8; 32]` | **array of numbers** |
| `receipt.metadata` | `Vec<u8>` | **array of numbers** |
| `amount`, `nonce`, `receipt.version` | integers | number |
| `tx_type` | unit enum | the variant name as a string |
| `payload` | enum | externally tagged: `{"AnchorReceipt": {…}}` |

Hex appears exactly where a custom serializer exists — `Address` has its own
`impl Serialize`, and the 64-byte signatures use `serde_arr64`. Plain
`[u8; 32]` and `Vec<u8>` carry no annotation and fall through to serde's
default sequence handling.

This block records current runtime behaviour as interoperability evidence. It
does **not** define a protocol rule, and `docs/specs/rpc_v0.2.md` is unchanged.
That document's general statement about byte arrays does not describe these
four nested fields; reconciling the wording is a separate governance decision,
and pinning the object here is what a future decision can be made against.

## How the expected values were derived

The same rule as the receipt vectors: **nothing here was produced by encoding
with production Rust.**

| Value | Source |
|---|---|
| receipt bytes, hash, signature | resolved from `../receipt/receipt-v1.json` — the only machine input |
| signing payload | laid out by hand from the field rules: a SCALE struct is its fields concatenated in declaration order, an enum is one `codec(index)` byte |
| `u64` and `u128` bytes | explicit fixed-width little-endian construction |
| transaction signatures | an independent Ed25519 implementation |
| transaction hash | an independent BLAKE3 implementation |
| JSON object | assembled by hand from the serde annotations, then compared against `serde_json::to_value` |

`crates/mbongo-core/tests/transaction_vectors.rs` is a **consumer**: it must
agree with values it did not produce. TypeScript will do the same in #85, and
two independent implementations meeting on pinned constants is what makes the
vectors worth anything.

The transaction hash rule is mirrored in the test rather than called, because
`compute_tx_hash` in `crates/mbongo-node/src/backend.rs` is `pub(crate)`. It is
exactly BLAKE3 over the full SCALE encoding.

## Key material

The signing key is the TEST ONLY key from the receipt fixture — seed `0x2a`
repeated 32 times, a public constant. It is resolved from that file rather than
restated here. **Never a production key.**
