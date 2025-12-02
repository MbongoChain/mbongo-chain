# Mbongo Chain — Node Setup Overview

> **Document Type:** Infrastructure Guide  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Audience:** Node Operators, Validators, Compute Providers

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Hardware Requirements](#2-hardware-requirements)
3. [Software Requirements](#3-software-requirements)
4. [Node Installation Overview](#4-node-installation-overview)
5. [Network Modes](#5-network-modes)
6. [Security Overview](#6-security-overview)
7. [Cross Links](#7-cross-links)

---

## 1. Introduction

### 1.1 What is a Node?

A **node** is a computer running Mbongo Chain software that participates in the network. Nodes maintain copies of the blockchain, validate transactions, and propagate data to other nodes.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MBONGO CHAIN NODE TYPES                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                        FULL NODE                                     │  │
│   │                                                                      │  │
│   │  • Stores complete blockchain history                               │  │
│   │  • Validates all transactions and blocks                            │  │
│   │  • Relays data to other nodes                                       │  │
│   │  • Does NOT participate in consensus                                │  │
│   │                                                                      │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                      VALIDATOR NODE (PoS)                            │  │
│   │                                                                      │  │
│   │  • Full node + consensus participation                              │  │
│   │  • Stakes MBO as collateral (min 50,000 MBO)                        │  │
│   │  • Proposes and attests to blocks                                   │  │
│   │  • Earns 50% of block rewards                                       │  │
│   │                                                                      │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                   COMPUTE PROVIDER NODE (PoUW)                       │  │
│   │                                                                      │  │
│   │  • Full node + compute execution                                    │  │
│   │  • Runs heterogeneous compute hardware (GPU/TPU/CPU/FPGA/ASIC)      │  │
│   │  • Executes assigned compute tasks                                  │  │
│   │  • Submits verifiable compute receipts                              │  │
│   │  • Earns 50% of block rewards                                       │  │
│   │                                                                      │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                       LIGHT CLIENT (Future)                          │  │
│   │                                                                      │  │
│   │  • Stores only block headers                                        │  │
│   │  • Queries full nodes for data                                      │  │
│   │  • Minimal resource requirements                                    │  │
│   │                                                                      │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Validator Node vs Compute Provider Node

| Aspect | Validator Node (PoS) | Compute Provider Node (PoUW) |
|--------|---------------------|------------------------------|
| **Primary Role** | Secure consensus, propose/attest blocks | Execute compute tasks, submit proofs |
| **Hardware Focus** | CPU, storage, network | Accelerators (GPU/TPU/CPU/FPGA/ASIC) |
| **Stake Required** | 50,000 MBO minimum | Optional (improves priority) |
| **Reward Share** | 50% of block rewards | 50% of block rewards |
| **Slashing Risk** | Double-sign, downtime | Invalid compute receipts |
| **Uptime Requirement** | >99% recommended | Task-dependent |
| **Network Role** | Block production, attestation | Task execution, proof generation |

### 1.3 Why Run a Node?

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BENEFITS OF RUNNING A NODE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   💰 EARN REWARDS                                                           │
│   ───────────────                                                           │
│   • Validators earn MBO from block rewards + priority fees                 │
│   • Compute providers earn MBO from compute tasks + block rewards          │
│   • Delegators can earn passive income through validators                  │
│                                                                             │
│   🔒 STRENGTHEN SECURITY                                                    │
│   ──────────────────────                                                    │
│   • More validators = more decentralization                                │
│   • More compute providers = stronger PoUW security                        │
│   • Participate in network governance                                      │
│                                                                             │
│   🖥️ ACCESS COMPUTE MARKETPLACE                                             │
│   ─────────────────────────────                                             │
│   • Monetize idle compute hardware                                         │
│   • Participate in AI/ML inference marketplace                             │
│   • Provide rendering, ZK proofs, scientific compute                       │
│                                                                             │
│   🏛️ PARTICIPATE IN CONSENSUS                                               │
│   ───────────────────────────                                               │
│   • Vote on protocol upgrades                                              │
│   • Shape network direction                                                │
│   • Be part of decentralized infrastructure                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.4 Requirements to Join

| Network | Status | Requirements |
|---------|--------|--------------|
| **Testnet** | Open | Hardware requirements met, test MBO from faucet |
| **Mainnet** | Permissionless | Hardware requirements met, real MBO stake (validators) |

**Testnet Access:**
- No stake required for full nodes
- Test MBO available from faucet
- Recommended for first-time operators

**Mainnet Access:**
- Validators: 50,000 MBO minimum stake
- Compute Providers: Hardware + optional stake for priority
- Full Nodes: No stake required

---

## 2. Hardware Requirements

### 2.1 Validator Node Hardware (PoS)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    VALIDATOR NODE HARDWARE REQUIREMENTS                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   COMPONENT       │ MINIMUM            │ RECOMMENDED         │ NOTES       │
│   ────────────────┼────────────────────┼─────────────────────┼─────────────│
│   CPU             │ 4 cores            │ 8+ cores            │ x86_64      │
│   RAM             │ 8 GB               │ 16 GB               │ DDR4/DDR5   │
│   Storage         │ 512 GB NVMe        │ 1 TB NVMe           │ High IOPS   │
│   Network         │ 10 Mbps            │ 100 Mbps            │ Stable      │
│   OS              │ Ubuntu 22.04 LTS   │ Ubuntu 22.04 LTS    │ Linux       │
│                   │ Windows Server*    │                     │ Experimental│
│                                                                             │
│   * Windows Server support is experimental; Linux is strongly recommended. │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| **CPU** | 4 cores | 8+ cores | Modern x86_64 processor |
| **RAM** | 8 GB | 16 GB | DDR4 or DDR5 |
| **Storage** | 512 GB NVMe | 1 TB NVMe | High IOPS required |
| **Network** | 10 Mbps | 100 Mbps | Stable, low latency |
| **OS** | Ubuntu 22.04 LTS | Ubuntu 22.04 LTS | Windows Server (experimental) |

### 2.2 Compute Provider Hardware (PoUW)

> ⚠️ **IMPORTANT: PoUW is Fully Heterogeneous Compute**
>
> The Proof-of-Useful-Work (PoUW) layer supports **heterogeneous compute hardware**:
> - **GPUs**: NVIDIA (Turing → Ada), AMD (RDNA2+)
> - **TPUs**: Google TPU v3/v4
> - **CPUs**: High-core-count server processors
> - **FPGAs**: Xilinx, Intel Agilex
> - **ASICs**: Custom accelerators with supported drivers
>
> Any hardware class can participate, provided it runs deterministic jobs and submits verifiable proofs. Performance scores are **normalized across hardware types**.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                 COMPUTE PROVIDER HARDWARE REQUIREMENTS                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   TIER             │ HARDWARE EXAMPLES              │ USE CASES             │
│   ─────────────────┼────────────────────────────────┼───────────────────────│
│   Entry            │ RTX 3080, RTX 3090             │ Light inference       │
│   Standard         │ RTX 4080, RTX 4090             │ ML inference          │
│   Professional     │ A100, L40S, TPU v4             │ Training, ZK proofs   │
│   Enterprise       │ H100, Custom ASIC              │ Large-scale compute   │
│                                                                             │
│   ─────────────────────────────────────────────────────────────────────────│
│                                                                             │
│   HARDWARE TYPE    │ SUPPORTED          │ DRIVER/RUNTIME                   │
│   ─────────────────┼────────────────────┼────────────────────────────────── │
│   NVIDIA GPU       │ ✓ Full support     │ CUDA 12.x                        │
│   AMD GPU          │ ✓ Full support     │ ROCm 5.x+                        │
│   Google TPU       │ ✓ Full support     │ TPU Runtime                      │
│   Intel CPU        │ ✓ Full support     │ oneAPI                           │
│   AMD CPU          │ ✓ Full support     │ Standard runtime                 │
│   Xilinx FPGA      │ ✓ Supported        │ Vitis/Vivado                     │
│   Intel FPGA       │ ✓ Supported        │ Quartus                          │
│   Custom ASIC      │ ○ On request       │ Custom driver                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| **Accelerator** | RTX 3080 / equivalent | RTX 4090 / A100 / TPU v4 | Any supported type |
| **Accelerator RAM** | 10 GB | 24–80 GB | Depends on task type |
| **System RAM** | 16 GB | 32–64 GB | Higher for large models |
| **Storage** | 512 GB NVMe | 1–2 TB NVMe | Fast storage for models |
| **Network** | 100 Mbps | 1 Gbps | High throughput preferred |
| **Power** | 300W headroom | 600W+ headroom | For multi-accelerator |

### 2.3 Combined Validator + Compute Provider

Operators can run **both** a Validator Node and Compute Provider on the same machine:

| Component | Requirement |
|-----------|-------------|
| **CPU** | 8+ cores |
| **RAM** | 32+ GB |
| **Storage** | 2 TB NVMe |
| **GPU/Accelerator** | As per compute tier |
| **Network** | 100+ Mbps |

---

## 3. Software Requirements

### 3.1 Required Software Stack

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SOFTWARE REQUIREMENTS                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   CORE SOFTWARE                                                             │
│   ═════════════                                                             │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  Rust Toolchain (rustup)                                            │  │
│   │  • Version: 1.75+ (stable)                                          │  │
│   │  • Components: rustc, cargo, rustfmt, clippy                        │  │
│   │  • Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  Mbongo CLI                                                         │  │
│   │  • Command-line interface for node management                       │  │
│   │  • Install: cargo install mbongo-cli                                │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  Mbongo Node                                                        │  │
│   │  • Core node binary                                                 │  │
│   │  • Install: cargo install mbongo-node                               │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Software by Node Type

| Software | Full Node | Validator | Compute Provider |
|----------|-----------|-----------|------------------|
| **Rust Toolchain** | ✓ Required | ✓ Required | ✓ Required |
| **Mbongo CLI** | ✓ Required | ✓ Required | ✓ Required |
| **Mbongo Node** | ✓ Required | ✓ Required | ✓ Required |
| **Validator Client** | — | ✓ Required | — |
| **Compute Provider Daemon** | — | — | ✓ Required |
| **GPU Drivers (CUDA/ROCm)** | — | — | If GPU |
| **TPU Runtime** | — | — | If TPU |
| **FPGA Framework** | — | — | If FPGA |

### 3.3 Driver Requirements (Compute Providers)

| Hardware | Driver/Runtime | Version | Install Command |
|----------|----------------|---------|-----------------|
| **NVIDIA GPU** | CUDA Toolkit | 12.x | `apt install cuda-toolkit-12-x` |
| **AMD GPU** | ROCm | 5.7+ | See AMD docs |
| **Intel GPU** | oneAPI | 2025.x | See Intel docs |
| **Google TPU** | TPU Runtime | Latest | Cloud-specific |
| **Xilinx FPGA** | Vitis | 2023.x | Xilinx installer |
| **Intel FPGA** | Quartus | 23.x | Intel installer |

### 3.4 Firewall Rules

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         REQUIRED FIREWALL PORTS                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   PORT        │ PROTOCOL │ DIRECTION │ PURPOSE                             │
│   ────────────┼──────────┼───────────┼─────────────────────────────────────│
│   30303       │ TCP/UDP  │ Inbound   │ P2P network (required)              │
│   8545        │ TCP      │ Inbound   │ HTTP RPC (optional, localhost only) │
│   8546        │ TCP      │ Inbound   │ WebSocket RPC (optional)            │
│   9090        │ TCP      │ Inbound   │ Metrics endpoint (optional)         │
│   6060        │ TCP      │ Inbound   │ pprof debugging (disabled default)  │
│                                                                             │
│   RECOMMENDATIONS                                                           │
│   ───────────────                                                           │
│   • Open 30303 (P2P) to all                                                │
│   • Restrict RPC ports (8545, 8546) to localhost or trusted IPs            │
│   • Use reverse proxy (nginx) for public RPC access                        │
│   • Enable rate limiting on RPC endpoints                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.5 System Services

| Service | Purpose | Auto-start |
|---------|---------|------------|
| `mbongo-node.service` | Core node daemon | Yes |
| `mbongo-validator.service` | Validator client | Yes (if validator) |
| `mbongo-compute.service` | Compute provider daemon | Yes (if provider) |

Example systemd service:

```ini
[Unit]
Description=Mbongo Node
After=network.target

[Service]
Type=simple
User=mbongo
ExecStart=/usr/local/bin/mbongo node start --config /etc/mbongo/config.yaml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

---

## 4. Node Installation Overview

### 4.1 Installation Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    NODE INSTALLATION PIPELINE                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  STEP 1: INSTALL DEPENDENCIES                                       │  │
│   │                                                                      │  │
│   │  • Update system packages                                           │  │
│   │  • Install build tools (gcc, make, pkg-config)                      │  │
│   │  • Install Rust toolchain                                           │  │
│   │  • Install hardware drivers (GPU/TPU if compute provider)           │  │
│   │                                                                      │  │
│   └─────────────────────────────────┬───────────────────────────────────┘  │
│                                     │                                       │
│                                     ▼                                       │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  STEP 2: INSTALL MBONGO CHAIN BINARIES                              │  │
│   │                                                                      │  │
│   │  • Install mbongo-cli via cargo                                     │  │
│   │  • Install mbongo-node via cargo                                    │  │
│   │  • Verify installation: mbongo --version                            │  │
│   │  • Download genesis file for target network                         │  │
│   │                                                                      │  │
│   └─────────────────────────────────┬───────────────────────────────────┘  │
│                                     │                                       │
│                                     ▼                                       │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  STEP 3: CONFIGURE NODE TYPE                                        │  │
│   │                                                                      │  │
│   │  Full Node:                                                         │  │
│   │  • mbongo config init --network mainnet                             │  │
│   │                                                                      │  │
│   │  Validator:                                                         │  │
│   │  • mbongo config init --network mainnet --validator                 │  │
│   │  • mbongo wallet create --keystore ./validator-key.json             │  │
│   │  • Register validator stake on-chain                                │  │
│   │                                                                      │  │
│   │  Compute Provider:                                                  │  │
│   │  • mbongo config init --network mainnet --compute-provider          │  │
│   │  • Configure hardware detection                                     │  │
│   │  • Register provider on-chain                                       │  │
│   │                                                                      │  │
│   └─────────────────────────────────┬───────────────────────────────────┘  │
│                                     │                                       │
│                                     ▼                                       │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  STEP 4: SYNC WITH THE NETWORK                                      │  │
│   │                                                                      │  │
│   │  • Start node: mbongo node start                                    │  │
│   │  • Monitor sync progress: mbongo node sync-status                   │  │
│   │  • Wait for full sync (may take hours depending on chain height)    │  │
│   │  • Verify sync: mbongo node info                                    │  │
│   │                                                                      │  │
│   └─────────────────────────────────┬───────────────────────────────────┘  │
│                                     │                                       │
│                                     ▼                                       │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │  STEP 5: START MONITORING & SECURITY                                │  │
│   │                                                                      │  │
│   │  • Enable systemd service for auto-restart                          │  │
│   │  • Configure log rotation                                           │  │
│   │  • Set up Prometheus/Grafana metrics                                │  │
│   │  • Configure alerts (downtime, sync issues)                         │  │
│   │  • Enable slashing protection (validators)                          │  │
│   │  • Backup keystore files securely                                   │  │
│   │                                                                      │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Quick Start Commands

```bash
# Step 1: Install dependencies
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential pkg-config libssl-dev git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Step 2: Install Mbongo binaries
cargo install mbongo-cli mbongo-node

# Step 3: Initialize configuration
mbongo config init --network testnet

# Step 4: Start node and sync
mbongo node start

# Step 5: Check sync status
mbongo node sync-status
```

---

## 5. Network Modes

### 5.1 Network Selection

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         NETWORK MODES                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   TESTNET                                                                   │
│   ═══════                                                                   │
│   • Purpose: Testing, development, experimentation                         │
│   • Chain ID: 11155111                                                     │
│   • Tokens: Test MBO (no real value)                                       │
│   • Faucet: Available for free test tokens                                 │
│   • Reset: May be reset periodically                                       │
│   • Recommended for: First-time operators, developers                      │
│                                                                             │
│   MAINNET                                                                   │
│   ═══════                                                                   │
│   • Purpose: Production network                                            │
│   • Chain ID: 1                                                            │
│   • Tokens: Real MBO (has economic value)                                  │
│   • Stake: Real collateral at risk                                         │
│   • Slashing: Real penalties for misbehavior                               │
│   • Recommended for: Production validators, compute providers              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

| Network | Chain ID | RPC Endpoint | Use Case |
|---------|----------|--------------|----------|
| **Mainnet** | 1 | `https://rpc.mbongo.io` | Production |
| **Testnet** | 11155111 | `https://testnet-rpc.mbongo.io` | Testing |
| **Devnet** | 31337 | `http://localhost:8545` | Local development |

### 5.2 RPC Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Disabled** | No RPC server | Maximum security |
| **Local Only** | RPC on localhost (127.0.0.1) | Personal use |
| **LAN** | RPC on private network | Internal services |
| **Public** | RPC on public interface | DApp infrastructure |

```bash
# Local-only RPC (recommended)
mbongo node start --rpc-addr 127.0.0.1:8545

# Public RPC (use with caution + rate limiting)
mbongo node start --rpc-addr 0.0.0.0:8545 --rpc-rate-limit 100
```

### 5.3 Node Sync Modes

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SYNC MODES                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   MODE           │ STORAGE    │ SYNC TIME   │ CAPABILITIES                 │
│   ───────────────┼────────────┼─────────────┼──────────────────────────────│
│   Full           │ ~500 GB    │ Hours       │ Full validation, RPC         │
│   Archive        │ ~2+ TB     │ Days        │ Full history, queries        │
│   Light (Future) │ ~10 GB     │ Minutes     │ Basic queries only           │
│                                                                             │
│   RECOMMENDATIONS                                                           │
│   ───────────────                                                           │
│   • Validators: Full mode (default)                                        │
│   • Compute Providers: Full mode (default)                                 │
│   • Block Explorers: Archive mode                                          │
│   • End Users: Light mode (when available)                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Security Overview

### 6.1 Key Management

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         KEY SECURITY                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ⚠️ CRITICAL: PROTECT YOUR KEYS                                            │
│                                                                             │
│   KEY TYPES                                                                 │
│   ═════════                                                                 │
│   • Node Identity Key: Identifies your node on P2P network                 │
│   • Validator Key: Signs blocks and attestations (HIGH VALUE)              │
│   • Withdrawal Key: Controls stake withdrawal (CRITICAL)                   │
│   • Compute Provider Key: Signs compute receipts                           │
│                                                                             │
│   STORAGE RULES                                                             │
│   ═════════════                                                             │
│   ✓ Store keys in encrypted keystore files                                 │
│   ✓ Use strong passwords (16+ characters, random)                          │
│   ✓ Back up keystore to offline storage                                    │
│   ✓ Test recovery process before going live                                │
│   ✗ Never share keys or mnemonics                                          │
│   ✗ Never store keys in plain text                                         │
│   ✗ Never commit keys to git repositories                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Encrypted Keystore

All keys should be stored in encrypted keystore format:

```bash
# Create encrypted keystore
mbongo wallet create --keystore ./keys/validator.json

# Export with encryption
mbongo wallet export --keystore ./keys/validator.json --output ./backup/

# Import from backup
mbongo wallet import --keystore ./backup/validator.json
```

### 6.3 Backup Rules

| Item | Backup Method | Frequency | Storage |
|------|---------------|-----------|---------|
| **Validator Key** | Encrypted export | Once (at creation) | Offline, multiple locations |
| **Withdrawal Key** | Encrypted export | Once (at creation) | Cold storage |
| **Node Config** | File copy | Weekly | Secure backup |
| **Slashing DB** | File copy | Daily | Local + remote |

### 6.4 Slashing Protection

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SLASHING PROTECTION (VALIDATORS)                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   SLASHABLE OFFENSES                                                        │
│   ══════════════════                                                        │
│   • Double-signing: Signing two blocks for same slot (5% stake)            │
│   • Surround vote: Conflicting attestations (5% stake)                     │
│   • Extended downtime: Missing 500+ consecutive slots (0.5% stake)         │
│                                                                             │
│   PROTECTION MEASURES                                                       │
│   ═══════════════════                                                       │
│   ✓ Enable slashing protection database                                    │
│   ✓ Never run same validator key on multiple machines                      │
│   ✓ Wait for full sync before starting validator                           │
│   ✓ Use UPS for power protection                                           │
│   ✓ Monitor for duplicate validator alerts                                 │
│                                                                             │
│   SLASHING PROTECTION DB                                                    │
│   ══════════════════════                                                    │
│   Location: ~/.mbongo/slashing_protection.db                               │
│   Purpose: Prevents signing conflicting messages                           │
│   Backup: Include in regular backup routine                                │
│   Migration: Export before moving to new machine                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.5 Compute Provider Safety

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                 COMPUTE PROVIDER SECURITY                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   RECEIPT FRAUD PREVENTION                                                  │
│   ════════════════════════                                                  │
│   • All compute receipts are cryptographically signed                      │
│   • Results are verified via replicated execution (sampling)               │
│   • Invalid receipts result in slashing (1,000 MBO)                        │
│   • Repeated fraud leads to permanent ban                                  │
│                                                                             │
│   EXECUTION ISOLATION                                                       │
│   ═══════════════════                                                       │
│   • Run compute tasks in isolated containers                               │
│   • No network access during execution                                     │
│   • Memory cleared between tasks                                           │
│   • Deterministic execution settings enforced                              │
│                                                                             │
│   BEST PRACTICES                                                            │
│   ══════════════                                                            │
│   ✓ Keep drivers updated                                                   │
│   ✓ Monitor hardware health (temps, errors)                                │
│   ✓ Use ECC memory if available                                            │
│   ✓ Implement graceful shutdown on failure                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.6 RPC Safety

| Risk | Mitigation |
|------|------------|
| **DoS attacks** | Rate limiting, fail2ban |
| **Unauthorized access** | IP whitelist, authentication |
| **Data leakage** | Disable sensitive methods |
| **Resource exhaustion** | Connection limits, timeouts |

```bash
# Recommended RPC configuration
mbongo node start \
  --rpc-addr 127.0.0.1:8545 \
  --rpc-rate-limit 100 \
  --rpc-max-connections 50 \
  --rpc-timeout 30s
```

---

## 7. Cross Links

### 7.1 Setup Guides

| Document | Description |
|----------|-------------|
| [validator_setup.md](./validator_setup.md) | Complete validator node setup guide |
| [compute_provider_setup.md](./compute_provider_setup.md) | Complete compute provider setup guide |
| [testnet_guide.md](./testnet_guide.md) | Testnet participation guide |
| [production_guide.md](./production_guide.md) | Production deployment best practices |

### 7.2 CLI Documentation

| Document | Description |
|----------|-------------|
| [cli_overview.md](./cli_overview.md) | CLI commands overview |
| [cli_node.md](./cli_node.md) | Node management commands |
| [cli_wallet.md](./cli_wallet.md) | Wallet and key commands |
| [cli_config.md](./cli_config.md) | Configuration commands |

### 7.3 SDK Documentation

| Document | Description |
|----------|-------------|
| [rust_sdk_overview.md](./rust_sdk_overview.md) | Rust SDK reference |
| [ts_sdk_overview.md](./ts_sdk_overview.md) | TypeScript SDK reference |
| [rpc_overview.md](./rpc_overview.md) | RPC API reference |
| [openapi_reference.md](./openapi_reference.md) | OpenAPI specification |

### 7.4 Architecture Documentation

| Document | Description |
|----------|-------------|
| [architecture_master_overview.md](./architecture_master_overview.md) | Full architecture overview |
| [compute_engine_overview.md](./compute_engine_overview.md) | PoUW compute engine |
| [consensus_validation.md](./consensus_validation.md) | Consensus mechanism |
| [staking_model.md](./staking_model.md) | Staking economics |

### 7.5 Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    NODE SETUP QUICK REFERENCE                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   INSTALLATION                                                              │
│   ────────────                                                              │
│   cargo install mbongo-cli mbongo-node                                     │
│   mbongo config init --network testnet                                     │
│   mbongo node start                                                        │
│                                                                             │
│   VALIDATOR SETUP                                                           │
│   ───────────────                                                           │
│   mbongo config init --network mainnet --validator                         │
│   mbongo wallet create --keystore ./validator.json                         │
│   mbongo validator register --stake 50000                                  │
│   mbongo validator start                                                   │
│                                                                             │
│   COMPUTE PROVIDER SETUP                                                    │
│   ──────────────────────                                                    │
│   mbongo config init --network mainnet --compute-provider                  │
│   mbongo compute register --hardware-type gpu                              │
│   mbongo compute start                                                     │
│                                                                             │
│   MONITORING                                                                │
│   ──────────                                                                │
│   mbongo node status                                                       │
│   mbongo node sync-status                                                  │
│   mbongo node peers                                                        │
│   mbongo node metrics                                                      │
│                                                                             │
│   PORTS                                                                     │
│   ─────                                                                     │
│   P2P:       30303 (TCP/UDP)                                               │
│   RPC:       8545 (TCP)                                                    │
│   WebSocket: 8546 (TCP)                                                    │
│   Metrics:   9090 (TCP)                                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides a high-level overview of Mbongo Chain node setup. For detailed step-by-step instructions, see the specific setup guides linked above.*

