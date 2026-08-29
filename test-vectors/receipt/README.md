# Receipt cross-language golden vectors

`receipt-v1.json` is the **single shared source of truth** for canonical
receipt encoding, hashing and signing. It is owned by no language: Rust reads
it today, and the TypeScript SDK will read the same file rather than carrying
a copy of the constants.

## Why it exists

Canonical vectors previously lived only inside Rust `#[cfg(test)]` code. They
were correct, but a second implementation could not consume them — so a
TypeScript encoder could disagree with the node and nothing would notice
until a receipt was rejected in production.

## Authority

- [`RECEIPT_SPEC_v0.1.md`](../../docs/specs/RECEIPT_SPEC_v0.1.md) — structure,
  canonical encoding, hash rule
- [`RFC 0002`](../../docs/rfcs/0002-receipt-anchoring-v0.3.md) — the activated
  v0.3 rules, and `MAX_RECEIPT_METADATA_BYTES = 4096`
- [`PROTOCOL_LOCK_v0.3.md`](../../docs/specs/PROTOCOL_LOCK_v0.3.md) — FROZEN;
  the metadata bound is normative here, **not** in `RECEIPT_SPEC_v0.1`, which
  omits it

## The rules being pinned

A receipt has seven fields in this fixed order:

```
version           u8          1 byte
task_id           [u8; 32]    32 bytes
input_commitment  [u8; 32]    32 bytes
output_commitment [u8; 32]    32 bytes
executor          [u8; 32]    32 bytes
metadata          Vec<u8>     SCALE compact length prefix, then the bytes
signature         [u8; 64]    64 bytes
```

- **signing payload** = SCALE of fields 1–6. The signature is excluded, and
  since it is the last field the payload is a strict prefix of the full
  encoding.
- **receipt_hash** = `BLAKE3(signing payload)`.
- **executor signature** = Ed25519 over the **raw 32 bytes** of
  `receipt_hash` — never over its hex text.
- **full encoding** = signing payload ‖ signature.

## The boundary that motivated the vector set

The SCALE compact length prefix is not always one byte:

| metadata length | prefix width | prefix | signing payload | full encoding |
|---|---|---|---|---|
| 0 | 1 | `00` | 130 | 194 |
| 3 | 1 | `0c` | 133 | 197 |
| 63 | 1 | `fc` | 193 | 257 |
| **64** | **2** | **`01 01`** | 195 | 259 |
| **4096** | **2** | **`01 40`** | 4227 | 4291 |

**At the consensus maximum the prefix is two bytes.** An implementation that
assumed one byte would pass a three-byte-metadata vector and fail at the very
size the protocol permits. The 63 → 64 transition and the 4096 bound are the
two cases worth having.

Lengths of 16384 and above, where the prefix widens to four bytes, are
deliberately absent: consensus rejects anything over 4096, so those encodings
are unreachable.

## Where the expected values came from

They were **not** produced by the Rust encoder and then handed back to it —
that would prove nothing.

1. **Signing payload bytes** were assembled by hand from the field order and
   widths above, with the compact prefix computed from the SCALE rule
   (`n ≤ 63` → `n << 2`; `n ≤ 16383` → `(n << 2) | 0b01` little-endian).
2. **Hashes** were computed with an independent BLAKE3 implementation over
   those hand-derived bytes.
3. **The key pair and signatures** came from an independent Ed25519
   implementation, not from `ed25519-dalek`.
4. **Only then** does the production Rust encoder have to agree, in
   `crates/mbongo-core/tests/receipt_vectors.rs`.

The remaining half of the proof arrives with the TypeScript primitives: a
second implementation, with its own SCALE, BLAKE3 and Ed25519, meeting the
same pinned constants. Rust agreeing with pinned literals is regression
protection; two independent stacks agreeing is the interoperability proof.

## Conventions

- Hex is **lowercase, without an `0x` prefix**, so a consumer never has to
  strip anything before decoding. The parser rejects both uppercase and a
  leading `0x`.
- Metadata is expressed as `{"pattern": "repeat", "byte": "ab", "length": n}`,
  which expands to that byte repeated `length` times. `repeat` is the only
  pattern the schema defines; anything else is a fixture error. This keeps a
  4096-byte vector readable instead of adding 8 KB of hex.
- Small vectors (metadata ≤ 64) additionally pin the **full** signing payload
  and full encoding as hex. The 4096 vector pins the compact prefix, both
  exact lengths, the hash and the signature — enough to catch a prefix-width
  bug, which changes all four, without the byte dump.

## The test key

```
seed:       2a2a…2a  (32 bytes of 0x2a)
public key: 197f6b23e16c8532c6abc838facd5ea789be0c76b2920334039bfa8b3d368d61
```

**TEST ONLY. NOT A PRODUCTION KEY.** The seed is published here on purpose;
anything signed with it is worthless. It is the same key the existing
in-crate receipt tests already use, so vectors and unit tests share one
identity.

## Invalid vectors

Three, each isolating a different failure so an independent implementation
can tell them apart:

| Vector | Rejected by | What it proves |
|---|---|---|
| `metadata-over-consensus-bound` | `metadata_bound` | 4097 bytes encode and hash cleanly; only the consensus bound rejects it |
| `field-mutated-after-signing` | `signature` | the verifier recomputes the hash, so a mutated field surfaces as a signature failure, not a stored-hash mismatch |
| `signature-from-wrong-key` | `signature` | every byte is canonical and the hash is right; only the signer is wrong |

The consensus bound itself is enforced in `apply_block`, and
`metadata_cap_enforced` in `crates/mbongo-node/src/backend.rs` already proves
4097 is rejected there and 4096 accepted. This fixture does not duplicate
that; it records the intent so a TypeScript implementation knows which
failure to expect.

## Consumers

- **Rust** — `crates/mbongo-core/tests/receipt_vectors.rs`, run by
  `cargo test --workspace` and therefore by existing CI.
- **TypeScript** — not yet. The receipt primitives are a separate slice, and
  they must read this file rather than copy from it.
