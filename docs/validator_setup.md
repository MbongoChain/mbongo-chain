# Mbongo Chain — Validator Setup Guide

> **Document Version:** 1.0.0  
> **Last Updated:** November 2025  
> **Target Audience:** Validators, Node Operators, System Administrators

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Hardware Requirements](#2-hardware-requirements)
3. [Software Prerequisites](#3-software-prerequisites)
4. [Key Management](#4-key-management)
5. [Installation Steps](#5-installation-steps)
6. [Security & Slashing Prevention](#6-security--slashing-prevention)
7. [Testnet vs Mainnet Differences](#7-testnet-vs-mainnet-differences)
8. [Troubleshooting](#8-troubleshooting)
9. [Cross-References](#9-cross-references)

---

## 1. Introduction

### What is a Validator?

In Mbongo Chain's Proof-of-Stake (PoS) consensus mechanism, **validators** are the backbone of network security and block production. Validators are responsible for:

- **Proposing blocks** — Creating new blocks containing transactions
- **Attesting blocks** — Voting on the validity of proposed blocks
- **Maintaining consensus** — Participating in the finalization process
- **Securing the network** — Staking MBG tokens as collateral

```
┌─────────────────────────────────────────────────────────────────┐
│                    VALIDATOR LIFECYCLE                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐ │
│   │  Deposit │───▶│  Active  │───▶│ Proposer │───▶│ Attester │ │
│   │   Stake  │    │  Queue   │    │   Role   │    │   Role   │ │
│   └──────────┘    └──────────┘    └──────────┘    └──────────┘ │
│        │                                               │        │
│        │         ┌──────────────────────────┐         │        │
│        └────────▶│    REWARDS / PENALTIES   │◀────────┘        │
│                  └──────────────────────────┘                  │
│                              │                                  │
│                              ▼                                  │
│                  ┌──────────────────────────┐                  │
│                  │   Voluntary Exit or      │                  │
│                  │   Slashing Event         │                  │
│                  └──────────────────────────┘                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Validator Responsibilities

| Responsibility | Description | Frequency |
|----------------|-------------|-----------|
| **Block Proposal** | Create and broadcast new blocks when selected | Per epoch (probabilistic) |
| **Attestation** | Vote on block validity and chain head | Every epoch |
| **Sync Committee** | Participate in light client support | Periodic rotation |
| **Uptime** | Maintain 24/7 availability | Continuous |

### Slashing Risks

> ⚠️ **CRITICAL WARNING**: Slashing results in permanent loss of staked tokens!

Validators can be **slashed** (penalized) for malicious or negligent behavior:

| Offense | Penalty | Recovery |
|---------|---------|----------|
| **Double Signing** | Up to 100% of stake | Irreversible |
| **Surround Voting** | Up to 100% of stake | Irreversible |
| **Extended Downtime** | Gradual stake reduction | Recoverable |
| **Invalid Attestations** | Minor penalty | Recoverable |

```
┌───────────────────────────────────────────────────────────────┐
│                    SLASHING SCENARIOS                         │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│  ❌ DOUBLE SIGNING (MOST SEVERE)                              │
│  ┌─────────┐         ┌─────────┐                              │
│  │ Block A │◀───┬───▶│ Block B │  Same slot, same validator  │
│  └─────────┘    │    └─────────┘                              │
│              ┌──┴──┐                                          │
│              │ YOU │  ← SLASHED!                              │
│              └─────┘                                          │
│                                                               │
│  ❌ SURROUND VOTE                                              │
│  Attestation 1: [────────────]                                │
│  Attestation 2:    [──────]     ← Surrounded = SLASHED!      │
│                                                               │
│  ⚠️ EXTENDED DOWNTIME                                         │
│  Online: ████████░░░░░░░░░░░░  40% uptime = Penalties        │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

### Required Skills

Before proceeding, ensure you have:

- ✅ Basic command-line interface (CLI) proficiency
- ✅ System administration experience (Linux or Windows)
- ✅ Understanding of networking concepts (ports, firewalls)
- ✅ Familiarity with cryptographic key management
- ✅ Ability to monitor and maintain 24/7 services

### Supported Operating Systems

| OS | Version | Support Level |
|----|---------|---------------|
| **Ubuntu** | 22.04 LTS | ✅ Full Support |
| **Windows** | 10/11, Server 2019+ | ✅ Full Support |
| Debian | 11+ | 🟡 Community Support |
| macOS | 12+ | 🟡 Development Only |

---

## 2. Hardware Requirements

### Minimum & Recommended Specifications

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| **CPU** | 4 cores | 8+ cores | Modern x86_64 with AES-NI |
| **RAM** | 16 GB | 32 GB | ECC RAM preferred |
| **Storage** | 512 GB NVMe | 1 TB+ NVMe | IOPS > 10,000 |
| **Network** | 50 Mbps | 200+ Mbps | Low latency, stable |
| **Redundancy** | — | Dual NIC | Optional but recommended |

### Storage Considerations

```
┌────────────────────────────────────────────────────────────────┐
│                    STORAGE BREAKDOWN                           │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │ Blockchain Data         │████████████████████│  ~400 GB  │ │
│  ├──────────────────────────────────────────────────────────┤ │
│  │ State Database          │██████████│          ~100 GB    │ │
│  ├──────────────────────────────────────────────────────────┤ │
│  │ Validator Keys & Config │█│                   ~1 GB      │ │
│  ├──────────────────────────────────────────────────────────┤ │
│  │ Logs & Temp Files       │██│                  ~10 GB     │ │
│  ├──────────────────────────────────────────────────────────┤ │
│  │ Growth Buffer           │████│                ~50 GB     │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                │
│  TOTAL RECOMMENDED: 512 GB - 1 TB NVMe SSD                    │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### Network Requirements

| Metric | Requirement | Purpose |
|--------|-------------|---------|
| **Bandwidth** | 50–200 Mbps | Block propagation, peer sync |
| **Latency** | < 100ms | Timely attestations |
| **Ports** | 30303/tcp, 30303/udp | P2P communication |
| **Static IP** | Recommended | Peer discovery stability |

> 💡 **TIP**: A redundant internet connection (failover) significantly reduces downtime risks.

---

## 3. Software Prerequisites

### Ubuntu 22.04 LTS

#### System Packages

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install essential packages
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    cmake \
    git \
    curl \
    wget \
    jq \
    unzip \
    htop \
    tmux \
    ufw

# Verify installations
gcc --version
openssl version
git --version
```

#### Rust Toolchain

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Reload shell environment
source $HOME/.cargo/env

# Verify Rust installation
rustc --version
cargo --version

# Install required Rust components
rustup component add clippy rustfmt

# Set stable toolchain as default
rustup default stable
```

### Windows (PowerShell)

#### System Requirements

```powershell
# Run PowerShell as Administrator

# Check Windows version
Get-ComputerInfo | Select-Object WindowsVersion, OsArchitecture

# Enable required Windows features
Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Windows-Subsystem-Linux -NoRestart
Enable-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform -NoRestart

# Install Chocolatey package manager (if not installed)
Set-ExecutionPolicy Bypass -Scope Process -Force
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

# Install required packages
choco install -y `
    git `
    visualstudio2022buildtools `
    cmake `
    openssl `
    wget `
    jq

# Refresh environment
refreshenv
```

#### Rust Toolchain (Windows)

```powershell
# Download and run rustup installer
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y

# Reload PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Verify installation
rustc --version
cargo --version

# Install components
rustup component add clippy rustfmt
```

### Software Component Overview

```
┌────────────────────────────────────────────────────────────────┐
│                 SOFTWARE STACK                                 │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌─────────────────────┐  ┌─────────────────────┐             │
│  │   Mbongo CLI        │  │  Validator Client   │             │
│  │   (mbongo-cli)      │  │  (mbongo-validator) │             │
│  └──────────┬──────────┘  └──────────┬──────────┘             │
│             │                        │                         │
│             └───────────┬────────────┘                         │
│                         │                                      │
│             ┌───────────▼───────────┐                         │
│             │    Rust Runtime       │                         │
│             │    (rustc + cargo)    │                         │
│             └───────────┬───────────┘                         │
│                         │                                      │
│             ┌───────────▼───────────┐                         │
│             │   Operating System    │                         │
│             │  (Ubuntu / Windows)   │                         │
│             └───────────────────────┘                         │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

---

## 4. Key Management

### Understanding Validator Keys

Mbongo Chain validators use a **two-key architecture** for maximum security:

```
┌─────────────────────────────────────────────────────────────────┐
│                    VALIDATOR KEY ARCHITECTURE                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    SIGNING KEY                            │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │  Purpose: Sign blocks, attestations, votes         │  │  │
│  │  │  Location: HOT (on validator machine)              │  │  │
│  │  │  Security: Encrypted keystore                      │  │  │
│  │  │  Risk: Compromise = Slashing possible              │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              │ Signs operations                 │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                   WITHDRAWAL KEY                          │  │
│  │  ┌────────────────────────────────────────────────────┐  │  │
│  │  │  Purpose: Withdraw staked funds                    │  │  │
│  │  │  Location: COLD (offline, hardware wallet)         │  │  │
│  │  │  Security: Air-gapped, never online                │  │  │
│  │  │  Risk: Compromise = Complete fund loss             │  │  │
│  │  └────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ⚠️  NEVER store both keys in the same location!               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Key Generation

> ⚠️ **SECURITY WARNING**: Generate keys ONLY on a secure, air-gapped machine when possible!

#### Step 1: Create Secure Environment

**Ubuntu:**
```bash
# Create isolated directory with restricted permissions
mkdir -p ~/.mbongo/validator_keys
chmod 700 ~/.mbongo
chmod 700 ~/.mbongo/validator_keys

# Verify no network access (for air-gapped setup)
# Disconnect from internet before generating keys
```

**Windows (PowerShell):**
```powershell
# Create isolated directory
New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.mbongo\validator_keys"

# Set restrictive permissions
$acl = Get-Acl "$env:USERPROFILE\.mbongo"
$acl.SetAccessRuleProtection($true, $false)
$rule = New-Object System.Security.AccessControl.FileSystemAccessRule($env:USERNAME, "FullControl", "ContainerInherit,ObjectInherit", "None", "Allow")
$acl.SetAccessRule($rule)
Set-Acl "$env:USERPROFILE\.mbongo" $acl
```

#### Step 2: Generate Validator Keys

```bash
# Generate new validator keypair
mbongo-cli validator keygen \
    --output-dir ~/.mbongo/validator_keys \
    --network mainnet

# Example output:
# ✅ Signing key generated: validator_signing.json
# ✅ Withdrawal key generated: validator_withdrawal.json
# ✅ Deposit data generated: deposit_data.json
#
# Public Key: 0x8a7b3c...
# Withdrawal Address: 0x1234...
```

#### Step 3: Encrypt Keystore

```bash
# Encrypt the signing key with a strong password
mbongo-cli validator encrypt-keystore \
    --input ~/.mbongo/validator_keys/validator_signing.json \
    --output ~/.mbongo/validator_keys/keystore.json

# You will be prompted for a password:
# Enter keystore password (min 12 characters): ********
# Confirm password: ********
#
# ✅ Keystore encrypted successfully

# Verify keystore integrity
mbongo-cli validator verify-keystore \
    --keystore ~/.mbongo/validator_keys/keystore.json
```

#### Step 4: Create validator.json

```bash
# Create validator configuration
mbongo-cli validator init \
    --keystore ~/.mbongo/validator_keys/keystore.json \
    --output ~/.mbongo/validator_keys/validator.json \
    --fee-recipient 0xYourFeeRecipientAddress \
    --graffiti "MyValidatorName"

# Example validator.json structure:
```

```json
{
  "version": "1.0.0",
  "pubkey": "0x8a7b3c4d5e6f...",
  "keystore_path": "/home/user/.mbongo/validator_keys/keystore.json",
  "fee_recipient": "0xYourFeeRecipientAddress",
  "graffiti": "MyValidatorName",
  "slashing_protection_db": "/home/user/.mbongo/validator_keys/slashing_protection.db",
  "network": "mainnet",
  "created_at": "2025-11-27T00:00:00Z"
}
```

### Secure Storage Best Practices

```
┌─────────────────────────────────────────────────────────────────┐
│                    3-2-1 BACKUP STRATEGY                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  3 COPIES of your data                                    │ │
│  │    • Primary: Validator machine (encrypted)               │ │
│  │    • Secondary: Encrypted USB drive (offline)             │ │
│  │    • Tertiary: Encrypted cloud backup (optional)          │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  2 DIFFERENT storage media                                │ │
│  │    • SSD (primary machine)                                │ │
│  │    • USB/External HDD (backup)                            │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  1 OFFSITE copy                                           │ │
│  │    • Geographic separation from primary location          │ │
│  │    • Bank safe deposit box (recommended)                  │ │
│  │    • Trusted family member's secure location              │ │
│  └───────────────────────────────────────────────────────────┘ │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Slashing Protection Database

> 🛡️ **CRITICAL**: The slashing protection database prevents accidental double-signing!

```bash
# Initialize slashing protection database
mbongo-cli validator slashing-protection init \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db

# Export slashing protection (for migration)
mbongo-cli validator slashing-protection export \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db \
    --output slashing_protection_backup.json

# Import slashing protection (on new machine)
mbongo-cli validator slashing-protection import \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db \
    --input slashing_protection_backup.json
```

```
┌─────────────────────────────────────────────────────────────────┐
│              SLASHING PROTECTION FLOW                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐                                                │
│  │ Sign Request│                                                │
│  └──────┬──────┘                                                │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────────────────────────────┐                       │
│  │ Check Slashing Protection DB        │                       │
│  │ • Has this slot been signed before? │                       │
│  │ • Would this surround a prior vote? │                       │
│  └──────┬──────────────────────────────┘                       │
│         │                                                       │
│    ┌────┴────┐                                                  │
│    │         │                                                  │
│    ▼         ▼                                                  │
│  ┌─────┐  ┌─────────┐                                          │
│  │ OK  │  │ BLOCKED │                                          │
│  └──┬──┘  └────┬────┘                                          │
│     │          │                                                │
│     ▼          ▼                                                │
│  ┌──────┐  ┌────────────────┐                                  │
│  │ Sign │  │ Refuse to sign │                                  │
│  │  &   │  │ Log warning    │                                  │
│  │Record│  └────────────────┘                                  │
│  └──────┘                                                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Security Checklist

- [ ] Keys generated on air-gapped machine
- [ ] Withdrawal key stored offline (cold storage)
- [ ] Signing key encrypted with strong password (16+ characters)
- [ ] Password stored separately from keys
- [ ] 3-2-1 backup strategy implemented
- [ ] Slashing protection database initialized
- [ ] Private keys NEVER exposed online
- [ ] Keystore permissions restricted (chmod 600)

---

## 5. Installation Steps

### Step 1 — Install Dependencies

#### Ubuntu 22.04 LTS

```bash
#!/bin/bash
# Mbongo Validator Dependency Installation Script

set -e

echo "=== Updating System ==="
sudo apt update && sudo apt upgrade -y

echo "=== Installing Build Dependencies ==="
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    libclang-dev \
    cmake \
    git \
    curl \
    wget \
    jq \
    unzip

echo "=== Installing Rust ==="
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

echo "=== Verifying Rust Installation ==="
rustc --version
cargo --version

echo "=== Installing Rust Components ==="
rustup component add clippy rustfmt

echo "=== Creating Mbongo Directories ==="
mkdir -p ~/.mbongo/{bin,config,data,logs,validator_keys}
chmod 700 ~/.mbongo

echo "=== Dependencies Installed Successfully ==="
```

#### Windows (PowerShell)

```powershell
# Mbongo Validator Dependency Installation Script
# Run as Administrator

Write-Host "=== Installing Build Dependencies ===" -ForegroundColor Cyan

# Install Chocolatey if not present
if (!(Get-Command choco -ErrorAction SilentlyContinue)) {
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
}

# Install required packages
choco install -y git visualstudio2022buildtools cmake openssl wget jq

Write-Host "=== Installing Rust ===" -ForegroundColor Cyan

# Install Rust
if (!(Get-Command rustc -ErrorAction SilentlyContinue)) {
    Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile "$env:TEMP\rustup-init.exe"
    Start-Process -FilePath "$env:TEMP\rustup-init.exe" -ArgumentList "-y" -Wait
}

# Refresh environment
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

Write-Host "=== Verifying Installation ===" -ForegroundColor Cyan
rustc --version
cargo --version

Write-Host "=== Creating Mbongo Directories ===" -ForegroundColor Cyan
$mbongoPath = "$env:USERPROFILE\.mbongo"
New-Item -ItemType Directory -Force -Path "$mbongoPath\bin"
New-Item -ItemType Directory -Force -Path "$mbongoPath\config"
New-Item -ItemType Directory -Force -Path "$mbongoPath\data"
New-Item -ItemType Directory -Force -Path "$mbongoPath\logs"
New-Item -ItemType Directory -Force -Path "$mbongoPath\validator_keys"

Write-Host "=== Dependencies Installed Successfully ===" -ForegroundColor Green
```

### Step 2 — Download Mbongo Validator Client

#### Ubuntu

```bash
# Set version (replace with latest release)
MBONGO_VERSION="v1.0.0"

# Download validator client binary
wget -O mbongo-validator.tar.gz \
    "https://github.com/mbongo-chain/mbongo-chain/releases/download/${MBONGO_VERSION}/mbongo-validator-linux-amd64.tar.gz"

# Verify checksum (IMPORTANT!)
wget -O checksums.txt \
    "https://github.com/mbongo-chain/mbongo-chain/releases/download/${MBONGO_VERSION}/checksums.txt"
sha256sum -c checksums.txt --ignore-missing

# Extract binary
tar -xzf mbongo-validator.tar.gz

# Move to bin directory
mv mbongo-validator ~/.mbongo/bin/
mv mbongo-cli ~/.mbongo/bin/

# Add to PATH
echo 'export PATH="$HOME/.mbongo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify installation
mbongo-validator --version
mbongo-cli --version

# Set executable permissions
chmod +x ~/.mbongo/bin/mbongo-validator
chmod +x ~/.mbongo/bin/mbongo-cli
```

#### Windows (PowerShell)

```powershell
# Set version
$MBONGO_VERSION = "v1.0.0"

# Download validator client
$downloadUrl = "https://github.com/mbongo-chain/mbongo-chain/releases/download/$MBONGO_VERSION/mbongo-validator-windows-amd64.zip"
$zipPath = "$env:TEMP\mbongo-validator.zip"

Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath

# Extract to bin directory
Expand-Archive -Path $zipPath -DestinationPath "$env:USERPROFILE\.mbongo\bin" -Force

# Add to PATH permanently
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
$mbongoBin = "$env:USERPROFILE\.mbongo\bin"
if ($currentPath -notlike "*$mbongoBin*") {
    [Environment]::SetEnvironmentVariable("Path", "$currentPath;$mbongoBin", "User")
}

# Refresh current session PATH
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# Verify installation
mbongo-validator --version
mbongo-cli --version
```

### Step 3 — Configure the Validator

#### Create Configuration Directory Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                  VALIDATOR DIRECTORY LAYOUT                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ~/.mbongo/                                                     │
│  ├── bin/                                                       │
│  │   ├── mbongo-validator        # Validator binary             │
│  │   └── mbongo-cli              # CLI tool                     │
│  │                                                              │
│  ├── config/                                                    │
│  │   ├── config.toml             # Main configuration           │
│  │   └── validator.json          # Validator-specific config    │
│  │                                                              │
│  ├── data/                                                      │
│  │   ├── blockchain/             # Chain data                   │
│  │   └── state/                  # State database               │
│  │                                                              │
│  ├── validator_keys/                                            │
│  │   ├── keystore.json           # Encrypted signing key        │
│  │   ├── slashing_protection.db  # Slashing protection DB       │
│  │   └── deposit_data.json       # Deposit information          │
│  │                                                              │
│  └── logs/                                                      │
│      ├── validator.log           # Validator logs               │
│      └── archive/                # Rotated logs                 │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### Create config.toml

**Ubuntu:**
```bash
cat > ~/.mbongo/config/config.toml << 'EOF'
# Mbongo Validator Configuration
# Version: 1.0.0

[general]
# Network: "mainnet" or "testnet"
network = "mainnet"

# Data directory
data_dir = "~/.mbongo/data"

# Log level: "trace", "debug", "info", "warn", "error"
log_level = "info"

# Log file path
log_file = "~/.mbongo/logs/validator.log"

[validator]
# Enable validator mode
enabled = true

# Keystore path
keystore_path = "~/.mbongo/validator_keys/keystore.json"

# Slashing protection database
slashing_protection_db = "~/.mbongo/validator_keys/slashing_protection.db"

# Fee recipient address (receives block rewards)
fee_recipient = "0xYOUR_FEE_RECIPIENT_ADDRESS"

# Graffiti (32 bytes max, appears in blocks you propose)
graffiti = "Mbongo Validator"

# Suggest priority fee (in gwei)
suggested_fee_recipient_priority = 1

[network]
# P2P listen address
listen_addr = "0.0.0.0"

# P2P port
port = 30303

# Maximum peers
max_peers = 50

# Bootnodes (mainnet)
bootnodes = [
    "/dns4/bootnode1.mbongo.network/tcp/30303/p2p/BOOTNODE_PEER_ID_1",
    "/dns4/bootnode2.mbongo.network/tcp/30303/p2p/BOOTNODE_PEER_ID_2",
]

[rpc]
# Enable RPC server
enabled = true

# RPC listen address (use 127.0.0.1 for local only)
listen_addr = "127.0.0.1"

# RPC port
port = 8545

# Enable WebSocket
ws_enabled = true
ws_port = 8546

# CORS origins (empty = disabled, "*" = all)
cors_origins = []

[metrics]
# Enable Prometheus metrics
enabled = true

# Metrics listen address
listen_addr = "127.0.0.1"

# Metrics port
port = 9090

[telemetry]
# Enable telemetry reporting
enabled = false

# Telemetry endpoint
# endpoint = "https://telemetry.mbongo.network"
EOF
```

**Windows (PowerShell):**
```powershell
@"
# Mbongo Validator Configuration
# Version: 1.0.0

[general]
network = "mainnet"
data_dir = "C:\\Users\\$env:USERNAME\\.mbongo\\data"
log_level = "info"
log_file = "C:\\Users\\$env:USERNAME\\.mbongo\\logs\\validator.log"

[validator]
enabled = true
keystore_path = "C:\\Users\\$env:USERNAME\\.mbongo\\validator_keys\\keystore.json"
slashing_protection_db = "C:\\Users\\$env:USERNAME\\.mbongo\\validator_keys\\slashing_protection.db"
fee_recipient = "0xYOUR_FEE_RECIPIENT_ADDRESS"
graffiti = "Mbongo Validator"
suggested_fee_recipient_priority = 1

[network]
listen_addr = "0.0.0.0"
port = 30303
max_peers = 50
bootnodes = [
    "/dns4/bootnode1.mbongo.network/tcp/30303/p2p/BOOTNODE_PEER_ID_1",
    "/dns4/bootnode2.mbongo.network/tcp/30303/p2p/BOOTNODE_PEER_ID_2",
]

[rpc]
enabled = true
listen_addr = "127.0.0.1"
port = 8545
ws_enabled = true
ws_port = 8546
cors_origins = []

[metrics]
enabled = true
listen_addr = "127.0.0.1"
port = 9090

[telemetry]
enabled = false
"@ | Out-File -FilePath "$env:USERPROFILE\.mbongo\config\config.toml" -Encoding UTF8
```

### Step 4 — Import Validator Keys

#### Import Encrypted Keystore

```bash
# Import keystore into validator client
mbongo-cli validator import-keystore \
    --keystore ~/.mbongo/validator_keys/keystore.json \
    --config ~/.mbongo/config/config.toml

# Enter password when prompted
# Password: ********

# Verify key was imported successfully
mbongo-cli validator list-keys \
    --config ~/.mbongo/config/config.toml

# Expected output:
# Validator Keys:
# ┌────────────────────────────────────────────────────────┐
# │ Index │ Public Key                      │ Status       │
# ├────────────────────────────────────────────────────────┤
# │ 0     │ 0x8a7b3c4d5e6f...               │ Ready        │
# └────────────────────────────────────────────────────────┘
```

#### Initialize Slashing Protection

```bash
# Initialize slashing protection database
mbongo-cli validator slashing-protection init \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db

# Verify slashing protection is active
mbongo-cli validator slashing-protection status \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db

# Expected output:
# Slashing Protection Database Status:
# ✅ Database initialized
# ✅ No previous signing history (fresh validator)
# ✅ Protection enabled
```

### Step 5 — Start the Validator

#### Ubuntu — systemd Service

```bash
# Create systemd service file
sudo tee /etc/systemd/system/mbongo-validator.service << 'EOF'
[Unit]
Description=Mbongo Chain Validator
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=YOUR_USERNAME
Group=YOUR_USERNAME
ExecStart=/home/YOUR_USERNAME/.mbongo/bin/mbongo-validator \
    --config /home/YOUR_USERNAME/.mbongo/config/config.toml
Restart=always
RestartSec=10
LimitNOFILE=65535

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=/home/YOUR_USERNAME/.mbongo

# Environment
Environment="RUST_LOG=info"
Environment="RUST_BACKTRACE=1"

[Install]
WantedBy=multi-user.target
EOF

# Replace YOUR_USERNAME with actual username
sudo sed -i "s/YOUR_USERNAME/$USER/g" /etc/systemd/system/mbongo-validator.service

# Reload systemd
sudo systemctl daemon-reload

# Enable service to start on boot
sudo systemctl enable mbongo-validator

# Start the validator
sudo systemctl start mbongo-validator

# Check status
sudo systemctl status mbongo-validator
```

#### Windows — PowerShell Service

```powershell
# Create Windows Service using NSSM (Non-Sucking Service Manager)
# First, install NSSM
choco install -y nssm

# Create the service
$serviceName = "MbongoValidator"
$binaryPath = "$env:USERPROFILE\.mbongo\bin\mbongo-validator.exe"
$configPath = "$env:USERPROFILE\.mbongo\config\config.toml"
$logPath = "$env:USERPROFILE\.mbongo\logs"

# Install service with NSSM
nssm install $serviceName $binaryPath
nssm set $serviceName AppParameters "--config `"$configPath`""
nssm set $serviceName AppDirectory "$env:USERPROFILE\.mbongo"
nssm set $serviceName AppStdout "$logPath\validator-stdout.log"
nssm set $serviceName AppStderr "$logPath\validator-stderr.log"
nssm set $serviceName AppRotateFiles 1
nssm set $serviceName AppRotateBytes 10485760
nssm set $serviceName Start SERVICE_AUTO_START
nssm set $serviceName ObjectName LocalSystem

# Start the service
nssm start $serviceName

# Check status
nssm status $serviceName
Get-Service $serviceName
```

#### Status Check Commands

**Ubuntu:**
```bash
# Check service status
sudo systemctl status mbongo-validator

# View recent logs
sudo journalctl -u mbongo-validator -f --no-hostname

# Check validator is syncing
mbongo-cli validator status \
    --rpc http://127.0.0.1:8545

# Check peer connections
mbongo-cli node peers \
    --rpc http://127.0.0.1:8545

# Check sync status
mbongo-cli node sync-status \
    --rpc http://127.0.0.1:8545
```

**Windows (PowerShell):**
```powershell
# Check service status
Get-Service MbongoValidator

# View logs (tail equivalent)
Get-Content "$env:USERPROFILE\.mbongo\logs\validator.log" -Tail 100 -Wait

# Check validator status
mbongo-cli validator status --rpc http://127.0.0.1:8545

# Check peer connections
mbongo-cli node peers --rpc http://127.0.0.1:8545
```

#### Log Monitoring

```bash
# Real-time log monitoring (Ubuntu)
tail -f ~/.mbongo/logs/validator.log

# Search for errors
grep -i "error\|warn" ~/.mbongo/logs/validator.log

# Monitor attestation performance
grep "attestation" ~/.mbongo/logs/validator.log | tail -20

# Monitor block proposals
grep "proposed_block" ~/.mbongo/logs/validator.log
```

#### RPC Health Checks

```bash
# Check node health
curl -s http://127.0.0.1:8545/health | jq .

# Check sync status via RPC
curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"mbongo_syncStatus","params":[],"id":1}' | jq .

# Check validator duties
curl -s -X POST http://127.0.0.1:8545 \
    -H "Content-Type: application/json" \
    -d '{"jsonrpc":"2.0","method":"validator_duties","params":["latest"],"id":1}' | jq .
```

---

## 6. Security & Slashing Prevention

### Golden Rules of Validator Security

```
┌─────────────────────────────────────────────────────────────────┐
│              🛡️ VALIDATOR SECURITY COMMANDMENTS                │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1️⃣  NEVER run multiple validators with the same key           │
│                                                                 │
│  2️⃣  ALWAYS backup slashing protection database before         │
│      migration                                                  │
│                                                                 │
│  3️⃣  NEVER share your keystore password                        │
│                                                                 │
│  4️⃣  ALWAYS use encrypted keystores                            │
│                                                                 │
│  5️⃣  NEVER expose RPC ports to the public internet             │
│                                                                 │
│  6️⃣  ALWAYS keep your validator client updated                 │
│                                                                 │
│  7️⃣  NEVER import keys without slashing protection data        │
│                                                                 │
│  8️⃣  ALWAYS test failover procedures before production         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Preventing Double-Signing

```
┌─────────────────────────────────────────────────────────────────┐
│              DOUBLE-SIGNING PREVENTION                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  DANGEROUS SCENARIO:                                            │
│  ┌─────────────┐              ┌─────────────┐                  │
│  │ Validator A │              │ Validator B │                  │
│  │ (Primary)   │              │ (Backup)    │                  │
│  └──────┬──────┘              └──────┬──────┘                  │
│         │                            │                          │
│         │  Same Key!                 │                          │
│         ▼                            ▼                          │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    NETWORK                               │   │
│  │           ⚠️ TWO SIGNATURES = SLASHED!                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  SAFE MIGRATION PROCEDURE:                                      │
│  1. Stop Validator A completely                                 │
│  2. Export slashing protection database                         │
│  3. Transfer keystore + slashing DB to Validator B              │
│  4. Import slashing protection on Validator B                   │
│  5. Start Validator B                                           │
│  6. Verify Validator A remains stopped                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Required Uptime

| Uptime | Impact | Recommendation |
|--------|--------|----------------|
| **99%+** | Optimal rewards | Target this |
| **95-99%** | Minor penalties | Acceptable |
| **90-95%** | Noticeable penalties | Investigate |
| **< 90%** | Significant stake loss | Critical issue |

### Firewall Configuration

#### Ubuntu (UFW)

```bash
# Enable UFW
sudo ufw enable

# Allow SSH (essential!)
sudo ufw allow ssh

# Allow P2P ports
sudo ufw allow 30303/tcp comment 'Mbongo P2P TCP'
sudo ufw allow 30303/udp comment 'Mbongo P2P UDP'

# DENY public RPC access (critical!)
# RPC should only be accessible locally
sudo ufw deny 8545/tcp comment 'Block external RPC'
sudo ufw deny 8546/tcp comment 'Block external WebSocket'

# Allow metrics only from monitoring server (if applicable)
# sudo ufw allow from MONITORING_IP to any port 9090

# Check status
sudo ufw status verbose
```

#### Windows Firewall (PowerShell)

```powershell
# Allow P2P ports
New-NetFirewallRule -DisplayName "Mbongo P2P TCP" -Direction Inbound -Protocol TCP -LocalPort 30303 -Action Allow
New-NetFirewallRule -DisplayName "Mbongo P2P UDP" -Direction Inbound -Protocol UDP -LocalPort 30303 -Action Allow

# Block external RPC access
New-NetFirewallRule -DisplayName "Block External RPC" -Direction Inbound -Protocol TCP -LocalPort 8545 -Action Block -RemoteAddress Any
New-NetFirewallRule -DisplayName "Block External WebSocket" -Direction Inbound -Protocol TCP -LocalPort 8546 -Action Block -RemoteAddress Any

# Allow local RPC only
New-NetFirewallRule -DisplayName "Allow Local RPC" -Direction Inbound -Protocol TCP -LocalPort 8545 -Action Allow -RemoteAddress 127.0.0.1
```

### Server Hardening Checklist

- [ ] Disable root SSH login
- [ ] Use SSH key authentication only
- [ ] Enable automatic security updates
- [ ] Install and configure fail2ban
- [ ] Set up intrusion detection (AIDE/OSSEC)
- [ ] Regular system audits
- [ ] Encrypted disk partitions
- [ ] Disable unnecessary services
- [ ] Configure system resource limits
- [ ] Set up monitoring and alerting

### Backup Slashing Database

```bash
# Create automated backup script
cat > ~/.mbongo/scripts/backup_slashing_db.sh << 'EOF'
#!/bin/bash
set -e

BACKUP_DIR="$HOME/.mbongo/backups/slashing_protection"
DB_PATH="$HOME/.mbongo/validator_keys/slashing_protection.db"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

mkdir -p "$BACKUP_DIR"

# Export slashing protection
mbongo-cli validator slashing-protection export \
    --db-path "$DB_PATH" \
    --output "$BACKUP_DIR/slashing_protection_$TIMESTAMP.json"

# Keep only last 30 backups
ls -t "$BACKUP_DIR"/*.json | tail -n +31 | xargs -r rm

echo "Backup completed: slashing_protection_$TIMESTAMP.json"
EOF

chmod +x ~/.mbongo/scripts/backup_slashing_db.sh

# Add to crontab (daily backup at 2 AM)
(crontab -l 2>/dev/null; echo "0 2 * * * $HOME/.mbongo/scripts/backup_slashing_db.sh") | crontab -
```

---

## 7. Testnet vs Mainnet Differences

### Quick Comparison

| Aspect | Testnet | Mainnet |
|--------|---------|---------|
| **Tokens** | Free (faucet) | Real value |
| **Risk** | None | Financial loss |
| **Purpose** | Testing, learning | Production |
| **Uptime Requirements** | Relaxed | Strict |
| **Network ID** | 1001 | 1 |

### Testnet Configuration

#### config.toml Changes

```toml
[general]
# Change network to testnet
network = "testnet"

[network]
# Testnet bootnodes
bootnodes = [
    "/dns4/testnet-bootnode1.mbongo.network/tcp/30303/p2p/TESTNET_PEER_ID_1",
    "/dns4/testnet-bootnode2.mbongo.network/tcp/30303/p2p/TESTNET_PEER_ID_2",
]
```

### Getting Testnet Tokens (Faucet)

```bash
# Generate testnet address
mbongo-cli wallet create --network testnet

# Request testnet tokens via faucet
mbongo-cli faucet request \
    --address 0xYOUR_TESTNET_ADDRESS \
    --network testnet

# Check balance
mbongo-cli wallet balance \
    --address 0xYOUR_TESTNET_ADDRESS \
    --network testnet

# Alternative: Web faucet
# https://faucet.testnet.mbongo.network
```

### Testnet Bootnodes

```
┌─────────────────────────────────────────────────────────────────┐
│                    TESTNET BOOTNODES                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Primary:                                                       │
│  /dns4/testnet-bootnode1.mbongo.network/tcp/30303/p2p/...      │
│                                                                 │
│  Secondary:                                                     │
│  /dns4/testnet-bootnode2.mbongo.network/tcp/30303/p2p/...      │
│                                                                 │
│  Backup:                                                        │
│  /dns4/testnet-bootnode3.mbongo.network/tcp/30303/p2p/...      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Reset Testnet Validator

```bash
# Stop validator
sudo systemctl stop mbongo-validator

# Backup existing data (optional)
mv ~/.mbongo/data ~/.mbongo/data_backup_$(date +%Y%m%d)

# Clear data directories
rm -rf ~/.mbongo/data/*

# Reset slashing protection (CAUTION: only for testnet!)
rm ~/.mbongo/validator_keys/slashing_protection.db
mbongo-cli validator slashing-protection init \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db

# Restart validator
sudo systemctl start mbongo-validator

# Monitor sync
mbongo-cli node sync-status --rpc http://127.0.0.1:8545
```

### Joining Mainnet from Scratch

```bash
# 1. Ensure you have real MBG tokens for staking
mbongo-cli wallet balance --address 0xYOUR_ADDRESS --network mainnet

# 2. Update config.toml
sed -i 's/network = "testnet"/network = "mainnet"/' ~/.mbongo/config/config.toml

# 3. Update bootnodes to mainnet
# (Edit config.toml manually with mainnet bootnodes)

# 4. Generate new keys for mainnet (IMPORTANT: separate from testnet!)
mbongo-cli validator keygen \
    --output-dir ~/.mbongo/validator_keys_mainnet \
    --network mainnet

# 5. Clear testnet data
rm -rf ~/.mbongo/data/*

# 6. Import mainnet keys
mbongo-cli validator import-keystore \
    --keystore ~/.mbongo/validator_keys_mainnet/keystore.json \
    --config ~/.mbongo/config/config.toml

# 7. Initialize fresh slashing protection
mbongo-cli validator slashing-protection init \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db

# 8. Submit deposit transaction
mbongo-cli validator deposit \
    --keystore ~/.mbongo/validator_keys_mainnet/keystore.json \
    --amount 32000 \
    --network mainnet

# 9. Start validator and wait for activation
sudo systemctl start mbongo-validator
```

---

## 8. Troubleshooting

### Common Problems and Solutions

#### Problem 1: Validator Not Syncing

**Symptoms:**
- Sync progress stuck at 0%
- No new blocks being processed
- Peer count remains 0

**Solutions:**

```bash
# Check peer connections
mbongo-cli node peers --rpc http://127.0.0.1:8545

# Verify bootnodes in config
grep -A5 "bootnodes" ~/.mbongo/config/config.toml

# Check firewall allows P2P ports
sudo ufw status | grep 30303

# Test bootnode connectivity
nc -vz bootnode1.mbongo.network 30303

# Check system time is synchronized
timedatectl status

# If needed, sync time
sudo timedatectl set-ntp true

# Check disk space
df -h ~/.mbongo

# Restart with verbose logging
mbongo-validator --config ~/.mbongo/config/config.toml --log-level debug
```

---

#### Problem 2: RPC Timeout

**Symptoms:**
- CLI commands timeout
- "Connection refused" errors
- RPC requests fail

**Solutions:**

```bash
# Check if validator is running
sudo systemctl status mbongo-validator

# Check RPC is enabled in config
grep -A5 "\[rpc\]" ~/.mbongo/config/config.toml

# Verify RPC port is listening
netstat -tlnp | grep 8545
# or
ss -tlnp | grep 8545

# Test local RPC
curl -s http://127.0.0.1:8545/health

# Check for port conflicts
sudo lsof -i :8545

# Increase RPC timeout in requests
mbongo-cli --rpc-timeout 60 node status
```

---

#### Problem 3: Slashing DB Missing

**Symptoms:**
- "Slashing protection database not found" error
- Validator refuses to start

**Solutions:**

```bash
# Check if database exists
ls -la ~/.mbongo/validator_keys/slashing_protection.db

# Initialize new database (ONLY if no previous signing history!)
mbongo-cli validator slashing-protection init \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db

# Restore from backup
mbongo-cli validator slashing-protection import \
    --db-path ~/.mbongo/validator_keys/slashing_protection.db \
    --input /path/to/backup/slashing_protection_backup.json

# Verify config points to correct path
grep "slashing_protection" ~/.mbongo/config/config.toml
```

---

#### Problem 4: Wrong Keystore Password

**Symptoms:**
- "Invalid password" error
- Decryption failure
- Key import fails

**Solutions:**

```bash
# Verify you're entering correct password
# Check for leading/trailing spaces

# Test keystore decryption
mbongo-cli validator verify-keystore \
    --keystore ~/.mbongo/validator_keys/keystore.json

# If password lost, you must restore from seed phrase
# and create new keystore (requires original mnemonic!)

# Re-encrypt with new password
mbongo-cli validator change-password \
    --keystore ~/.mbongo/validator_keys/keystore.json \
    --current-password "OLD_PASSWORD" \
    --new-password "NEW_PASSWORD"
```

---

#### Problem 5: Peer Connection Issues

**Symptoms:**
- Very few peers (< 5)
- Unstable connections
- Frequent disconnects

**Solutions:**

```bash
# Check current peer count
mbongo-cli node peers --rpc http://127.0.0.1:8545 | wc -l

# Check network configuration
grep -A10 "\[network\]" ~/.mbongo/config/config.toml

# Increase max_peers if needed
sed -i 's/max_peers = .*/max_peers = 100/' ~/.mbongo/config/config.toml

# Check NAT/router configuration
# Ensure ports 30303/tcp and 30303/udp are forwarded

# Use external IP if behind NAT
# Add to config.toml:
# external_ip = "YOUR_PUBLIC_IP"

# Check if ISP blocks P2P traffic
# Try using a VPN as a test

# Ban problematic peers
mbongo-cli node ban-peer --peer-id PEER_ID
```

---

#### Problem 6: Corrupted Database

**Symptoms:**
- Validator crashes on startup
- "Database corruption" errors
- Inconsistent state

**Solutions:**

```bash
# Stop validator
sudo systemctl stop mbongo-validator

# Backup corrupted database
mv ~/.mbongo/data ~/.mbongo/data_corrupted_$(date +%Y%m%d)

# Attempt database repair
mbongo-cli db repair --data-dir ~/.mbongo/data_corrupted_$(date +%Y%m%d)

# If repair fails, resync from scratch
mkdir -p ~/.mbongo/data
sudo systemctl start mbongo-validator

# Monitor sync progress
watch -n 5 'mbongo-cli node sync-status --rpc http://127.0.0.1:8545'
```

---

#### Problem 7: Outdated Client Version

**Symptoms:**
- Protocol version mismatch errors
- Unable to connect to network
- Consensus failures

**Solutions:**

```bash
# Check current version
mbongo-validator --version

# Check latest release
curl -s https://api.github.com/repos/mbongo-chain/mbongo-chain/releases/latest | jq -r '.tag_name'

# Stop validator
sudo systemctl stop mbongo-validator

# Download latest version
LATEST=$(curl -s https://api.github.com/repos/mbongo-chain/mbongo-chain/releases/latest | jq -r '.tag_name')
wget -O mbongo-validator.tar.gz \
    "https://github.com/mbongo-chain/mbongo-chain/releases/download/${LATEST}/mbongo-validator-linux-amd64.tar.gz"

# Verify and install
tar -xzf mbongo-validator.tar.gz
mv mbongo-validator ~/.mbongo/bin/
chmod +x ~/.mbongo/bin/mbongo-validator

# Restart validator
sudo systemctl start mbongo-validator

# Verify new version
mbongo-validator --version
```

---

#### Problem 8: Memory Spikes

**Symptoms:**
- OOM killer terminates process
- System becomes unresponsive
- Memory usage exceeds available RAM

**Solutions:**

```bash
# Check current memory usage
free -h
htop

# Add memory limits to systemd service
sudo tee -a /etc/systemd/system/mbongo-validator.service.d/memory.conf << 'EOF'
[Service]
MemoryMax=14G
MemoryHigh=12G
EOF

# Configure swap (if not present)
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile
echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab

# Reduce state cache in config
# Add to config.toml:
# [performance]
# state_cache_size = "4GB"

# Restart with limits
sudo systemctl daemon-reload
sudo systemctl restart mbongo-validator
```

---

#### Problem 9: Firewall Blocking P2P

**Symptoms:**
- Zero peers despite correct config
- Connection timeouts
- Network unreachable

**Solutions:**

```bash
# Check UFW rules
sudo ufw status verbose

# Ensure P2P ports are open
sudo ufw allow 30303/tcp
sudo ufw allow 30303/udp
sudo ufw reload

# Check iptables directly
sudo iptables -L -n | grep 30303

# Test external connectivity
nc -vz YOUR_SERVER_IP 30303

# Check cloud provider firewall (AWS/GCP/Azure)
# Security groups must also allow 30303/tcp and 30303/udp

# Check if running in Docker (additional port mapping needed)
docker ps
# Map ports: -p 30303:30303/tcp -p 30303:30303/udp
```

---

#### Problem 10: Log Rotation Setup

**Symptoms:**
- Disk filling up with logs
- Old logs consuming space
- Performance degradation

**Solutions:**

```bash
# Create logrotate configuration
sudo tee /etc/logrotate.d/mbongo-validator << 'EOF'
/home/*/.mbongo/logs/*.log {
    daily
    rotate 14
    compress
    delaycompress
    missingok
    notifempty
    create 640 root root
    sharedscripts
    postrotate
        systemctl reload mbongo-validator > /dev/null 2>&1 || true
    endscript
}
EOF

# Test logrotate config
sudo logrotate -d /etc/logrotate.d/mbongo-validator

# Force rotation (testing)
sudo logrotate -f /etc/logrotate.d/mbongo-validator

# Alternative: Use systemd journal
# Add to config.toml:
# log_to_journald = true

# View journald logs with size limits
sudo journalctl --vacuum-size=1G
sudo journalctl --vacuum-time=14d

# Check log directory size
du -sh ~/.mbongo/logs/
```

---

## 9. Cross-References

### Related Documentation

| Document | Description | Link |
|----------|-------------|------|
| **Node Setup Overview** | General node setup guide | [node_setup_overview.md](./node_setup_overview.md) |
| **Compute Provider Setup** | PoUW compute node setup | [compute_provider_setup.md](./compute_provider_setup.md) |
| **CLI Wallet Guide** | Wallet management commands | [cli_wallet.md](./cli_wallet.md) |
| **CLI Validator Guide** | Validator-specific CLI | [cli_validator.md](./cli_validator.md) |
| **SDK Documentation** | Developer SDK reference | [sdk/](./sdk/) |
| **Architecture Overview** | System architecture | [architecture_master_overview.md](./architecture_master_overview.md) |

### External Resources

- **Mbongo Chain GitHub**: `https://github.com/mbongo-chain/mbongo-chain`
- **Block Explorer**: `https://explorer.mbongo.network`
- **Testnet Faucet**: `https://faucet.testnet.mbongo.network`
- **Community Discord**: `https://discord.gg/mbongo`
- **Documentation Portal**: `https://docs.mbongo.network`

### Quick Command Reference

```bash
# Validator Management
mbongo-cli validator status           # Check validator status
mbongo-cli validator list-keys        # List imported keys
mbongo-cli validator import-keystore  # Import new keystore
mbongo-cli validator exit             # Initiate voluntary exit

# Node Operations
mbongo-cli node sync-status           # Check sync status
mbongo-cli node peers                 # List connected peers
mbongo-cli node version               # Check client version

# Key Management
mbongo-cli validator keygen           # Generate new keys
mbongo-cli validator encrypt-keystore # Encrypt keystore
mbongo-cli validator slashing-protection export  # Backup slashing DB

# Diagnostics
mbongo-cli debug logs                 # View recent logs
mbongo-cli debug metrics              # View performance metrics
mbongo-cli debug config               # Validate configuration
```

---

## Appendix: Quick Start Checklist

Use this checklist to ensure your validator is properly set up:

### Pre-Installation
- [ ] Hardware meets minimum requirements
- [ ] Ubuntu 22.04 LTS or Windows 10/11 installed
- [ ] Stable internet connection (50+ Mbps)
- [ ] Static IP or dynamic DNS configured

### Installation
- [ ] Dependencies installed (Rust, build tools)
- [ ] Mbongo validator client downloaded
- [ ] Binary added to PATH
- [ ] Directory structure created

### Key Management
- [ ] Keys generated securely (air-gapped preferred)
- [ ] Keystore encrypted with strong password
- [ ] Withdrawal key stored offline
- [ ] 3-2-1 backup strategy implemented
- [ ] Slashing protection database initialized

### Configuration
- [ ] config.toml created and customized
- [ ] Fee recipient address set
- [ ] Network (mainnet/testnet) specified
- [ ] Bootnodes configured

### Security
- [ ] Firewall configured (UFW/Windows Firewall)
- [ ] RPC ports restricted to localhost
- [ ] SSH hardened (key-only auth)
- [ ] System updates automated

### Operations
- [ ] Systemd service (Ubuntu) or NSSM service (Windows) created
- [ ] Service enabled for auto-start
- [ ] Log rotation configured
- [ ] Monitoring and alerting set up

### Final Verification
- [ ] Validator syncing with network
- [ ] Peers connected (10+ recommended)
- [ ] Attestations being made
- [ ] No errors in logs

---

> **🎉 Congratulations!** You've completed the Mbongo Chain Validator Setup Guide.  
> Your validator should now be operational and securing the network.
>
> **Questions?** Join our Discord community for support.

---

*Document maintained by the Mbongo Chain Core Team*  
*Last reviewed: November 2025*
