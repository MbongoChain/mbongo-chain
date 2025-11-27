# Mbongo Chain — TypeScript SDK Overview

> **Document Type:** SDK Reference  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Audience:** Frontend Developers, Full-Stack Engineers, DApp Builders

---

## Table of Contents

1. [Purpose of the TypeScript SDK](#1-purpose-of-the-typescript-sdk)
2. [Architecture](#2-architecture)
3. [Installation](#3-installation)
4. [Platform Usage](#4-platform-usage)
5. [Core Modules](#5-core-modules)
6. [Code Examples](#6-code-examples)
7. [Security Notes](#7-security-notes)
8. [Cross-Links](#8-cross-links)

---

## 1. Purpose of the TypeScript SDK

### 1.1 What is the Mbongo TypeScript SDK?

The Mbongo TypeScript SDK (`@mbongo/sdk`) is the official JavaScript/TypeScript client library for interacting with Mbongo Chain. It provides type-safe, async APIs optimized for web applications, Node.js backends, and modern frameworks like Next.js.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TYPESCRIPT SDK CAPABILITIES                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   BLOCKCHAIN QUERIES                    TRANSACTION MANAGEMENT              │
│   ══════════════════                    ══════════════════════              │
│   • Query blocks and transactions       • Build typed transactions         │
│   • Get account balances                • Sign with various providers      │
│   • Read contract state                 • Submit to network                │
│   • Subscribe to events                 • Track confirmations              │
│                                                                             │
│   WALLET INTEGRATION                    PoS STAKING                         │
│   ══════════════════                    ══════════                          │
│   • WalletConnect support               • Query validator set              │
│   • MetaMask-style injection            • Stake/delegate operations        │
│   • Server-side signing                 • Rewards management               │
│   • Hardware wallet support             • Unbonding tracking               │
│                                                                             │
│   PoUW COMPUTE                          GOVERNANCE                          │
│   ════════════                          ══════════                          │
│   • Submit compute tasks                • Vote on proposals                │
│   • Query compute receipts              • Create proposals                 │
│   • Monitor provider status             • Delegate voting power            │
│   • Validate proofs                     • Query governance state           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 When to Use the TypeScript SDK

| Use Case | Recommended |
|----------|-------------|
| **React/Next.js DApps** | ✓ Yes |
| **Node.js backends** | ✓ Yes |
| **Browser extensions** | ✓ Yes |
| **Mobile (React Native)** | ✓ Yes |
| **Serverless functions** | ✓ Yes |
| **High-frequency trading** | Consider Rust SDK |
| **Validator software** | Consider Rust SDK |

### 1.3 Key Features

- **Full TypeScript Support**: Complete type definitions for all APIs
- **Tree-Shakeable**: Import only what you need
- **Isomorphic**: Works in Node.js and browsers
- **Modern ESM**: Native ES modules with top-level await
- **Framework Agnostic**: Works with React, Vue, Svelte, etc.
- **Zero Dependencies**: Minimal footprint for browsers

---

## 2. Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SDK ARCHITECTURE                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   APPLICATION LAYER                                                         │
│   ═════════════════                                                         │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  React / Next.js / Vue / Node.js Application                         │  │
│   └───────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼                                         │
│   SDK LAYER                                                                 │
│   ═════════                                                                 │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                       @mbongo/sdk                                    │  │
│   │                                                                       │  │
│   │   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐      │  │
│   │   │ Wallet  │ │ Compute │ │ Mempool │ │Validator│ │Governance│     │  │
│   │   │ Module  │ │ Module  │ │ Module  │ │ Module  │ │ Module  │      │  │
│   │   └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘      │  │
│   │        │           │           │           │           │            │  │
│   │        └───────────┴───────────┼───────────┴───────────┘            │  │
│   │                                │                                     │  │
│   │   ┌────────────────────────────┴────────────────────────────────┐   │  │
│   │   │                     Provider Layer                           │   │  │
│   │   │                                                               │   │  │
│   │   │   ┌────────────┐  ┌────────────┐  ┌────────────┐            │   │  │
│   │   │   │ HTTP/Fetch │  │ WebSocket  │  │  Injected  │            │   │  │
│   │   │   │ Provider   │  │ Provider   │  │  Provider  │            │   │  │
│   │   │   └────────────┘  └────────────┘  └────────────┘            │   │  │
│   │   │                                                               │   │  │
│   │   └─────────────────────────────────────────────────────────────┘   │  │
│   │                                                                       │  │
│   │   ┌─────────────────────────────────────────────────────────────┐   │  │
│   │   │                     Signing Layer                            │   │  │
│   │   │                                                               │   │  │
│   │   │   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   │   │  │
│   │   │   │  Local   │  │ Injected │  │ Wallet   │  │  Server  │   │   │  │
│   │   │   │  Signer  │  │  Wallet  │  │ Connect  │  │   Side   │   │   │  │
│   │   │   └──────────┘  └──────────┘  └──────────┘  └──────────┘   │   │  │
│   │   │                                                               │   │  │
│   │   └─────────────────────────────────────────────────────────────┘   │  │
│   │                                                                       │  │
│   └───────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼                                         │
│   NETWORK LAYER                                                             │
│   ═════════════                                                             │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                       Mbongo Node (RPC)                              │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Provider Types

```typescript
// HTTP Provider (default)
import { HttpProvider } from '@mbongo/sdk';
const provider = new HttpProvider('https://rpc.mbongo.io');

// WebSocket Provider (subscriptions)
import { WebSocketProvider } from '@mbongo/sdk';
const provider = new WebSocketProvider('wss://ws.mbongo.io');

// Injected Provider (browser wallets)
import { InjectedProvider } from '@mbongo/sdk';
const provider = new InjectedProvider(window.mbongo);

// Combined Provider (auto-select)
import { createProvider } from '@mbongo/sdk';
const provider = createProvider({
  http: 'https://rpc.mbongo.io',
  ws: 'wss://ws.mbongo.io',
});
```

### 2.3 Signing Layer

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SIGNING OPTIONS                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   SIGNER TYPE      │ ENVIRONMENT    │ SECURITY     │ USE CASE              │
│   ─────────────────┼────────────────┼──────────────┼───────────────────────│
│   LocalSigner      │ Node.js only   │ High risk    │ Testing, scripts      │
│   InjectedSigner   │ Browser        │ User-managed │ DApps                 │
│   WalletConnect    │ Browser/Mobile │ User-managed │ Mobile DApps          │
│   ServerSigner     │ Server         │ Secure       │ Backend operations    │
│   LedgerSigner     │ Both           │ Maximum      │ High-value txs        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Browser Compatibility

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BROWSER SUPPORT                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   BROWSER              │ VERSION   │ NOTES                                 │
│   ─────────────────────┼───────────┼───────────────────────────────────────│
│   Chrome               │ 90+       │ Full support                          │
│   Firefox              │ 90+       │ Full support                          │
│   Safari               │ 15+       │ Full support                          │
│   Edge                 │ 90+       │ Full support                          │
│   Mobile Safari        │ 15+       │ Full support                          │
│   Chrome Android       │ 90+       │ Full support                          │
│                                                                             │
│   REQUIRED APIS                                                             │
│   ═════════════                                                             │
│   • fetch()                                                                │
│   • WebSocket                                                              │
│   • crypto.subtle (for signing)                                            │
│   • BigInt                                                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Installation

### 3.1 Package Managers

```bash
# npm
npm install @mbongo/sdk

# yarn
yarn add @mbongo/sdk

# pnpm
pnpm add @mbongo/sdk

# bun
bun add @mbongo/sdk
```

### 3.2 Package Structure

```
@mbongo/sdk
├── dist/
│   ├── index.mjs          # ES Module (Node.js, bundlers)
│   ├── index.cjs          # CommonJS (legacy Node.js)
│   ├── browser.mjs        # Browser bundle
│   └── types/             # TypeScript declarations
├── package.json
└── README.md
```

### 3.3 TypeScript Configuration

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "types": ["@mbongo/sdk"]
  }
}
```

### 3.4 Bundler Configuration

```javascript
// vite.config.ts
import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    include: ['@mbongo/sdk'],
  },
  build: {
    rollupOptions: {
      // Tree-shake unused modules
      treeshake: true,
    },
  },
});
```

---

## 4. Platform Usage

### 4.1 Node.js Backend

```typescript
// server.ts
import { MbongoClient, Wallet, type Block } from '@mbongo/sdk';

// Top-level await in ES modules
const client = await MbongoClient.connect('http://localhost:8545');

// Get chain info
const chainId = await client.getChainId();
const blockNumber = await client.getBlockNumber();

console.log(`Connected to chain ${chainId} at block ${blockNumber}`);

// Load wallet from environment
const wallet = Wallet.fromPrivateKey(process.env.PRIVATE_KEY!);

// Send transaction
const tx = await client.sendTransaction({
  from: wallet.address,
  to: '0x8Ba1f109551bD432803012645Ac136ddd64DBA72',
  value: parseUnits('100', 18), // 100 MBO
  signer: wallet,
});

const receipt = await tx.wait();
console.log(`Transaction confirmed: ${receipt.transactionHash}`);
```

### 4.2 Next.js 15 App Router

#### Server Component (Query Only)

```typescript
// app/page.tsx
import { MbongoClient } from '@mbongo/sdk/server';

export default async function HomePage() {
  const client = await MbongoClient.connect(process.env.RPC_URL!);
  
  const blockNumber = await client.getBlockNumber();
  const validators = await client.getValidatorSet();
  
  return (
    <main>
      <h1>Mbongo Chain</h1>
      <p>Current block: {blockNumber.toString()}</p>
      <p>Active validators: {validators.length}</p>
    </main>
  );
}
```

#### Client Component (With Wallet)

```typescript
// app/components/WalletConnect.tsx
'use client';

import { useState, useEffect } from 'react';
import { MbongoClient, InjectedProvider } from '@mbongo/sdk';

export function WalletConnect() {
  const [address, setAddress] = useState<string | null>(null);
  const [balance, setBalance] = useState<string>('0');
  
  async function connect() {
    if (typeof window.mbongo === 'undefined') {
      alert('Please install Mbongo Wallet');
      return;
    }
    
    const provider = new InjectedProvider(window.mbongo);
    const client = new MbongoClient(provider);
    
    const accounts = await provider.requestAccounts();
    setAddress(accounts[0]);
    
    const bal = await client.getBalance(accounts[0]);
    setBalance(formatUnits(bal, 18));
  }
  
  return (
    <div>
      {address ? (
        <div>
          <p>Connected: {address}</p>
          <p>Balance: {balance} MBO</p>
        </div>
      ) : (
        <button onClick={connect}>Connect Wallet</button>
      )}
    </div>
  );
}
```

#### API Route (Server-Side Signing)

```typescript
// app/api/transfer/route.ts
import { NextRequest, NextResponse } from 'next/server';
import { MbongoClient, Wallet, parseUnits } from '@mbongo/sdk/server';

export async function POST(request: NextRequest) {
  const { to, amount } = await request.json();
  
  // Server-side wallet (from secure environment)
  const client = await MbongoClient.connect(process.env.RPC_URL!);
  const wallet = Wallet.fromPrivateKey(process.env.SERVER_PRIVATE_KEY!);
  
  try {
    const tx = await client.sendTransaction({
      from: wallet.address,
      to,
      value: parseUnits(amount, 18),
      signer: wallet,
    });
    
    const receipt = await tx.wait();
    
    return NextResponse.json({
      success: true,
      txHash: receipt.transactionHash,
    });
  } catch (error) {
    return NextResponse.json(
      { success: false, error: (error as Error).message },
      { status: 400 }
    );
  }
}
```

### 4.3 Browser Applications

```html
<!-- index.html -->
<!DOCTYPE html>
<html>
<head>
  <script type="module">
    import { MbongoClient, formatUnits } from 'https://esm.sh/@mbongo/sdk';
    
    const client = await MbongoClient.connect('https://rpc.mbongo.io');
    
    const blockNumber = await client.getBlockNumber();
    document.getElementById('block').textContent = blockNumber.toString();
    
    // Listen for new blocks
    client.on('block', (block) => {
      document.getElementById('block').textContent = block.number.toString();
    });
  </script>
</head>
<body>
  <h1>Mbongo Chain</h1>
  <p>Current block: <span id="block">Loading...</span></p>
</body>
</html>
```

#### React Hook

```typescript
// hooks/useMbongo.ts
'use client';

import { useState, useEffect, useCallback } from 'react';
import { MbongoClient, type Block } from '@mbongo/sdk';

export function useMbongo(rpcUrl: string) {
  const [client, setClient] = useState<MbongoClient | null>(null);
  const [blockNumber, setBlockNumber] = useState<bigint>(0n);
  const [isConnected, setIsConnected] = useState(false);
  
  useEffect(() => {
    let mounted = true;
    
    async function connect() {
      const c = await MbongoClient.connect(rpcUrl);
      if (mounted) {
        setClient(c);
        setIsConnected(true);
        
        const bn = await c.getBlockNumber();
        setBlockNumber(bn);
        
        // Subscribe to new blocks
        c.on('block', (block: Block) => {
          setBlockNumber(block.number);
        });
      }
    }
    
    connect();
    
    return () => {
      mounted = false;
      client?.disconnect();
    };
  }, [rpcUrl]);
  
  return { client, blockNumber, isConnected };
}
```

---

## 5. Core Modules

### 5.1 Module Overview

```typescript
import {
  // Client
  MbongoClient,
  HttpProvider,
  WebSocketProvider,
  
  // Wallet
  Wallet,
  LocalSigner,
  InjectedSigner,
  
  // Compute (PoUW)
  ComputeClient,
  ComputeReceipt,
  
  // Mempool
  MempoolClient,
  
  // Validator
  ValidatorClient,
  
  // Governance
  GovernanceClient,
  
  // Types
  type Block,
  type Transaction,
  type TransactionReceipt,
  type Address,
  type Hash,
  
  // Utilities
  parseUnits,
  formatUnits,
  keccak256,
  toHex,
  fromHex,
} from '@mbongo/sdk';
```

### 5.2 Wallet Module

```typescript
import { Wallet, type Signer } from '@mbongo/sdk';

// Create random wallet
const wallet = Wallet.createRandom();
console.log('Address:', wallet.address);
console.log('Mnemonic:', wallet.mnemonic?.phrase);

// From mnemonic
const restored = Wallet.fromMnemonic(
  'abandon ability able about above absent absorb abstract absurd abuse access accident'
);

// From private key (Node.js only!)
const fromKey = Wallet.fromPrivateKey('0xac0974bec39a17e36ba4a6b4d238ff944...');

// From encrypted keystore
const fromKeystore = await Wallet.fromEncryptedJson(
  keystoreJson,
  'password'
);

// Sign message
const signature = await wallet.signMessage('Hello, Mbongo!');

// Sign typed data (EIP-712)
const typedSignature = await wallet.signTypedData(domain, types, value);

// Export encrypted keystore
const encrypted = await wallet.encrypt('password');
```

### 5.3 Compute Module (PoUW)

```typescript
import { ComputeClient, type ComputeTask, type ComputeReceipt } from '@mbongo/sdk';

const compute = new ComputeClient(client);

// Get available tasks
const tasks = await compute.getPendingTasks();
for (const task of tasks) {
  console.log(`Task ${task.id}: ${task.type}, reward: ${task.reward} MBO`);
}

// Submit compute receipt
const receipt: ComputeReceipt = {
  taskId: '0xdef456...',
  resultHash: '0x789abc...',
  workUnits: 100000000n,
  timestamp: Date.now(),
};

const tx = await compute.submitReceipt(receipt, { signer: wallet });
await tx.wait();

// Get provider status
const status = await compute.getProviderStatus(wallet.address);
console.log(`Work units: ${status.workUnits}`);
console.log(`Rewards: ${formatUnits(status.rewards, 18)} MBO`);

// Validate receipt
const isValid = await compute.validateReceipt(receipt);
console.log(`Receipt valid: ${isValid}`);
```

### 5.4 Mempool Module

```typescript
import { MempoolClient } from '@mbongo/sdk';

const mempool = new MempoolClient(client);

// Get pending transactions
const pending = await mempool.getPendingTransactions();
console.log(`Pending txs: ${pending.length}`);

// Subscribe to pending transactions
mempool.on('pending', (tx) => {
  console.log(`New pending tx: ${tx.hash}`);
});

// Get transaction status
const status = await mempool.getTransactionStatus(txHash);
switch (status.state) {
  case 'pending':
    console.log('Transaction in mempool');
    break;
  case 'included':
    console.log(`Included in block ${status.blockNumber}`);
    break;
  case 'dropped':
    console.log(`Dropped: ${status.reason}`);
    break;
}

// Estimate gas
const gasEstimate = await mempool.estimateGas({
  from: wallet.address,
  to: recipient,
  value: parseUnits('100', 18),
});
```

### 5.5 Validator Module

```typescript
import { ValidatorClient } from '@mbongo/sdk';

const validator = new ValidatorClient(client);

// Get validator set
const validators = await validator.getValidatorSet();
for (const v of validators) {
  console.log(`${v.address}: ${formatUnits(v.stake, 18)} MBO (${v.uptime * 100}% uptime)`);
}

// Get staking info
const staking = await validator.getStakingInfo(address);
console.log(`Staked: ${formatUnits(staking.staked, 18)} MBO`);
console.log(`Delegated: ${formatUnits(staking.delegated, 18)} MBO`);
console.log(`Rewards: ${formatUnits(staking.rewards, 18)} MBO`);

// Stake MBO (validators)
const stakeTx = await validator.stake(parseUnits('50000', 18), { signer: wallet });
await stakeTx.wait();

// Delegate to validator
const delegateTx = await validator.delegate(
  validatorAddress,
  parseUnits('1000', 18),
  { signer: wallet }
);
await delegateTx.wait();

// Withdraw rewards
const withdrawTx = await validator.withdrawRewards({ signer: wallet });
await withdrawTx.wait();
```

### 5.6 Governance Module

```typescript
import { GovernanceClient, type Proposal, ProposalState } from '@mbongo/sdk';

const governance = new GovernanceClient(client);

// Get active proposals
const proposals = await governance.getProposals({ state: ProposalState.Active });
for (const p of proposals) {
  console.log(`Proposal #${p.id}: ${p.title}`);
  console.log(`  For: ${p.votesFor}, Against: ${p.votesAgainst}`);
  console.log(`  Ends: ${new Date(p.endTime * 1000).toISOString()}`);
}

// Vote on proposal
const voteTx = await governance.vote(
  proposalId,
  true, // support
  { signer: wallet }
);
await voteTx.wait();

// Create proposal (requires minimum stake)
const createTx = await governance.createProposal({
  title: 'Increase gas limit',
  description: 'Proposal to increase block gas limit to 30M',
  actions: [
    {
      target: governanceAddress,
      calldata: encodeFunctionData('setGasLimit', [30000000n]),
    },
  ],
  signer: wallet,
});
await createTx.wait();

// Get voting power
const votingPower = await governance.getVotingPower(wallet.address);
console.log(`Voting power: ${formatUnits(votingPower, 18)}`);
```

---

## 6. Code Examples

### 6.1 Connect to RPC Endpoint

```typescript
import { MbongoClient } from '@mbongo/sdk';

// Basic connection
const client = await MbongoClient.connect('https://rpc.mbongo.io');

// With options
const clientWithOptions = await MbongoClient.connect('https://rpc.mbongo.io', {
  timeout: 30000,
  retries: 3,
  headers: {
    'X-API-Key': process.env.API_KEY,
  },
});

// WebSocket connection
const wsClient = await MbongoClient.connect('wss://ws.mbongo.io');

// Multiple endpoints (failover)
const reliableClient = await MbongoClient.connect([
  'https://rpc1.mbongo.io',
  'https://rpc2.mbongo.io',
  'https://rpc3.mbongo.io',
]);

// Check connection
const chainId = await client.getChainId();
const blockNumber = await client.getBlockNumber();
const syncing = await client.isSyncing();

console.log(`Chain: ${chainId}`);
console.log(`Block: ${blockNumber}`);
console.log(`Syncing: ${syncing}`);
```

### 6.2 Fetch Blocks and Receipts

```typescript
import { MbongoClient, type Block, type ComputeReceipt } from '@mbongo/sdk';

const client = await MbongoClient.connect('https://rpc.mbongo.io');

// Get latest block
const latestBlock = await client.getBlock('latest');
console.log(`Block ${latestBlock.number}:`);
console.log(`  Hash: ${latestBlock.hash}`);
console.log(`  Timestamp: ${new Date(Number(latestBlock.timestamp) * 1000)}`);
console.log(`  Transactions: ${latestBlock.transactions.length}`);
console.log(`  PoUW Score: ${latestBlock.pouwScore}`);

// Get block by number
const block = await client.getBlock(12345678n);

// Get block with full transactions
const blockWithTxs = await client.getBlock('latest', { full: true });
for (const tx of blockWithTxs.transactions) {
  console.log(`  Tx: ${tx.hash} (${formatUnits(tx.value, 18)} MBO)`);
}

// Get compute receipts
const receipts = await client.getComputeReceipts(latestBlock.number);
console.log(`Compute receipts in block: ${receipts.length}`);
for (const receipt of receipts) {
  console.log(`  Task: ${receipt.taskId}`);
  console.log(`  Provider: ${receipt.provider}`);
  console.log(`  Work units: ${receipt.workUnits}`);
}

// Subscribe to new blocks
client.on('block', (block: Block) => {
  console.log(`New block: ${block.number}`);
});
```

### 6.3 Send Transactions

```typescript
import { MbongoClient, Wallet, parseUnits, formatUnits } from '@mbongo/sdk';

const client = await MbongoClient.connect('https://rpc.mbongo.io');
const wallet = Wallet.fromPrivateKey(process.env.PRIVATE_KEY!);

// Simple transfer
const tx = await client.sendTransaction({
  from: wallet.address,
  to: '0x8Ba1f109551bD432803012645Ac136ddd64DBA72',
  value: parseUnits('100', 18), // 100 MBO
  signer: wallet,
});

console.log(`Tx submitted: ${tx.hash}`);

// Wait for confirmation
const receipt = await tx.wait();
console.log(`Confirmed in block ${receipt.blockNumber}`);
console.log(`Gas used: ${receipt.gasUsed}`);
console.log(`Status: ${receipt.status === 1 ? 'Success' : 'Failed'}`);

// With custom gas settings
const txWithGas = await client.sendTransaction({
  from: wallet.address,
  to: recipient,
  value: parseUnits('50', 18),
  gasLimit: 21000n,
  maxFeePerGas: parseUnits('20', 9), // 20 gwei
  maxPriorityFeePerGas: parseUnits('1', 9), // 1 gwei
  signer: wallet,
});

// Wait for multiple confirmations
const confirmedReceipt = await txWithGas.wait(3); // 3 confirmations

// Check balance after
const balance = await client.getBalance(wallet.address);
console.log(`New balance: ${formatUnits(balance, 18)} MBO`);
```

### 6.4 Submit Compute Proofs

```typescript
import { 
  MbongoClient, 
  ComputeClient, 
  Wallet,
  type ComputeReceipt 
} from '@mbongo/sdk';

const client = await MbongoClient.connect('https://rpc.mbongo.io');
const compute = new ComputeClient(client);
const wallet = Wallet.fromPrivateKey(process.env.PROVIDER_KEY!);

// Register as compute provider
const registerTx = await compute.register({
  gpuSpecs: 'NVIDIA RTX 4090 x 4',
  capabilities: ['inference', 'rendering', 'zk-proof'],
  signer: wallet,
});
await registerTx.wait();
console.log('Registered as compute provider');

// Poll for tasks
async function processTask() {
  const tasks = await compute.getPendingTasks({
    types: ['inference'],
    minReward: parseUnits('0.01', 18),
  });
  
  if (tasks.length === 0) {
    console.log('No pending tasks');
    return;
  }
  
  const task = tasks[0];
  console.log(`Processing task ${task.id}...`);
  
  // Execute computation (your implementation)
  const result = await executeComputation(task);
  
  // Build receipt
  const receipt: ComputeReceipt = {
    taskId: task.id,
    resultHash: result.hash,
    workUnits: result.workUnits,
    executionTime: result.executionTime,
    timestamp: Date.now(),
  };
  
  // Submit receipt
  const submitTx = await compute.submitReceipt(receipt, { signer: wallet });
  const txReceipt = await submitTx.wait();
  
  console.log(`Receipt submitted: ${txReceipt.transactionHash}`);
}

// Run processing loop
setInterval(processTask, 10000);
```

### 6.5 Validate Receipts

```typescript
import { 
  MbongoClient, 
  ComputeClient,
  type ComputeReceipt,
  type ValidationResult 
} from '@mbongo/sdk';

const client = await MbongoClient.connect('https://rpc.mbongo.io');
const compute = new ComputeClient(client);

async function validateBlockReceipts(blockNumber: bigint) {
  // Get all receipts in block
  const receipts = await client.getComputeReceipts(blockNumber);
  
  console.log(`Validating ${receipts.length} receipts in block ${blockNumber}`);
  
  const results: ValidationResult[] = [];
  
  for (const receipt of receipts) {
    // Verify provider signature
    const signatureValid = await compute.verifyReceiptSignature(receipt);
    
    // Verify attester signatures
    const attestersValid = await compute.verifyAttesterSignatures(receipt);
    
    // Verify result hash (if re-execution is possible)
    const resultValid = await compute.verifyResultHash(receipt);
    
    // Check provider is registered and not slashed
    const providerStatus = await compute.getProviderStatus(receipt.provider);
    const providerValid = providerStatus.registered && !providerStatus.slashed;
    
    const isValid = signatureValid && attestersValid && resultValid && providerValid;
    
    results.push({
      taskId: receipt.taskId,
      provider: receipt.provider,
      isValid,
      checks: {
        signature: signatureValid,
        attesters: attestersValid,
        result: resultValid,
        provider: providerValid,
      },
    });
    
    console.log(`Receipt ${receipt.taskId}: ${isValid ? '✓ Valid' : '✗ Invalid'}`);
    if (!isValid) {
      console.log(`  Signature: ${signatureValid ? '✓' : '✗'}`);
      console.log(`  Attesters: ${attestersValid ? '✓' : '✗'}`);
      console.log(`  Result: ${resultValid ? '✓' : '✗'}`);
      console.log(`  Provider: ${providerValid ? '✓' : '✗'}`);
    }
  }
  
  return results;
}

// Validate receipts in latest block
const blockNumber = await client.getBlockNumber();
await validateBlockReceipts(blockNumber);
```

### 6.6 Complete DApp Example

```typescript
// lib/mbongo.ts
import { MbongoClient, Wallet, formatUnits, parseUnits } from '@mbongo/sdk';

let client: MbongoClient | null = null;

export async function getClient(): Promise<MbongoClient> {
  if (!client) {
    client = await MbongoClient.connect(process.env.NEXT_PUBLIC_RPC_URL!);
  }
  return client;
}

export async function getBalance(address: string): Promise<string> {
  const c = await getClient();
  const balance = await c.getBalance(address);
  return formatUnits(balance, 18);
}

export async function transfer(
  to: string,
  amount: string,
  signer: Wallet
): Promise<string> {
  const c = await getClient();
  const tx = await c.sendTransaction({
    from: signer.address,
    to,
    value: parseUnits(amount, 18),
    signer,
  });
  const receipt = await tx.wait();
  return receipt.transactionHash;
}

// hooks/useWallet.ts
'use client';

import { useState, useCallback } from 'react';
import { InjectedProvider, MbongoClient } from '@mbongo/sdk';

export function useWallet() {
  const [address, setAddress] = useState<string | null>(null);
  const [client, setClient] = useState<MbongoClient | null>(null);
  
  const connect = useCallback(async () => {
    if (typeof window.mbongo === 'undefined') {
      throw new Error('Wallet not found');
    }
    
    const provider = new InjectedProvider(window.mbongo);
    const c = new MbongoClient(provider);
    
    const accounts = await provider.requestAccounts();
    
    setClient(c);
    setAddress(accounts[0]);
    
    return accounts[0];
  }, []);
  
  const disconnect = useCallback(() => {
    setClient(null);
    setAddress(null);
  }, []);
  
  return { address, client, connect, disconnect };
}
```

---

## 7. Security Notes

### 7.1 Never Expose Private Keys in Browser

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║   ⚠️  CRITICAL: BROWSER KEY SECURITY                                        ║
║                                                                             ║
║   NEVER in browser code:                                                   ║
║   ─────────────────────                                                     ║
║   ✗ Store private keys in localStorage                                     ║
║   ✗ Include private keys in client-side bundles                            ║
║   ✗ Use Wallet.fromPrivateKey() in browser                                 ║
║   ✗ Log or display private keys                                            ║
║   ✗ Send private keys to any API                                           ║
║                                                                             ║
║   INSTEAD:                                                                  ║
║   ────────                                                                  ║
║   ✓ Use injected wallets (MetaMask-style)                                  ║
║   ✓ Use WalletConnect for mobile                                           ║
║   ✓ Use server-side signing via API routes                                 ║
║   ✓ Use hardware wallets for high-value operations                         ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

```typescript
// ❌ WRONG: Private key in browser
const wallet = Wallet.fromPrivateKey('0xac0974bec...'); // NEVER DO THIS

// ✓ CORRECT: Use injected wallet
const provider = new InjectedProvider(window.mbongo);
const accounts = await provider.requestAccounts();
const signer = provider.getSigner();

// ✓ CORRECT: Server-side signing
const response = await fetch('/api/transfer', {
  method: 'POST',
  body: JSON.stringify({ to, amount }),
});
```

### 7.2 Server-Side Signing with Next.js

```typescript
// app/api/sign/route.ts
import { NextRequest, NextResponse } from 'next/server';
import { MbongoClient, Wallet, parseUnits } from '@mbongo/sdk/server';

// Private key only accessible on server
const serverWallet = Wallet.fromPrivateKey(process.env.SERVER_PRIVATE_KEY!);

export async function POST(request: NextRequest) {
  // Verify request authentication
  const authHeader = request.headers.get('authorization');
  if (!verifyAuth(authHeader)) {
    return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
  }
  
  const { to, amount } = await request.json();
  
  // Validate inputs
  if (!isValidAddress(to)) {
    return NextResponse.json({ error: 'Invalid address' }, { status: 400 });
  }
  
  if (parseFloat(amount) > 1000) {
    return NextResponse.json({ error: 'Amount exceeds limit' }, { status: 400 });
  }
  
  const client = await MbongoClient.connect(process.env.RPC_URL!);
  
  try {
    const tx = await client.sendTransaction({
      from: serverWallet.address,
      to,
      value: parseUnits(amount, 18),
      signer: serverWallet,
    });
    
    const receipt = await tx.wait();
    
    return NextResponse.json({
      success: true,
      txHash: receipt.transactionHash,
    });
  } catch (error) {
    console.error('Transaction failed:', error);
    return NextResponse.json(
      { error: 'Transaction failed' },
      { status: 500 }
    );
  }
}
```

### 7.3 Recommended Keystore Formats

```typescript
import { Wallet } from '@mbongo/sdk';

// Encrypt wallet for storage
const wallet = Wallet.createRandom();
const encrypted = await wallet.encrypt('strong-password', {
  scrypt: {
    N: 131072, // Higher = more secure but slower
    r: 8,
    p: 1,
  },
});

// Save encrypted JSON (safe to store)
await fs.writeFile('./keystore.json', encrypted);

// Load later
const loaded = await Wallet.fromEncryptedJson(
  await fs.readFile('./keystore.json', 'utf8'),
  'strong-password'
);
```

### 7.4 Environment Variables

```bash
# .env.local (NEVER commit this file)

# Server-side only (not exposed to browser)
SERVER_PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944...
RPC_URL=https://rpc.mbongo.io

# Client-side (prefixed with NEXT_PUBLIC_)
NEXT_PUBLIC_RPC_URL=https://rpc.mbongo.io
NEXT_PUBLIC_CHAIN_ID=1
```

```typescript
// next.config.js
module.exports = {
  // Ensure server-only vars are not exposed
  serverRuntimeConfig: {
    privateKey: process.env.SERVER_PRIVATE_KEY,
  },
  publicRuntimeConfig: {
    rpcUrl: process.env.NEXT_PUBLIC_RPC_URL,
  },
};
```

---

## 8. Cross-Links

### SDK Documentation

| Document | Description |
|----------|-------------|
| [rust_sdk_overview.md](./rust_sdk_overview.md) | Rust SDK reference |
| [rpc_overview.md](./rpc_overview.md) | JSON-RPC API |

### CLI Documentation

| Document | Description |
|----------|-------------|
| [cli_overview.md](./cli_overview.md) | CLI commands |
| [cli_wallet.md](./cli_wallet.md) | Wallet management |

### Architecture Documentation

| Document | Description |
|----------|-------------|
| [compute_engine_overview.md](./compute_engine_overview.md) | PoUW compute |
| [staking_model.md](./staking_model.md) | Staking mechanics |
| [governance_model.md](./governance_model.md) | Governance |

### Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TYPESCRIPT SDK QUICK REFERENCE                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   INSTALLATION                                                              │
│   ────────────                                                              │
│   npm install @mbongo/sdk                                                  │
│                                                                             │
│   IMPORTS                                                                   │
│   ───────                                                                   │
│   import { MbongoClient, Wallet, parseUnits, formatUnits } from '@mbongo/sdk';
│                                                                             │
│   CONNECTION                                                                │
│   ──────────                                                                │
│   const client = await MbongoClient.connect('https://rpc.mbongo.io');      │
│                                                                             │
│   QUERIES                                                                   │
│   ───────                                                                   │
│   await client.getBlockNumber()                                            │
│   await client.getBalance(address)                                         │
│   await client.getBlock('latest')                                          │
│                                                                             │
│   TRANSACTIONS                                                              │
│   ────────────                                                              │
│   const tx = await client.sendTransaction({ from, to, value, signer });    │
│   const receipt = await tx.wait();                                         │
│                                                                             │
│   SUBSCRIPTIONS                                                             │
│   ─────────────                                                             │
│   client.on('block', (block) => { ... });                                  │
│                                                                             │
│   UTILITIES                                                                 │
│   ─────────                                                                 │
│   parseUnits('100', 18)     // 100 MBO in wei                              │
│   formatUnits(wei, 18)      // wei to MBO string                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides the TypeScript SDK overview for Mbongo Chain. For Rust SDK, see [rust_sdk_overview.md](./rust_sdk_overview.md). For RPC API, see [rpc_overview.md](./rpc_overview.md).*

