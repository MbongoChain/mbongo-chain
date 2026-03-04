# Mbongo Chain RPC Specification v0.1

**Status:** FROZEN  
**Breaking changes require version bump.**

---

## Overview

JSON-RPC 2.0 over HTTP POST. Endpoint: `/rpc`. Content-Type: `application/json`.

---

## Methods

### submit_transaction

Submits a signed transaction for inclusion in a future block.

| Field   | Value                    |
|---------|--------------------------|
| Method  | `submit_transaction`     |
| Params  | `[signed_tx: string]` — hex-encoded signed transaction |
| Returns | `{ tx_hash: string }`    |

**Error cases:**

- `-32602` Invalid params (malformed hex, invalid SCALE)
- `-32000` Invalid signature
- `-32000` Invalid nonce
- `-32000` Insufficient balance

**Idempotent:** Re-submitting the same transaction returns the same tx_hash. No duplicate enqueue.

---

### produce_block

Builds and persists a block from the current mempool. Manual production only.

| Field   | Value                    |
|---------|--------------------------|
| Method  | `produce_block`          |
| Params  | `[]` or `[max_txs: u32]` — optional limit on transactions included |
| Returns | `{ block_hash: string, height: u64 }` |

**Error cases:**

- `-32603` Internal error (storage failure)
- `-32000` No valid transactions in mempool (optional; implementation may return empty block)

---

### get_block_height

Returns the latest finalized block height.

| Field   | Value                    |
|---------|--------------------------|
| Method  | `get_block_height`       |
| Params  | `[]`                     |
| Returns | `u64`                    |

**Error cases:**

- `-32603` Internal error (storage unavailable)

---

### ping

Liveness check. No side effects.

| Field   | Value                    |
|---------|--------------------------|
| Method  | `ping`                   |
| Params  | `[]`                     |
| Returns | `{ ok: true }`           |

**Error cases:** None expected.
