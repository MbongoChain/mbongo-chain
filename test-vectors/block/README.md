# Block signature vectors

`block-signature-v1.json` holds neutral, language-agnostic vectors for
authenticated blocks: RFC 0006 §3 and rules (t), (u) and (v) of §5. Neither
implementation owns it.

## The rules

```text
SCALE(BlockHeader) = parent_hash || state_root || transactions_root
                     || timestamp (u64 LE) || height (u64 LE) || producer      144 bytes
SCALE(Block)       = SCALE(BlockHeader) || SCALE(BlockBody) || producer_signature (64)
block hash         = BLAKE3(SCALE(Block))
signing hash       = BLAKE3("mbongo-block:v1" || chain_id || SCALE(BlockHeader))
```

`producer` and `producer_signature` are appended: each is the last field of
its structure. The producer signs the 32-byte signing hash with Ed25519. The
block hash covers the signature.

A block is valid only if, in this order, after height, parent and
`transactions_root`, and before any transaction:

| Rule | Check | Error |
|---|---|---|
| (t) | `header.producer` is the genesis authority | `UnauthorizedProducer` |
| (u) | `producer_signature` is strictly valid for this `chain_id` | `InvalidBlockSignature` |
| (v) | `timestamp >= parent.timestamp`, with no upper bound | `InvalidTimestamp` |

"Strictly valid" is the definition of `../signature/README.md`: `s` below the
group order, `R` and the key canonical and not of small order, and the
equation without the cofactor.

The genesis block carries a zero producer and a zero signature. It is derived,
never received, and the rules do not apply to it.

## Contents

**`chains`:** two networks whose genesis files name the same authority key:

- `dev`, the built-in dev genesis (`crates/mbongo-core/genesis/dev.json`);
- `other`, a file of this fixture with timestamp `1700000000`.

Each one gives its `chain_id`, `state_root(0)`, and genesis block encoding and
hash.

**`cases`, 24 blocks at height 1,** each with its header fields, header
encoding, signing hash, signature, block encoding and block hash. Each case
also gives:

- `expected`, the strict verdict of its signature: 6 are valid and 18 invalid;
- `rule`, the first strict requirement the signature does not meet;
- `apply_block`, what a node of that chain does with the block when it holds
  only its genesis.

| `apply_block` | Cases |
|---|---|
| `accepted` | the dev block, the other chain's block at its parent's timestamp, a timestamp of `u64::MAX` |
| `BadParent` | the dev block replayed whole on the other chain |
| `UnauthorizedProducer` | another key signing correctly, a zero producer, a producer key of small order, and a block failing (t), (u) and (v) together |
| `InvalidBlockSignature` | the dev header signed for the other chain by the same key; `s + L`; `s` with bit 255 set; `R` of order 1; a non-canonical `R`; an `R` that is not a point; a signature valid only under the cofactored equation; one over another header; one over the preimage instead of its hash; one without the domain; one under the transaction domain; a zero signature; another key signing for the authority; and a block failing (u) and (v) together |
| `InvalidTimestamp` | validly signed blocks one second before their parent, and at 0 |

A valid case carries `state_root(1)` of an empty block, so it also passes
rule (w).

## Derivation

The fixture was derived by `derive-block-signature-v1.mjs`, without the Rust
crates or the SDK:

- **Ed25519:** the BigInt implementation from RFC 8032 that S1-2 checked
  against RFC 8032 tests 1 to 3, OpenSSL, and libsodium's small-order
  blocklist. Every honest signature here is produced by OpenSSL as well and
  must be byte-identical.
- **BLAKE3:** the implementation written from its specification in S1-5.
- **SCALE:** laid by hand.
- **Layout, signing message and rule order:** read from the RFC text.
- **State roots:** `state_root(0)` uses the formula of
  `../state/state-root-v1.json`, checked against that file's genesis.

Re-running the script reproduces this file.

## Consumers

| Consumer | Checks |
|---|---|
| `crates/mbongo-core/tests/block_signature_vectors.rs` | every header and block encoding, block hash, signing hash and strict verdict; the genesis blocks; that an S1-6 block does not decode |
| `crates/mbongo-node/src/backend_block_tests.rs` | each block applied on a node of its chain: the stated outcome, and nothing written on refusal |
| `sdk/typescript/test/block-signature.test.mjs` | the header layout, signing hash, block hash and strict verdict, through the SDK's verifier |

## Key material

**TEST ONLY.** The dev authority seed is public:
`BLAKE3("mbongo:dev-authority:v1")`. The other seed is `0x5c` repeated. Every
dev network shares this authority. An operator must not name it in the
genesis of a real network.
