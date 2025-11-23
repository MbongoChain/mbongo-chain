# Mbongo Chain — Developer Onboarding Guide  
Status: Canonical  
Version: v1.0

Welcome to Mbongo Chain.  
This document provides a complete onboarding path for developers, contributors, and ecosystem builders.

Mbongo Chain is a **Rust-native, compute-first Layer 1 blockchain** that integrates:

- Proof-of-Stake (PoS)  
- Proof-of-Useful-Work (PoUW)  
- Proof-of-Compute (PoC)  
- AIDA economic regulator  
- deterministic Rust execution  
- WASM runtime (future)  
- GPU/TPU/NPU offload  

This onboarding guide will help you set up the environment, run nodes, interact with the chain, create modules, and contribute effectively.

---

# 1. Prerequisites

Before contributing, ensure you have:

### 1.1 Required Software  
- **Rust (stable toolchain)**  
  https://rustup.rs  
- **Cargo** (installed with Rust)  
- **Git**  
- **Node.js 18+** (for SDK + tooling)  
- **pnpm or npm**  
- **Docker** (optional: infra / testnet tooling)

### 1.2 Hardware Requirements (Local Dev)
- CPU: 4+ cores  
- RAM: 8 GB  
- Storage: SSD recommended  
- GPU (optional for PoUW testing)

---

# 2. Repository Structure

Mbongo Chain uses a **Rust workspace monorepo**:

mbongo-chain/
node/ # Core L1 node (networking, mempool, consensus)
runtime/ # State machine, modules, execution engine
pouw/ # PoUW compute engine + verification
crypto/ # Signature algorithms, hashing, VRF, SMT
wallet/ # CLI wallet & key manager
sdk/ # TypeScript + Rust SDKs
infra/ # Deployment scripts, monitoring, containers
docs/ # Specifications & technical documents
tests/ # Integration & scenario tests
Cargo.toml # Workspace configuration

yaml
Copier le code

Each crate is isolated, tested, documented, and versioned together.

---

# 3. Building the Project

Clone the repository:

```bash
git clone https://github.com/mbongo-chain/mbongo-chain
cd mbongo-chain
Build everything:

bash
Copier le code
cargo build --workspace --release
This produces binaries:

mbongo-node

mbongo-wallet

mbongo-pouw

mbongo-indexer

4. Running a Local Development Network
You can spawn a local single-node devnet:

bash
Copier le code
./target/release/mbongo-node start --dev
Features:

in-memory state

hot reload (no consensus)

no staking requirement

instant block production

Use this mode to experiment with:

transactions

modules

PoUW job submission

RPC interactions

5. Running a Validator Node
Generate keys:

bash
Copier le code
./target/release/mbongo-wallet keygen --output validator.json
Initialize:

bash
Copier le code
./target/release/mbongo-node init \
  --validator \
  --key validator.json \
  --data-dir ~/.mbongo/validator
Start the validator:

bash
Copier le code
./target/release/mbongo-node start \
  --config ~/.mbongo/validator/config.toml
A validator will:

participate in PoS

verify PoUW receipts

contribute to finality

receive rewards

6. Running a PoUW Compute Node
Install GPU drivers:

NVIDIA:

bash
Copier le code
sudo apt install nvidia-driver-550
sudo apt install cuda-toolkit-12-3
AMD:

bash
Copier le code
sudo apt install rocm-dev
Initialize:

bash
Copier le code
./target/release/mbongo-pouw init
Start worker:

bash
Copier le code
./target/release/mbongo-pouw start \
  --gpu \
  --jobs ai,zk,rendering \
  --rpc http://localhost:8545
A compute node:

retrieves PoUW tasks

executes AI/compute workloads

produces VWP (Validatable Work Proof) receipts

receives fees + PoUW rewards

7. Submitting Transactions
Use the wallet CLI:

bash
Copier le code
./target/release/mbongo-wallet send \
  --to <address> \
  --amount 10 \
  --rpc http://localhost:8545
Query balance:

bash
Copier le code
./target/release/mbongo-wallet balance <address>
List AIDA parameters:

bash
Copier le code
./target/release/mbongo-wallet aida-state
8. Interacting with RPC API
Mbongo provides:

JSON-RPC

WebSocket

gRPC

Examples:

Get latest block:

bash
Copier le code
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"method":"mbongo_latestBlock","params":[],"id":1}'
Get AIDA economics:

nginx
Copier le code
mbongo_getAIDAState
Get PoUW multipliers:

nginx
Copier le code
mbongo_getPoUWMultiplier
9. Using the TypeScript SDK
Install:

bash
Copier le code
pnpm add @mbongo/sdk
Example:

ts
Copier le code
import { MbongoClient } from "@mbongo/sdk";

const client = new MbongoClient("http://localhost:8545");

const block = await client.getLatestBlock();
console.log(block);

await client.sendTransaction({
  to: "MBO123...",
  amount: 10,
});
10. Using the Rust SDK
The Rust SDK allows:

contract development (future WASM)

off-chain workers

indexers

verifier logic

Example:

rust
Copier le code
use mbongo_sdk::client::Client;

let client = Client::new("http://localhost:8545");
let block = client.latest_block()?;
11. Creating a Runtime Module
Modules are located in:

bash
Copier le code
runtime/src/modules/
A module implements:

rust
Copier le code
fn validate(&self, tx: &Transaction) -> Result<()>;
fn execute(&self, tx: &Transaction, state: &mut State) -> Result<()>;
Use cases:

accounts

staking

governance

markets

PoUW

AIDA logic (read-only)

12. Creating Tests
Unit tests:

bash
Copier le code
cargo test --workspace
Scenario tests:

Copier le code
tests/
  pouw_sanity_test.rs
  staking_flow.rs
  governance_flow.rs
Integration example:

rust
Copier le code
#[test]
fn test_pouw_receipt_validation() {
    let mut env = TestEnv::new();
    env.submit_compute_job(...);
    env.run_block();
    assert!(env.receipt_is_valid());
}
13. Governance & Restrictions for Developers
Some parameters cannot be modified without:

DAO approval

Founder Council (10-year window)

These include:

emission schedule

max supply (31,536,000 MBO)

AIDA ranges

PoS/PoUW reward split

fee algorithm

slashing conditions

All changes must follow:

90-day Safety Review Window

governance proposal

simulations (AIDA advisory)

multi-sig Founder Council approval

14. Contributing to Mbongo Chain
Steps:

Fork repository

Create feature branch

Implement module / fix / improvement

Write unit + integration tests

Run cargo fmt + cargo clippy

Open PR with technical description

Request review from core engineering team

Contribution guidelines are stored in:

arduino
Copier le code
docs/contributing.md  (future)
15. Developer Roadmap
2025–2026 planned milestones include:

WASM smart contract support

compute sharding

AI model hosting layer

ZK-integrated PoUW verification

multi-GPU orchestration

hardware-based PoC (TEE support)

cross-chain bridges

16. Summary
Mbongo Chain offers:

a Rust-native, compute-first blockchain

PoS + PoUW hybrid consensus

verifiable AI/GPU workloads

deterministic execution engine

AIDA-regulated economics

modern SDKs (TS + Rust)

developer-friendly monorepo

This onboarding guide is the foundation for joining and contributing to the Mbongo Chain ecosystem.

Welcome aboard.
