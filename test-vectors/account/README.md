# Account vectors

Neutral, language-agnostic golden vectors for `SCALE(Account)`. Rust and
TypeScript read `account-v1.json`. Neither implementation owns it.

## Why this exists

RFC 0006 §4 makes `SCALE(Account)` a consensus input: it is the value of a
kind 0 entry in `state_root`, and the encoding is locked in v0.5. These
vectors pin the current encoding now, unchanged, so that S1-6 builds the
state commitment on a layout two languages already agree on.

## Layout

```
address[32] || balance u128 little-endian[16] || nonce u64 little-endian[8]
```

The encoding is always 56 bytes. It has no length prefix, and integers are
never compact.

## Vector kinds

**`valid`** has four cases:

- a new account;
- non-palindromic diagnostic values, so a big-endian or compact encoding
  cannot match;
- the state after the last applicable transaction (nonce `u64::MAX`);
- both maxima.

**`malformed_encodings`** has three byte strings that must not decode as an
account:

- empty;
- one byte short;
- one trailing byte. Decoding must consume all input.

## Derivation

Each encoding was laid out by hand from the field rules. No production
encoder was used.

- **Rust** (`crates/mbongo-core/tests/nonce_boundary_vectors.rs`) encodes and
  decodes with the production type.
- **TypeScript** (`sdk/typescript/test/nonce-boundary.test.mjs`) reads the
  fields at their fixed offsets. The SDK has no account codec, and this
  fixture adds none.

## Key material

`<test_key>` is the TEST ONLY key of `../receipt/receipt-v1.json`: seed `0x2a`
repeated 32 times, a public constant. **Never a production key.**
