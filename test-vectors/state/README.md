# State commitment vectors

`state-root-v1.json` holds neutral, language-agnostic vectors for the state
commitment of RFC 0006 §4 and rule (w) of §5. Neither implementation owns it.

## The rules

```text
state_root(0) = BLAKE3("mbongo:state:v1" || chain_id || SCALE(allocations))
state_root(h) = BLAKE3("mbongo:state:v1" || state_root(h-1) || SCALE(changes))
```

- `allocations` is `Vec<(Address, u128)>`, in file order, which RFC 0006 §1
  makes ascending by address.
- `changes` is `Vec<(u8, [u8; 32], Vec<u8>)>`, sorted by kind, then by key
  bytewise. There is one entry per key, holding the value after the whole
  block.

| Kind | Key | Value | Present when |
|---|---|---|---|
| 0 | Address | `SCALE(Account)` | The account is the sender of an applied transaction, or the receiver of an applied `Transfer` or `Stake`, even if unchanged |
| 1 | `task_id` | `SCALE(ComputeTask)` | The block registers the task |
| 2 | `task_id` | `SCALE(Receipt)` | The block anchors the receipt |

A skipped transaction, a stored `Transfer` or `Stake` included again,
contributes nothing. An empty block has empty `changes`, encoded as one byte,
`0x00`.

`SCALE(Account)` is `address || balance (u128 LE) || nonce (u64 LE)`, 56
bytes, as `test-vectors/account/account-v1.json` pins it.

## Contents

**`genesis`:** a canonical genesis file whose allocations sum to exactly
`u128::MAX`, with its `chain_id`, allocation encoding and `state_root(0)`.

**`blocks`, 9 blocks**, each listing its transactions, its `changes` entry
by entry, their encoding, and the parent and resulting roots:

1. an empty block;
2. a transfer;
3. a self-transfer of the whole balance: one entry;
4. a transfer of 0 to an account that did not exist: it is created;
5. a stake of the whole balance;
6. a task and its receipt in one block: kind 1 and kind 2 entries with the
   same key;
7. a second task;
8. only a stored transfer, repeated: skipped, so `changes` is empty;
9. a mixed block: entries in key order, not body order, and one account
   changed twice holding its final value.

**`production`:** five candidates for the next block. Three fail and are left
out with no effect, including no nonce consumed; the root is that of the two
included.

## Derivation

Derived by a script independent of the Rust crates and of the SDK: BLAKE3
written from its specification, Ed25519 public keys from OpenSSL, SCALE
written by hand, and the entries of each block from a model of RFC 0006 §6
and §4 written from the RFC text. Task and receipt bytes are inputs, from
`compute-task-v1.json` and `anchor-binding-v2.json`, and are checked against
each other.

## Consumers

- `crates/mbongo-core/src/state.rs`: recomputes every encoding and root from
  the listed entries.
- `crates/mbongo-node/src/backend/tests/state_commitment.rs`: signs the
  transactions for the file's `chain_id`, produces each block on a real node,
  checks the node's `state_root` and entries against the file, and has a
  second node apply every block.

The test keys are public. They are for these vectors only.
