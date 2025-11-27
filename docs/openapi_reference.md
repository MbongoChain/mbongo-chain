# Mbongo Chain — API Reference

> **Document Type:** OpenAPI-Style Reference  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Version:** v1.0.0  
> **Audience:** API Developers, SDK Integrators, DApp Builders

---

## Table of Contents

1. [Purpose](#1-purpose)
2. [Base URLs](#2-base-urls)
3. [Authentication](#3-authentication)
4. [Core Endpoints](#4-core-endpoints)
5. [JSON Schemas](#5-json-schemas)
6. [Errors](#6-errors)
7. [Cross-Links](#7-cross-links)

---

## 1. Purpose

This document is the canonical API reference for Mbongo Chain. It describes all available endpoints, request/response formats, and error handling.

### 1.1 Compatibility

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         API COMPATIBILITY MATRIX                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   SDK / PLATFORM              │ SUPPORT    │ NOTES                         │
│   ────────────────────────────┼────────────┼───────────────────────────────│
│   TypeScript SDK              │ ✓ Full     │ @mbongo/sdk                   │
│   Rust SDK                    │ ✓ Full     │ mbongo-sdk crate              │
│   Next.js API Routes          │ ✓ Full     │ Server-side integration       │
│   Node.js                     │ ✓ Full     │ Native fetch / axios          │
│   Browser                     │ ✓ Full     │ CORS-enabled endpoints        │
│   Mobile (React Native)       │ ✓ Full     │ Via TypeScript SDK            │
│   Mobile (Swift/Kotlin)       │ ○ Planned  │ Future native SDKs            │
│   Python                      │ ✓ Via RPC  │ JSON-RPC compatible           │
│   Go                          │ ✓ Via RPC  │ JSON-RPC compatible           │
│                                                                             │
│   Legend: ✓ Supported   ○ Planned   ✗ Not supported                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Protocol Support

| Protocol | Format | Use Case |
|----------|--------|----------|
| **JSON-RPC 2.0** | `POST` with JSON body | All operations |
| **REST-like** | `GET/POST` with path params | Query operations |
| **WebSocket** | JSON-RPC over WS | Subscriptions |

### 1.3 Design Principles

- **Deterministic**: Same inputs always produce same outputs
- **Stateless**: No session management required
- **Idempotent**: Safe to retry failed requests
- **Type-Safe**: All responses follow strict schemas

---

## 2. Base URLs

### 2.1 Network Endpoints

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         NETWORK ENDPOINTS                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   MAINNET                                                                   │
│   ───────                                                                   │
│   RPC (HTTP):     https://rpc.mbongo.io                                    │
│   RPC (HTTPS):    https://rpc.mbongo.io                                    │
│   WebSocket:      wss://ws.mbongo.io                                       │
│   Chain ID:       1                                                         │
│                                                                             │
│   TESTNET                                                                   │
│   ───────                                                                   │
│   RPC (HTTP):     https://testnet-rpc.mbongo.io                            │
│   WebSocket:      wss://testnet-ws.mbongo.io                               │
│   Chain ID:       11155111                                                  │
│                                                                             │
│   LOCAL DEVELOPMENT                                                         │
│   ─────────────────                                                         │
│   RPC (HTTP):     http://localhost:8545                                    │
│   WebSocket:      ws://localhost:8546                                      │
│   IPC:            /path/to/mbongo.ipc                                      │
│   Chain ID:       31337                                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 URL Patterns

```
# JSON-RPC (all operations)
POST https://rpc.mbongo.io

# REST-like queries (convenience)
GET  https://rpc.mbongo.io/v1/block/latest
GET  https://rpc.mbongo.io/v1/block/{height}
GET  https://rpc.mbongo.io/v1/tx/{hash}

# WebSocket subscriptions
WSS  wss://ws.mbongo.io
```

### 2.3 Request Headers

```http
Content-Type: application/json
Accept: application/json

# Optional: API key for rate limit increase
X-API-Key: your-api-key

# Optional: Request tracing
X-Request-ID: uuid-v4
```

---

## 3. Authentication

### 3.1 Blockchain-Native Authentication

Mbongo Chain uses **wallet-based authentication** instead of traditional API keys:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AUTHENTICATION MODEL                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   READ OPERATIONS (No Auth Required)                                       │
│   ══════════════════════════════════                                       │
│   • Query blocks                                                           │
│   • Query transactions                                                     │
│   • Query balances                                                         │
│   • Query validators                                                       │
│   • Query governance proposals                                             │
│                                                                             │
│   WRITE OPERATIONS (Signature Required)                                    │
│   ═════════════════════════════════════                                    │
│   • Send transactions         → Signed by sender wallet                   │
│   • Stake/Unstake            → Signed by validator/delegator              │
│   • Submit compute proofs    → Signed by compute provider                 │
│   • Vote on proposals        → Signed by token holder                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Transaction Signing

All write operations require a signed transaction:

```typescript
// Transaction structure
interface SignedTransaction {
  // Transaction data
  from: Address;
  to: Address;
  value: BigInt;
  data: HexString;
  nonce: number;
  gasLimit: BigInt;
  maxFeePerGas: BigInt;
  maxPriorityFeePerGas: BigInt;
  chainId: number;
  
  // Signature (ECDSA)
  v: number;
  r: HexString;
  s: HexString;
}
```

### 3.3 Signature Verification

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SIGNING FLOW                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   1. BUILD              2. HASH              3. SIGN              4. SEND  │
│   ┌─────────┐          ┌─────────┐          ┌─────────┐          ┌───────┐│
│   │   TX    │─────────▶│Keccak256│─────────▶│ ECDSA   │─────────▶│ Node  ││
│   │  Data   │          │  Hash   │          │  Sign   │          │  RPC  ││
│   └─────────┘          └─────────┘          └─────────┘          └───────┘│
│                                                                             │
│   Client builds         Hash the            Sign with             Submit   │
│   transaction           RLP-encoded         private key           to node  │
│   payload               transaction         (v, r, s)                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**References:**
- [cli_wallet.md](./cli_wallet.md) — CLI wallet commands
- [ts_sdk_overview.md](./ts_sdk_overview.md) — TypeScript signing
- [rust_sdk_overview.md](./rust_sdk_overview.md) — Rust signing

---

## 4. Core Endpoints

### 4.1 Blocks

#### GET /block/latest

Get the latest block.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "eth_getBlockByNumber",
  "params": ["latest", false],
  "id": 1
}
```

**REST:**
```http
GET /v1/block/latest
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "number": "0xbc614e",
    "hash": "0x1234567890abcdef...",
    "parentHash": "0xabcdef1234567890...",
    "timestamp": "0x6567a8b0",
    "miner": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
    "gasUsed": "0x5208",
    "gasLimit": "0x1c9c380",
    "transactions": ["0xtx1...", "0xtx2..."],
    "pouwScore": "0x64",
    "receiptsRoot": "0x...",
    "stateRoot": "0x..."
  }
}
```

---

#### GET /block/{height}

Get block by height.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "eth_getBlockByNumber",
  "params": ["0xbc614e", true],
  "id": 1
}
```

**REST:**
```http
GET /v1/block/12345678
GET /v1/block/0xbc614e
```

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `height` | `number \| hex` | Yes | Block number (decimal or hex) |
| `full` | `boolean` | No | Include full transactions (default: false) |

**Response:** Same as `/block/latest`

---

#### GET /block/{height}/receipts

Get compute receipts for a block.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_getComputeReceipts",
  "params": ["0xbc614e"],
  "id": 1
}
```

**REST:**
```http
GET /v1/block/12345678/receipts
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "taskId": "0xdef456...",
      "provider": "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7",
      "resultHash": "0x789abc...",
      "workUnits": "0x5f5e100",
      "timestamp": 1701388800,
      "signature": "0x...",
      "attesters": ["0x...", "0x..."]
    }
  ]
}
```

---

### 4.2 Transactions

#### POST /tx/send

Submit a signed transaction.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "eth_sendRawTransaction",
  "params": ["0xf86c808504a817c80082520894..."],
  "id": 1
}
```

**REST:**
```http
POST /v1/tx/send
Content-Type: application/json

{
  "signedTransaction": "0xf86c808504a817c80082520894..."
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
}
```

**Errors:**

| Code | Message | Description |
|------|---------|-------------|
| `-32000` | `INVALID_SIGNATURE` | Signature verification failed |
| `-32001` | `NONCE_TOO_LOW` | Transaction nonce already used |
| `-32002` | `INSUFFICIENT_FUNDS` | Not enough MBO for value + gas |
| `-32003` | `GAS_LIMIT_EXCEEDED` | Gas limit exceeds block limit |

---

#### GET /tx/{hash}/status

Get transaction status and receipt.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "eth_getTransactionReceipt",
  "params": ["0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"],
  "id": 1
}
```

**REST:**
```http
GET /v1/tx/0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b/status
```

**Response (Pending):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": null
}
```

**Response (Confirmed):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "transactionHash": "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b",
    "blockNumber": "0xbc614e",
    "blockHash": "0x1234567890abcdef...",
    "transactionIndex": "0x0",
    "from": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
    "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7",
    "gasUsed": "0x5208",
    "cumulativeGasUsed": "0x5208",
    "status": "0x1",
    "logs": []
  }
}
```

---

### 4.3 Mempool

#### GET /mempool/pending

Get pending transactions in mempool.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "txpool_content",
  "params": [],
  "id": 1
}
```

**REST:**
```http
GET /v1/mempool/pending
GET /v1/mempool/pending?limit=100&from=0x8Ba1f109551bD432803012645Ac136ddd64DBA72
```

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `limit` | `number` | No | Max transactions to return (default: 100) |
| `from` | `address` | No | Filter by sender address |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "pending": {
      "0x8Ba1f109551bD432803012645Ac136ddd64DBA72": {
        "0": {
          "hash": "0x...",
          "nonce": "0x0",
          "from": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
          "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7",
          "value": "0x56bc75e2d63100000",
          "gasPrice": "0x4a817c800"
        }
      }
    },
    "queued": {}
  }
}
```

---

#### GET /mempool/stats

Get mempool statistics.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "txpool_status",
  "params": [],
  "id": 1
}
```

**REST:**
```http
GET /v1/mempool/stats
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "pending": "0x10",
    "queued": "0x5",
    "baseFee": "0x3b9aca00",
    "nextBaseFee": "0x3b9aca00"
  }
}
```

---

### 4.4 Validators (PoS)

#### GET /validators

Get active validator set.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_getValidators",
  "params": [],
  "id": 1
}
```

**REST:**
```http
GET /v1/validators
GET /v1/validators?epoch=latest&active=true
```

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `epoch` | `number \| "latest"` | No | Epoch number (default: latest) |
| `active` | `boolean` | No | Only active validators (default: true) |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "epoch": 1234,
    "validators": [
      {
        "address": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
        "stake": "0xa968163f0a57b400000",
        "commission": 500,
        "uptime": 9998,
        "status": "active",
        "delegators": 150,
        "totalDelegated": "0x21e19e0c9bab2400000"
      }
    ],
    "totalStake": "0x52b7d2dcc80cd2e4000000"
  }
}
```

---

#### GET /validators/{id}

Get validator details.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_getValidator",
  "params": ["0x8Ba1f109551bD432803012645Ac136ddd64DBA72"],
  "id": 1
}
```

**REST:**
```http
GET /v1/validators/0x8Ba1f109551bD432803012645Ac136ddd64DBA72
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "address": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
    "stake": "0xa968163f0a57b400000",
    "commission": 500,
    "uptime": 9998,
    "status": "active",
    "blocksProposed": 12500,
    "blocksAttested": 125000,
    "slashingHistory": [],
    "delegators": [
      {
        "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7",
        "amount": "0x56bc75e2d63100000"
      }
    ],
    "rewards": {
      "total": "0x1a055690d9db80000",
      "pending": "0xde0b6b3a7640000"
    }
  }
}
```

---

#### POST /validators/stake

Stake MBO (requires signed transaction).

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_stake",
  "params": [{
    "amount": "0xa968163f0a57b400000",
    "signature": "0x..."
  }],
  "id": 1
}
```

**REST:**
```http
POST /v1/validators/stake
Content-Type: application/json

{
  "signedTransaction": "0xf86c808504a817c80082520894..."
}
```

**Parameters (in transaction data):**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `amount` | `BigInt` | Yes | Amount to stake (min: 50,000 MBO) |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "txHash": "0x...",
    "validator": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
    "newStake": "0xa968163f0a57b400000"
  }
}
```

---

#### POST /validators/unstake

Begin unstaking (requires signed transaction).

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_unstake",
  "params": [{
    "amount": "0x56bc75e2d63100000",
    "signature": "0x..."
  }],
  "id": 1
}
```

**REST:**
```http
POST /v1/validators/unstake
Content-Type: application/json

{
  "signedTransaction": "0xf86c808504a817c80082520894..."
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "txHash": "0x...",
    "validator": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
    "amount": "0x56bc75e2d63100000",
    "unbondingCompletesAt": 1703980800
  }
}
```

---

### 4.5 Compute Providers (PoUW)

#### POST /compute/submit

Submit compute receipt (requires signed receipt).

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_submitComputeReceipt",
  "params": [{
    "taskId": "0xdef456...",
    "resultHash": "0x789abc...",
    "workUnits": "0x5f5e100",
    "timestamp": 1701388800,
    "signature": "0x..."
  }],
  "id": 1
}
```

**REST:**
```http
POST /v1/compute/submit
Content-Type: application/json

{
  "receipt": {
    "taskId": "0xdef456...",
    "resultHash": "0x789abc...",
    "workUnits": "0x5f5e100",
    "timestamp": 1701388800,
    "signature": "0x..."
  }
}
```

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `taskId` | `Hash` | Yes | Task identifier |
| `resultHash` | `Hash` | Yes | Hash of computation result |
| `workUnits` | `BigInt` | Yes | Verified work units |
| `timestamp` | `number` | Yes | Unix timestamp of completion |
| `signature` | `Signature` | Yes | Provider signature |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "receiptId": "0x...",
    "status": "accepted",
    "reward": "0xde0b6b3a7640000"
  }
}
```

**Errors:**

| Code | Message | Description |
|------|---------|-------------|
| `-32010` | `INVALID_RECEIPT` | Receipt validation failed |
| `-32011` | `TASK_NOT_FOUND` | Task ID does not exist |
| `-32012` | `DUPLICATE_RECEIPT` | Receipt already submitted |
| `-32013` | `PROVIDER_NOT_REGISTERED` | Provider not in registry |

---

#### GET /compute/receipt/{id}

Get compute receipt by ID.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_getComputeReceipt",
  "params": ["0xdef456..."],
  "id": 1
}
```

**REST:**
```http
GET /v1/compute/receipt/0xdef456...
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "taskId": "0xdef456...",
    "provider": "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7",
    "resultHash": "0x789abc...",
    "workUnits": "0x5f5e100",
    "timestamp": 1701388800,
    "blockNumber": "0xbc614e",
    "reward": "0xde0b6b3a7640000",
    "status": "verified",
    "attestations": 3
  }
}
```

---

#### GET /compute/tasks/pending

Get pending compute tasks.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_getPendingComputeTasks",
  "params": [{
    "types": ["inference", "rendering"],
    "minReward": "0x2386f26fc10000"
  }],
  "id": 1
}
```

**REST:**
```http
GET /v1/compute/tasks/pending?types=inference,rendering&minReward=0x2386f26fc10000
```

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `types` | `string[]` | No | Filter by task type |
| `minReward` | `BigInt` | No | Minimum reward threshold |
| `limit` | `number` | No | Max tasks to return (default: 50) |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "taskId": "0xabc123...",
      "type": "inference",
      "submitter": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
      "reward": "0xde0b6b3a7640000",
      "deadline": 1701392400,
      "requirements": {
        "minGpu": "RTX 3080",
        "minVram": 10240
      },
      "inputHash": "0x..."
    }
  ]
}
```

---

### 4.6 Governance

#### GET /governance/proposals

Get governance proposals.

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_getProposals",
  "params": [{
    "state": "active"
  }],
  "id": 1
}
```

**REST:**
```http
GET /v1/governance/proposals?state=active
```

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `state` | `string` | No | Filter: `pending`, `active`, `passed`, `rejected`, `executed` |
| `proposer` | `address` | No | Filter by proposer |
| `limit` | `number` | No | Max proposals (default: 20) |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "id": 42,
      "title": "Increase block gas limit",
      "description": "Proposal to increase block gas limit to 30M",
      "proposer": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
      "state": "active",
      "tier": 2,
      "votesFor": "0x52b7d2dcc80cd2e4000000",
      "votesAgainst": "0x21e19e0c9bab2400000",
      "quorum": "0x295be96e64066972000000",
      "startBlock": "0xbc5000",
      "endBlock": "0xbc9000",
      "actions": [
        {
          "target": "0x...",
          "value": "0x0",
          "calldata": "0x..."
        }
      ]
    }
  ]
}
```

---

#### POST /governance/propose

Create new proposal (requires signed transaction).

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_propose",
  "params": [{
    "title": "Increase block gas limit",
    "description": "Proposal to increase block gas limit to 30M",
    "tier": 2,
    "actions": [{
      "target": "0x...",
      "value": "0x0",
      "calldata": "0x..."
    }],
    "signature": "0x..."
  }],
  "id": 1
}
```

**REST:**
```http
POST /v1/governance/propose
Content-Type: application/json

{
  "signedTransaction": "0xf86c808504a817c80082520894..."
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "proposalId": 43,
    "txHash": "0x...",
    "startBlock": "0xbc6000",
    "endBlock": "0xbca000"
  }
}
```

---

#### POST /governance/vote

Vote on proposal (requires signed transaction).

**JSON-RPC:**
```json
{
  "jsonrpc": "2.0",
  "method": "mbongo_vote",
  "params": [{
    "proposalId": 42,
    "support": true,
    "signature": "0x..."
  }],
  "id": 1
}
```

**REST:**
```http
POST /v1/governance/vote
Content-Type: application/json

{
  "signedTransaction": "0xf86c808504a817c80082520894..."
}
```

**Parameters (in transaction data):**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `proposalId` | `number` | Yes | Proposal ID |
| `support` | `boolean` | Yes | `true` = for, `false` = against |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "txHash": "0x...",
    "proposalId": 42,
    "voter": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
    "votingPower": "0x56bc75e2d63100000",
    "support": true
  }
}
```

---

## 5. JSON Schemas

### 5.1 Block

```typescript
interface Block {
  // Header
  number: HexNumber;           // Block height
  hash: Hash;                  // Block hash
  parentHash: Hash;            // Parent block hash
  timestamp: HexNumber;        // Unix timestamp
  
  // Proposer
  miner: Address;              // Block proposer address
  
  // Gas
  gasUsed: HexNumber;          // Total gas used
  gasLimit: HexNumber;         // Block gas limit
  baseFeePerGas: HexNumber;    // Base fee (EIP-1559)
  
  // State
  stateRoot: Hash;             // State trie root
  receiptsRoot: Hash;          // Receipts trie root
  transactionsRoot: Hash;      // Transactions trie root
  
  // PoUW
  pouwScore: HexNumber;        // Aggregate PoUW score
  
  // Transactions
  transactions: Hash[] | Transaction[];  // Tx hashes or full txs
}
```

### 5.2 Transaction

```typescript
interface Transaction {
  // Identity
  hash: Hash;                  // Transaction hash
  nonce: HexNumber;            // Sender nonce
  
  // Parties
  from: Address;               // Sender address
  to: Address | null;          // Recipient (null for contract creation)
  
  // Value
  value: HexNumber;            // MBO in wei
  
  // Gas
  gas: HexNumber;              // Gas limit
  gasPrice?: HexNumber;        // Legacy gas price
  maxFeePerGas?: HexNumber;    // EIP-1559 max fee
  maxPriorityFeePerGas?: HexNumber;  // EIP-1559 priority fee
  
  // Data
  input: HexString;            // Call data
  
  // Signature
  v: HexNumber;                // Recovery ID
  r: Hash;                     // ECDSA r
  s: Hash;                     // ECDSA s
  
  // Metadata (when included)
  blockHash?: Hash;
  blockNumber?: HexNumber;
  transactionIndex?: HexNumber;
}
```

### 5.3 TransactionReceipt

```typescript
interface TransactionReceipt {
  // Identity
  transactionHash: Hash;
  transactionIndex: HexNumber;
  
  // Block
  blockHash: Hash;
  blockNumber: HexNumber;
  
  // Parties
  from: Address;
  to: Address | null;
  contractAddress: Address | null;  // If contract creation
  
  // Execution
  status: HexNumber;           // 0x1 = success, 0x0 = failure
  gasUsed: HexNumber;
  cumulativeGasUsed: HexNumber;
  effectiveGasPrice: HexNumber;
  
  // Logs
  logs: Log[];
  logsBloom: HexString;
}
```

### 5.4 Validator

```typescript
interface Validator {
  // Identity
  address: Address;
  
  // Stake
  stake: HexNumber;            // Self-stake in wei
  totalDelegated: HexNumber;   // Delegated stake
  
  // Performance
  uptime: number;              // Uptime in basis points (10000 = 100%)
  commission: number;          // Commission in basis points
  blocksProposed: number;
  blocksAttested: number;
  
  // Status
  status: "pending" | "active" | "exiting" | "slashed";
  
  // Delegators
  delegators: number;
  
  // Slashing
  slashingHistory: SlashingEvent[];
}

interface SlashingEvent {
  epoch: number;
  reason: "double_sign" | "downtime";
  amount: HexNumber;
  timestamp: number;
}
```

### 5.5 ComputeTask

```typescript
interface ComputeTask {
  // Identity
  taskId: Hash;
  type: "inference" | "rendering" | "zk_proof" | "encoding" | "training";
  
  // Submitter
  submitter: Address;
  
  // Economics
  reward: HexNumber;           // Reward in wei
  deadline: number;            // Unix timestamp
  
  // Requirements
  requirements: {
    minGpu?: string;           // e.g., "RTX 3080"
    minVram?: number;          // In MB
    estimatedWorkUnits?: HexNumber;
  };
  
  // Data
  inputHash: Hash;             // Hash of input data
  
  // Status
  status: "pending" | "assigned" | "completed" | "expired";
  assignedTo?: Address;        // Assigned provider
}
```

### 5.6 ComputeReceipt

```typescript
interface ComputeReceipt {
  // Identity
  taskId: Hash;
  receiptId?: Hash;
  
  // Provider
  provider: Address;
  
  // Result
  resultHash: Hash;
  workUnits: HexNumber;
  executionTime?: number;      // In milliseconds
  
  // Timing
  timestamp: number;           // Completion timestamp
  
  // Verification
  signature: Signature;
  attesters: Address[];
  attestations: number;
  
  // On-chain (after inclusion)
  blockNumber?: HexNumber;
  reward?: HexNumber;
  status?: "pending" | "verified" | "rejected";
}
```

---

## 6. Errors

### 6.1 Standard Error Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "INVALID_SIGNATURE",
    "data": {
      "expected": "0x8Ba1f109551bD432803012645Ac136ddd64DBA72",
      "recovered": "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7"
    }
  }
}
```

### 6.2 Error Codes Table

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ERROR CODES                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   CODE     │ NAME                    │ DESCRIPTION                         │
│   ─────────┼─────────────────────────┼─────────────────────────────────────│
│                                                                             │
│   STANDARD JSON-RPC ERRORS                                                  │
│   ────────────────────────                                                  │
│   -32700   │ PARSE_ERROR            │ Invalid JSON                         │
│   -32600   │ INVALID_REQUEST        │ Invalid request object               │
│   -32601   │ METHOD_NOT_FOUND       │ Method does not exist                │
│   -32602   │ INVALID_PARAMS         │ Invalid method parameters            │
│   -32603   │ INTERNAL_ERROR         │ Internal JSON-RPC error              │
│                                                                             │
│   TRANSACTION ERRORS (-32000 to -32009)                                    │
│   ─────────────────────────────────────                                    │
│   -32000   │ INVALID_SIGNATURE      │ Signature verification failed        │
│   -32001   │ NONCE_TOO_LOW          │ Transaction nonce already used       │
│   -32002   │ INSUFFICIENT_FUNDS     │ Not enough MBO for value + gas       │
│   -32003   │ GAS_LIMIT_EXCEEDED     │ Gas limit exceeds block limit        │
│   -32004   │ GAS_PRICE_TOO_LOW      │ Gas price below minimum              │
│   -32005   │ TX_UNDERPRICED         │ Transaction underpriced              │
│   -32006   │ TX_POOL_FULL           │ Transaction pool is full             │
│   -32007   │ REPLACEMENT_UNDERPRICED│ Replacement tx gas too low           │
│   -32008   │ INVALID_CHAIN_ID       │ Wrong chain ID                       │
│   -32009   │ TX_ALREADY_KNOWN       │ Transaction already in pool          │
│                                                                             │
│   COMPUTE ERRORS (-32010 to -32019)                                        │
│   ─────────────────────────────────                                        │
│   -32010   │ INVALID_RECEIPT        │ Compute receipt validation failed    │
│   -32011   │ TASK_NOT_FOUND         │ Task ID does not exist               │
│   -32012   │ DUPLICATE_RECEIPT      │ Receipt already submitted            │
│   -32013   │ PROVIDER_NOT_REGISTERED│ Provider not in registry             │
│   -32014   │ PROVIDER_SLASHED       │ Provider has been slashed            │
│   -32015   │ TASK_EXPIRED           │ Task deadline passed                 │
│   -32016   │ INVALID_WORK_PROOF     │ Work proof verification failed       │
│                                                                             │
│   STAKING ERRORS (-32020 to -32029)                                        │
│   ─────────────────────────────────                                        │
│   -32020   │ STAKE_TOO_LOW          │ Below minimum stake (50,000 MBO)     │
│   -32021   │ ALREADY_VALIDATOR      │ Address already registered           │
│   -32022   │ NOT_VALIDATOR          │ Address is not a validator           │
│   -32023   │ UNBONDING_IN_PROGRESS  │ Already unbonding                    │
│   -32024   │ SLASHED_VALIDATOR      │ Validator has been slashed           │
│   -32025   │ INVALID_DELEGATION     │ Delegation validation failed         │
│                                                                             │
│   GOVERNANCE ERRORS (-32030 to -32039)                                     │
│   ─────────────────────────────────────                                    │
│   -32030   │ PROPOSAL_NOT_FOUND     │ Proposal ID does not exist           │
│   -32031   │ VOTING_CLOSED          │ Voting period has ended              │
│   -32032   │ ALREADY_VOTED          │ Address already voted                │
│   -32033   │ INSUFFICIENT_VOTING_POWER │ Not enough voting power          │
│   -32034   │ INVALID_PROPOSAL_TIER  │ Proposal tier mismatch               │
│   -32035   │ QUORUM_NOT_REACHED     │ Quorum not reached                   │
│                                                                             │
│   ACCESS ERRORS (-32040 to -32049)                                         │
│   ────────────────────────────────                                         │
│   -32040   │ FORBIDDEN              │ Operation not permitted              │
│   -32041   │ RATE_LIMITED           │ Too many requests                    │
│   -32042   │ RESOURCE_NOT_FOUND     │ Requested resource not found         │
│                                                                             │
│   NODE ERRORS (-32050 to -32059)                                           │
│   ──────────────────────────────                                           │
│   -32050   │ NODE_SYNCING           │ Node is syncing                      │
│   -32051   │ BLOCK_NOT_FOUND        │ Block not found                      │
│   -32052   │ STATE_NOT_AVAILABLE    │ State not available at height        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 HTTP Status Codes

| HTTP Code | Meaning | When Used |
|-----------|---------|-----------|
| `200` | OK | Successful request (even if RPC returns error) |
| `400` | Bad Request | Malformed JSON |
| `401` | Unauthorized | Invalid API key (if required) |
| `403` | Forbidden | IP blocked or rate limited |
| `404` | Not Found | Invalid endpoint path |
| `429` | Too Many Requests | Rate limit exceeded |
| `500` | Internal Server Error | Node error |
| `503` | Service Unavailable | Node syncing or offline |

---

## 7. Cross-Links

### API & SDK Documentation

| Document | Description |
|----------|-------------|
| [rpc_overview.md](./rpc_overview.md) | JSON-RPC protocol details |
| [rust_sdk_overview.md](./rust_sdk_overview.md) | Rust SDK reference |
| [ts_sdk_overview.md](./ts_sdk_overview.md) | TypeScript SDK reference |

### CLI Documentation

| Document | Description |
|----------|-------------|
| [cli_overview.md](./cli_overview.md) | CLI commands overview |
| [cli_wallet.md](./cli_wallet.md) | Wallet management |
| [cli_node.md](./cli_node.md) | Node management |

### Architecture Documentation

| Document | Description |
|----------|-------------|
| [compute_engine_overview.md](./compute_engine_overview.md) | PoUW compute engine |
| [staking_model.md](./staking_model.md) | Staking mechanics |
| [governance_model.md](./governance_model.md) | Governance system |
| [fee_model.md](./fee_model.md) | Gas and fees |

### Quick Integration Checklist

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         INTEGRATION CHECKLIST                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ☐ Choose RPC endpoint (mainnet/testnet/local)                            │
│   ☐ Install SDK (@mbongo/sdk or mbongo-sdk crate)                          │
│   ☐ Configure chain ID                                                     │
│   ☐ Set up wallet/signing                                                  │
│   ☐ Handle error codes properly                                            │
│   ☐ Implement retry logic for transient errors                             │
│   ☐ Subscribe to events (WebSocket) if needed                              │
│   ☐ Test on testnet before mainnet                                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides the canonical API reference for Mbongo Chain. For SDK usage, see [ts_sdk_overview.md](./ts_sdk_overview.md) or [rust_sdk_overview.md](./rust_sdk_overview.md).*

