# Mbongo Chain TypeScript SDK (v0.1)

## Status

Unstable. Matches JSON-RPC spec v0.1. Breaking changes are allowed until v1.0.

## Install (local)

```bash
npm install
npm run build
```

## Usage Example

```typescript
import { MbongoClient } from "@mbongo/sdk";

const client = new MbongoClient("http://localhost:8080/rpc");

const blockNumber = await client.getBlockNumber();
console.log(blockNumber);
```

## RPC Spec

Reference: [docs/specs/jsonrpc_v0.1.md](../../docs/specs/jsonrpc_v0.1.md)

## Stability

Breaking changes are allowed until v1.0.
