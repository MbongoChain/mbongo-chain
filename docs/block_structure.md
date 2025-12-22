# Block Structure

Level: [L2] Detailed specification  
Status: Draft (Foundational)

This document specifies the canonical Mbongo Chain Block structure used across core components and APIs.

## Overview

A `Block` consists of a `header` and a `body`. The header commits to the block contents and chain position; the body holds the ordered transactions.

```
Block {
  header: BlockHeader {
    parent_hash: Hash,          // Linkage to parent block
    state_root: Hash,           // Commitment to post-state after executing body
    transactions_root: Hash,    // Commitment to body.transactions
    timestamp: u64,             // Unix seconds
    height: u64,                // Genesis = 0
  },
  body: BlockBody {
    transactions: Vec<Transaction>, // Opaque transaction bytes
  }
}
```

## Serialization

- All public types derive `serde::{Serialize, Deserialize}`.
- `Hash` is encoded as a hex string with `0x` prefix (32 bytes => 64 hex chars).
- `Transaction` is encoded as a hex string with `0x` prefix (opaque bytes).

Example JSON (pretty-printed):

```json
{
  "header": {
    "parent_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "state_root": "0x0000000000000000000000000000000000000000000000000000000000000000",
    "transactions_root": "0x2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae",
    "timestamp": 1700000000,
    "height": 1
  },
  "body": {
    "transactions": [
      "0x010203",
      "0x0405"
    ]
  }
}
```

## Transactions Root

`transactions_root` is computed via a simple Blake3 commitment over the ordered list of transactions with length-prefixing to avoid ambiguity:

```
root = blake3( concat( for tx in transactions: len(tx) as u32 LE || tx_bytes ) )
```

The helper function `mbongo_core::compute_transactions_root(&[Transaction]) -> Hash` is provided.

## References

- MVP Tasks: `docs/mvp_tasks.md` – Task 2.1.1 (Foundation)
- Core crate: `crates/mbongo-core`
