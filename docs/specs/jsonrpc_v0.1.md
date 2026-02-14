# Mbongo Chain JSON-RPC Specification v0.1

## 1. Overview

- JSON-RPC 2.0 over HTTP
- Endpoint: `POST /rpc`
- Content-Type: `application/json`
- All methods prefixed with `mbg_`

## 2. Standard JSON-RPC Format

### Request Example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "mbg_getBlockNumber",
  "params": []
}
```

### Success Response Example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": 12345
}
```

### Error Response Example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": "address: invalid format"
  }
}
```

**Reserved error codes:**

| Code   | Meaning           | Scope        |
|--------|-------------------|--------------|
| -32600 | Invalid Request   | JSON-RPC 2.0 |
| -32601 | Method Not Found  | JSON-RPC 2.0 |
| -32602 | Invalid Params    | JSON-RPC 2.0 |
| -32603 | Internal Error    | JSON-RPC 2.0 |
| -32000 | Execution Error   | Mbongo-specific |

## 3. Core v0.1 Methods

### 3.1 mbg_getBlockNumber

Returns the latest finalized block number.

**Params:** `[]` (none)

**Example request:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "mbg_getBlockNumber",
  "params": []
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": 12345
}
```

---

### 3.2 mbg_getBlockByNumber

Returns a minimal block object for the given block number.

**Params:** `[blockNumber: u64]`

**Example request:**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "mbg_getBlockByNumber",
  "params": [12345]
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "number": 12345,
    "hash": "0x1a2b3c4d5e6f...",
    "parentHash": "0x0a1b2c3d4e5f...",
    "timestamp": 1704067200
  }
}
```

---

### 3.3 mbg_getAccount

Returns account state for the given address.

**Params:** `[address: string]`

**Example request:**

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "mbg_getAccount",
  "params": ["0x1234567890abcdef..."]
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "address": "0x1234567890abcdef...",
    "balance": "1000000",
    "nonce": 42,
    "validator_data": {
      "stake": "50000",
      "is_active": true,
      "compute_score": 1200
    }
  }
}
```

For non-validator accounts, `validator_data` is `null`:

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "address": "0x1234567890abcdef...",
    "balance": "1000000",
    "nonce": 42,
    "validator_data": null
  }
}
```

---

### 3.4 mbg_sendTransaction

Submits a signed transaction. Returns the transaction hash.

**Params:** `[signedTx: string]` — hex-encoded signed transaction

**Example request:**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "mbg_sendTransaction",
  "params": ["0xdeadbeef1234..."]
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "txHash": "0xabcdef1234567890..."
  }
}
```

---

### 3.5 mbg_getTransaction

Returns transaction status and execution result.

**Params:** `[txHash: string]`

**Example request:**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "mbg_getTransaction",
  "params": ["0xabcdef1234567890..."]
}
```

**Example response (confirmed):**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "status": "confirmed",
    "blockNumber": 12346,
    "executionResult": {
      "success": true
    }
  }
}
```

**Example response (pending):**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "status": "pending",
    "blockNumber": null,
    "executionResult": null
  }
}
```

**Example response (failed):**

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "status": "failed",
    "blockNumber": 12346,
    "executionResult": {
      "success": false,
      "error": "insufficient balance"
    }
  }
}
```

---

### 3.6 mbg_getValidatorSet

Returns the current validator set.

**Params:** `[]` (none)

**Example request:**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "mbg_getValidatorSet",
  "params": []
}
```

**Example response:**

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": [
    {
      "address": "0x1111...",
      "stake": "100000",
      "active": true
    },
    {
      "address": "0x2222...",
      "stake": "80000",
      "active": true
    }
  ]
}
```

---

## 4. Data Structures

### Block

| Field       | Type   | Description                    |
|-------------|--------|--------------------------------|
| number      | u64    | Block height                   |
| hash        | string | Block hash (hex)               |
| parentHash  | string | Parent block hash (hex)        |
| timestamp   | u64    | Unix timestamp (seconds)       |

### Account

| Field          | Type          | Description                    |
|----------------|---------------|--------------------------------|
| address        | string        | Account address (hex)          |
| balance        | string        | Balance (decimal string)       |
| nonce          | u64           | Transaction nonce              |
| validator_data | ValidatorData \| null | Validator info if applicable |

### ValidatorData

| Field         | Type   | Description                    |
|---------------|--------|--------------------------------|
| stake         | string | Staked amount (decimal string) |
| is_active     | bool   | Whether validator is active    |
| compute_score | u64    | Proof-of-compute score         |

### TransactionStatus

| Value      | Description                          |
|------------|--------------------------------------|
| pending    | In mempool, not yet included         |
| confirmed  | Included in a finalized block        |
| failed     | Included but execution failed        |

---

## 5. Non-Goals (Explicitly State)

The following are **not** in scope for v0.1:

- No event subscriptions
- No WebSocket support
- No gas model defined yet
- No smart contract interface yet
- No historical state queries
- No pagination support yet

---

## 6. Versioning Policy

- v0.1 is **unstable**. Breaking changes are allowed.
- Breaking changes are permitted until v1.0.
- Method removals require a deprecation note in a prior release.
