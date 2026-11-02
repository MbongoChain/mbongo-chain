# ComputeTask vectors

Neutral, language-agnostic golden vectors for RFC 0005 `ComputeTask`
envelopes and the transactions that carry them. Owned by no implementation:
Rust reads them today, and the TypeScript SDK will read the same file under
Workstream D of the Compute vertical epic (#126).

`compute-task-v1.json` pins the canonical SCALE task bytes, the `task_id`
preimage and derivation, the transaction signing bytes, the transaction
signature, the full signed encoding, the transaction hash, and the serialised
`Transaction` JSON object.

## Authority

- [RFC 0005](../../docs/rfcs/0005-compute-task-commitment-v1.md) — Accepted.
  §2.1 the envelope, §2.2 identity, §2.7 the payload variant, §2.10 the bound,
  §12 what a fixture must pin
- `crates/mbongo-core/src/compute_task.rs` — the envelope type, `DOMAIN_TASK`,
  `ComputeTask::task_id`
- `crates/mbongo-core/src/primitives.rs` — `Transaction`, `TransactionType`,
  `TransactionPayload`, `Transaction::signing_payload`
- `../receipt/receipt-v1.json` — supplies the submitter test key, which is
  resolved from there and never restated

## The envelope

Six fields in this fixed order. `task_id` is **not** a field.

```
version           u8          1 byte, must be 1
submitter         [u8; 32]    32 bytes
executor          [u8; 32]    32 bytes
salt              [u8; 32]    32 bytes
input_commitment  [u8; 32]    32 bytes
execution_spec    Vec<u8>     SCALE compact length prefix, then the bytes
```

```
task_id = BLAKE3( DOMAIN_TASK || canonical task bytes )
DOMAIN_TASK = the 22 ASCII bytes  mbongo:compute-task:v1
```

The tag is prepended **raw**: no NUL terminator, no SCALE length prefix, and
the hash is over bytes, never over a hex rendering. The
`wrong_tag_diagnostics` block pins what each of those mistakes would produce
so a consumer cannot pass by accident.

## The boundary that motivated the vector set

`execution_spec` is the only variable-length field, and its compact prefix is
not always one byte:

| `execution_spec` length | prefix | canonical task | preimage |
|---|---|---|---|
| 0 | `00` | 130 | 152 |
| 3 | `0c` | 133 | 155 |
| **64** | **`01 01`** | 195 | 217 |
| **1024** (maximum) | **`01 10`** | **1155** | **1177** |
| 1025 (rejected) | `05 10` | 1156 | 1178 |

At the consensus maximum the prefix is two bytes, and the maximal sizes are
exactly the 1155 / 1177 that RFC 0005 §2.10 states. The 1025-byte envelope
encodes and hashes like any other — the bound is consensus rule (n), not an
encoding limit — and appears under `rejected` so a consumer can prove it
refuses the transaction rather than failing to parse it.

## The signing formula

```
transaction signing payload =
    0x01                          TransactionType::ComputeTask
 || sender[32]
 || receiver[32]                  the zero address (rule l)
 || amount   u128 little-endian, 16 bytes, must be 0 (rule l)
 || nonce    u64  little-endian,  8 bytes
 || 0x02                          TransactionPayload::ComputeTask
 || <canonical task bytes>

full transaction = signing payload || transaction_signature[64]
transaction hash = BLAKE3(full transaction)
```

The transaction signature is over the **raw** signing payload; there is no
prehash. The envelope carries **no signature of its own** (RFC 0005 §2.5):
the transaction signature is the only authentication, and consensus requires
`sender == task.submitter` (rule o). Everything before the task is
fixed-width, so the task bytes always begin at **offset 90**.

`TransactionType::ComputeTask` keeps the codec index `1` frozen at v0.3.
`TransactionPayload::ComputeTask` is the new index `2`, appended after
`None` (`0`) and `AnchorReceipt` (`1`), both unchanged.

## Vector kinds

**`tasks`** — four envelopes: `canonical` (three-byte specification),
`empty-spec`, `spec-64` (first two-byte prefix) and `spec-max-1024`.

**`rejected`** — `spec-1025`, one byte past the bound, with its encoding and
`task_id` pinned and `consensus.valid = false`.

**`identity`** — five variants of the canonical envelope, each changing
exactly one field, each with its expected `task_id`. Changing the executor
changes the task: that is RFC 0005 §2.6, and it is what makes an off-chain
executor reassignment impossible to express as a mutation of an existing
task. The transaction nonce is not in the envelope; the two `canonical*`
transaction vectors carry the same task under different nonces and pin that
the `task_id` is unchanged while the transaction hash is not.

**`valid`** — three consensus-valid transactions: `canonical` with a
deliberately non-palindromic nonce so byte order is provable,
`canonical-nonce-zero`, and `spec-max-1024`.

**`serialized_transaction`** — the exact serde JSON object for the canonical
vector. Interoperability evidence for Workstream C, not a protocol rule:
`rpc_v0.2.md` pins `payload` as `None | AnchorReceipt(Receipt)`, and this is
the shape the next RPC version has to describe. As with the receipt, `Address`
fields are `0x` hex and the plain byte fields (`salt`, `input_commitment`,
`execution_spec`) are arrays of numbers.

Consensus rules beyond the envelope — duplicate `task_id`, wrong nonce,
sender mismatch — are deliberately absent. They are node behaviour, tested in
`crates/mbongo-node/src/backend.rs`, and would make this a second copy of
that suite.

## How the expected values were derived

**Nothing here was produced by encoding with production Rust or with the
TypeScript SDK.**

| Value | Source |
|---|---|
| canonical task bytes | laid out by hand from the field rules; the compact prefix written from the SCALE rule |
| domain tag | the literal ASCII bytes |
| `task_id`, transaction hash | an independent BLAKE3 |
| `u64` and `u128` bytes | explicit fixed-width little-endian construction |
| transaction signatures | an independent Ed25519 |
| JSON object | assembled by hand from the serde annotations |

`crates/mbongo-core/tests/compute_task_vectors.rs` is a **consumer**: it must
agree with values it did not produce. The transaction hash rule is mirrored in
the test rather than called, because `compute_tx_hash` in
`crates/mbongo-node/src/backend.rs` is `pub(crate)`.

## Anchoring-binding vectors (`anchor-binding-v1.json`)

RFC 0005 §3 adds three rules to `AnchorReceipt` validation, after RFC 0002's
(a)–(j):

```
(q)  a task with receipt.task_id exists in prior chain state or earlier in the block
(r)  receipt.input_commitment == task.input_commitment
(s)  receipt.executor == task.executor
```

They add **no encoding**. The receipt bytes follow `../receipt/receipt-v1.json`
and the transaction bytes follow `../transaction/anchor-receipt-v1.json`,
both unchanged; no receipt field is added or changed. What is new is the
*relation* between a receipt and a registered task, so `anchor-binding-v1.json`
holds one vector per outcome RFC 0005 §12 asks for:

| vector | task_vector | verdict |
|---|---|---|
| `bound-named-executor` | `canonical` | valid |
| `bound-maximal-task` | `spec-max-1024` | valid |
| `unknown-task` | none registered | rejected, rule (q) |
| `input-commitment-mismatch` | `canonical` | rejected, rule (r) |
| `executor-not-named` | `canonical` | rejected, rule (s) — the submitter answering its own task, the squatting receipt of RFC 0005 §9.1 |

Each vector names its task from `compute-task-v1.json` (identities are never
restated), gives the receipt fields, and pins the receipt hash, the executor
signature, the full receipt encoding, the anchoring transaction's signing
payload, signature, full encoding and hash, plus the consensus verdict and the
rule that decides it. The anchoring sender is the receipt's executor, which
rule (g) requires; `executor-not-named` is therefore authenticated and still
unauthorised.

The `unknown-task` vector is exactly a v0.3-valid anchor: before RFC 0005 it
would have been accepted. That is the validity break RFC 0005 §5.2 records;
the wire format did not change.

`crates/mbongo-node/src/backend.rs` (`anchor_binding_vectors_drive_consensus`)
registers each referenced task in a block, rebuilds the receipt and transaction
from the fixture fields, checks the pinned bytes, hash and signatures against
production, and applies the anchor: the verdict must be the one the fixture
states. Expected values were derived the same way as the task vectors — hand
SCALE, independent BLAKE3 and Ed25519 — with the two task fixtures as the only
machine inputs.

## Key material

Two TEST ONLY keys, both public constants, never production keys:

- **submitter** — the receipt fixture key, seed `0x2a` repeated, resolved from
  `../receipt/receipt-v1.json`
- **executor** — seed `0x2b` repeated, recorded in the fixture. Only its public
  key participates in these vectors; the seed is kept so the anchoring-binding
  vectors that follow RFC 0005 rules (q)–(s) can sign receipts with the same
  identity.
