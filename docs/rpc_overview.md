# Mbongo Chain — RPC Overview

> **Document Type:** API Reference  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Audience:** Developers, Integrators, Node Operators

---

## Table of Contents

1. [Purpose](#1-purpose)
2. [Supported Protocols](#2-supported-protocols)
3. [JSON-RPC Format](#3-json-rpc-format)
4. [Core RPC Methods](#4-core-rpc-methods)
5. [WebSocket Channels](#5-websocket-channels)
6. [Examples](#6-examples)
7. [Security Notes](#7-security-notes)
8. [Cross-Links](#8-cross-links)

---

## 1. Purpose

### 1.1 What is the Mbongo RPC API?

The Mbongo RPC API provides programmatic access to Mbongo Chain nodes. It enables applications to query blockchain state, submit transactions, monitor events, and interact with PoS/PoUW consensus mechanisms.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RPC API CAPABILITIES                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   QUERY OPERATIONS                      TRANSACTION OPERATIONS              │
│   ════════════════                      ══════════════════════              │
│   • Get block by hash/number            • Submit signed transactions       │
│   • Get transaction by hash             • Estimate gas costs               │
│   • Get account balance                 • Get transaction receipt          │
│   • Get nonce                           • Call contract methods            │
│   • Get gas price                       • Simulate execution               │
│                                                                             │
│   SUBSCRIPTION OPERATIONS               NODE OPERATIONS                     │
│   ═══════════════════════               ════════════════                    │
│   • New block headers                   • Get node info                    │
│   • Pending transactions                • Get peer count                   │
│   • Log events                          • Get sync status                  │
│   • Compute receipts                    • Get network ID                   │
│                                                                             │
│   PoS OPERATIONS                        PoUW OPERATIONS                     │
│   ══════════════                        ═══════════════                     │
│   • Get validator set                   • Get compute tasks                │
│   • Get staking info                    • Submit compute receipts          │
│   • Get rewards                         • Get provider status              │
│   • Get slashing history                • Get compute rewards              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 When to Use RPC

| Use Case | Recommended Approach |
|----------|---------------------|
| **Web Applications** | JSON-RPC over HTTP/WebSocket |
| **Backend Services** | Rust SDK or direct JSON-RPC |
| **Mobile Apps** | TypeScript SDK |
| **CLI Scripts** | `mbongo` CLI or curl |
| **Real-time Updates** | WebSocket subscriptions |
| **High-frequency Trading** | Direct IPC connection |

---

## 2. Supported Protocols

### 2.1 Protocol Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SUPPORTED PROTOCOLS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   PROTOCOL     │ ENDPOINT              │ USE CASE                          │
│   ─────────────┼───────────────────────┼───────────────────────────────────│
│   HTTP         │ http://host:8545      │ Request-response queries          │
│   HTTPS        │ https://host:8545     │ Secure production access          │
│   WebSocket    │ ws://host:8546        │ Subscriptions, real-time          │
│   WSS          │ wss://host:8546       │ Secure WebSocket                  │
│   IPC          │ /path/to/mbongo.ipc   │ Local high-performance            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 HTTP/HTTPS

Standard request-response pattern for queries and transaction submission.

```bash
# HTTP endpoint
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

**Configuration:**

```yaml
# In rpc.yaml
rpc:
  http:
    enabled: true
    addr: "127.0.0.1"
    port: 8545
```

### 2.3 WebSocket

Bidirectional communication for subscriptions and real-time updates.

```javascript
const ws = new WebSocket('ws://localhost:8546');
ws.send(JSON.stringify({
  jsonrpc: '2.0',
  method: 'eth_subscribe',
  params: ['newHeads'],
  id: 1
}));
```

**Configuration:**

```yaml
rpc:
  ws:
    enabled: true
    addr: "127.0.0.1"
    port: 8546
```

### 2.4 IPC (Unix Socket)

Highest performance for local applications.

```bash
# IPC endpoint (Linux/macOS)
echo '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | \
  nc -U ~/.mbongo/mbongo.ipc
```

---

## 3. JSON-RPC Format

### 3.1 Request Format

All requests follow the JSON-RPC 2.0 specification:

```json
{
  "jsonrpc": "2.0",
  "method": "method_name",
  "params": [param1, param2],
  "id": 1
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `jsonrpc` | string | Yes | Always `"2.0"` |
| `method` | string | Yes | RPC method name |
| `params` | array/object | No | Method parameters |
| `id` | number/string | Yes | Request identifier |

### 3.2 Response Format

**Success Response:**

```json
{
  "jsonrpc": "2.0",
  "result": "0xc94",
  "id": 1
}
```

**Error Response:**

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32602,
    "message": "Invalid params",
    "data": "expected hex string"
  },
  "id": 1
}
```

### 3.3 Error Codes

| Code | Message | Description |
|------|---------|-------------|
| `-32700` | Parse error | Invalid JSON |
| `-32600` | Invalid request | Invalid request object |
| `-32601` | Method not found | Unknown method |
| `-32602` | Invalid params | Invalid parameters |
| `-32603` | Internal error | Server error |
| `-32000` | Server error | Generic server error |
| `-32001` | Resource not found | Block/tx not found |
| `-32002` | Resource unavailable | Node syncing |
| `-32003` | Transaction rejected | Validation failed |
| `-32004` | Method not supported | Method disabled |

### 3.4 Data Types

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         DATA TYPE ENCODING                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   TYPE          │ FORMAT                    │ EXAMPLE                       │
│   ──────────────┼───────────────────────────┼───────────────────────────────│
│   Quantity      │ Hex with "0x" prefix      │ "0x1a4" (420)                 │
│   Data          │ Hex with "0x" prefix      │ "0xabcd1234..."               │
│   Address       │ 20-byte hex               │ "0x742d35Cc..."               │
│   Hash          │ 32-byte hex               │ "0xabc123..."                 │
│   Block Tag     │ "latest", "pending", etc. │ "latest"                      │
│   Boolean       │ true/false                │ true                          │
│                                                                             │
│   BLOCK TAGS                                                                │
│   ══════════                                                                │
│   "latest"      │ Most recent finalized block                              │
│   "pending"     │ Pending state (mempool)                                  │
│   "earliest"    │ Genesis block                                            │
│   "safe"        │ Safe head (finalized)                                    │
│   "finalized"   │ Finalized block                                          │
│   "0x..."       │ Specific block number                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Core RPC Methods

### 4.1 Method Namespaces

| Namespace | Description | Methods |
|-----------|-------------|---------|
| `eth_` | Ethereum-compatible | Blocks, transactions, accounts |
| `net_` | Network info | Peers, version, listening |
| `web3_` | Client info | Version, SHA3 |
| `mbongo_` | Mbongo-specific | PoS, PoUW, governance |
| `debug_` | Debugging | Tracing, profiling |
| `admin_` | Administration | Peers, node control |

### 4.2 Chain & Block Methods

#### `eth_chainId`

Returns the chain ID.

```json
// Request
{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x1","id":1}
```

#### `eth_blockNumber`

Returns the latest block number.

```json
// Request
{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}

// Response
{"jsonrpc":"2.0","result":"0xbc614e","id":1}
```

#### `eth_getBlockByNumber`

Returns block by number.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0xbc614e", true],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "number": "0xbc614e",
    "hash": "0xabc123...",
    "parentHash": "0xdef456...",
    "timestamp": "0x6741a2b0",
    "gasLimit": "0x1c9c380",
    "gasUsed": "0x5208",
    "miner": "0x742d35Cc...",
    "transactions": [...],
    "pouwScore": "0x64",
    "computeReceiptsRoot": "0x789..."
  },
  "id": 1
}
```

#### `eth_getBlockByHash`

Returns block by hash.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getBlockByHash","params":["0xabc123...", false],"id":1}
```

### 4.3 Account Methods

#### `eth_getBalance`

Returns account balance in wei.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x742d35Cc...", "latest"],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x2b5e3af16b1880000","id":1}
```

#### `eth_getTransactionCount`

Returns account nonce.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getTransactionCount","params":["0x742d35Cc...", "latest"],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x2a","id":1}
```

#### `eth_getCode`

Returns contract bytecode.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getCode","params":["0x742d35Cc...", "latest"],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x608060405234801561001057600080fd5b50...","id":1}
```

### 4.4 Transaction Methods

#### `eth_sendRawTransaction`

Submits a signed transaction.

```json
// Request
{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0xf86c..."],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b","id":1}
```

#### `eth_getTransactionByHash`

Returns transaction details.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getTransactionByHash","params":["0x88df01..."],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "hash": "0x88df01...",
    "nonce": "0x2a",
    "from": "0x742d35Cc...",
    "to": "0x8Ba1f109...",
    "value": "0xde0b6b3a7640000",
    "gasPrice": "0x4a817c800",
    "gas": "0x5208",
    "blockHash": "0xabc123...",
    "blockNumber": "0xbc614e",
    "transactionIndex": "0x0"
  },
  "id": 1
}
```

#### `eth_getTransactionReceipt`

Returns transaction receipt.

```json
// Request
{"jsonrpc":"2.0","method":"eth_getTransactionReceipt","params":["0x88df01..."],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "transactionHash": "0x88df01...",
    "blockHash": "0xabc123...",
    "blockNumber": "0xbc614e",
    "from": "0x742d35Cc...",
    "to": "0x8Ba1f109...",
    "gasUsed": "0x5208",
    "status": "0x1",
    "logs": []
  },
  "id": 1
}
```

#### `eth_estimateGas`

Estimates gas for a transaction.

```json
// Request
{
  "jsonrpc": "2.0",
  "method": "eth_estimateGas",
  "params": [{
    "from": "0x742d35Cc...",
    "to": "0x8Ba1f109...",
    "value": "0xde0b6b3a7640000"
  }],
  "id": 1
}

// Response
{"jsonrpc":"2.0","result":"0x5208","id":1}
```

#### `eth_gasPrice`

Returns current gas price.

```json
// Request
{"jsonrpc":"2.0","method":"eth_gasPrice","params":[],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x4a817c800","id":1}
```

### 4.5 Mbongo-Specific Methods

#### `mbongo_getValidatorSet`

Returns active validator set.

```json
// Request
{"jsonrpc":"2.0","method":"mbongo_getValidatorSet","params":["latest"],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "epoch": 1234,
    "validators": [
      {
        "address": "0x742d35Cc...",
        "stake": "0xa968163f0a57b400000",
        "active": true,
        "uptime": "0.9987"
      }
    ],
    "totalStake": "0x21e19e0c9bab2400000"
  },
  "id": 1
}
```

#### `mbongo_getStakingInfo`

Returns staking information for an address.

```json
// Request
{"jsonrpc":"2.0","method":"mbongo_getStakingInfo","params":["0x742d35Cc..."],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "address": "0x742d35Cc...",
    "staked": "0xa968163f0a57b400000",
    "delegated": "0x152d02c7e14af6800000",
    "rewards": "0xde0b6b3a7640000",
    "unbonding": "0x0",
    "unbondingComplete": null
  },
  "id": 1
}
```

#### `mbongo_getComputeProvider`

Returns compute provider status.

```json
// Request
{"jsonrpc":"2.0","method":"mbongo_getComputeProvider","params":["0x742d35Cc..."],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "address": "0x742d35Cc...",
    "registered": true,
    "gpuCount": 4,
    "workUnits": "0x174876e800",
    "rewards": "0xde0b6b3a7640000",
    "activeTasks": 3,
    "reputation": "0.95"
  },
  "id": 1
}
```

#### `mbongo_getComputeReceipts`

Returns compute receipts in a block.

```json
// Request
{"jsonrpc":"2.0","method":"mbongo_getComputeReceipts","params":["0xbc614e"],"id":1}

// Response
{
  "jsonrpc": "2.0",
  "result": {
    "blockNumber": "0xbc614e",
    "receipts": [
      {
        "taskId": "0xdef456...",
        "provider": "0x742d35Cc...",
        "workUnits": "0x5f5e100",
        "resultHash": "0x789abc...",
        "reward": "0x8ac7230489e80000"
      }
    ],
    "totalWorkUnits": "0x5f5e100"
  },
  "id": 1
}
```

#### `mbongo_submitComputeReceipt`

Submits a compute receipt (provider only).

```json
// Request
{
  "jsonrpc": "2.0",
  "method": "mbongo_submitComputeReceipt",
  "params": [{
    "taskId": "0xdef456...",
    "resultHash": "0x789abc...",
    "workUnits": "0x5f5e100",
    "signature": "0x1234..."
  }],
  "id": 1
}

// Response
{"jsonrpc":"2.0","result":"0x88df016429689c...","id":1}
```

### 4.6 Network Methods

#### `net_version`

Returns network ID.

```json
// Request
{"jsonrpc":"2.0","method":"net_version","params":[],"id":1}

// Response
{"jsonrpc":"2.0","result":"1","id":1}
```

#### `net_peerCount`

Returns connected peer count.

```json
// Request
{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x2a","id":1}
```

#### `net_listening`

Returns listening status.

```json
// Request
{"jsonrpc":"2.0","method":"net_listening","params":[],"id":1}

// Response
{"jsonrpc":"2.0","result":true,"id":1}
```

### 4.7 Method Reference Table

| Method | Description | Params |
|--------|-------------|--------|
| `eth_chainId` | Get chain ID | — |
| `eth_blockNumber` | Get latest block number | — |
| `eth_getBlockByNumber` | Get block by number | `blockNumber`, `fullTx` |
| `eth_getBlockByHash` | Get block by hash | `blockHash`, `fullTx` |
| `eth_getBalance` | Get account balance | `address`, `blockTag` |
| `eth_getTransactionCount` | Get nonce | `address`, `blockTag` |
| `eth_getCode` | Get contract code | `address`, `blockTag` |
| `eth_sendRawTransaction` | Send signed tx | `signedTx` |
| `eth_getTransactionByHash` | Get tx by hash | `txHash` |
| `eth_getTransactionReceipt` | Get tx receipt | `txHash` |
| `eth_estimateGas` | Estimate gas | `txObject` |
| `eth_gasPrice` | Get gas price | — |
| `eth_call` | Call contract | `txObject`, `blockTag` |
| `eth_getLogs` | Get event logs | `filterObject` |
| `mbongo_getValidatorSet` | Get validators | `blockTag` |
| `mbongo_getStakingInfo` | Get staking info | `address` |
| `mbongo_getComputeProvider` | Get provider info | `address` |
| `mbongo_getComputeReceipts` | Get receipts | `blockNumber` |
| `mbongo_submitComputeReceipt` | Submit receipt | `receiptObject` |

---

## 5. WebSocket Channels

### 5.1 Subscription Model

WebSocket enables real-time subscriptions using `eth_subscribe` and `eth_unsubscribe`.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SUBSCRIPTION FLOW                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   CLIENT                              SERVER                                │
│   ══════                              ══════                                │
│                                                                             │
│   ┌─────────────────┐                                                      │
│   │ eth_subscribe   │ ─────────────────────────────────▶                   │
│   │ "newHeads"      │                                                      │
│   └─────────────────┘                                                      │
│                                                                             │
│                      ◀───────────────────────────────── subscription_id    │
│                                                                             │
│                      ◀───────────────────────────────── notification       │
│                      ◀───────────────────────────────── notification       │
│                      ◀───────────────────────────────── notification       │
│                      ...                                                   │
│                                                                             │
│   ┌─────────────────┐                                                      │
│   │ eth_unsubscribe │ ─────────────────────────────────▶                   │
│   │ subscription_id │                                                      │
│   └─────────────────┘                                                      │
│                                                                             │
│                      ◀───────────────────────────────── true               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Available Subscriptions

#### `newHeads`

Subscribe to new block headers.

```json
// Subscribe
{"jsonrpc":"2.0","method":"eth_subscribe","params":["newHeads"],"id":1}

// Response
{"jsonrpc":"2.0","result":"0x1234abcd","id":1}

// Notification
{
  "jsonrpc": "2.0",
  "method": "eth_subscription",
  "params": {
    "subscription": "0x1234abcd",
    "result": {
      "number": "0xbc614f",
      "hash": "0xdef789...",
      "parentHash": "0xabc123...",
      "timestamp": "0x6741a2b1",
      "miner": "0x742d35Cc..."
    }
  }
}
```

#### `newPendingTransactions`

Subscribe to pending transactions.

```json
// Subscribe
{"jsonrpc":"2.0","method":"eth_subscribe","params":["newPendingTransactions"],"id":1}

// Notification
{
  "jsonrpc": "2.0",
  "method": "eth_subscription",
  "params": {
    "subscription": "0x5678efgh",
    "result": "0x88df016429689c079f3b2f6ad39fa052532c56795b733da78a91ebe6a713944b"
  }
}
```

#### `logs`

Subscribe to event logs with filters.

```json
// Subscribe with filter
{
  "jsonrpc": "2.0",
  "method": "eth_subscribe",
  "params": [
    "logs",
    {
      "address": "0x742d35Cc...",
      "topics": ["0xddf252ad..."]
    }
  ],
  "id": 1
}

// Notification
{
  "jsonrpc": "2.0",
  "method": "eth_subscription",
  "params": {
    "subscription": "0x9abcijkl",
    "result": {
      "address": "0x742d35Cc...",
      "topics": ["0xddf252ad..."],
      "data": "0x...",
      "blockNumber": "0xbc614e",
      "transactionHash": "0x88df01..."
    }
  }
}
```

#### `mbongo_computeReceipts`

Subscribe to compute receipts (Mbongo-specific).

```json
// Subscribe
{"jsonrpc":"2.0","method":"eth_subscribe","params":["mbongo_computeReceipts"],"id":1}

// Notification
{
  "jsonrpc": "2.0",
  "method": "eth_subscription",
  "params": {
    "subscription": "0xmnop1234",
    "result": {
      "taskId": "0xdef456...",
      "provider": "0x742d35Cc...",
      "workUnits": "0x5f5e100",
      "blockNumber": "0xbc614e"
    }
  }
}
```

#### `mbongo_validatorUpdates`

Subscribe to validator set changes.

```json
// Subscribe
{"jsonrpc":"2.0","method":"eth_subscribe","params":["mbongo_validatorUpdates"],"id":1}

// Notification
{
  "jsonrpc": "2.0",
  "method": "eth_subscription",
  "params": {
    "subscription": "0xqrst5678",
    "result": {
      "type": "validator_joined",
      "address": "0x8Ba1f109...",
      "stake": "0xa968163f0a57b400000",
      "epoch": 1235
    }
  }
}
```

### 5.3 Unsubscribe

```json
// Unsubscribe
{"jsonrpc":"2.0","method":"eth_unsubscribe","params":["0x1234abcd"],"id":1}

// Response
{"jsonrpc":"2.0","result":true,"id":1}
```

---

## 6. Examples

### 6.1 cURL Examples

#### Get Balance

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_getBalance",
    "params": ["0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7", "latest"],
    "id": 1
  }'
```

#### Send Transaction

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0xf86c808504a817c80082520894..."],
    "id": 1
  }'
```

#### Get Validator Set

```bash
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "mbongo_getValidatorSet",
    "params": ["latest"],
    "id": 1
  }'
```

### 6.2 JavaScript/TypeScript Examples

#### Basic Query

```typescript
const response = await fetch('http://localhost:8545', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    method: 'eth_blockNumber',
    params: [],
    id: 1
  })
});

const { result } = await response.json();
console.log('Block number:', parseInt(result, 16));
```

#### WebSocket Subscription

```typescript
const ws = new WebSocket('ws://localhost:8546');

ws.onopen = () => {
  // Subscribe to new blocks
  ws.send(JSON.stringify({
    jsonrpc: '2.0',
    method: 'eth_subscribe',
    params: ['newHeads'],
    id: 1
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  
  if (data.method === 'eth_subscription') {
    console.log('New block:', data.params.result.number);
  }
};
```

### 6.3 Rust Examples

```rust
use reqwest::Client;
use serde_json::json;

async fn get_block_number() -> Result<u64, Box<dyn std::error::Error>> {
    let client = Client::new();
    
    let response = client
        .post("http://localhost:8545")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    let hex = response["result"].as_str().unwrap();
    let block_number = u64::from_str_radix(&hex[2..], 16)?;
    
    Ok(block_number)
}
```

### 6.4 Python Examples

```python
import requests
import json

def get_balance(address):
    response = requests.post(
        'http://localhost:8545',
        json={
            'jsonrpc': '2.0',
            'method': 'eth_getBalance',
            'params': [address, 'latest'],
            'id': 1
        }
    )
    
    result = response.json()['result']
    balance_wei = int(result, 16)
    balance_mbo = balance_wei / 10**18
    
    return balance_mbo

# Usage
balance = get_balance('0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7')
print(f'Balance: {balance} MBO')
```

---

## 7. Security Notes

### 7.1 RPC Security Best Practices

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║   ⚠️  RPC SECURITY CHECKLIST                                                ║
║                                                                             ║
║   NETWORK                                                                   ║
║   ───────                                                                   ║
║   ✗ Never expose RPC to 0.0.0.0 without firewall                           ║
║   ✓ Bind to 127.0.0.1 for local-only access                                ║
║   ✓ Use TLS (HTTPS/WSS) in production                                      ║
║   ✓ Implement IP whitelisting                                              ║
║                                                                             ║
║   AUTHENTICATION                                                            ║
║   ──────────────                                                            ║
║   ✓ Use API keys for public endpoints                                      ║
║   ✓ Implement JWT for sensitive methods                                    ║
║   ✓ Rate limit by IP and API key                                           ║
║                                                                             ║
║   METHODS                                                                   ║
║   ───────                                                                   ║
║   ✗ Disable admin_* methods on public nodes                                ║
║   ✗ Disable debug_* methods in production                                  ║
║   ✓ Whitelist only required methods                                        ║
║   ✓ Disable eth_sendTransaction (use eth_sendRawTransaction)               ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

### 7.2 Production Configuration

```yaml
# Secure RPC configuration for production
rpc:
  http:
    enabled: true
    addr: "127.0.0.1"  # Local only - use reverse proxy
    port: 8545
    cors: []  # Disable CORS or whitelist specific origins
    
  ws:
    enabled: true
    addr: "127.0.0.1"
    port: 8546
    max_connections: 100
    
  rate_limit:
    enabled: true
    requests_per_minute: 1000
    burst: 50
    
  methods:
    # Whitelist safe methods only
    enabled:
      - "eth_chainId"
      - "eth_blockNumber"
      - "eth_getBalance"
      - "eth_getTransactionCount"
      - "eth_sendRawTransaction"
      - "eth_getTransactionByHash"
      - "eth_getTransactionReceipt"
      - "eth_estimateGas"
      - "eth_gasPrice"
      - "eth_call"
      - "eth_getLogs"
      - "net_version"
      - "mbongo_getValidatorSet"
      - "mbongo_getStakingInfo"
    # Explicitly disabled
    disabled:
      - "admin_*"
      - "debug_*"
      - "personal_*"
      - "eth_sendTransaction"
```

### 7.3 Reverse Proxy Setup

```nginx
# Nginx reverse proxy for RPC
server {
    listen 443 ssl http2;
    server_name rpc.mbongo.example.com;
    
    ssl_certificate /etc/ssl/certs/mbongo.crt;
    ssl_certificate_key /etc/ssl/private/mbongo.key;
    
    # Rate limiting
    limit_req_zone $binary_remote_addr zone=rpc:10m rate=100r/s;
    
    location / {
        limit_req zone=rpc burst=50 nodelay;
        
        proxy_pass http://127.0.0.1:8545;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

### 7.4 Common Attack Vectors

| Attack | Risk | Mitigation |
|--------|------|------------|
| **DoS via heavy queries** | High | Rate limiting, query limits |
| **Method abuse** | High | Method whitelisting |
| **Data exfiltration** | Medium | Disable debug methods |
| **Transaction spam** | Medium | Minimum gas price |
| **Replay attacks** | Low | Chain ID validation |

---

## 8. Cross-Links

### SDK Documentation

| Document | Description |
|----------|-------------|
| **[sdk_rust.md](./sdk_rust.md)** | Rust SDK (planned) |
| **[sdk_typescript.md](./sdk_typescript.md)** | TypeScript SDK (planned) |

### CLI Documentation

| Document | Description |
|----------|-------------|
| [cli_overview.md](./cli_overview.md) | CLI overview |
| [cli_node.md](./cli_node.md) | Node commands |
| [cli_config.md](./cli_config.md) | Configuration |

### Economic Documentation

| Document | Description |
|----------|-------------|
| [fee_model.md](./fee_model.md) | Gas and fee structure |
| [staking_model.md](./staking_model.md) | Staking mechanics |
| [compute_value.md](./compute_value.md) | PoUW compute |

### Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RPC QUICK REFERENCE                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ENDPOINTS                                                                 │
│   ─────────                                                                 │
│   HTTP:      http://localhost:8545                                         │
│   WebSocket: ws://localhost:8546                                           │
│   IPC:       ~/.mbongo/mbongo.ipc                                          │
│                                                                             │
│   COMMON METHODS                                                            │
│   ──────────────                                                            │
│   eth_chainId                    Get chain ID                              │
│   eth_blockNumber                Get latest block                          │
│   eth_getBalance                 Get account balance                       │
│   eth_sendRawTransaction         Submit transaction                        │
│   eth_getTransactionReceipt      Get tx receipt                            │
│   mbongo_getValidatorSet         Get validators                            │
│   mbongo_getStakingInfo          Get staking info                          │
│                                                                             │
│   SUBSCRIPTIONS                                                             │
│   ─────────────                                                             │
│   newHeads                       New block headers                         │
│   newPendingTransactions         Pending transactions                      │
│   logs                           Event logs                                │
│   mbongo_computeReceipts         Compute receipts                          │
│   mbongo_validatorUpdates        Validator changes                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides the RPC API reference for Mbongo Chain. For CLI access, see [cli_overview.md](./cli_overview.md). For SDK integration, see the SDK documentation.*

