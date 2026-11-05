# RFC 0006 — Chain Identity, Authenticated Blocks and State Commitment

**Status:** Draft
**Tracking:** #205, under #209
**Author:** Gilbert Kalombo
**Created:** 2026-11-05
**Protocol version:** v0.4 → v0.5
**Locked surfaces affected:** [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) §2 (signing input), §5 (validation rules), §6 (storage schema), §7 (receipts), §8 (RPC), §9 (P2P identifiers), §10 (genesis), §11 (vectors); block encoding, transaction signing, hashing inputs; [`RECEIPT_SPEC_v0.1.md`](../specs/RECEIPT_SPEC_v0.1.md) (signature verification only)

> Nothing here authorizes implementation. The review status is on the pull request.

---

## Motivation

A node today cannot tell who produced a block, which network it belongs to, or
whether it agrees with its peers on state:

| Gap | Today | Consequence |
|---|---|---|
| Block origin | `BlockHeader` has no producer and no signature | Any peer can extend the tip with an otherwise valid block |
| Network identity | No chain identifier. `--chain` is printed and ignored | A signed transaction is valid on every network |
| Genesis | Code-defined, one public test account | No network can have its own accounts or operators |
| State agreement | `state_root` is always zero | Two nodes can diverge on balances with no error |
| State transition | Described by code, not by a normative definition | Two implementations can differ, and neither is wrong |

None of this matters on a local devnet. All of it blocks a network run by more than
one operator, which is the goal of MVP v0.2 (#209). If we do nothing, MVP v0.2 can only
be a single-operator demonstration.

---

## Scope

- [x] Block/transaction SCALE encoding: `BlockHeader`, `Block`
- [x] Hashing rules: block hash input changes with the block, new domains
- [x] `apply_block` validity rules: new rules (t)–(w), one strict signature definition, and a normative transfer definition that supersedes v0.4 behaviour where they differ
- [x] Producer obligations: one block per height (section 3)
- [x] `write_batch`: atomicity unchanged. An authority node's block commit becomes a synced write (section 3)
- [x] Storage: schema 4, chain identity metadata, refusal of older directories
- [x] P2P protocol strings: new versions and chain binding. Framing unchanged
- [x] RPC: `rpc_v0.4`, a result shape and one new method
- [x] Transaction signing payload
- [x] Genesis definition (Lock §10)
- [x] Protocol vectors (Lock §11)

## Non-Goals

- Validator elections, staking, slashing, rewards, fork choice or finality.
- Rotating the authority set without a new genesis.
- Fees, issuance or any monetary decision. RFC 0003 and RFC 0004 stay non-normative.
- Transaction gossip. See S3 (#207).
- Membership proofs or light clients.
- Any change to the `ComputeTask` or `Receipt` encoding, or to `task_id`.

---

## Design

`||` is byte concatenation. Domain strings are ASCII, with no terminator. In the genesis
file, hex is lowercase with no prefix.

### 1. Genesis file and chain identifier

A network is defined by one file, at most 1 MiB:

```json
{"allocations":[{"address":"<hex>","amount":"<decimal>"}],"authorities":["<hex>"],"name":"<name>","schema":"mbongo-genesis:v1","timestamp":0}
```

The file must already be canonical, or it is refused. This RFC defines canonical here,
and does not depend on any other code:

- UTF-8. No whitespace outside strings, and no escape sequence in any string. Object keys
  sorted bytewise. No duplicate keys, no unknown keys, all five top-level keys present.
- `schema`: exactly `mbongo-genesis:v1`.
- `name`: 1 to 64 characters from `[a-z0-9-]`.
- `timestamp`: digits only, no leading zero unless it is `0`, fitting in `u64`.
- `address`, `authorities[]`: 64 hex characters. Each authority is a canonical encoding of
  an Ed25519 point that is not of small order. `authorities` holds **exactly one** in
  `mbongo-genesis:v1`. It stays a list so that a later schema can define more, under its
  own review.
- `allocations`: sorted ascending by address, with no duplicates. May be empty. Each one
  creates an account with its amount and nonce 0, including an amount of `"0"`.
- `amount`: a decimal string with no sign and no leading zero, or `"0"`. Each amount and
  their sum fit in `u128`.

`chain_id = BLAKE3("mbongo:chain-id:v1" || file_bytes)`, 32 bytes.

The genesis block is derived, never received: height 0, `parent_hash` zero, `timestamp`
from the file, empty body, `transactions_root` as computed today for an empty list,
`producer` zero, `producer_signature` zero, and `state_root` per section 4. Rules (t) to (w) do not apply to it. The genesis block, the allocations
and the chain identity are written in one `write_batch`.

**Node flags.** `--genesis <file>` loads a file. `--dev` selects the built-in dev genesis.
Exactly one is required, and `--chain` is removed.

**The dev genesis** is a fixed file in the repository: `name` is `dev`, `timestamp` is 0,
with the existing public dev account and its existing amount, and one authority, the public key of the seed
`BLAKE3("mbongo:dev-authority:v1")`. With `--dev --producer`, that key is used
automatically. It is a public test key.

**Consumers that change** to pass one of these flags: `docker-compose.yml`, `Dockerfile`,
`scripts/devnet/*`, `scripts/mvp-launch.mjs`, `scripts/mvp-d-acceptance.mjs`, and every
harness that starts a node: `replay_harness`, `devnet_harness`, `compute_vertical`,
`compute_harness`, `mvp_inference_harness`, `provider_harness`, and the tests that start
one. `MBONGO_CHAIN` goes with `--chain`.

### 2. Transaction signing

The signed bytes become:

```text
"mbongo-tx:v2" || chain_id || SCALE(tx_type, sender, receiver, amount, nonce, payload)
```

The `Transaction` encoding and its hash are unchanged: only the signature's domain
changes. A signature made for one network does not verify on another. `ComputeTask`
and `Receipt` need no change: an anchoring transaction carries a chain-bound signature.
A `Receipt` signature alone does not name a network.

Transaction signatures and receipt signatures (rule (h)) are verified by the strict
definition of section 3, so that this is a protocol rule and not a library default. Their
signed bytes are as stated here and in Lock §5.

### 3. Authenticated blocks

```rust
pub struct BlockHeader {
    pub parent_hash: Hash,
    pub state_root: Hash,
    pub transactions_root: Hash,
    pub timestamp: u64,
    pub height: u64,
    pub producer: Address,            // new, appended
}

pub struct Block {
    pub header: BlockHeader,
    pub body: BlockBody,
    pub producer_signature: [u8; 64], // new, appended
}
```

`producer_signature` is Ed25519 over the 32 bytes of
`BLAKE3("mbongo-block:v1" || chain_id || SCALE(header))`. Verification is strict, because
the block hash covers the signature, and a malleable signature would give one block two
hashes: `s` is below the group order, `R` and the public key are canonical and not of
small order, and the equation is checked without the cofactor (RFC 8032). `ed25519-dalek`'s
`verify_strict` does not check canonical encoding by itself, so an implementation built on
it adds that check. `compute_block_hash` keeps hashing the SCALE of the whole block.

The producer of every block is `authorities[0]`, the only authority this schema allows.

`--producer` needs `--producer-key-file`, a file holding a 32-byte Ed25519 seed as 64 hex
characters, as the client key files do. Its key must be the authority, or the node
refuses to start. The only exception is `--dev`, where the flag is not used and the
public dev authority key is taken automatically, and `--dev` with `--producer-key-file` is
refused.

**A producer builds a block in this order:** execute the transactions on an uncommitted
view, compute `state_root`, fill the header with
`timestamp = max(now, parent.timestamp)`, sign, then call `apply_block`, which validates
it like any other block and commits it. A transaction that fails in the view is left out
of the block, with no effect on the view, and does not fail the block. What then happens
to it is node-local mempool policy.

**One block per height.** With no fork choice, two signed blocks at one height split a
network for good, and honest failures can cause that as well as malice. So an authority
node:

- runs production and block application under one writer;
- commits a block with a synced write **before** broadcasting it. A crash before the
  commit leaves nothing that anyone has seen, so signing a different block at that height
  afterwards is harmless;
- after a start, if peers are configured, produces nothing until one of them has answered
  and every valid block they serve within a timeout is applied. A peer that does not
  answer is not counted. If none answers, it waits for an operator to pass
  `--producer-start-alone`, which is also how a new network's first start proceeds;
- stops producing only on a block above its tip that passes rules (t) and (u), which it
  applies first. **A height a peer merely reports never stops production,** and is
  dropped when no valid block backs it within that timeout;
- is the only running node holding that key. Nothing can enforce this: it is an operating
  rule.

These bind the producer. A receiving node cannot check them.

**Recovery.** If two blocks exist at one height anyway, nodes stay on the first they saw,
and no rule reconciles them. The operators pick one branch. Nodes on the other stop, clear
their directories and resync. If no branch can be chosen, the network restarts from a new
genesis.

### 4. State commitment

`state_root` is a hash chain over what each block changes:

```text
state_root(0) = BLAKE3("mbongo:state:v1" || chain_id || SCALE(allocations))
state_root(h) = BLAKE3("mbongo:state:v1" || state_root(h-1) || SCALE(changes))
```

- `allocations` is `Vec<(Address, u128)>`, in file order.
- `changes` is `Vec<(u8, [u8; 32], Vec<u8>)>`, sorted ascending by kind, then by key,
  compared bytewise.
- Entries, one per key, hold the value after the whole block:

| Kind | Key | Value | Present when |
|---|---|---|---|
| 0 | Address | `SCALE(Account)` | The account is the sender of an applied transaction, or the receiver of an applied `Transfer` or `Stake`, even if unchanged |
| 1 | `task_id` | `SCALE(ComputeTask)` | The block registers the task |
| 2 | `task_id` | `SCALE(Receipt)` | The block anchors the receipt |

A skipped transaction (section 6) contributes nothing. An empty block has empty `changes`.
`SCALE(Account)` becomes a consensus input, so the `Account` encoding is locked in v0.5.

The cost is proportional to the block, and storage needs no iteration. This detects
divergence in what blocks change, at the first block where that differs. A difference in
an account stays invisible until a block touches it. **It is not a Merkle root:** it gives no
membership proof and no snapshot sync. A full state tree was considered and left out:
it is much larger, and nothing in this version needs a proof.

### 5. New validity rules

Evaluated in this order, after the existing checks of height, then parent, then
`transactions_root`, and before any transaction:

| Rule | Check | Error |
|---|---|---|
| (t) | `header.producer == authorities[0]` | `UnauthorizedProducer` |
| (u) | `producer_signature` verifies | `InvalidBlockSignature` |
| (v) | `timestamp >= parent.timestamp` | `InvalidTimestamp` |

Evaluated after every transaction is applied, before the atomic write:

| Rule | Check | Error |
|---|---|---|
| (w) | `state_root` equals the value computed locally | `StateRootMismatch` |

The first failure rejects the block. Existing rules and their precedence are otherwise
unchanged. `timestamp` has no upper bound: a wall-clock check would make validity depend
on when a block is replayed.

### 6. State transition

Rule (w) proves that nodes agree, not that the state is right. This section makes the
transfer path normative. **Where v0.4 behaviour differs from it, this section supersedes
it,** including in which transactions are accepted. v0.5 starts from a fresh genesis, so
no chain is reinterpreted.

**Stored** means committed by an earlier block. It never means the uncommitted view of the
block being applied.

A stored `Transfer` or `Stake` transaction is **skipped**, as Lock §5 defines: it changes
nothing, and is not *applied*. A stored `ComputeTask` or `AnchorReceipt` transaction is
refused, by its existing rule. A second copy, within one block, of a transaction not yet
stored is evaluated like any other, and fails at its nonce. Every copy of a stored
`Transfer` or `Stake` is skipped. Everything below concerns
applied transactions.

Accounts are one mapping from address to state. Applying a `Transfer` or `Stake`
transaction performs these steps, in order, each on the result of the one before:

1. Read `state[sender]`. If absent, `InsufficientBalance`. This is the code's error for
   these types, and it stands.
2. If `tx.nonce != state[sender].nonce`, `InvalidNonce`. Otherwise add one to it.
3. If `state[sender].balance < tx.amount`, `InsufficientBalance`. Otherwise subtract.
4. If `state[receiver]` is absent, create it with zero balance and nonce.
5. Add `tx.amount` to `state[receiver].balance`. On overflow, `InsufficientBalance`.

Any failure rejects the block, and nothing is written. These steps imply invariants that
bind every implementation:

| # | Invariant |
|---|---|
| I1 | Total supply is unchanged by every block. Only genesis creates balances |
| I2 | An applied transaction advances its sender's nonce by exactly one |
| I3 | Balances change by exactly `amount`, down at the sender and up at the receiver, as the steps define |
| I4 | A step reads what the previous step wrote |

### 7. P2P and RPC

- Negotiation identifiers become `/mbongo-sync/3/<c>` and `/mbongo/block_notify/0.3.0/<c>`,
  where `<c>` is the first 16 lowercase hex characters of `chain_id`. Peers on another network fail
  negotiation, before any block is exchanged.
- Identify metadata becomes `/mbongo/0.5.0`, informational as before.
- Message shapes and framing are otherwise unchanged. `Block` carries the new fields.
- **A new RPC version, `rpc_v0.4`,** because a result shape changes: block results gain
  `producer` and `producer_signature`, in the `0x`-prefixed hex of `rpc_v0.3`.
- New method `get_chain_info`, no parameters, returning
  `{"chain_id": "0x…", "genesis_hash": "0x…", "authorities": ["0x…"]}`.
- **A client takes `chain_id` from its own configuration,** and uses `get_chain_info` only
  to compare. An endpoint must not choose the network a user signs for.

---

## Compatibility

**A clean version boundary, as at v0.3 and v0.4.** No activation height, and no
migration. A v0.5 network starts from a new genesis in an empty directory.

- **Existing nodes:** v0.4 and v0.5 nodes do not negotiate.
- **Existing data:** abandoned. Storage schema 3 → 4. Unlike earlier versions, a v0.5 node
  **refuses** a directory below schema 4, and does not stamp it. Schema 4 stores `chain_id`
  and the genesis hash, and a node refuses a directory holding different ones. A
  directory is new when it has no chain identity and no block, and its stamp is absent or
  4, so a crash during genesis creation is recoverable, once the column families exist as
  the Lock's open sequence requires. Any stamp below 4 is refused,
  block or no block.
- **Existing clients:** `@mbongo/sdk` needs a new version for signing, block decoding and
  `get_chain_info`. 0.2.0 stays valid for v0.4 only. Its signature verification must
  meet the strict definition, which a library default may not do. Every other transaction signer
  changes with the node: `mbongo-wallet`, and in `mbongo-compute` its client, worker,
  service and harnesses.
- **Rollback:** nothing to roll back. An abandoned v0.5 directory leaves v0.4 and its tag
  as they are.
- **`mvp-v0.1.0`:** not changed by this RFC. It stays pinned to its release and protocol.
- **Vectors:** see Testing.

---

## Security

| Threat | Effect of this RFC |
|---|---|
| A peer forges blocks | Refused by (t) and (u) |
| A transaction replayed on another network | Refused by the signing domain |
| Two nodes silently disagreeing on state | The differing block is refused by (w), and the node stays at the last agreed height. It may keep re-requesting that block |
| A state transition that differs from section 6 | Refused by review and tests, not at runtime: nodes that agree pass rule (w) |
| An authority key is stolen | **Not addressed.** Replacing it needs a new genesis |
| An authority signs two blocks at one height by accident | Section 3 reduces this. It remains possible: a wiped or restored authority started with `--producer-start-alone`, or two nodes sharing the key. Recovery is manual |
| A peer reports a false height | No effect on production: only a valid signed block stops it. A peer that withholds blocks delays a start until the timeout |
| An authority equivocates on purpose | **Not addressed.** Peers keep the first block seen. No fork choice, no slashing |
| An authority censors or stops | **Not addressed.** With one authority the chain halts |
| A dev network | All `--dev` networks share one `chain_id`, so transactions replay between them. Nothing stops a genesis naming the public dev authority: an operator must not |
| Off-chain signatures | Provider HTTP, lease and capability signatures are not bound to a network by this RFC |

Authority keys become the most sensitive keys on a network. New denial-of-service path:
signature verification on every block from any peer, bounded by the peer rate limits of
#207, which are a prerequisite for exposing a node to untrusted peers.

## Testing

- **Vectors, independently derived** as Lock §11 requires, shared with the TypeScript SDK:
  new fixtures for `chain_id`, the genesis file, the transaction signing domain, the block
  signature and `state_root`.
- **Existing fixtures that change, and need new versions:** `compute-task-v1`,
  `anchor-receipt-v1`, `anchor-binding-v1` and `compute-task-rpc-v1`, wherever they hold a
  signature or a block. `receipt-v1` is unchanged. The v0.4 fixtures stay as records.
- `apply_block` negative tests for (t) to (w), each asserting its own error.
- Negative vectors for the strict definition: non-canonical `s`, `R` and key forms, and
  small-order `R` and key, and a signature that passes only the cofactored equation, for
  blocks, transactions and receipts.
- For each transaction type: a stored copy, a second copy in one block, and two copies of
  a stored `Transfer` or `Stake`, with a vector.
- A transaction failing in the view is left out with no partial effect, and the rest of the
  block is valid.
- A genesis with more than one authority is refused.
- Concurrent production gives one block per height. A crash before the commit, a restart
  behind a peer, and a restart with no peer answering, each give no second block at a height.
- A false height from a peer does not stop production.
- Genesis: every malformed form refused, and a directory left by a crash during creation
  restarts as new.
- Two nodes with different genesis files refuse to sync, at negotiation.
- A transaction signed for another `chain_id` is refused.
- Two nodes forced to diverge stop at the first differing block.
- Generated blocks over every sender and receiver combination, with boundary amounts,
  checked against section 6.
- A schema-3 directory is refused, and a directory of another chain is refused.
- `replay_harness` must apply blocks through `apply_block`, which it does not do today.
- Devnet convergence and compute vertical harnesses on v0.5.

**Acceptance criteria:** every item above passes in CI on the exact release SHA, and
the vectors match their independent derivation in Rust and in TypeScript.

## Rollout

1. `READY_FOR_REVIEW`, 7-day cooling period, written adversarial review (`RFC_PROCESS.md`).
2. Acceptance.
3. Implementation, under the gates of `MVP_v0.2_NETWORK_READINESS.md`: conformance to
   section 6 first, as its own change, then `mbongo-core`, `mbongo-storage`, `mbongo-node`,
   `mbongo-network`, `mbongo-compute`, then the SDK.
4. `PROTOCOL_LOCK_v0.5.md`, including the `Account` encoding, `rpc_v0.4.md`, an updated
   `storage_invariants.md`, and a `v0.5-devnet-stable` tag. v0.4 stays as a
   historical lock. No coordination is needed: there is no network to upgrade.
