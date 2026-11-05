# RFC 0006 — Chain Identity, Authenticated Blocks and State Commitment

**Status:** Draft
**Tracking:** #205, under #209
**Author:** Gilbert Kalombo
**Created:** 2026-11-05
**Protocol version:** v0.4 → v0.5
**Locked surfaces affected:** [`PROTOCOL_LOCK_v0.4.md`](../specs/PROTOCOL_LOCK_v0.4.md) §5 (validation rules), §8 (RPC, additive), §9 (P2P identifiers), §10 (genesis); block and transaction encoding and signing

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

None of this matters on a local devnet. All of it blocks a network run by more than
one operator, which is the goal of MVP v0.2 (#209).
If we do nothing, MVP v0.2 can only be a single-operator demonstration.

---

## Scope

- [x] Block/transaction SCALE encoding: `BlockHeader`, `Block`
- [ ] Hashing rules: unchanged, new domains only
- [x] `apply_block` validity rules: new rules (t)–(w), and state-transition invariants
- [ ] Atomic `write_batch` requirement: unchanged
- [ ] Storage trait semantics: unchanged. Schema bumps for the new block shape
- [x] P2P protocol strings: new version and chain binding. Framing unchanged
- [x] RPC: additive, `get_chain_info`. Block JSON gains two fields
- [x] Transaction signing payload
- [x] Genesis definition (Lock §10)

## Non-Goals

- Validator elections, staking, slashing, rewards, fork choice or finality.
- Rotating the authority set without a new genesis.
- Fees, issuance or any monetary decision. RFC 0003 and RFC 0004 stay non-normative.
- Transaction gossip. See S3 in the MVP specification.
- Membership proofs or light clients.
- Any change to `ComputeTask`, `Receipt` or `task_id`.

---

## Design

### 1. Genesis file and chain identifier

A network is defined by one canonical JSON file:

```json
{
  "schema": "mbongo-genesis:v1",
  "name": "<network name>",
  "timestamp": 0,
  "allocations": [{ "address": "<hex>", "amount": "<decimal string>" }],
  "authorities": ["<hex address>"]
}
```

The file is encoded with the constrained canonical JSON of `mbongo-compute`'s inference
contract: sorted keys, no whitespace, strings and non-negative integers only. A
non-canonical file is refused. Addresses are unique within `allocations` and within
`authorities`. `authorities` is not empty. Amounts fit in `u128`, and their sum too.

`chain_id = BLAKE3("mbongo:chain-id:v1" || canonical_bytes)`, 32 bytes.

The genesis block is computed from the file, at height 0, with `parent_hash` zero,
an empty body, `producer` zero and `producer_signature` zero. Rules (t) and (u) do not
apply to it: a node derives it, and never receives it.

- `mbongo-node --genesis <file>` loads it. `--chain` is removed.
- The node refuses a non-empty data directory created under another `chain_id`.
- A built-in dev genesis stays available for tests under an explicit `--dev` flag,
  which is refused together with `--genesis`. Exactly one of the two is required.
- Allocations are test values. Their meaning is a matter for the network, not for this RFC.

### 2. Transaction signing

The signed bytes become:

```text
"mbongo-tx:v2" || chain_id || SCALE(tx_type, sender, receiver, amount, nonce, payload)
```

The `Transaction` encoding and its hash are unchanged: only the signature's domain
changes. A signature made for one network does not verify on another. `ComputeTask`
and `Receipt` need no change: an anchoring transaction carries a chain-bound signature.
A `Receipt` signature alone does not name a network, and a client that checks one
outside a chain should say so.

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
    pub producer_signature: [u8; 64], // new
}
```

`producer_signature` is Ed25519 over
`BLAKE3("mbongo-block:v1" || chain_id || SCALE(header))`. The header commits to the
body through `transactions_root`. `compute_block_hash` keeps hashing the whole block.

`authorities` is a fixed list. The producer at height `h` is `authorities[h % n]`.
MVP v0.2 uses `n = 1`. A node started with `--producer` must hold that key, through
`--producer-key-file`, and otherwise refuses to start.

### 4. State commitment

`state_root` becomes a hash chain over what each block changes:

```text
state_root(0) = BLAKE3("mbongo:state:v1" || chain_id || SCALE(sorted allocations))
state_root(h) = BLAKE3("mbongo:state:v1" || state_root(h-1) || SCALE(sorted changes))
```

`changes` are the final account, task and receipt records the block writes: one entry
per key, each `(kind, key, SCALE(record))`, sorted by `kind` then `key`. An empty block
has empty `changes`. The cost is proportional to the block, and storage needs no iteration.

The producer applies the block, computes `state_root`, then signs the header.

This detects divergence at the first differing block. **It is not a Merkle root:** it
gives no membership proof and no snapshot sync. A full state tree was considered and
left out: it is much larger, and nothing in this version needs a proof. Light clients
would need a later RFC.

### 5. New validity rules

Evaluated after the existing height and parent checks, before any transaction:

| Rule | Check | Error |
|---|---|---|
| (t) | `header.producer == authorities[height % n]` | `UnauthorizedProducer` |
| (u) | `producer_signature` verifies | `InvalidBlockSignature` |
| (v) | `timestamp >= parent.timestamp` | `InvalidTimestamp` |

Evaluated after every transaction is applied, before the atomic write:

| Rule | Check | Error |
|---|---|---|
| (w) | `state_root` equals the value computed locally | `StateRootMismatch` |

Rule order is fixed as listed, and the first failure rejects the block. The existing
rules and their precedence are otherwise unchanged. `timestamp` has no upper bound: a
wall-clock check would make validity depend on when a block is replayed.

### 6. State-transition invariants

Rule (w) proves that nodes agree, not that the state is right: a transition applied
identically everywhere passes it. These invariants govern `apply_block` for every
accepted transaction and every account involved, and bind every implementation:

| # | Invariant |
|---|---|
| I1 | The total supply is unchanged by every block. Only genesis creates balances |
| I2 | An accepted transaction advances its sender's nonce by exactly one |
| I3 | Its sender's balance falls by exactly what leaves that account, and no account gains more than another loses |
| I4 | Every account a transaction touches is read once and written once, from one view of its state |

A transaction that a correct implementation cannot apply under these invariants is
applied in the way that satisfies them. Validity rules are unchanged by this section.

### 7. P2P and RPC

- `/mbongo-sync/3/<chain_id prefix>` and `/mbongo/block_notify/0.3.0/<chain_id prefix>`,
  with the first 16 hex characters of `chain_id`. Peers on another network fail
  negotiation, before any block is exchanged.
- Message shapes and framing are otherwise unchanged. `Block` carries the new fields.
- A new read method, `get_chain_info`, returns `chain_id`, `genesis_hash` and
  `authorities`. Clients need `chain_id` to sign.
- Block responses gain `producer` and `producer_signature`.

---

## Compatibility

**A clean version boundary, as at v0.3 and v0.4.** No activation height, and no
migration. A v0.5 network starts from a new genesis in an empty directory.

- **Existing nodes:** v0.4 and v0.5 nodes do not negotiate.
- **Existing data:** abandoned. Storage schema 3 → 4.
- **Existing clients:** `@mbongo/sdk` needs a new version for signing and block
  decoding. 0.2.0 stays valid for v0.4 only.
- **`mvp-v0.1.0`:** unaffected. It stays pinned to its release and protocol.

---

## Security

| Threat | Effect of this RFC |
|---|---|
| A peer forges blocks | Refused by (t) and (u) |
| A transaction replayed on another network | Refused by the signing domain |
| Two nodes silently disagreeing on state | Refused by (w), at the first differing block |
| A state transition that breaks an invariant | Refused by review and tests, not at runtime: nodes that agree pass rule (w) |
| An authority key is stolen | **Not addressed.** Replacing it needs a new genesis |
| An authority equivocates | **Not addressed.** Peers keep the first block seen. No fork choice, no slashing |
| An authority censors or stops | **Not addressed.** With `n = 1` the chain halts |

Authority keys become the most sensitive keys on a network. New denial-of-service
paths: signature verification on every block from any peer, which a block-rate limit
in S3 bounds.

## Testing

- Vectors for `chain_id`, the transaction signing domain, the block signature and
  `state_root`, shared with the TypeScript SDK.
- `apply_block` negative tests for (t)–(w), each asserting its own error.
- Two nodes with different genesis files refuse to sync, at negotiation.
- A transaction signed for another `chain_id` is refused.
- Two nodes forced to diverge stop at the first differing block.
- Property tests of I1 to I4 over generated blocks, with every combination of accounts
  a transaction can name, and boundary amounts.
- Replay, devnet convergence and compute vertical harnesses on v0.5.

## Rollout

1. `READY_FOR_REVIEW`, 7-day cooling period, written adversarial review (`RFC_PROCESS.md`).
2. Acceptance. Implementation in `mbongo-core`, `mbongo-node`, `mbongo-network`, then the SDK.
3. Corrections needed for the invariants of section 6 land first, under this RFC.
4. A new protocol lock, `PROTOCOL_LOCK_v0.5.md`, and a `v0.5-devnet-stable` tag. v0.4
   stays as a historical lock. No coordination is needed: there is no network to upgrade.
