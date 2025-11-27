# Mbongo Chain — Rust SDK Overview

> **Document Type:** SDK Reference  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Audience:** Rust Developers, Backend Engineers, Node Operators

---

## Table of Contents

1. [Purpose of the Rust SDK](#1-purpose-of-the-rust-sdk)
2. [Architecture](#2-architecture)
3. [Installation](#3-installation)
4. [Core Modules](#4-core-modules)
5. [Code Examples](#5-code-examples)
6. [Security Notes](#6-security-notes)
7. [Cross-Links](#7-cross-links)

---

## 1. Purpose of the Rust SDK

### 1.1 What is the Mbongo Rust SDK?

The Mbongo Rust SDK (`mbongo-sdk`) is the official Rust client library for interacting with Mbongo Chain. It provides type-safe, async-first APIs for all blockchain operations.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RUST SDK CAPABILITIES                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   BLOCKCHAIN QUERIES                    TRANSACTION MANAGEMENT              │
│   ══════════════════                    ══════════════════════              │
│   • Query blocks, transactions          • Build transactions               │
│   • Get account balances                • Sign with local keys             │
│   • Read contract state                 • Submit to mempool                │
│   • Subscribe to events                 • Track confirmations              │
│                                                                             │
│   WALLET OPERATIONS                     PoS STAKING                         │
│   ═════════════════                     ══════════                          │
│   • Key generation (BIP-39/44)          • Query validator set              │
│   • Keystore encryption                 • Stake/unstake operations         │
│   • Transaction signing                 • Delegation management            │
│   • Hardware wallet support             • Rewards withdrawal               │
│                                                                             │
│   PoUW COMPUTE                          VALIDATION TOOLS                    │
│   ════════════                          ════════════════                    │
│   • Submit compute tasks                • Verify signatures                │
│   • Generate compute receipts           • Validate receipts                │
│   • Query provider status               • Check proofs                     │
│   • Monitor task execution              • Audit transactions               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 When to Use the Rust SDK

| Use Case | Recommended |
|----------|-------------|
| **Backend services** | ✓ Yes |
| **High-performance applications** | ✓ Yes |
| **Validator tooling** | ✓ Yes |
| **Compute provider software** | ✓ Yes |
| **CLI applications** | ✓ Yes |
| **WASM/browser** | Consider TypeScript SDK |
| **Mobile apps** | Consider TypeScript SDK |

### 1.3 Design Principles

- **Type Safety**: Strongly typed APIs prevent runtime errors
- **Async-First**: Built on Tokio for high concurrency
- **Zero-Copy**: Efficient serialization with minimal allocations
- **Idiomatic Rust**: Follows Rust conventions and best practices
- **Deterministic**: Reproducible results across executions

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
│   │                        Your Application                              │  │
│   └───────────────────────────────┬─────────────────────────────────────┘  │
│                                   │                                         │
│                                   ▼                                         │
│   SDK LAYER                                                                 │
│   ═════════                                                                 │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                       mbongo-sdk                                     │  │
│   │                                                                       │  │
│   │   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐      │  │
│   │   │ Wallet  │ │ Compute │ │Consensus│ │ Mempool │ │Validator│      │  │
│   │   │ Module  │ │ Module  │ │ Module  │ │ Module  │ │ Module  │      │  │
│   │   └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘      │  │
│   │        │           │           │           │           │            │  │
│   │        └───────────┴───────────┼───────────┴───────────┘            │  │
│   │                                │                                     │  │
│   │   ┌────────────────────────────┴────────────────────────────────┐   │  │
│   │   │                     Transport Layer                          │   │  │
│   │   │                                                               │   │  │
│   │   │   ┌───────────┐  ┌───────────┐  ┌───────────┐               │   │  │
│   │   │   │   HTTP    │  │ WebSocket │  │    IPC    │               │   │  │
│   │   │   │ (reqwest) │  │(tokio-ws) │  │  (tokio)  │               │   │  │
│   │   │   └───────────┘  └───────────┘  └───────────┘               │   │  │
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

### 2.2 Async Runtime (Tokio)

The SDK is built on [Tokio](https://tokio.rs/) for async I/O:

```rust
// The SDK requires a Tokio runtime
#[tokio::main]
async fn main() -> Result<(), mbongo_sdk::Error> {
    let client = MbongoClient::connect("http://localhost:8545").await?;
    
    // All operations are async
    let block = client.get_block_number().await?;
    println!("Current block: {}", block);
    
    Ok(())
}
```

**Runtime Configuration:**

```rust
// Custom runtime for high-performance applications
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(4)
    .enable_all()
    .build()?;

runtime.block_on(async {
    let client = MbongoClient::connect("http://localhost:8545").await?;
    // ...
    Ok::<_, mbongo_sdk::Error>(())
})?;
```

### 2.3 Transport Layer

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TRANSPORT OPTIONS                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   TRANSPORT     │ CRATE       │ USE CASE                                   │
│   ──────────────┼─────────────┼────────────────────────────────────────────│
│   HTTP/HTTPS    │ reqwest     │ Request-response queries                   │
│   WebSocket     │ tokio-tungstenite │ Subscriptions, real-time            │
│   IPC           │ tokio       │ Local high-performance                     │
│                                                                             │
│   SELECTION                                                                 │
│   ═════════                                                                 │
│                                                                             │
│   // HTTP (default)                                                        │
│   let client = MbongoClient::connect("http://localhost:8545").await?;      │
│                                                                             │
│   // WebSocket                                                             │
│   let client = MbongoClient::connect("ws://localhost:8546").await?;        │
│                                                                             │
│   // IPC                                                                   │
│   let client = MbongoClient::connect_ipc("/path/to/mbongo.ipc").await?;    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Key Management

```rust
// Key management architecture
pub mod wallet {
    /// In-memory signer (testing only)
    pub struct LocalSigner { /* ... */ }
    
    /// Encrypted keystore (production)
    pub struct KeystoreSigner { /* ... */ }
    
    /// Hardware wallet (maximum security)
    pub struct LedgerSigner { /* ... */ }
    
    /// Trait for all signers
    pub trait Signer: Send + Sync {
        async fn sign(&self, message: &[u8]) -> Result<Signature, Error>;
        fn address(&self) -> Address;
    }
}
```

### 2.5 Signing Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SIGNING PIPELINE                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌───────────┐  │
│   │ Transaction │───▶│   Encode    │───▶│    Hash     │───▶│   Sign    │  │
│   │   Builder   │    │   (RLP)     │    │ (Keccak256) │    │  (ECDSA)  │  │
│   └─────────────┘    └─────────────┘    └─────────────┘    └─────┬─────┘  │
│                                                                   │        │
│                                                                   ▼        │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌───────────┐  │
│   │   Confirm   │◀───│  Broadcast  │◀───│   Attach    │◀───│ Signature │  │
│   │   Receipt   │    │  to Node    │    │  Signature  │    │  (v,r,s)  │  │
│   └─────────────┘    └─────────────┘    └─────────────┘    └───────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Installation

### 3.1 Cargo.toml

```toml
[dependencies]
# Core SDK
mbongo-sdk = "0.1"

# Required async runtime
tokio = { version = "1", features = ["full"] }

# Optional: Enhanced logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Optional: CLI applications
clap = { version = "4", features = ["derive"] }

# Optional: Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

### 3.2 Feature Flags

```toml
[dependencies]
mbongo-sdk = { version = "0.1", features = ["full"] }

# Available features:
# - "http"       : HTTP transport (default)
# - "ws"         : WebSocket transport
# - "ipc"        : IPC transport
# - "keystore"   : Encrypted keystore support
# - "ledger"     : Ledger hardware wallet
# - "compute"    : PoUW compute provider tools
# - "validator"  : Validator tooling
# - "full"       : All features
```

### 3.3 Minimal Example

```toml
# Cargo.toml
[package]
name = "my-mbongo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
mbongo-sdk = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

```rust
// src/main.rs
use mbongo_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = MbongoClient::connect("http://localhost:8545").await?;
    
    let block_number = client.get_block_number().await?;
    println!("Latest block: {}", block_number);
    
    Ok(())
}
```

---

## 4. Core Modules

### 4.1 Module Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SDK MODULES                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   mbongo_sdk::                                                             │
│   ├── client          # RPC client and connection management               │
│   ├── wallet          # Key management and signing                         │
│   ├── types           # Core types (Address, Hash, Block, Tx)              │
│   ├── compute         # PoUW compute provider tools                        │
│   ├── consensus       # Consensus queries and monitoring                   │
│   ├── mempool         # Transaction pool operations                        │
│   ├── validator       # Validator tooling                                  │
│   ├── staking         # Staking operations                                 │
│   ├── utils           # Encoding, hashing, conversion                      │
│   └── prelude         # Common imports                                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Wallet Module

```rust
use mbongo_sdk::wallet::{
    Wallet, LocalSigner, KeystoreSigner, Signer,
    Mnemonic, DerivationPath,
};

// Create wallet from mnemonic
let mnemonic = Mnemonic::generate(24)?;
let wallet = Wallet::from_mnemonic(&mnemonic, None)?;

// Create from keystore
let wallet = Wallet::from_keystore("./keystore.json", "password")?;

// Sign a message
let signature = wallet.sign_message(b"Hello, Mbongo!").await?;

// Get address
let address = wallet.address();
println!("Address: {}", address);
```

**Wallet API:**

| Method | Description |
|--------|-------------|
| `Wallet::new()` | Create random wallet |
| `Wallet::from_mnemonic()` | Restore from phrase |
| `Wallet::from_keystore()` | Load encrypted keystore |
| `wallet.sign_message()` | Sign arbitrary message |
| `wallet.sign_transaction()` | Sign transaction |
| `wallet.address()` | Get wallet address |
| `wallet.export_keystore()` | Export encrypted |

### 4.3 Compute Module (PoUW)

```rust
use mbongo_sdk::compute::{
    ComputeProvider, ComputeTask, ComputeReceipt,
    TaskType, ReceiptBuilder,
};

// Initialize compute provider
let provider = ComputeProvider::new(client.clone(), wallet.clone());

// Register as provider
provider.register(gpu_specs).await?;

// Fetch available tasks
let tasks = provider.get_pending_tasks().await?;

// Execute and submit receipt
for task in tasks {
    let result = execute_task(&task).await?;
    
    let receipt = ReceiptBuilder::new()
        .task_id(task.id)
        .result_hash(result.hash())
        .work_units(result.work_units)
        .build()?;
    
    provider.submit_receipt(receipt).await?;
}

// Query rewards
let rewards = provider.get_rewards().await?;
println!("Pending rewards: {} MBO", rewards.format_mbo());
```

**Compute API:**

| Method | Description |
|--------|-------------|
| `ComputeProvider::register()` | Register as provider |
| `provider.get_pending_tasks()` | List available tasks |
| `provider.submit_receipt()` | Submit compute proof |
| `provider.get_rewards()` | Query earned rewards |
| `provider.get_status()` | Provider status |

### 4.4 Consensus Module

```rust
use mbongo_sdk::consensus::{
    ConsensusClient, ValidatorSet, BlockHeader,
    FinalityStatus,
};

let consensus = ConsensusClient::new(client.clone());

// Get validator set
let validators = consensus.get_validator_set().await?;
for v in validators.iter() {
    println!("Validator: {} stake: {}", v.address, v.stake);
}

// Check finality
let status = consensus.get_finality_status(block_hash).await?;
match status {
    FinalityStatus::Finalized => println!("Block is final"),
    FinalityStatus::Pending(confirmations) => {
        println!("Pending: {} confirmations", confirmations);
    }
}

// Subscribe to new blocks
let mut blocks = consensus.subscribe_blocks().await?;
while let Some(block) = blocks.next().await {
    println!("New block: {} by {}", block.number, block.proposer);
}
```

### 4.5 Mempool Module

```rust
use mbongo_sdk::mempool::{
    MempoolClient, PendingTransaction, TxStatus,
};

let mempool = MempoolClient::new(client.clone());

// Get pending transactions
let pending = mempool.get_pending_transactions().await?;
println!("Pending txs: {}", pending.len());

// Subscribe to pending transactions
let mut txs = mempool.subscribe_pending().await?;
while let Some(tx) = txs.next().await {
    println!("New pending tx: {}", tx.hash);
}

// Check transaction status
let status = mempool.get_tx_status(tx_hash).await?;
match status {
    TxStatus::Pending => println!("In mempool"),
    TxStatus::Included(block) => println!("In block {}", block),
    TxStatus::Dropped(reason) => println!("Dropped: {}", reason),
}
```

### 4.6 Validator Tools Module

```rust
use mbongo_sdk::validator::{
    ValidatorClient, AttestationDuty, ProposalDuty,
    SlashingProtection,
};

let validator = ValidatorClient::new(client.clone(), wallet.clone());

// Check duties
let duties = validator.get_duties(epoch).await?;
for duty in duties {
    match duty {
        Duty::Propose(slot) => {
            println!("Propose at slot {}", slot);
        }
        Duty::Attest(slot, committee) => {
            println!("Attest at slot {} committee {}", slot, committee);
        }
    }
}

// Submit attestation with slashing protection
let slashing_db = SlashingProtection::open("./slashing.db")?;
let attestation = validator.create_attestation(duty, &slashing_db).await?;
validator.submit_attestation(attestation).await?;

// Get validator status
let status = validator.get_status().await?;
println!("Status: {:?}", status);
println!("Balance: {} MBO", status.balance.format_mbo());
```

---

## 5. Code Examples

### 5.1 Connect to Node

```rust
use mbongo_sdk::prelude::*;

#[tokio::main]
async fn main() -> Result<(), mbongo_sdk::Error> {
    // HTTP connection
    let client = MbongoClient::builder()
        .endpoint("http://localhost:8545")
        .timeout(Duration::from_secs(30))
        .retry_count(3)
        .build()
        .await?;
    
    // Verify connection
    let chain_id = client.get_chain_id().await?;
    let block_number = client.get_block_number().await?;
    
    println!("Connected to chain {} at block {}", chain_id, block_number);
    
    Ok(())
}
```

### 5.2 Query Block

```rust
use mbongo_sdk::prelude::*;

async fn query_blocks(client: &MbongoClient) -> Result<(), mbongo_sdk::Error> {
    // Get latest block
    let latest = client.get_block(BlockId::Latest).await?;
    println!("Latest block: {}", latest.number);
    println!("  Hash: {}", latest.hash);
    println!("  Timestamp: {}", latest.timestamp);
    println!("  Transactions: {}", latest.transactions.len());
    println!("  PoUW Score: {}", latest.pouw_score);
    
    // Get block by number
    let block = client.get_block(BlockId::Number(12345678)).await?;
    
    // Get block by hash
    let block = client.get_block(BlockId::Hash(hash)).await?;
    
    // Get block with full transactions
    let block = client.get_block_with_txs(BlockId::Latest).await?;
    for tx in block.transactions {
        println!("  Tx: {} -> {} ({} MBO)", 
            tx.from, 
            tx.to.unwrap_or_default(),
            tx.value.format_mbo()
        );
    }
    
    Ok(())
}
```

### 5.3 Submit Transaction

```rust
use mbongo_sdk::prelude::*;
use mbongo_sdk::wallet::Wallet;
use mbongo_sdk::types::{TransactionRequest, U256};

async fn send_transaction(
    client: &MbongoClient,
    wallet: &Wallet,
) -> Result<TxHash, mbongo_sdk::Error> {
    // Build transaction
    let tx = TransactionRequest::new()
        .to("0x8Ba1f109551bD432803012645Ac136ddd64DBA72".parse()?)
        .value(U256::from_mbo(100))  // 100 MBO
        .gas_price(client.get_gas_price().await?)
        .gas(21000);
    
    // Get nonce
    let nonce = client.get_transaction_count(wallet.address(), BlockId::Latest).await?;
    let tx = tx.nonce(nonce);
    
    // Sign transaction
    let signed_tx = wallet.sign_transaction(tx, client.chain_id()).await?;
    
    // Send and wait for receipt
    let pending_tx = client.send_raw_transaction(signed_tx).await?;
    println!("Tx submitted: {}", pending_tx.tx_hash());
    
    // Wait for confirmation
    let receipt = pending_tx.await?;
    
    match receipt.status {
        TxStatus::Success => {
            println!("Transaction confirmed in block {}", receipt.block_number);
            println!("Gas used: {}", receipt.gas_used);
        }
        TxStatus::Failure => {
            println!("Transaction failed");
        }
    }
    
    Ok(receipt.transaction_hash)
}
```

### 5.4 Validate Compute Receipts

```rust
use mbongo_sdk::prelude::*;
use mbongo_sdk::compute::{ComputeReceipt, ReceiptValidator};

async fn validate_receipts(
    client: &MbongoClient,
    block_number: u64,
) -> Result<(), mbongo_sdk::Error> {
    // Get receipts from block
    let receipts = client.get_compute_receipts(block_number).await?;
    
    // Create validator
    let validator = ReceiptValidator::new(client.clone());
    
    for receipt in receipts {
        // Validate receipt
        let result = validator.validate(&receipt).await?;
        
        match result {
            ValidationResult::Valid => {
                println!("Receipt {} is valid", receipt.task_id);
                println!("  Provider: {}", receipt.provider);
                println!("  Work units: {}", receipt.work_units);
                println!("  Result hash: {}", receipt.result_hash);
            }
            ValidationResult::Invalid(reason) => {
                println!("Receipt {} is INVALID: {}", receipt.task_id, reason);
            }
        }
    }
    
    // Verify receipt signatures
    for receipt in receipts {
        let is_valid = receipt.verify_signature()?;
        let attester_valid = receipt.verify_attester_signatures()?;
        
        println!("Receipt {} signature: {}, attesters: {}",
            receipt.task_id,
            if is_valid { "✓" } else { "✗" },
            if attester_valid { "✓" } else { "✗" }
        );
    }
    
    Ok(())
}
```

### 5.5 Subscribe to Events

```rust
use mbongo_sdk::prelude::*;
use futures::StreamExt;

async fn subscribe_events(client: &MbongoClient) -> Result<(), mbongo_sdk::Error> {
    // Subscribe to new blocks
    let mut blocks = client.subscribe_blocks().await?;
    
    tokio::spawn(async move {
        while let Some(block) = blocks.next().await {
            println!("New block: {} ({})", block.number, block.hash);
        }
    });
    
    // Subscribe to pending transactions
    let mut pending_txs = client.subscribe_pending_transactions().await?;
    
    tokio::spawn(async move {
        while let Some(tx_hash) = pending_txs.next().await {
            println!("New pending tx: {}", tx_hash);
        }
    });
    
    // Subscribe to logs with filter
    let filter = LogFilter::new()
        .address("0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7".parse()?)
        .topic0(keccak256("Transfer(address,address,uint256)"));
    
    let mut logs = client.subscribe_logs(filter).await?;
    
    while let Some(log) = logs.next().await {
        println!("Event: {:?}", log);
    }
    
    Ok(())
}
```

### 5.6 Staking Operations

```rust
use mbongo_sdk::prelude::*;
use mbongo_sdk::staking::StakingClient;

async fn staking_example(
    client: &MbongoClient,
    wallet: &Wallet,
) -> Result<(), mbongo_sdk::Error> {
    let staking = StakingClient::new(client.clone(), wallet.clone());
    
    // Check current stake
    let info = staking.get_staking_info(wallet.address()).await?;
    println!("Current stake: {} MBO", info.staked.format_mbo());
    println!("Delegated: {} MBO", info.delegated.format_mbo());
    println!("Rewards: {} MBO", info.rewards.format_mbo());
    
    // Deposit stake (for validators)
    let tx = staking.deposit(U256::from_mbo(50000)).await?;
    println!("Stake deposited: {}", tx.tx_hash());
    
    // Delegate to validator
    let validator_address = "0x8Ba1f109...".parse()?;
    let tx = staking.delegate(validator_address, U256::from_mbo(1000)).await?;
    println!("Delegated: {}", tx.tx_hash());
    
    // Withdraw rewards
    let tx = staking.withdraw_rewards().await?;
    println!("Rewards withdrawn: {}", tx.tx_hash());
    
    // Begin unbonding
    let tx = staking.unbond(U256::from_mbo(500)).await?;
    println!("Unbonding started: {}", tx.tx_hash());
    
    Ok(())
}
```

### 5.7 Complete Application Example

```rust
use mbongo_sdk::prelude::*;
use mbongo_sdk::wallet::Wallet;
use std::time::Duration;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Connect to node
    let client = MbongoClient::builder()
        .endpoint("http://localhost:8545")
        .timeout(Duration::from_secs(30))
        .build()
        .await?;
    
    info!("Connected to Mbongo Chain");
    
    // Load wallet from keystore
    let wallet = Wallet::from_keystore(
        "./keystore/wallet.json",
        &std::env::var("WALLET_PASSWORD")?,
    )?;
    
    info!("Wallet loaded: {}", wallet.address());
    
    // Check balance
    let balance = client.get_balance(wallet.address(), BlockId::Latest).await?;
    info!("Balance: {} MBO", balance.format_mbo());
    
    // Subscribe to new blocks
    let client_clone = client.clone();
    tokio::spawn(async move {
        let mut blocks = client_clone.subscribe_blocks().await.unwrap();
        while let Some(block) = blocks.next().await {
            info!("New block: {} by {}", block.number, block.proposer);
        }
    });
    
    // Main application loop
    loop {
        // Your application logic here
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
```

---

## 6. Security Notes

### 6.1 Key Management Security

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║   ⚠️  KEY SECURITY BEST PRACTICES                                           ║
║                                                                             ║
║   NEVER                                                                     ║
║   ─────                                                                     ║
║   ✗ Store private keys in source code                                      ║
║   ✗ Log private keys or mnemonics                                          ║
║   ✗ Use LocalSigner in production                                          ║
║   ✗ Disable keystore encryption                                            ║
║                                                                             ║
║   ALWAYS                                                                    ║
║   ──────                                                                    ║
║   ✓ Use encrypted keystores (KeystoreSigner)                               ║
║   ✓ Load passwords from environment variables                              ║
║   ✓ Use hardware wallets for high-value operations                         ║
║   ✓ Implement slashing protection for validators                           ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

```rust
// ❌ WRONG: Private key in code
let wallet = Wallet::from_private_key("0xac0974bec39a17e36...")?;

// ✓ CORRECT: Encrypted keystore with env password
let password = std::env::var("WALLET_PASSWORD")
    .expect("WALLET_PASSWORD must be set");
let wallet = Wallet::from_keystore("./keystore.json", &password)?;

// ✓ BETTER: Hardware wallet
let wallet = Wallet::from_ledger(derivation_path).await?;
```

### 6.2 Signing Safety

```rust
use mbongo_sdk::wallet::SigningPolicy;

// Configure signing policy
let wallet = Wallet::from_keystore("./keystore.json", &password)?
    .with_policy(SigningPolicy::builder()
        // Require confirmation for large amounts
        .confirm_above(U256::from_mbo(1000))
        // Reject transactions to unknown addresses
        .whitelist_recipients(vec![known_address])
        // Rate limit signing
        .max_signatures_per_minute(10)
        .build()
    );
```

### 6.3 Async Safety

```rust
// ✓ CORRECT: Use Arc for shared client
use std::sync::Arc;

let client = Arc::new(MbongoClient::connect("http://localhost:8545").await?);

let client_clone = client.clone();
tokio::spawn(async move {
    // Safe to use across tasks
    let block = client_clone.get_block_number().await.unwrap();
});

// ✓ CORRECT: Use Mutex for shared mutable state
use tokio::sync::Mutex;

let nonce_manager = Arc::new(Mutex::new(NonceManager::new(client.clone())));

let nonce_manager_clone = nonce_manager.clone();
tokio::spawn(async move {
    let mut manager = nonce_manager_clone.lock().await;
    let nonce = manager.get_next_nonce(address).await.unwrap();
});
```

### 6.4 Error Handling

```rust
use mbongo_sdk::{Error, ErrorKind};

async fn safe_operation(client: &MbongoClient) -> Result<(), Error> {
    match client.get_block_number().await {
        Ok(block) => {
            println!("Block: {}", block);
            Ok(())
        }
        Err(e) => {
            match e.kind() {
                ErrorKind::Connection => {
                    eprintln!("Connection failed, retrying...");
                    // Implement retry logic
                }
                ErrorKind::Timeout => {
                    eprintln!("Request timed out");
                }
                ErrorKind::InvalidResponse => {
                    eprintln!("Invalid response from node");
                }
                _ => {
                    eprintln!("Unexpected error: {}", e);
                }
            }
            Err(e)
        }
    }
}
```

### 6.5 Slashing Protection (Validators)

```rust
use mbongo_sdk::validator::SlashingProtection;

// Initialize slashing protection database
let slashing_db = SlashingProtection::open("./slashing_protection.db")?;

// Check before signing attestation
if slashing_db.check_attestation(&attestation_data)? {
    // Safe to sign
    let signature = wallet.sign_attestation(&attestation_data).await?;
    
    // Record after signing
    slashing_db.record_attestation(&attestation_data)?;
} else {
    // Would cause slashing!
    error!("Attestation would cause slashing!");
}
```

---

## 7. Cross-Links

### CLI Documentation

| Document | Description |
|----------|-------------|
| [cli_overview.md](./cli_overview.md) | CLI commands overview |
| [cli_node.md](./cli_node.md) | Node management |
| [cli_wallet.md](./cli_wallet.md) | Wallet commands |
| [cli_config.md](./cli_config.md) | Configuration |

### API Documentation

| Document | Description |
|----------|-------------|
| [rpc_overview.md](./rpc_overview.md) | JSON-RPC API reference |

### Economic Documentation

| Document | Description |
|----------|-------------|
| [compute_engine_overview.md](./compute_engine_overview.md) | PoUW compute engine |
| [staking_model.md](./staking_model.md) | Staking mechanics |
| [fee_model.md](./fee_model.md) | Gas and fees |

### Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RUST SDK QUICK REFERENCE                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   IMPORTS                                                                   │
│   ───────                                                                   │
│   use mbongo_sdk::prelude::*;                                              │
│   use mbongo_sdk::wallet::Wallet;                                          │
│   use mbongo_sdk::compute::ComputeProvider;                                │
│                                                                             │
│   CONNECTION                                                                │
│   ──────────                                                                │
│   let client = MbongoClient::connect("http://localhost:8545").await?;      │
│   let client = MbongoClient::connect("ws://localhost:8546").await?;        │
│                                                                             │
│   WALLET                                                                    │
│   ──────                                                                    │
│   let wallet = Wallet::from_keystore("./key.json", "pass")?;               │
│   let address = wallet.address();                                          │
│   let sig = wallet.sign_message(msg).await?;                               │
│                                                                             │
│   QUERIES                                                                   │
│   ───────                                                                   │
│   client.get_block_number().await?                                         │
│   client.get_balance(addr, BlockId::Latest).await?                         │
│   client.get_block(BlockId::Number(n)).await?                              │
│                                                                             │
│   TRANSACTIONS                                                              │
│   ────────────                                                              │
│   let tx = TransactionRequest::new().to(addr).value(amount);               │
│   let signed = wallet.sign_transaction(tx, chain_id).await?;               │
│   let receipt = client.send_raw_transaction(signed).await?.await?;         │
│                                                                             │
│   SUBSCRIPTIONS                                                             │
│   ─────────────                                                             │
│   let mut blocks = client.subscribe_blocks().await?;                       │
│   while let Some(block) = blocks.next().await { ... }                      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides the Rust SDK overview for Mbongo Chain. For RPC details, see [rpc_overview.md](./rpc_overview.md). For CLI access, see [cli_overview.md](./cli_overview.md).*

