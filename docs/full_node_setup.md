# Mbongo Chain — Full Node Setup Guide

> **Document Version:** 1.0.0  
> **Last Updated:** November 2025  
> **Target Audience:** Node Operators, Infrastructure Engineers, DevOps Teams

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Hardware Requirements](#2-hardware-requirements)
3. [Software Requirements](#3-software-requirements)
4. [Installation Guide](#4-installation-guide)
5. [Node Configuration](#5-node-configuration)
6. [Running the Node](#6-running-the-node)
7. [RPC Node Setup](#7-rpc-node-setup)
8. [Firewall & Security](#8-firewall--security)
9. [Backup & Restore](#9-backup--restore)
10. [Troubleshooting](#10-troubleshooting)
11. [Cross-References](#11-cross-references)

---

## 1. Introduction

### What is a Mbongo Full Node?

A **Mbongo Full Node** is a software instance that maintains a complete copy of the Mbongo blockchain, validates all transactions and blocks, and participates in the peer-to-peer network. Full nodes are the backbone of the Mbongo Chain network, ensuring decentralization, security, and data availability.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MBONGO NETWORK TOPOLOGY                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                        ┌─────────────┐                              │
│                        │  Validators │                              │
│                        │  (PoS)      │                              │
│                        └──────┬──────┘                              │
│                               │                                     │
│           ┌───────────────────┼───────────────────┐                │
│           │                   │                   │                │
│           ▼                   ▼                   ▼                │
│    ┌────────────┐      ┌────────────┐      ┌────────────┐         │
│    │ Full Node  │◀────▶│ Full Node  │◀────▶│ Full Node  │         │
│    │    (A)     │      │    (B)     │      │    (C)     │         │
│    └─────┬──────┘      └─────┬──────┘      └─────┬──────┘         │
│          │                   │                   │                 │
│          │    ┌──────────────┼──────────────┐    │                 │
│          │    │              │              │    │                 │
│          ▼    ▼              ▼              ▼    ▼                 │
│    ┌──────────────┐   ┌──────────────┐   ┌──────────────┐         │
│    │ Light Nodes  │   │  RPC Nodes   │   │ Compute      │         │
│    │ (Wallets)    │   │  (DApps)     │   │ Providers    │         │
│    └──────────────┘   └──────────────┘   └──────────────┘         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Node Types Comparison

| Node Type | Description | Storage | Sync Time | Use Case |
|-----------|-------------|---------|-----------|----------|
| **Light Node** | Stores only block headers | ~5 GB | Minutes | Mobile wallets, quick queries |
| **Full Node** | Stores full blockchain state | ~500 GB | Hours | General participation, validation |
| **Archive Node** | Stores all historical states | ~2+ TB | Days | Historical queries, block explorers |
| **RPC Node** | Full node with public API | ~500 GB | Hours | DApp backends, API services |

```
┌─────────────────────────────────────────────────────────────────────┐
│                    NODE TYPE COMPARISON                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  LIGHT NODE                                                         │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Headers │ Headers │ Headers │ Headers │ ... │ Latest Header │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  Storage: ~5 GB | Validates: Header chain only                     │
│                                                                     │
│  FULL NODE                                                          │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Block 1 │ Block 2 │ Block 3 │ ... │ Current State           │   │
│  │ + State │ + State │ + State │     │ (Pruned History)        │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  Storage: ~500 GB | Validates: All blocks + current state          │
│                                                                     │
│  ARCHIVE NODE                                                       │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Block 1 │ Block 2 │ Block 3 │ ... │ ALL Historical States   │   │
│  │ + State │ + State │ + State │     │ (Never Pruned)          │   │
│  │ @ Blk 1 │ @ Blk 2 │ @ Blk 3 │     │                         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│  Storage: ~2+ TB | Validates: Everything + historical queries      │
│                                                                     │
│  RPC NODE                                                           │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Full Node + │ JSON-RPC API │ WebSocket API │ Rate Limiting  │   │
│  │             │ Public/Private│ Subscriptions │ Load Balancing│   │
│  └─────────────────────────────────────────────────────────────┘   │
│  Storage: ~500 GB | Purpose: Serve external clients                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Roles in PoS + PoUW Hybrid Network

Mbongo Chain operates a unique hybrid consensus combining **Proof of Stake (PoS)** for block production and **Proof of Useful Work (PoUW)** for computational task validation:

| Role | Node Type Required | Responsibility |
|------|-------------------|----------------|
| **Block Production** | Validator (Full Node + Stake) | Propose and attest blocks |
| **Transaction Relay** | Full Node | Propagate transactions |
| **State Verification** | Full Node | Validate block state transitions |
| **Compute Verification** | Full Node | Verify PoUW task receipts |
| **API Services** | RPC Node | Serve DApps and wallets |
| **Historical Queries** | Archive Node | Provide historical state access |

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PoS + PoUW HYBRID CONSENSUS                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    PROOF OF STAKE (PoS)                        │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │ │
│  │  │ Validator 1 │  │ Validator 2 │  │ Validator N │            │ │
│  │  │ Stake: 32K  │  │ Stake: 32K  │  │ Stake: 32K  │            │ │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘            │ │
│  │         │                │                │                    │ │
│  │         └────────────────┼────────────────┘                    │ │
│  │                          │                                     │ │
│  │                          ▼                                     │ │
│  │               ┌──────────────────┐                            │ │
│  │               │ Block Production │                            │ │
│  │               │ & Finalization   │                            │ │
│  │               └──────────────────┘                            │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                          │                                          │
│                          ▼                                          │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                 PROOF OF USEFUL WORK (PoUW)                    │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │ │
│  │  │ Compute      │  │ Compute      │  │ Compute      │         │ │
│  │  │ Provider 1   │  │ Provider 2   │  │ Provider N   │         │ │
│  │  │ (GPU/TPU)    │  │ (GPU/TPU)    │  │ (GPU/TPU)    │         │ │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘         │ │
│  │         │                 │                 │                  │ │
│  │         └─────────────────┼─────────────────┘                  │ │
│  │                           │                                    │ │
│  │                           ▼                                    │ │
│  │                ┌──────────────────┐                           │ │
│  │                │ Task Execution & │                           │ │
│  │                │ Receipt Proofs   │                           │ │
│  │                └──────────────────┘                           │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
│  Full Nodes: Validate both PoS blocks AND PoUW receipts            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Hardware Expectations Summary

| Node Type | CPU | RAM | Storage | Network |
|-----------|-----|-----|---------|---------|
| **Light Node** | 2 cores | 4 GB | 10 GB SSD | 10 Mbps |
| **Full Node** | 4 cores | 16 GB | 500 GB NVMe | 50 Mbps |
| **Archive Node** | 8 cores | 64 GB | 2+ TB NVMe | 100 Mbps |
| **RPC Node** | 8 cores | 32 GB | 1 TB NVMe | 200+ Mbps |

> ⚠️ **Important**: Full nodes do **NOT** require a GPU. GPUs are only needed for Compute Providers participating in PoUW.

---

## 2. Hardware Requirements

### Minimum Specifications

| Component | Light Node | Full Node | Archive Node | RPC Node |
|-----------|------------|-----------|--------------|----------|
| **CPU** | 2 cores | 4 cores | 8 cores | 8+ cores |
| **RAM** | 4 GB | 16 GB | 64 GB | 32 GB |
| **Storage** | 10 GB SSD | 500 GB NVMe | 2 TB NVMe | 1 TB NVMe |
| **IOPS** | 1,000 | 5,000 | 10,000 | 10,000 |
| **Network** | 10 Mbps | 50 Mbps | 100 Mbps | 200+ Mbps |

### Recommended Production Specifications

| Component | Full Node | Archive Node | High-Traffic RPC |
|-----------|-----------|--------------|------------------|
| **CPU** | 8 cores / 3.5 GHz | 16 cores / 3.5 GHz | 32 cores / 3.5 GHz |
| **RAM** | 32 GB ECC | 128 GB ECC | 64 GB ECC |
| **Storage** | 1 TB NVMe Gen4 | 4 TB NVMe RAID | 2 TB NVMe Gen4 |
| **IOPS** | 50,000+ | 100,000+ | 100,000+ |
| **Network** | 100 Mbps | 200 Mbps | 1 Gbps |

### Storage Growth Estimates

```
┌─────────────────────────────────────────────────────────────────────┐
│                    STORAGE GROWTH PROJECTION                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  FULL NODE (Pruned State)                                          │
│  Year 1: ████████████████████░░░░░░░░░░░░░░░░░░░░  ~400 GB         │
│  Year 2: ████████████████████████░░░░░░░░░░░░░░░░  ~600 GB         │
│  Year 3: ████████████████████████████░░░░░░░░░░░░  ~800 GB         │
│                                                                     │
│  ARCHIVE NODE (Full History)                                       │
│  Year 1: ████████████████████████████████████████  ~2 TB           │
│  Year 2: ████████████████████████████████████████████████  ~4 TB   │
│  Year 3: ████████████████████████████████████████████████████ ~6 TB│
│                                                                     │
│  Growth Rate: ~50 GB/month (full node) | ~150 GB/month (archive)   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Cloud vs Local Server Comparison

| Aspect | Cloud (AWS/GCP/Azure) | Local / Bare Metal |
|--------|----------------------|-------------------|
| **Upfront Cost** | None | High |
| **Monthly Cost** | $150-$500+ | $50-$100 (electricity) |
| **Scalability** | Easy | Hardware limited |
| **Latency** | Region dependent | Network dependent |
| **Control** | Limited | Full |
| **Maintenance** | Provider managed | Self-managed |
| **Reliability** | 99.9%+ SLA | Self-dependent |

#### Recommended Cloud Instances

| Provider | Full Node | Archive Node | RPC Node |
|----------|-----------|--------------|----------|
| **AWS** | m6i.xlarge | r6i.4xlarge | c6i.4xlarge |
| **GCP** | n2-standard-4 | n2-highmem-16 | c2-standard-16 |
| **Azure** | Standard_D4s_v5 | Standard_E16s_v5 | Standard_F16s_v2 |
| **DigitalOcean** | CPU-Optimized 8GB | Memory-Optimized 64GB | CPU-Optimized 32GB |

### GPU Requirements

> ℹ️ **Full nodes do NOT require GPUs.**

GPUs are only required for:
- ✅ Compute Providers (PoUW task execution)
- ❌ Full Nodes (not required)
- ❌ Archive Nodes (not required)
- ❌ RPC Nodes (not required)
- ❌ Validators (not required)

---

## 3. Software Requirements

### Operating System Support

| OS | Version | Support Level |
|----|---------|---------------|
| **Ubuntu** | 22.04 LTS | ✅ Full Support |
| **Ubuntu** | 24.04 LTS | ✅ Full Support |
| **Windows Server** | 2022 | ✅ Full Support |
| **Windows** | 10/11 | ✅ Full Support |
| **Debian** | 11/12 | 🟡 Community |
| **CentOS/RHEL** | 8/9 | 🟡 Community |
| **macOS** | 13+ | 🟡 Development Only |

### Ubuntu 22.04 LTS Requirements

#### System Dependencies

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install essential packages
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    librocksdb-dev \
    liblz4-dev \
    libzstd-dev \
    libsnappy-dev \
    cmake \
    git \
    curl \
    wget \
    jq \
    unzip \
    htop \
    iotop \
    tmux \
    ufw

# Verify installations
gcc --version
openssl version
```

#### Rust Toolchain

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Reload environment
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version

# Install components
rustup component add clippy rustfmt

# Set stable as default
rustup default stable
```

### Windows Server 2022 Requirements

#### System Dependencies

```powershell
# Run PowerShell as Administrator

# Install Chocolatey
Set-ExecutionPolicy Bypass -Scope Process -Force
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

# Install required packages
choco install -y `
    git `
    cmake `
    visualstudio2022buildtools `
    openssl `
    wget `
    jq `
    7zip

# Install Visual C++ Build Tools
choco install -y visualstudio2022-workload-vctools

# Refresh environment
refreshenv
```

#### Rust Toolchain (Windows)

```powershell
# Download and install Rust
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile "$env:TEMP\rustup-init.exe"
Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait

# Reload PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Verify installation
rustc --version
cargo --version

# Install components
rustup component add clippy rustfmt
```

### Mbongo Node Binary

The Mbongo Node binary (`mbongo-node`) is the core software for running a full node.

**Download options:**
1. Pre-built binaries (recommended)
2. Build from source

### Dependencies Summary

| Dependency | Version | Purpose |
|------------|---------|---------|
| **OpenSSL** | 1.1.1+ | TLS/cryptography |
| **RocksDB** | 7.x | Database backend |
| **LZ4** | 1.9+ | Compression |
| **Zstd** | 1.5+ | Compression |
| **Snappy** | 1.1+ | Compression |
| **libclang** | 14+ | Build tooling |

---

## 4. Installation Guide

### Installation Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                    INSTALLATION PIPELINE                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  STEP 1        STEP 2        STEP 3        STEP 4        STEP 5    │
│  ┌──────┐      ┌──────┐      ┌──────┐      ┌──────┐      ┌──────┐  │
│  │ Down │─────▶│Extract│─────▶│Config│─────▶│ Run  │─────▶│ Sync │  │
│  │ load │      │      │      │ ure  │      │      │      │      │  │
│  └──────┘      └──────┘      └──────┘      └──────┘      └──────┘  │
│     │             │             │             │             │       │
│     ▼             ▼             ▼             ▼             ▼       │
│  ┌──────┐      ┌──────┐      ┌──────┐      ┌──────┐      ┌──────┐  │
│  │GitHub│      │ Bin  │      │ TOML │      │Start │      │Catch │  │
│  │Release│     │ Dir  │      │ File │      │ Node │      │  Up  │  │
│  └──────┘      └──────┘      └──────┘      └──────┘      └──────┘  │
│                                                                     │
│  Duration: Download ~5 min | Extract ~1 min | Config ~5 min        │
│            Run ~1 min | Sync ~4-24 hours (depending on mode)       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Ubuntu Installation

```bash
#!/bin/bash
# Mbongo Full Node Installation Script - Ubuntu 22.04

set -e

echo "=============================================="
echo "  Mbongo Chain Full Node Installation"
echo "=============================================="

# Step 1: Install system dependencies
echo ""
echo "=== Step 1: Installing System Dependencies ==="
sudo apt update && sudo apt upgrade -y
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    librocksdb-dev \
    liblz4-dev \
    libzstd-dev \
    cmake \
    git \
    curl \
    wget \
    jq \
    ufw

# Step 2: Install Rust (if not installed)
echo ""
echo "=== Step 2: Installing Rust Toolchain ==="
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi
rustc --version

# Step 3: Create directories
echo ""
echo "=== Step 3: Creating Directory Structure ==="
mkdir -p ~/.mbongo/{bin,config,data,logs}
chmod 700 ~/.mbongo

# Step 4: Download Mbongo Node binary
echo ""
echo "=== Step 4: Downloading Mbongo Node ==="
MBONGO_VERSION="v1.0.0"
DOWNLOAD_URL="https://github.com/mbongo-chain/mbongo-chain/releases/download/${MBONGO_VERSION}/mbongo-node-linux-amd64.tar.gz"

wget -O /tmp/mbongo-node.tar.gz "$DOWNLOAD_URL"

# Verify checksum
wget -O /tmp/checksums.txt \
    "https://github.com/mbongo-chain/mbongo-chain/releases/download/${MBONGO_VERSION}/checksums.txt"
cd /tmp && sha256sum -c checksums.txt --ignore-missing
cd -

# Step 5: Extract binary
echo ""
echo "=== Step 5: Extracting Binary ==="
tar -xzf /tmp/mbongo-node.tar.gz -C ~/.mbongo/bin/
chmod +x ~/.mbongo/bin/mbongo-node

# Step 6: Add to PATH
echo ""
echo "=== Step 6: Configuring PATH ==="
if ! grep -q '.mbongo/bin' ~/.bashrc; then
    echo 'export PATH="$HOME/.mbongo/bin:$PATH"' >> ~/.bashrc
fi
export PATH="$HOME/.mbongo/bin:$PATH"

# Step 7: Verify installation
echo ""
echo "=== Step 7: Verifying Installation ==="
mbongo-node --version

# Step 8: Generate default configuration
echo ""
echo "=== Step 8: Generating Configuration ==="
mbongo-node init --config ~/.mbongo/config/full_node.toml --network mainnet

echo ""
echo "=============================================="
echo "  Installation Complete!"
echo "=============================================="
echo ""
echo "Next steps:"
echo "  1. Edit configuration: ~/.mbongo/config/full_node.toml"
echo "  2. Start node: mbongo-node --config ~/.mbongo/config/full_node.toml"
echo "  3. Or enable systemd service (see documentation)"
echo ""
```

### Windows Installation

```powershell
# Mbongo Full Node Installation Script - Windows
# Run PowerShell as Administrator

Write-Host "==============================================" -ForegroundColor Cyan
Write-Host "  Mbongo Chain Full Node Installation" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan

# Step 1: Install Chocolatey (if not installed)
Write-Host ""
Write-Host "=== Step 1: Installing Package Manager ===" -ForegroundColor Yellow

if (!(Get-Command choco -ErrorAction SilentlyContinue)) {
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
}

# Step 2: Install dependencies
Write-Host ""
Write-Host "=== Step 2: Installing Dependencies ===" -ForegroundColor Yellow

choco install -y git cmake openssl wget jq 7zip visualstudio2022buildtools

# Refresh environment
refreshenv

# Step 3: Install Rust
Write-Host ""
Write-Host "=== Step 3: Installing Rust Toolchain ===" -ForegroundColor Yellow

if (!(Get-Command rustc -ErrorAction SilentlyContinue)) {
    Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile "$env:TEMP\rustup-init.exe"
    Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait
}

# Reload PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

rustc --version

# Step 4: Create directories
Write-Host ""
Write-Host "=== Step 4: Creating Directory Structure ===" -ForegroundColor Yellow

$mbongoPath = "$env:USERPROFILE\.mbongo"
New-Item -ItemType Directory -Force -Path "$mbongoPath\bin" | Out-Null
New-Item -ItemType Directory -Force -Path "$mbongoPath\config" | Out-Null
New-Item -ItemType Directory -Force -Path "$mbongoPath\data" | Out-Null
New-Item -ItemType Directory -Force -Path "$mbongoPath\logs" | Out-Null

# Step 5: Download Mbongo Node
Write-Host ""
Write-Host "=== Step 5: Downloading Mbongo Node ===" -ForegroundColor Yellow

$MBONGO_VERSION = "v1.0.0"
$downloadUrl = "https://github.com/mbongo-chain/mbongo-chain/releases/download/$MBONGO_VERSION/mbongo-node-windows-amd64.zip"
$zipPath = "$env:TEMP\mbongo-node.zip"

Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath

# Step 6: Extract binary
Write-Host ""
Write-Host "=== Step 6: Extracting Binary ===" -ForegroundColor Yellow

Expand-Archive -Path $zipPath -DestinationPath "$mbongoPath\bin" -Force

# Step 7: Add to PATH
Write-Host ""
Write-Host "=== Step 7: Configuring PATH ===" -ForegroundColor Yellow

$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($currentPath -notlike "*$mbongoPath\bin*") {
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$mbongoPath\bin", "User")
}

# Reload PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Step 8: Verify installation
Write-Host ""
Write-Host "=== Step 8: Verifying Installation ===" -ForegroundColor Yellow

mbongo-node --version

# Step 9: Generate configuration
Write-Host ""
Write-Host "=== Step 9: Generating Configuration ===" -ForegroundColor Yellow

mbongo-node init --config "$mbongoPath\config\full_node.toml" --network mainnet

Write-Host ""
Write-Host "==============================================" -ForegroundColor Green
Write-Host "  Installation Complete!" -ForegroundColor Green
Write-Host "==============================================" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:" -ForegroundColor White
Write-Host "  1. Edit configuration: $mbongoPath\config\full_node.toml" -ForegroundColor White
Write-Host "  2. Start node: mbongo-node --config $mbongoPath\config\full_node.toml" -ForegroundColor White
Write-Host "  3. Or enable Windows service (see documentation)" -ForegroundColor White
```

### Build from Source (Optional)

```bash
# Clone repository
git clone https://github.com/mbongo-chain/mbongo-chain.git
cd mbongo-chain

# Build release binary
cargo build --release --package mbongo-node

# Binary location
ls -la target/release/mbongo-node

# Install to user path
cp target/release/mbongo-node ~/.mbongo/bin/
```

---

## 5. Node Configuration

### Configuration File Structure

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CONFIGURATION FILE LAYOUT                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ~/.mbongo/config/full_node.toml                                   │
│  │                                                                  │
│  ├── [general]           # Basic node settings                     │
│  │   ├── network         # mainnet / testnet                       │
│  │   ├── data_dir        # Blockchain data path                    │
│  │   └── log_level       # Logging verbosity                       │
│  │                                                                  │
│  ├── [sync]              # Synchronization settings                │
│  │   ├── mode            # fast / full / archive                   │
│  │   └── parallel_downloads                                        │
│  │                                                                  │
│  ├── [network]           # P2P network settings                    │
│  │   ├── listen_addr     # P2P listen address                      │
│  │   ├── port            # P2P port                                │
│  │   ├── max_peers       # Maximum peer connections                │
│  │   └── bootnodes       # Initial peer discovery                  │
│  │                                                                  │
│  ├── [rpc]               # JSON-RPC API settings                   │
│  │   ├── enabled         # Enable/disable RPC                      │
│  │   ├── listen_addr     # RPC listen address                      │
│  │   ├── port            # RPC port                                │
│  │   └── cors_origins    # CORS configuration                      │
│  │                                                                  │
│  ├── [metrics]           # Prometheus metrics                      │
│  │                                                                  │
│  └── [logging]           # Log configuration                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Full Sample Configuration (full_node.toml)

```toml
# Mbongo Chain Full Node Configuration
# Version: 1.0.0
# Documentation: https://docs.mbongo.network/node-config

# =============================================================================
# GENERAL SETTINGS
# =============================================================================
[general]
# Network to connect to: "mainnet" or "testnet"
network = "mainnet"

# Chain ID (1 = mainnet, 1001 = testnet)
chain_id = 1

# Data directory for blockchain storage
data_dir = "~/.mbongo/data"

# Node identity (optional, auto-generated if not set)
# identity = "my-node-001"

# =============================================================================
# SYNCHRONIZATION
# =============================================================================
[sync]
# Sync mode:
#   "fast"    - Download headers first, then state (fastest initial sync)
#   "full"    - Download and verify all blocks sequentially
#   "archive" - Full sync + keep all historical states (largest storage)
mode = "fast"

# Number of parallel block downloads
parallel_downloads = 8

# Target peers for sync
target_sync_peers = 5

# Enable checkpoint sync (faster initial sync using trusted checkpoints)
checkpoint_sync = true

# Checkpoint server URL (optional, uses default if not set)
# checkpoint_url = "https://checkpoints.mbongo.network"

# =============================================================================
# PEER-TO-PEER NETWORK
# =============================================================================
[network]
# P2P listen address (0.0.0.0 = all interfaces)
listen_addr = "0.0.0.0"

# P2P port
port = 30303

# Maximum number of peer connections
max_peers = 50

# Maximum inbound connections
max_inbound = 30

# Maximum outbound connections
max_outbound = 20

# Enable NAT traversal (UPnP)
nat_enabled = true

# External IP (optional, auto-detected if not set)
# external_ip = "203.0.113.50"

# Bootnodes for initial peer discovery
bootnodes = [
    "/dns4/bootnode1.mbongo.network/tcp/30303/p2p/12D3KooWBOOTNODE1...",
    "/dns4/bootnode2.mbongo.network/tcp/30303/p2p/12D3KooWBOOTNODE2...",
    "/dns4/bootnode3.mbongo.network/tcp/30303/p2p/12D3KooWBOOTNODE3...",
]

# Static peers (always connect to these)
# static_peers = []

# Banned peers
# banned_peers = []

# =============================================================================
# JSON-RPC API
# =============================================================================
[rpc]
# Enable JSON-RPC server
enabled = true

# RPC listen address
# Use "127.0.0.1" for local-only access (recommended for security)
# Use "0.0.0.0" for public access (requires additional security measures)
listen_addr = "127.0.0.1"

# HTTP JSON-RPC port
port = 8545

# Enable WebSocket RPC
ws_enabled = true

# WebSocket port
ws_port = 8546

# Maximum concurrent connections
max_connections = 100

# Request timeout (seconds)
timeout = 30

# Enable batch requests
batch_enabled = true

# Maximum batch size
max_batch_size = 100

# CORS allowed origins
# Empty = disabled, ["*"] = allow all (not recommended for production)
cors_origins = []

# Enabled RPC namespaces
namespaces = ["eth", "net", "web3", "mbongo", "txpool", "debug"]

# Rate limiting (requests per second per IP)
rate_limit = 100

# =============================================================================
# METRICS & MONITORING
# =============================================================================
[metrics]
# Enable Prometheus metrics endpoint
enabled = true

# Metrics listen address
listen_addr = "127.0.0.1"

# Metrics port
port = 9090

# Enable detailed metrics (higher overhead)
detailed = false

# =============================================================================
# LOGGING
# =============================================================================
[logging]
# Log level: "trace", "debug", "info", "warn", "error"
level = "info"

# Log output format: "text" or "json"
format = "text"

# Log file path (empty = stdout only)
file = "~/.mbongo/logs/node.log"

# Enable log rotation
rotation = true

# Maximum log file size (MB)
max_size_mb = 100

# Number of rotated files to keep
max_files = 10

# Log to stdout
stdout = true

# Module-specific log levels (optional)
# [logging.modules]
# network = "debug"
# sync = "debug"
# rpc = "info"

# =============================================================================
# DATABASE
# =============================================================================
[database]
# Database backend: "rocksdb" (recommended) or "leveldb"
backend = "rocksdb"

# Cache size (MB)
cache_size_mb = 512

# Enable compression
compression = true

# Compression algorithm: "lz4", "zstd", "snappy"
compression_algo = "lz4"

# =============================================================================
# TRANSACTION POOL
# =============================================================================
[txpool]
# Maximum transactions in pool
max_size = 10000

# Maximum pending transactions per account
max_per_account = 64

# Price bump percentage for replacement
price_bump = 10

# Transaction lifetime (seconds)
lifetime = 10800

# =============================================================================
# STATE PRUNING (Full Node Only)
# =============================================================================
[pruning]
# Enable state pruning (reduces storage, disable for archive nodes)
enabled = true

# Keep last N blocks of state
keep_blocks = 128

# Pruning interval (blocks)
interval = 256

# =============================================================================
# PERFORMANCE TUNING
# =============================================================================
[performance]
# Number of execution threads
execution_threads = 4

# Block processing cache size (MB)
block_cache_mb = 256

# State trie cache size (MB)
state_cache_mb = 256

# Enable prefetching
prefetch_enabled = true
```

### Sync Modes Explained

| Mode | Storage | Sync Time | Use Case |
|------|---------|-----------|----------|
| **fast** | ~400 GB | 4-8 hours | Standard full node |
| **full** | ~500 GB | 12-24 hours | High-security validation |
| **archive** | ~2+ TB | 24-72 hours | Block explorers, historical queries |

```bash
# Configure sync mode in full_node.toml
[sync]
mode = "fast"  # Options: fast, full, archive
```

### RPC Configuration Options

```toml
# Local-only RPC (secure, recommended default)
[rpc]
enabled = true
listen_addr = "127.0.0.1"
port = 8545
cors_origins = []

# Public RPC (requires reverse proxy + rate limiting)
[rpc]
enabled = true
listen_addr = "0.0.0.0"
port = 8545
cors_origins = ["https://yourdapp.com"]
rate_limit = 50
```

---

## 6. Running the Node

### Basic Command

```bash
# Start node with configuration file
mbongo-node --config ~/.mbongo/config/full_node.toml

# Start with verbose logging
mbongo-node --config ~/.mbongo/config/full_node.toml --log-level debug

# Start with specific data directory
mbongo-node --config ~/.mbongo/config/full_node.toml --data-dir /mnt/blockchain
```

### Ubuntu systemd Service

```bash
# Create systemd service file
sudo tee /etc/systemd/system/mbongo-node.service << 'EOF'
[Unit]
Description=Mbongo Chain Full Node
Documentation=https://docs.mbongo.network
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=YOUR_USERNAME
Group=YOUR_USERNAME

# Main process
ExecStart=/home/YOUR_USERNAME/.mbongo/bin/mbongo-node \
    --config /home/YOUR_USERNAME/.mbongo/config/full_node.toml

# Restart policy
Restart=always
RestartSec=10
TimeoutStopSec=300

# Resource limits
LimitNOFILE=65535
LimitNPROC=65535
MemoryMax=28G

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=/home/YOUR_USERNAME/.mbongo

# Environment
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=mbongo-node

[Install]
WantedBy=multi-user.target
EOF

# Replace YOUR_USERNAME with actual username
sudo sed -i "s/YOUR_USERNAME/$USER/g" /etc/systemd/system/mbongo-node.service

# Reload systemd configuration
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable mbongo-node

# Start the service
sudo systemctl start mbongo-node

# Check status
sudo systemctl status mbongo-node
```

### Windows NSSM Service

```powershell
# Install NSSM (Non-Sucking Service Manager)
choco install -y nssm

# Create the Windows service
$serviceName = "MbongoNode"
$binaryPath = "$env:USERPROFILE\.mbongo\bin\mbongo-node.exe"
$configPath = "$env:USERPROFILE\.mbongo\config\full_node.toml"
$logPath = "$env:USERPROFILE\.mbongo\logs"

# Install service
nssm install $serviceName $binaryPath
nssm set $serviceName AppParameters "--config `"$configPath`""
nssm set $serviceName AppDirectory "$env:USERPROFILE\.mbongo"
nssm set $serviceName DisplayName "Mbongo Chain Full Node"
nssm set $serviceName Description "Mbongo Chain Full Node Service"

# Configure logging
nssm set $serviceName AppStdout "$logPath\node-stdout.log"
nssm set $serviceName AppStderr "$logPath\node-stderr.log"
nssm set $serviceName AppRotateFiles 1
nssm set $serviceName AppRotateBytes 104857600

# Configure restart
nssm set $serviceName AppExit Default Restart
nssm set $serviceName AppRestartDelay 10000

# Set startup type
nssm set $serviceName Start SERVICE_AUTO_START

# Start the service
nssm start $serviceName

# Check status
Get-Service $serviceName
nssm status $serviceName
```

### Log Inspection

```bash
# Ubuntu - View systemd logs
sudo journalctl -u mbongo-node -f --no-hostname

# View last 100 lines
sudo journalctl -u mbongo-node -n 100

# Search for errors
sudo journalctl -u mbongo-node --since "1 hour ago" | grep -i error

# View log file directly
tail -f ~/.mbongo/logs/node.log

# Search for specific events
grep "block_imported" ~/.mbongo/logs/node.log | tail -20
grep "peer_connected" ~/.mbongo/logs/node.log | tail -10
```

```powershell
# Windows - View logs
Get-Content "$env:USERPROFILE\.mbongo\logs\node-stdout.log" -Tail 100 -Wait

# Search for errors
Select-String -Path "$env:USERPROFILE\.mbongo\logs\*.log" -Pattern "error" -CaseSensitive:$false
```

### Service Management Commands

```bash
# Ubuntu systemd
sudo systemctl start mbongo-node     # Start service
sudo systemctl stop mbongo-node      # Stop service
sudo systemctl restart mbongo-node   # Restart service
sudo systemctl status mbongo-node    # Check status
sudo systemctl enable mbongo-node    # Enable auto-start
sudo systemctl disable mbongo-node   # Disable auto-start
```

```powershell
# Windows NSSM
nssm start MbongoNode                # Start service
nssm stop MbongoNode                 # Stop service
nssm restart MbongoNode              # Restart service
nssm status MbongoNode               # Check status
Get-Service MbongoNode               # PowerShell status
```

### Restart Sequences

```bash
# Graceful restart (Ubuntu)
sudo systemctl reload-or-restart mbongo-node

# Force restart after update
sudo systemctl stop mbongo-node
sleep 5
sudo systemctl start mbongo-node

# Restart with database integrity check
sudo systemctl stop mbongo-node
mbongo-node db check --data-dir ~/.mbongo/data
sudo systemctl start mbongo-node
```

---

## 7. RPC Node Setup

### Public vs Private RPC

```
┌─────────────────────────────────────────────────────────────────────┐
│                    RPC ACCESS MODELS                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  PRIVATE RPC (Default - Recommended)                               │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                     localhost only                           │   │
│  │  ┌──────────┐        ┌──────────┐        ┌──────────┐       │   │
│  │  │ Your App │───────▶│ RPC Node │        │ Firewall │       │   │
│  │  │ (local)  │        │ 127.0.0.1│        │ BLOCKS   │       │   │
│  │  └──────────┘        └──────────┘        │ External │       │   │
│  │                                          └──────────┘       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  PUBLIC RPC (Requires Security Measures)                           │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐ │   │
│  │  │ Internet │──▶│ Firewall │──▶│ Reverse  │──▶│ RPC Node │ │   │
│  │  │          │   │          │   │ Proxy    │   │          │ │   │
│  │  └──────────┘   └──────────┘   │ (nginx)  │   └──────────┘ │   │
│  │                                │ + SSL    │                 │   │
│  │                                │ + Rate   │                 │   │
│  │                                │   Limit  │                 │   │
│  │                                └──────────┘                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Rate Limiting Configuration

```toml
# In full_node.toml
[rpc]
enabled = true
listen_addr = "127.0.0.1"
port = 8545

# Rate limiting
rate_limit = 100           # requests per second per IP
rate_limit_burst = 200     # burst allowance
rate_limit_window = 60     # window in seconds
```

### Whitelist/Blacklist

```toml
# In full_node.toml
[rpc.access]
# IP whitelist (if set, only these IPs can connect)
whitelist = [
    "192.168.1.0/24",
    "10.0.0.0/8",
]

# IP blacklist (blocked IPs)
blacklist = [
    "1.2.3.4",
    "5.6.7.0/24",
]

# API key authentication (optional)
api_key_enabled = true
api_keys = [
    "your-secret-api-key-1",
    "your-secret-api-key-2",
]
```

### DDoS Protection

```toml
# In full_node.toml
[rpc.protection]
# Maximum request body size (bytes)
max_request_size = 1048576  # 1 MB

# Maximum response size (bytes)
max_response_size = 10485760  # 10 MB

# Connection timeout (seconds)
connection_timeout = 30

# Slow request threshold (ms)
slow_request_threshold = 5000

# Maximum concurrent requests
max_concurrent_requests = 1000

# Enable request logging for suspicious activity
log_suspicious = true
```

### Nginx Reverse Proxy Configuration

```nginx
# /etc/nginx/sites-available/mbongo-rpc
upstream mbongo_rpc {
    server 127.0.0.1:8545;
    keepalive 32;
}

# Rate limiting zone
limit_req_zone $binary_remote_addr zone=rpc_limit:10m rate=10r/s;
limit_conn_zone $binary_remote_addr zone=conn_limit:10m;

server {
    listen 443 ssl http2;
    server_name rpc.yourdomain.com;

    # SSL certificates (use Let's Encrypt)
    ssl_certificate /etc/letsencrypt/live/rpc.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/rpc.yourdomain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256;
    ssl_prefer_server_ciphers off;

    # Security headers
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Rate limiting
    limit_req zone=rpc_limit burst=20 nodelay;
    limit_conn conn_limit 10;

    # Request size limits
    client_max_body_size 1m;
    client_body_buffer_size 16k;

    location / {
        proxy_pass http://mbongo_rpc;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # Timeouts
        proxy_connect_timeout 30s;
        proxy_send_timeout 30s;
        proxy_read_timeout 60s;

        # WebSocket support
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }

    # Health check endpoint
    location /health {
        proxy_pass http://mbongo_rpc/health;
        access_log off;
    }

    # Block certain methods (optional)
    location /block-debug {
        if ($request_body ~* "debug_") {
            return 403;
        }
        proxy_pass http://mbongo_rpc;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name rpc.yourdomain.com;
    return 301 https://$host$request_uri;
}
```

### Caddy Reverse Proxy Configuration

```caddyfile
# /etc/caddy/Caddyfile
rpc.yourdomain.com {
    # Automatic HTTPS with Let's Encrypt
    
    # Rate limiting
    rate_limit {
        zone dynamic_zone {
            key {remote_host}
            events 100
            window 1m
        }
    }
    
    # Headers
    header {
        X-Frame-Options "SAMEORIGIN"
        X-Content-Type-Options "nosniff"
        X-XSS-Protection "1; mode=block"
        -Server
    }
    
    # Reverse proxy to node
    reverse_proxy localhost:8545 {
        header_up Host {host}
        header_up X-Real-IP {remote}
        header_up X-Forwarded-For {remote}
        
        # Health checks
        health_uri /health
        health_interval 30s
    }
    
    # WebSocket support
    @websocket {
        header Connection *Upgrade*
        header Upgrade websocket
    }
    reverse_proxy @websocket localhost:8546
    
    # Access logging
    log {
        output file /var/log/caddy/mbongo-rpc.log
        format json
    }
}
```

```bash
# Enable Nginx configuration
sudo ln -s /etc/nginx/sites-available/mbongo-rpc /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx

# Or restart Caddy
sudo systemctl reload caddy
```

---

## 8. Firewall & Security

### Required Ports

| Port | Protocol | Direction | Purpose | Exposure |
|------|----------|-----------|---------|----------|
| **30303** | TCP/UDP | Inbound/Outbound | P2P network | Public |
| **8545** | TCP | Inbound | JSON-RPC HTTP | Private* |
| **8546** | TCP | Inbound | WebSocket RPC | Private* |
| **9090** | TCP | Inbound | Prometheus metrics | Private |
| **22** | TCP | Inbound | SSH | Restricted |

*\* Public only if running a public RPC node with proper security measures*

### UFW Rules (Ubuntu)

```bash
# Reset UFW to defaults
sudo ufw --force reset

# Set default policies
sudo ufw default deny incoming
sudo ufw default allow outgoing

# Allow SSH (IMPORTANT - do this first!)
sudo ufw allow ssh
# Or limit SSH attempts
sudo ufw limit ssh

# Allow P2P ports (required for node operation)
sudo ufw allow 30303/tcp comment 'Mbongo P2P TCP'
sudo ufw allow 30303/udp comment 'Mbongo P2P UDP'

# DENY external RPC access by default
sudo ufw deny 8545/tcp comment 'Block external RPC'
sudo ufw deny 8546/tcp comment 'Block external WebSocket'

# Allow RPC only from specific IPs (if needed)
# sudo ufw allow from 192.168.1.0/24 to any port 8545 comment 'Local RPC access'

# Allow metrics from monitoring server (optional)
# sudo ufw allow from MONITORING_IP to any port 9090 comment 'Prometheus metrics'

# Enable UFW
sudo ufw enable

# Check status
sudo ufw status verbose
```

### Windows Firewall Rules

```powershell
# Run as Administrator

# P2P Ports - Allow
New-NetFirewallRule -DisplayName "Mbongo P2P TCP Inbound" `
    -Direction Inbound -Protocol TCP -LocalPort 30303 -Action Allow
New-NetFirewallRule -DisplayName "Mbongo P2P UDP Inbound" `
    -Direction Inbound -Protocol UDP -LocalPort 30303 -Action Allow
New-NetFirewallRule -DisplayName "Mbongo P2P TCP Outbound" `
    -Direction Outbound -Protocol TCP -LocalPort 30303 -Action Allow
New-NetFirewallRule -DisplayName "Mbongo P2P UDP Outbound" `
    -Direction Outbound -Protocol UDP -LocalPort 30303 -Action Allow

# RPC Ports - Block external access
New-NetFirewallRule -DisplayName "Block External RPC" `
    -Direction Inbound -Protocol TCP -LocalPort 8545 -Action Block -RemoteAddress Any
New-NetFirewallRule -DisplayName "Block External WebSocket" `
    -Direction Inbound -Protocol TCP -LocalPort 8546 -Action Block -RemoteAddress Any

# Allow local RPC only
New-NetFirewallRule -DisplayName "Allow Local RPC" `
    -Direction Inbound -Protocol TCP -LocalPort 8545 -Action Allow -RemoteAddress 127.0.0.1
New-NetFirewallRule -DisplayName "Allow Local WebSocket" `
    -Direction Inbound -Protocol TCP -LocalPort 8546 -Action Allow -RemoteAddress 127.0.0.1

# View rules
Get-NetFirewallRule -DisplayName "Mbongo*" | Format-Table Name, DisplayName, Action, Direction
```

### SSH Hardening

```bash
# Edit SSH configuration
sudo nano /etc/ssh/sshd_config

# Apply these settings:
# =====================
# Disable root login
PermitRootLogin no

# Disable password authentication (use keys only)
PasswordAuthentication no
PubkeyAuthentication yes

# Limit authentication attempts
MaxAuthTries 3

# Disable empty passwords
PermitEmptyPasswords no

# Use SSH Protocol 2 only
Protocol 2

# Limit users (optional)
AllowUsers your_username

# Change default port (optional)
# Port 2222

# Idle timeout
ClientAliveInterval 300
ClientAliveCountMax 2
# =====================

# Restart SSH service
sudo systemctl restart sshd

# Install fail2ban for brute force protection
sudo apt install -y fail2ban
sudo systemctl enable fail2ban
sudo systemctl start fail2ban
```

### RPC Security Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                    RPC SECURITY LAYERS                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  LAYER 1: Network Level                                            │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  • Firewall rules (UFW/iptables)                             │   │
│  │  • VPN/Private network for internal access                   │   │
│  │  • DDoS protection at network edge                           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  LAYER 2: Transport Level                                          │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  • TLS/SSL encryption (HTTPS)                                │   │
│  │  • Certificate validation                                    │   │
│  │  • Reverse proxy (Nginx/Caddy)                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  LAYER 3: Application Level                                        │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  • Rate limiting (per IP/API key)                            │   │
│  │  • IP whitelist/blacklist                                    │   │
│  │  • API key authentication                                    │   │
│  │  • Method-level access control                               │   │
│  │  • Request size limits                                       │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  LAYER 4: Monitoring                                               │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  • Request logging and analysis                              │   │
│  │  • Anomaly detection                                         │   │
│  │  • Alerting on suspicious patterns                           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Security Checklist

- [ ] Firewall enabled with restrictive rules
- [ ] SSH hardened (key-only auth, fail2ban)
- [ ] RPC ports not exposed publicly (or behind reverse proxy)
- [ ] TLS/SSL enabled for any public endpoints
- [ ] Rate limiting configured
- [ ] Regular security updates applied
- [ ] Monitoring and alerting configured
- [ ] Backup procedures tested
- [ ] Disaster recovery plan documented

---

## 9. Backup & Restore

### Database Snapshot

```bash
# Stop the node before backup
sudo systemctl stop mbongo-node

# Create snapshot directory
BACKUP_DIR="$HOME/.mbongo/backups/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

# Method 1: Full directory copy
cp -r ~/.mbongo/data "$BACKUP_DIR/data"

# Method 2: Using rsync (faster for updates)
rsync -avz --progress ~/.mbongo/data/ "$BACKUP_DIR/data/"

# Method 3: Compressed archive
tar -czvf "$BACKUP_DIR/blockchain_data.tar.gz" -C ~/.mbongo data

# Verify backup
ls -lh "$BACKUP_DIR"

# Restart node
sudo systemctl start mbongo-node
```

### Automated Backup Script

```bash
#!/bin/bash
# ~/.mbongo/scripts/backup.sh

set -e

BACKUP_BASE="$HOME/.mbongo/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="$BACKUP_BASE/$TIMESTAMP"
KEEP_DAYS=7

echo "Starting backup: $TIMESTAMP"

# Create backup directory
mkdir -p "$BACKUP_DIR"

# Backup configuration
echo "Backing up configuration..."
cp -r ~/.mbongo/config "$BACKUP_DIR/config"

# Backup keystore (if validator)
if [ -d ~/.mbongo/validator_keys ]; then
    echo "Backing up validator keys..."
    cp -r ~/.mbongo/validator_keys "$BACKUP_DIR/validator_keys"
fi

# Option A: Hot backup (node running - may be inconsistent)
# rsync -avz ~/.mbongo/data/ "$BACKUP_DIR/data/"

# Option B: Cold backup (node stopped - consistent)
echo "Stopping node for consistent backup..."
sudo systemctl stop mbongo-node

echo "Backing up blockchain data..."
rsync -avz --progress ~/.mbongo/data/ "$BACKUP_DIR/data/"

echo "Restarting node..."
sudo systemctl start mbongo-node

# Compress backup
echo "Compressing backup..."
cd "$BACKUP_BASE"
tar -czvf "${TIMESTAMP}.tar.gz" "$TIMESTAMP"
rm -rf "$TIMESTAMP"

# Cleanup old backups
echo "Cleaning old backups..."
find "$BACKUP_BASE" -name "*.tar.gz" -mtime +$KEEP_DAYS -delete

echo "Backup completed: ${BACKUP_BASE}/${TIMESTAMP}.tar.gz"
```

```bash
# Make executable and schedule
chmod +x ~/.mbongo/scripts/backup.sh

# Add to crontab (weekly backup at 3 AM on Sundays)
(crontab -l 2>/dev/null; echo "0 3 * * 0 $HOME/.mbongo/scripts/backup.sh >> $HOME/.mbongo/logs/backup.log 2>&1") | crontab -
```

### Keystore Backup (Validator Nodes)

> ⚠️ **CRITICAL**: Validator keys require special handling to prevent slashing!

```bash
# Export slashing protection database BEFORE backing up keys
mbongo-cli validator slashing-protection export \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db \
    --output ~/.mbongo/backups/slashing_protection_$(date +%Y%m%d).json

# Encrypt and backup keystore
BACKUP_FILE="validator_keys_$(date +%Y%m%d).tar.gz.gpg"
tar -czf - -C ~/.mbongo validator_keys | gpg --symmetric --cipher-algo AES256 -o "$HOME/.mbongo/backups/$BACKUP_FILE"

echo "Encrypted backup created: $BACKUP_FILE"
echo "Store the GPG password securely!"
```

### Restore Procedure

```bash
# Stop node
sudo systemctl stop mbongo-node

# Clear existing data (CAUTION!)
rm -rf ~/.mbongo/data/*

# Restore from backup
# Method 1: From directory
cp -r /path/to/backup/data/* ~/.mbongo/data/

# Method 2: From tarball
tar -xzvf /path/to/backup/blockchain_data.tar.gz -C ~/.mbongo/

# Method 3: From encrypted backup
gpg --decrypt /path/to/backup/validator_keys_YYYYMMDD.tar.gz.gpg | tar -xzf - -C ~/.mbongo/

# Verify data integrity
mbongo-node db check --data-dir ~/.mbongo/data

# Start node
sudo systemctl start mbongo-node

# Monitor sync status
mbongo-node status --rpc http://127.0.0.1:8545
```

### Archive Node Best Practices

```bash
# Archive nodes have special backup considerations due to size

# Use incremental backups
rsync -avz --progress --link-dest=$PREVIOUS_BACKUP \
    ~/.mbongo/data/ "$BACKUP_DIR/data/"

# Consider using storage snapshots (cloud providers)
# AWS: EBS Snapshots
# GCP: Persistent Disk Snapshots
# Azure: Managed Disk Snapshots

# LVM snapshots (local)
sudo lvcreate -L 100G -s -n mbongo_backup /dev/vg0/mbongo_data

# ZFS snapshots (if using ZFS)
sudo zfs snapshot tank/mbongo@backup-$(date +%Y%m%d)
```

---

## 10. Troubleshooting

### Problem 1: Node Stuck at Block X

**Symptoms:**
- Sync progress stops at specific block
- No new blocks imported
- Peers connected but no progress

**Solutions:**

```bash
# Check current sync status
curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' | jq .

# Check peer count
curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' | jq .

# Check logs for errors
sudo journalctl -u mbongo-node --since "1 hour ago" | grep -i "error\|stuck\|timeout"

# Solution 1: Restart with peer refresh
sudo systemctl stop mbongo-node
mbongo-node peers clear --data-dir ~/.mbongo/data  # Clear peer database
sudo systemctl start mbongo-node

# Solution 2: Force resync from checkpoint
mbongo-node resync --from-checkpoint --data-dir ~/.mbongo/data

# Solution 3: Download fresh state snapshot
mbongo-node snapshot download --network mainnet --data-dir ~/.mbongo/data
```

---

### Problem 2: P2P Not Discovering Peers

**Symptoms:**
- Peer count stays at 0
- "No peers connected" warnings
- Network isolation

**Solutions:**

```bash
# Verify P2P port is open
nc -vz YOUR_PUBLIC_IP 30303

# Check firewall
sudo ufw status | grep 30303

# Verify bootnodes in config
grep -A10 "bootnodes" ~/.mbongo/config/full_node.toml

# Test bootnode connectivity
for bootnode in $(grep "bootnode" ~/.mbongo/config/full_node.toml | cut -d'"' -f2); do
    echo "Testing: $bootnode"
    nc -vz $(echo $bootnode | cut -d'/' -f3) 30303
done

# Check NAT/router port forwarding
# Ensure 30303/tcp and 30303/udp are forwarded to your machine

# Add more bootnodes
mbongo-node peers add-bootnode "/dns4/bootnode4.mbongo.network/tcp/30303/p2p/..."

# Enable UPnP (if behind NAT)
# In config: nat_enabled = true
```

---

### Problem 3: Corrupted Database

**Symptoms:**
- Node crashes on startup
- "Database corruption" errors
- "Invalid state root" errors

**Solutions:**

```bash
# Stop node
sudo systemctl stop mbongo-node

# Run database integrity check
mbongo-node db check --data-dir ~/.mbongo/data

# Attempt database repair
mbongo-node db repair --data-dir ~/.mbongo/data

# If repair fails, try rebuilding index
mbongo-node db rebuild-index --data-dir ~/.mbongo/data

# If still failing, restore from backup or resync
mv ~/.mbongo/data ~/.mbongo/data_corrupted_$(date +%Y%m%d)
mkdir -p ~/.mbongo/data

# Option A: Restore from backup
tar -xzvf ~/.mbongo/backups/latest.tar.gz -C ~/.mbongo/

# Option B: Fresh sync
sudo systemctl start mbongo-node
```

---

### Problem 4: RPC Timeout

**Symptoms:**
- RPC requests timeout
- "Request timed out" errors
- Slow API responses

**Solutions:**

```bash
# Check if RPC is responsive
curl -s -m 5 http://127.0.0.1:8545/health

# Check node resource usage
htop
iotop

# Check RPC configuration
grep -A20 "\[rpc\]" ~/.mbongo/config/full_node.toml

# Increase timeout in config
# timeout = 60

# Reduce concurrent requests if overloaded
# max_connections = 50

# Check for slow queries in logs
grep "slow_request" ~/.mbongo/logs/node.log

# Optimize database cache
# [database]
# cache_size_mb = 1024

# Restart node
sudo systemctl restart mbongo-node
```

---

### Problem 5: High Memory Usage

**Symptoms:**
- OOM killer terminates node
- System becomes unresponsive
- Memory usage exceeds available RAM

**Solutions:**

```bash
# Check memory usage
free -h
ps aux --sort=-%mem | head -10

# Check node memory usage
pmap -x $(pgrep mbongo-node) | tail -1

# Reduce cache sizes in config
# [database]
# cache_size_mb = 256
# 
# [performance]
# block_cache_mb = 128
# state_cache_mb = 128

# Enable memory limits in systemd
sudo systemctl edit mbongo-node
# Add:
# [Service]
# MemoryMax=14G
# MemoryHigh=12G

# Configure swap (if needed)
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab

# Restart with limits
sudo systemctl daemon-reload
sudo systemctl restart mbongo-node
```

---

### Problem 6: Wrong Chain ID

**Symptoms:**
- Connected to wrong network
- Transactions rejected
- Block validation failures

**Solutions:**

```bash
# Check current chain ID
curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' | jq .

# Expected: 0x1 (mainnet) or 0x3E9 (testnet = 1001)

# Verify config
grep -E "network|chain_id" ~/.mbongo/config/full_node.toml

# If wrong, stop node and fix config
sudo systemctl stop mbongo-node

# Edit configuration
nano ~/.mbongo/config/full_node.toml
# Set: network = "mainnet" and chain_id = 1

# Clear data if synced to wrong chain
rm -rf ~/.mbongo/data/*

# Restart
sudo systemctl start mbongo-node
```

---

### Problem 7: Slow Sync

**Symptoms:**
- Sync takes much longer than expected
- Low block import rate
- Disk I/O bottleneck

**Solutions:**

```bash
# Check sync mode
grep "mode" ~/.mbongo/config/full_node.toml

# Use fast sync for initial sync
# [sync]
# mode = "fast"

# Increase parallel downloads
# parallel_downloads = 16

# Check disk performance
iostat -xz 1 5
# Look for high await times (>10ms is slow)

# Optimize disk settings (Ubuntu)
echo 'deadline' | sudo tee /sys/block/nvme0n1/queue/scheduler

# Check peer quality
mbongo-node peers list --data-dir ~/.mbongo/data

# Add more high-quality peers
mbongo-node peers add-static "/ip4/FAST_PEER_IP/tcp/30303/p2p/PEER_ID"

# Use checkpoint sync
mbongo-node sync --checkpoint --data-dir ~/.mbongo/data
```

---

### Problem 8: Firewall Blocking

**Symptoms:**
- No incoming peer connections
- Port checks fail externally
- Node isolated from network

**Solutions:**

```bash
# Test port from external source
# From another machine:
nc -vz YOUR_SERVER_IP 30303

# Check UFW
sudo ufw status verbose

# Ensure P2P ports are open
sudo ufw allow 30303/tcp
sudo ufw allow 30303/udp
sudo ufw reload

# Check iptables directly
sudo iptables -L -n | grep 30303

# Check cloud provider firewall
# AWS: Security Groups
# GCP: Firewall Rules
# Azure: Network Security Groups

# Check if ISP blocks the port
# Try alternative port in config and firewall

# Verify no conflicting rules
sudo ufw status numbered
# Delete conflicting rules if found
```

---

### Problem 9: Config File Errors

**Symptoms:**
- Node fails to start
- "Configuration error" messages
- "Invalid TOML" errors

**Solutions:**

```bash
# Validate TOML syntax
mbongo-node config validate --config ~/.mbongo/config/full_node.toml

# Check for common errors:
# - Missing quotes around strings
# - Incorrect indentation
# - Duplicate keys
# - Invalid values

# Test configuration parsing
mbongo-node --config ~/.mbongo/config/full_node.toml --dry-run

# Regenerate default config
mbongo-node init --config ~/.mbongo/config/full_node.toml.new --network mainnet

# Compare with your config
diff ~/.mbongo/config/full_node.toml ~/.mbongo/config/full_node.toml.new

# Common fixes:
# - Ensure paths use ~ or absolute paths
# - Check boolean values are true/false (not "true"/"false")
# - Verify numeric values don't have quotes
```

---

### Problem 10: Disk Full

**Symptoms:**
- Node crashes unexpectedly
- "No space left on device" errors
- Database write failures

**Solutions:**

```bash
# Check disk usage
df -h ~/.mbongo

# Find large files
du -sh ~/.mbongo/* | sort -h

# Check log sizes
du -sh ~/.mbongo/logs/

# Rotate and compress logs
sudo logrotate -f /etc/logrotate.d/mbongo-node

# Clear old logs manually
find ~/.mbongo/logs -name "*.log.*" -mtime +7 -delete

# If running full node, consider pruning
# [pruning]
# enabled = true
# keep_blocks = 128

# Clear peer database (minor savings)
rm -rf ~/.mbongo/data/peers/

# Emergency: extend disk or migrate
# Cloud: resize volume
# Local: add storage or migrate data directory

# Monitor disk usage
watch -n 60 'df -h ~/.mbongo'
```

---

## 11. Cross-References

### Related Documentation

| Document | Description | Link |
|----------|-------------|------|
| **Architecture Overview** | System architecture details | [architecture_master_overview.md](./architecture_master_overview.md) |
| **Runtime Architecture** | Runtime module design | [runtime_architecture.md](./runtime_architecture.md) |
| **Sync & Validation** | Synchronization protocol | [sync_validation.md](./sync_validation.md) |
| **Validator Setup** | Validator node configuration | [validator_setup.md](./validator_setup.md) |
| **Compute Provider Setup** | PoUW compute provider guide | [compute_provider_setup.md](./compute_provider_setup.md) |
| **Economic Security** | Network security economics | [economic_security.md](./economic_security.md) |

### External Resources

- **Mbongo Chain GitHub**: `https://github.com/mbongo-chain/mbongo-chain`
- **Block Explorer**: `https://explorer.mbongo.network`
- **Network Status**: `https://status.mbongo.network`
- **Community Discord**: `https://discord.gg/mbongo`
- **Documentation Portal**: `https://docs.mbongo.network`

### Quick Command Reference

```bash
# Node Management
mbongo-node --version                    # Check version
mbongo-node --help                       # Show help
mbongo-node init                         # Initialize configuration
mbongo-node --config FILE                # Start with config

# Status Checks
mbongo-node status                       # Node status
mbongo-node peers list                   # List peers
mbongo-node db check                     # Check database

# RPC Queries
curl http://127.0.0.1:8545/health        # Health check
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://127.0.0.1:8545                  # Current block

# Service Management (Ubuntu)
sudo systemctl start mbongo-node         # Start
sudo systemctl stop mbongo-node          # Stop
sudo systemctl status mbongo-node        # Status
sudo journalctl -u mbongo-node -f        # Logs
```

---

## Appendix: Quick Start Checklist

Use this checklist to ensure your full node is properly configured:

### Pre-Installation
- [ ] Hardware meets minimum requirements
- [ ] Operating system installed and updated
- [ ] Stable internet connection
- [ ] Sufficient storage for node type

### Installation
- [ ] System dependencies installed
- [ ] Rust toolchain installed
- [ ] Mbongo node binary downloaded/built
- [ ] Binary added to PATH

### Configuration
- [ ] Directory structure created
- [ ] Configuration file created and customized
- [ ] Sync mode selected appropriately
- [ ] RPC configured (enabled/disabled as needed)
- [ ] P2P settings configured

### Security
- [ ] Firewall configured
- [ ] SSH hardened
- [ ] RPC ports protected
- [ ] Regular updates scheduled

### Operations
- [ ] Service file created (systemd/NSSM)
- [ ] Service enabled for auto-start
- [ ] Log rotation configured
- [ ] Monitoring set up

### Verification
- [ ] Node starts without errors
- [ ] Peers are connecting
- [ ] Sync is progressing
- [ ] RPC responding (if enabled)
- [ ] Logs are clean

---

> **🎉 Congratulations!** Your Mbongo Chain Full Node is now operational.
>
> **Need help?** Join our Discord community for support.

---

*Document maintained by the Mbongo Chain Core Team*  
*Last reviewed: November 2025*

