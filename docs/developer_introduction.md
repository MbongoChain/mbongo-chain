<!-- Verified against tokenomics.md -->
# Mbongo Chain — Developer Introduction

Welcome to the Mbongo Chain developer documentation. This guide provides everything you need to understand, build, and contribute to the project.

---

## 1. Welcome to Mbongo Chain

### Introduction

**Mbongo Chain** is a next-generation Layer 1 blockchain built entirely in Rust. It combines Proof of Stake (PoS) for economic security with Proof of Useful Work (PoUW) for verifiable compute, creating a platform optimized for AI, HPC, and decentralized compute markets.

### Key Goals

| Goal | Description |
|------|-------------|
| **Compute-First** | Optimize for verifiable computation, not just token transfers |
| **High Performance** | 1-second block times, high throughput |
| **Security** | Hybrid consensus with strong finality guarantees |
| **Developer Experience** | Clean APIs, comprehensive documentation, modern tooling |
| **Sustainability** | Useful work instead of wasted energy |

### Architecture Philosophy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MBONGO CHAIN PHILOSOPHY                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  RUST-NATIVE                                                                │
│  ────────────                                                               │
│  • Built from scratch in Rust                                              │
│  • No legacy code or compatibility layers                                  │
│  • Memory safety and performance by design                                 │
│  • Modern async/await patterns                                             │
│                                                                             │
│  COMPUTE-FIRST                                                              │
│  ─────────────                                                              │
│  • Designed around verifiable computation                                  │
│  • PoUW makes security productive                                          │
│  • Native support for AI/ML workloads                                      │
│  • GPU-ready architecture                                                  │
│                                                                             │
│  HYBRID CONSENSUS (PoS + PoUW)                                              │
│  ─────────────────────────────                                              │
│  • PoS provides economic security and finality                             │
│  • PoUW validates useful computation                                       │
│  • Combined weight determines canonical chain                              │
│  • Best of both worlds: security + utility                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### What Makes Mbongo Chain Unique

1. **Useful Work**: Security computation produces real value (AI inference, scientific computing)
2. **Modular Design**: Clear separation between consensus, execution, and networking
3. **Deterministic Execution**: Bit-for-bit reproducible across all nodes
4. **Developer-Friendly**: Comprehensive documentation, clean APIs, modern tooling
5. **Future-Ready**: Architecture supports ZK proofs, parallel execution, GPU acceleration

---

## 2. Repository Tour

### Directory Structure

```
mbongo-chain/
├── cli/           # Command-line interface
├── crypto/        # Cryptographic primitives
├── docs/          # Documentation
├── network/       # P2P networking
├── node/          # Node orchestration
├── pow/           # Proof of Useful Work
├── runtime/       # Execution engine
├── scripts/       # Development scripts
├── spec/          # Protocol specifications
├── Cargo.toml     # Workspace manifest
├── README.md      # Project overview
└── CONTRIBUTING.md # Contribution guide
```

### Module Overview

| Directory | Purpose | Dependencies |
|-----------|---------|--------------|
| `/node` | Node orchestration, lifecycle management, coordination | runtime, network, crypto, pow |
| `/network` | P2P networking, gossip protocol, peer management | None (standalone) |
| `/runtime` | State machine, transaction execution, gas metering | crypto |
| `/crypto` | Hashing, signatures, Merkle trees, key management | None (standalone) |
| `/pow` | PoUW verification, compute task management, scoring | crypto |
| `/cli` | Command-line interface, user commands | node |
| `/docs` | Developer documentation, architecture guides | — |
| `/scripts` | Build scripts, setup automation, CI helpers | — |
| `/spec` | Protocol specifications, consensus rules | — |

### Module Dependency Graph

```
                              ┌─────────────┐
                              │     cli     │
                              │  (binary)   │
                              └──────┬──────┘
                                     │
                                     ▼
                              ┌─────────────┐
                              │    node     │
                              │(orchestrator)│
                              └──────┬──────┘
                                     │
         ┌───────────────────────────┼───────────────────────────┐
         │                           │                           │
         ▼                           ▼                           ▼
  ┌─────────────┐            ┌─────────────┐            ┌─────────────┐
  │   network   │            │   runtime   │            │     pow     │
  └─────────────┘            └──────┬──────┘            └──────┬──────┘
                                    │                          │
                                    └────────────┬─────────────┘
                                                 │
                                                 ▼
                                          ┌─────────────┐
                                          │   crypto    │
                                          └─────────────┘
```

---

## 3. How the Chain Works (High-Level)

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MBONGO CHAIN SYSTEM OVERVIEW                            │
└─────────────────────────────────────────────────────────────────────────────┘

  USER                                                              NETWORK
    │                                                                  │
    │  Submit Transaction                                              │
    ▼                                                                  │
  ┌─────────────────────────────────────────────────────────────────────────┐
  │                           MBONGO NODE                                   │
  │                                                                         │
  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │
  │  │ NETWORKING  │◀──▶│   MEMPOOL   │◀──▶│  CONSENSUS  │                 │
  │  │             │    │             │    │  (PoS+PoUW) │                 │
  │  │ • Gossip    │    │ • Tx Queue  │    │             │                 │
  │  │ • Sync      │    │ • Priority  │    │ • Leader    │                 │
  │  │ • Discovery │    │ • Broadcast │    │ • Finality  │                 │
  │  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                 │
  │         │                  │                  │                        │
  │         └──────────────────┼──────────────────┘                        │
  │                            │                                           │
  │                            ▼                                           │
  │                   ┌─────────────────┐                                  │
  │                   │   EXECUTION     │                                  │
  │                   │                 │                                  │
  │                   │ • State Machine │                                  │
  │                   │ • Gas Metering  │                                  │
  │                   │ • Receipts      │                                  │
  │                   └────────┬────────┘                                  │
  │                            │                                           │
  │                            ▼                                           │
  │                   ┌─────────────────┐                                  │
  │                   │    STORAGE      │                                  │
  │                   │                 │                                  │
  │                   │ • State Trie    │                                  │
  │                   │ • Blocks        │                                  │
  │                   │ • Receipts      │                                  │
  │                   └─────────────────┘                                  │
  │                                                                         │
  └─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
                              BLOCKCHAIN
```

### Component Breakdown

#### Networking

The networking layer handles all peer-to-peer communication:

- **Peer Discovery**: Find and connect to other nodes
- **Gossip Protocol**: Propagate transactions and blocks
- **Sync**: Download missing blocks from peers
- **Message Routing**: Route messages to appropriate handlers

#### Mempool

The mempool manages pending transactions:

- **Transaction Queue**: Hold transactions awaiting inclusion
- **Prioritization**: Order by fee and other criteria
- **Validation**: Filter invalid transactions
- **Broadcasting**: Share transactions with peers

#### Consensus (PoS + PoUW)

Hybrid consensus combining stake and compute:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     HYBRID CONSENSUS                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│              PoS (50%)                         PoUW (50%)                  │
│         ┌─────────────────┐              ┌─────────────────┐               │
│         │                 │              │                 │               │
│         │  • Stake tokens │              │ • Compute tasks │               │
│         │  • Validate     │      +       │ • Submit proofs │               │
│         │  • Earn rewards │              │ • Earn rewards  │               │
│         │  • Risk slashing│              │ • Build rep     │               │
│         │                 │              │                 │               │
│         └────────┬────────┘              └────────┬────────┘               │
│                  │                                │                        │
│                  └────────────┬───────────────────┘                        │
│                               │                                            │
│                               ▼                                            │
│                    ┌─────────────────────┐                                 │
│                    │    CHAIN WEIGHT     │                                 │
│                    │                     │                                 │
│                    │  W = 0.7×Stake +    │                                 │
│                    │      0.3×Compute    │                                 │
│                    └─────────────────────┘                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### Execution Engine

The execution engine processes transactions:

- **State Machine**: Deterministic state transitions
- **Gas Metering**: Resource consumption tracking
- **Receipt Generation**: Execution results and logs

#### Storage

Persistent data storage:

- **State Trie**: Account balances, nonces, storage
- **Block Store**: Block headers and bodies
- **Receipt Store**: Transaction receipts and logs

#### Sync

Chain synchronization for new nodes:

- **Header Sync**: Download and verify headers
- **Body Sync**: Download block bodies in parallel
- **State Sync**: Fast sync via state snapshots (planned)

---

## 4. Developer Requirements

### Required Tools

| Tool | Version | Purpose | Installation |
|------|---------|---------|--------------|
| **Rust** | 1.70+ | Programming language | https://rustup.rs |
| **Cargo** | 1.70+ | Package manager | Bundled with Rust |
| **Clippy** | Latest | Linter | `rustup component add clippy` |
| **Rustfmt** | Latest | Code formatter | `rustup component add rustfmt` |
| **Git** | 2.30+ | Version control | https://git-scm.com |

### Recommended Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| **Cursor** | AI-assisted IDE | https://cursor.sh |
| **VS Code** | Alternative IDE | With rust-analyzer extension |
| **PowerShell** | Windows scripting | Pre-installed on Windows |
| **Make** | Build automation | Optional, for make commands |

### Optional (Advanced)

| Tool | Purpose | Notes |
|------|---------|-------|
| **GPU (NVIDIA/AMD)** | PoUW experimentation | CUDA/ROCm support planned |
| **Docker** | Containerized development | For devnet testing |
| **Prometheus** | Metrics collection | For monitoring |

---

## 5. Developer Setup Guide

Follow these steps to set up your development environment.

### Step 1: Clone the Repository

```powershell
git clone https://github.com/gkalombo21/mbongo-chain.git
cd mbongo-chain
```

### Step 2: Install Rust Toolchain

**Windows (PowerShell):**
```powershell
# Download and run rustup installer
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y
Remove-Item rustup-init.exe

# Restart PowerShell, then verify
rustc --version
cargo --version
```

**Linux/macOS:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustc --version
cargo --version
```

### Step 3: Install Developer Tools

```powershell
# Install Clippy (linter)
rustup component add clippy

# Install Rustfmt (formatter)
rustup component add rustfmt

# Verify installation
cargo clippy --version
cargo fmt --version
```

### Step 4: Build the Workspace

```powershell
# Build all modules in debug mode
cargo build --workspace

# Build in release mode (optimized)
cargo build --workspace --release
```

### Step 5: Run Clippy

```powershell
# Run Clippy on all modules
cargo clippy --workspace --all-targets

# Run with warnings as errors (CI mode)
cargo clippy --workspace --all-targets -- -D warnings
```

### Step 6: Format Code

```powershell
# Check formatting
cargo fmt --all -- --check

# Apply formatting
cargo fmt --all
```

### Step 7: Run the Node (Placeholder)

```powershell
# Run node help (once implemented)
cargo run -p node -- --help

# Run CLI help
cargo run -p cli -- --help

# Show version
cargo run -p cli -- version
```

### Quick Setup Script

Save this as `setup-dev.ps1` and run it:

```powershell
# Mbongo Chain Developer Setup
Write-Host "Setting up Mbongo Chain development environment..." -ForegroundColor Cyan

# Check Rust
if (Get-Command rustc -ErrorAction SilentlyContinue) {
    Write-Host "[OK] Rust installed: $(rustc --version)" -ForegroundColor Green
} else {
    Write-Host "[ERROR] Rust not found. Install from https://rustup.rs" -ForegroundColor Red
    exit 1
}

# Install components
rustup component add clippy rustfmt

# Build
Write-Host "Building workspace..." -ForegroundColor Cyan
cargo build --workspace

# Check
Write-Host "Running Clippy..." -ForegroundColor Cyan
cargo clippy --workspace --all-targets

# Format check
Write-Host "Checking formatting..." -ForegroundColor Cyan
cargo fmt --all -- --check

Write-Host "Setup complete!" -ForegroundColor Green
```

---

## 6. Contribution Workflow

### Branching Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     BRANCHING MODEL                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  main                                                                       │
│  ────                                                                       │
│  • Production-ready code                                                   │
│  • Protected branch                                                        │
│  • Merge only via PR                                                       │
│                                                                             │
│  develop                                                                    │
│  ───────                                                                    │
│  • Active development                                                      │
│  • Integration branch                                                      │
│  • PRs target this branch                                                  │
│                                                                             │
│  feature/<name>                                                             │
│  ──────────────                                                             │
│  • New features                                                            │
│  • Branch from: develop                                                    │
│  • Merge to: develop                                                       │
│                                                                             │
│  fix/<name>                                                                 │
│  ──────────                                                                 │
│  • Bug fixes                                                               │
│  • Branch from: develop                                                    │
│  • Merge to: develop                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### How to Open a PR

1. **Fork the repository** (if external contributor)

2. **Create a branch**:
   ```powershell
   git checkout develop
   git pull origin develop
   git checkout -b feature/my-feature
   ```

3. **Make changes** following code standards

4. **Run quality checks**:
   ```powershell
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo build --workspace
   cargo test --workspace
   ```

5. **Commit with conventional format**:
   ```powershell
   git add .
   git commit -m "feat: add new feature description"
   ```

6. **Push and create PR**:
   ```powershell
   git push origin feature/my-feature
   ```
   Then open a Pull Request on GitHub targeting `develop`.

### Code Quality Requirements

| Requirement | Command | Must Pass |
|-------------|---------|-----------|
| **Formatting** | `cargo fmt --all -- --check` | ✓ |
| **Linting** | `cargo clippy --workspace -- -D warnings` | ✓ |
| **Build** | `cargo build --workspace` | ✓ |
| **Tests** | `cargo test --workspace` | ✓ |
| **Documentation** | All public items documented | ✓ |

### Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new runtime interface
fix: resolve block validation bug
docs: update developer guide
refactor: improve pow module structure
test: add integration tests
chore: update dependencies
```

### Security Contact

**Do NOT report security vulnerabilities in public issues.**

Report privately to: **security@mbongo.money**

---

## 7. Important Architecture Documents

### Must-Read Documents

These documents provide essential understanding of the system:

| Document | Description | Priority |
|----------|-------------|----------|
| [final_architecture_overview.md](final_architecture_overview.md) | Complete system architecture | ⭐⭐⭐ |
| [runtime_architecture.md](runtime_architecture.md) | Execution engine design | ⭐⭐⭐ |
| [consensus_validation.md](consensus_validation.md) | Consensus rules and validation | ⭐⭐⭐ |
| [block_validation_pipeline.md](block_validation_pipeline.md) | Block processing flow | ⭐⭐ |
| [node_architecture.md](node_architecture.md) | Node internals | ⭐⭐ |
| [mempool_overview.md](mempool_overview.md) | Transaction pool design | ⭐⭐ |
| [networking_overview.md](networking_overview.md) | P2P networking | ⭐⭐ |
| [sync_validation.md](sync_validation.md) | Chain synchronization | ⭐ |
| [state_machine_validation.md](state_machine_validation.md) | State machine rules | ⭐ |

### Reading Order for New Developers

1. **Start here**: This document (`developer_introduction.md`)
2. **Architecture**: `final_architecture_overview.md`
3. **Your focus area**: Choose based on what you'll work on
   - Execution: `runtime_architecture.md`, `state_machine_validation.md`
   - Consensus: `consensus_validation.md`, `consensus_overview.md`
   - Networking: `networking_overview.md`, `sync_validation.md`
   - Node: `node_architecture.md`, `block_validation_pipeline.md`

### Quick Reference Links

| Topic | Document |
|-------|----------|
| Getting started | [getting_started.md](getting_started.md) |
| Development guide | [developer_guide.md](developer_guide.md) |
| Setup validation | [setup_validation.md](setup_validation.md) |
| Project roadmap | [roadmap.md](roadmap.md) |
| Guardian nodes | [guardian_status.md](guardian_status.md) |

---

## 8. Future Opportunities

### Research Areas

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     FUTURE OPPORTUNITIES                                    │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────────────────────────────────────────────────────────────┐
  │  GPU EXECUTION RESEARCH                                                 │
  │  ══════════════════════                                                 │
  │                                                                         │
  │  Current Status: Placeholder                                            │
  │                                                                         │
  │  Opportunities:                                                         │
  │  • Batch signature verification (ECDSA, BLS)                           │
  │  • Merkle tree computation                                             │
  │  • SNARK proof verification                                            │
  │  • PoUW compute task execution                                         │
  │                                                                         │
  │  Technologies: CUDA, ROCm, WebGPU                                       │
  └─────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────────────────────────────────────────────────────────────┐
  │  ZERO-KNOWLEDGE VALIDATION                                              │
  │  ═════════════════════════                                              │
  │                                                                         │
  │  Current Status: Research                                               │
  │                                                                         │
  │  Opportunities:                                                         │
  │  • ZK execution proofs (verify without re-executing)                   │
  │  • Privacy-preserving transactions                                     │
  │  • Succinct state proofs for light clients                             │
  │  • Cross-chain verification                                            │
  │                                                                         │
  │  Technologies: SNARK, STARK, Halo2, Plonk                              │
  └─────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────────────────────────────────────────────────────────────┐
  │  PARALLEL EXECUTION                                                     │
  │  ══════════════════                                                     │
  │                                                                         │
  │  Current Status: Planned                                                │
  │                                                                         │
  │  Opportunities:                                                         │
  │  • Transaction dependency analysis                                     │
  │  • Parallel execution lanes                                            │
  │  • Speculative execution                                               │
  │  • Multi-threaded state updates                                        │
  │                                                                         │
  │  Expected Improvement: 4-8x throughput                                  │
  └─────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────────────────────────────────────────────────────────────┐
  │  NETWORK UPGRADES                                                       │
  │  ════════════════                                                       │
  │                                                                         │
  │  Current Status: TCP implemented                                        │
  │                                                                         │
  │  Opportunities:                                                         │
  │  • QUIC transport (multiplexing, 0-RTT)                                │
  │  • Better NAT traversal                                                │
  │  • Peer reputation with ML                                             │
  │  • Dedicated relay network                                             │
  │                                                                         │
  │  Benefits: Lower latency, better connectivity                          │
  └─────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────────────────────────────────────────────────────────────┐
  │  VM DESIGN                                                              │
  │  ═════════                                                              │
  │                                                                         │
  │  Current Status: Native Rust execution                                  │
  │                                                                         │
  │  Opportunities:                                                         │
  │  • WASM VM integration (Wasmer/Wasmtime)                               │
  │  • RISC-V VM for ZK compatibility                                      │
  │  • Custom bytecode format                                              │
  │  • GPU-accelerated precompiles                                         │
  │                                                                         │
  │  Timeline: Q2-Q4 2026                                                   │
  └─────────────────────────────────────────────────────────────────────────┘
```

### Contribution Opportunities

| Area | Difficulty | Impact | Good First Issue |
|------|------------|--------|------------------|
| Documentation | Easy | High | ✓ |
| Test coverage | Easy | Medium | ✓ |
| CLI improvements | Medium | Medium | ✓ |
| Networking | Medium | High | |
| Consensus | Hard | High | |
| ZK integration | Hard | Very High | |
| GPU acceleration | Hard | Very High | |

### Getting Involved

1. **Start small**: Pick a "Good First Issue" from the issue tracker
2. **Read the code**: Understand the module you want to contribute to
3. **Ask questions**: Open a discussion or reach out to maintainers
4. **Submit PRs**: Start with documentation or test improvements
5. **Grow**: Take on more complex features as you learn the codebase

---

## Summary

Mbongo Chain is an ambitious project building a compute-first blockchain from scratch in Rust. We value:

- **Code quality**: Clean, well-documented, tested code
- **Collaboration**: Open discussion, constructive feedback
- **Innovation**: New ideas and approaches welcome
- **Security**: Security-first mindset in all decisions

We're excited to have you here. Start exploring, ask questions, and contribute!

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.

---

## Quick Reference

| Task | Command |
|------|---------|
| Clone repo | `git clone https://github.com/gkalombo21/mbongo-chain.git` |
| Build all | `cargo build --workspace` |
| Run tests | `cargo test --workspace` |
| Run Clippy | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format code | `cargo fmt --all` |
| Check format | `cargo fmt --all -- --check` |
| Run CLI | `cargo run -p cli -- --help` |
| Generate docs | `cargo doc --workspace --open` |

