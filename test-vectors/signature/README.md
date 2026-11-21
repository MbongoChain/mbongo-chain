# Strict Ed25519 vectors

`ed25519-strict-v1.json` holds neutral, language-agnostic vectors for the
strict Ed25519 definition of RFC 0006. Rust and TypeScript read it; neither
owns it.

## The definition

RFC 0006 §3 defines strict verification, and §2 applies it to transaction
signatures and receipt signatures (rule (h)). Block signatures join in S1-7.

A signature is valid only if all four hold:

1. `s` is below the group order `L`.
2. The public key and `R` are canonical encodings (RFC 8032 §5.1.3): `y < p`,
   and no sign bit when `x = 0`.
3. Neither the public key nor `R` is of small order.
4. `[s]B = R + [k]A` holds **without** the cofactor, with
   `k = SHA-512(R || A || M) mod L`.

Each case gives five things:

- the exact signed message (a transaction signing payload, or a 32-byte
  receipt hash);
- the public key;
- the signature;
- `expected`: `valid` or `invalid`;
- `rule`: the first requirement it does not meet.

## Cases

**19 cases** (14 transactions, 5 receipts), plus 3 malformed lengths:

| Rule | Cases |
|---|---|
| `valid` | two honest keys; the receipt fixture's executor signature; a canonical key with a torsion component and a challenge divisible by 8, which the definition accepts |
| `s-not-below-group-order` | `s + L`, and `s` with bit 255 set |
| `key-small-order` | keys of order 1 and order 8 |
| `key-non-canonical`, `r-non-canonical` | `y >= p` encodings |
| `key-not-a-point`, `r-not-a-point` | bytes that decode to no curve point |
| `r-small-order` | `R` of order 1 |
| `cofactored-equation-only` | `R` with a torsion component: the cofactored equation holds, the strict one does not |
| `wrong-message` | an honest signature over another payload |

The vectors alone cannot separate the canonical-encoding requirement from the
small-order one. Every non-canonical encoding a test can sign against is an
encoding of a small-order point. The Rust unit tests in
`crates/mbongo-core/src/crypto.rs` therefore check the encoding rules at
their boundaries directly.

## Derivation

Every expected verdict was computed by an Ed25519 implementation written
independently of `ed25519-dalek` and `@noble/curves`: BigInt arithmetic from
RFC 8032, with SHA-512 from `node:crypto`. Receipt hashes come from a BLAKE3
written from its specification.

Before deriving anything, that implementation was checked against three
sources:

- RFC 8032 tests 1 to 3;
- OpenSSL, on random keys and messages;
- the eight small-order encodings of libsodium's blocklist.

The valid receipt case reproduces the executor signature already pinned in
`../receipt/receipt-v1.json`.

## Consumers

| Consumer | Checks |
|---|---|
| `crates/mbongo-core/tests/strict_signature_vectors.rs` | every case through `verify_ed25519_strict`, and the transaction cases through `Transaction::verify_signature` |
| `crates/mbongo-verification/tests/strict_receipt_signatures.rs` | receipt cases through `verify_receipt_signature` |
| `crates/mbongo-node/src/backend.rs` tests | `apply_block` and admission follow the verdicts |
| `sdk/typescript/test/strict-signature.test.mjs` | every case through the SDK's strict verifier, and the receipt cases through `verifyReceiptSignature` |

## Key material

Every seed is TEST ONLY and public:

- `0x2a` repeated 32 times (the receipt fixture's `test_key`);
- `0x5b` repeated 32 times.

**Never production keys.**
