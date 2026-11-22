# RPC public-wire vectors

Neutral fixtures for the JSON that crosses the node's JSON-RPC boundary.
Owned by no implementation: Rust reads them at the boundary in
`crates/mbongo-network/tests/jsonrpc_tests.rs`, and the TypeScript SDK will
read the same file under Workstream D of the Compute vertical epic (#126).

The protocol fixtures under `../receipt/`, `../transaction/` and
`../compute-task/` pin **bytes**: SCALE encodings, hashes, signatures. This
directory pins the **JSON objects** built from those bytes, which is what an
RPC client actually sends and receives. Nothing here restates a byte the
protocol fixtures already own; every object resolves to a named vector there.

## `compute-task-rpc-v1.json`

Authority: [`docs/specs/rpc_v0.3.md`](../../docs/specs/rpc_v0.3.md) §1, §4.1,
§4.4. It pins:

- **`transactions`** — the `minimal` (empty `execution_spec`), `canonical`
  and `maximal` (1024-byte `execution_spec`) `ComputeTask` transaction
  objects, each with its `task_id`, full SCALE encoding and transaction hash,
  so a consumer can prove the object it reads or writes is the transaction
  the protocol fixture pins.
- **`block`** — one `get_block_by_height` result carrying all three payload
  variants in body order: a `Transfer` (`"None"`), the minimal `ComputeTask`,
  and a bound `AnchorReceipt`. Its `transactions_root` is the real commitment
  over those transactions.
- **`unknown_variant`** — payload objects the server rejects with `-32602`
  before any consensus code runs.
- **`old_client`** — the observed behaviour of the published rpc_v0.2 SDK
  (`@mbongo/sdk` 0.1.0) against such a block, recorded as evidence for D.

## The one rule this fixture adds nothing to

`rpc_v0.2` §1 already states how bytes appear in JSON: `Address` and `Hash`
are `0x` hex, a 64-byte signature is `0x` hex, and an unannotated `[u8; N]`
or `Vec<u8>` is an array of numbers. A `ComputeTask` follows that rule
exactly — `submitter` and `executor` are hex, `salt`, `input_commitment` and
`execution_spec` are arrays of numbers — and the payload is externally tagged
as `{"ComputeTask": {…}}`, like `{"AnchorReceipt": {…}}`. No new encoding
convention exists.

`execution_spec` crosses the wire as the exact bytes the submitter signed:
not decoded, not normalised, not interpreted. If a submitter publishes
private bytes there, the chain holds them and RPC returns them; RPC redacts
nothing, because nothing on chain is private.

`task_id` is **not** a transaction wire field. A client derives it per
RFC 0005 §2.2 from the canonical task bytes; on the wire it appears only as
the `task_id` array inside a receipt.

## How the expected values were derived

**Nothing here was produced by the node or the SDK.** The objects were
assembled by hand from the serde annotations; the transfer and the minimal
task were signed with an independent Ed25519 over hand-laid SCALE; the
canonical and maximal tasks were re-derived and compared byte-for-byte with
`../compute-task/compute-task-v1.json`; the anchor is the
`bound-named-executor` vector of `../compute-task/anchor-binding-v1.json`
rendered to JSON; `transactions_root` is an independent BLAKE3 over
length-prefixed SCALE transactions. The Rust boundary tests are consumers
that must agree.

## Key material

The TEST ONLY submitter key from `../receipt/receipt-v1.json` (seed `0x2a`
repeated), resolved from there and never restated. **Never a production key.**

## Version 2: the signing domain (RFC 0006 section 2, S1-5)

The `-v2.json` files supersede the `-v1.json` files of this directory, which
stay as the v0.4 record. In version 2 every transaction signature is over

```text
"mbongo-tx:v2"[12] || chain_id[32] || transaction_fields
```

`transaction_fields` is what version 1 called the signing payload: the SCALE
fields before the signature, which is also what the transaction encoding holds.
The encoding and the hash formula do not change; a hash changes only because
the signature in it does.

Each v2 file states the network it signs for in `chain`. That network is the dev
chain, `chain_id = 2fd700ac…d1a4`, from `crates/mbongo-core/genesis/dev.json`.
The file also gives an `other_chain_id`, for which no signature in it verifies.
Expected entries carry both `transaction_fields` and `signing_payload`, with their
lengths. Task encodings, receipts and receipt signatures are those of version 1.

**Derivation.** No production code was used:

- Each v2 file was derived from its v1 file by finding, for every signed
  transaction, the test seed that reproduces the v1 signature over the bare fields.
- The transaction was then signed again over the v2 bytes, with an Ed25519 that is
  independent of `ed25519-dalek` and `@noble` (OpenSSL through `node:crypto`).
- Its encoding and its BLAKE3 hash were recomputed with a BLAKE3 written from its
  specification. Its tree mode was cross-checked on 10,050 inputs of up to 66,561
  bytes.
- Every new signature was checked to verify for `chain_id`, and not over the bare
  fields or for `other_chain_id`.

Invalid signatures keep their defect under the new domain:

- one over the receipt hash is unchanged;
- one over a prehashed payload is over BLAKE3 of the v2 bytes;
- one from another transaction is that transaction's v2 signature.

`nonce-boundary-v2.json` adds two cases: a v0.4 signature over the bare fields, and
a signature for `other_chain_id`.

**Consumers.**

- Rust and the SDK read the v2 files.
- `crates/mbongo-core/tests/transaction_vectors.rs` and
  `sdk/typescript/test/signing-domain.test.mjs` also read
  `anchor-receipt-v1.json`, to prove that every v0.4 signature in it is refused.
