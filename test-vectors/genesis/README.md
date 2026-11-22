# Genesis vectors

`genesis-v1.json` holds neutral, language-agnostic vectors for the genesis file
and the chain identifier of RFC 0006 §1.

## The rules

A network is defined by one canonical file, at most 1 MiB:

```text
{"allocations":[{"address":"<hex>","amount":"<decimal>"}],"authorities":["<hex>"],"name":"<name>","schema":"mbongo-genesis:v1","timestamp":0}
```

**Canonical form:**

- UTF-8, with no whitespace outside strings and no escape.
- Object keys are sorted bytewise; none is unknown, repeated or missing.

**Field rules:**

- `name`: 1 to 64 characters from `[a-z0-9-]`.
- `timestamp`: a `u64` with no leading zero.
- Addresses and authorities: 64 lowercase hex characters.
- `authorities`: exactly one, a canonical Ed25519 point not of small order.
- `allocations`: sorted by address, with no repeat. May be empty.
- Amounts: canonical decimals; each amount, and their sum, fits in `u128`.

**Chain identifier:**

```text
chain_id = BLAKE3("mbongo:chain-id:v1" || file_bytes)
```

A file is its bytes: a trailing newline is a different file. It is refused,
since newline is whitespace outside a string.

## Cases

**`valid`, 4 files:**

- the dev genesis;
- no allocations;
- a zero-balance account next to amounts that sum exactly to `u128::MAX`;
- a 64-character name with timestamp `u64::MAX`.

Each one gives its `chain_id` and its parsed contents.

**`invalid`, 35 files.** Each differs from a valid file by one defect, and
gives the class it is refused with:

| Class | Cases |
|---|---|
| `NotCanonical` | trailing newline, space, byte-order mark, escape, keys unsorted, duplicated, unknown or missing, an unknown allocation key, a quoted timestamp, trailing data |
| `NotUtf8` | `0xff` in the name |
| `Schema` | `mbongo-genesis:v2` |
| `Name` | empty, 65 characters, uppercase, underscore |
| `Timestamp` | leading zero, above `u64` |
| `Address` | uppercase, 63 characters |
| `Amount` | leading zero, empty, signed, above `u128` |
| `SupplyOverflow` | the amounts sum past `u128::MAX` |
| `AllocationOrder` | unsorted, duplicated |
| `AuthorityCount` | none, two |
| `Authority` | order 1, 32 zero bytes (order 4), a non-canonical encoding of a point of large order, bytes that are no point, uppercase |

`TooLarge` needs a file over 1 MiB. A unit test in
`crates/mbongo-core/src/genesis.rs` covers it.

## The dev genesis

The dev genesis is `crates/mbongo-core/genesis/dev.json`, byte for byte. It
holds:

- `name` `dev` and `timestamp` 0;
- the existing dev account (seed `0xaa` repeated) with `1000000000`;
- one authority, whose seed is `BLAKE3("mbongo:dev-authority:v1")`.

Both seeds are public **test** keys. `.gitattributes` keeps the file and this
directory free of line-ending conversion.

## Derivation

Each file was written by hand from the rules. `chain_id` comes from a BLAKE3
written from its specification, and the keys from an Ed25519 implementation
independent of `ed25519-dalek` and `@noble`. Both were checked against
published values first, as described in `../signature/README.md`.

## Consumers

| Consumer | Checks |
|---|---|
| `crates/mbongo-core/tests/genesis_vectors.rs` | every valid file accepted with its contents and `chain_id`; every invalid file refused with its class; the committed dev file |
| `sdk/typescript/test/genesis.test.mjs` | `chain_id` of every valid file; the committed dev file |
