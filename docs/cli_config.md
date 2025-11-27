# Mbongo Chain — CLI Configuration Commands

> **Document Type:** CLI Reference  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Parent:** [cli_overview.md](./cli_overview.md)

---

## Table of Contents

1. [Purpose of Configuration Commands](#1-purpose-of-configuration-commands)
2. [Command Structure](#2-command-structure)
3. [Detailed Command Documentation](#3-detailed-command-documentation)
4. [Configuration File Layout](#4-configuration-file-layout)
5. [Best Practices & Security](#5-best-practices--security)
6. [ASCII Diagrams](#6-ascii-diagrams)
7. [Cross-Links](#7-cross-links)

---

## 1. Purpose of Configuration Commands

### 1.1 What Configuration Files Control

The `mbongo config` commands manage all runtime configuration for Mbongo Chain nodes. Configuration determines how your node operates, connects to the network, and participates in consensus.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONFIGURATION DOMAINS                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   NETWORK                               RPC                                 │
│   ═══════                               ═══                                 │
│   • Chain ID and network selection      • API endpoints                    │
│   • P2P listen addresses                • Port bindings                    │
│   • Bootnode connections                • CORS settings                    │
│   • Peer limits                         • Rate limiting                    │
│   • Gossip parameters                   • Authentication                   │
│                                                                             │
│   LOGGING                               KEYS                                │
│   ═══════                               ════                                │
│   • Log level (error→trace)             • Keystore paths                   │
│   • Output format (text/json)           • Validator key references         │
│   • File rotation                       • Node identity                    │
│   • Module-specific levels              • Session key config               │
│                                                                             │
│   PoS MODULE                            PoUW MODULE                         │
│   ══════════                            ═══════════                         │
│   • Validator settings                  • GPU provider config              │
│   • Staking parameters                  • Compute limits                   │
│   • Slashing protection                 • Task queue settings              │
│   • Attestation timing                  • Receipt submission               │
│                                                                             │
│   GAS & FEES                            STORAGE                             │
│   ══════════                            ═══════                             │
│   • Price strategy                      • Database backend                 │
│   • Priority defaults                   • Cache sizes                      │
│   • Mempool limits                      • Pruning settings                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Configuration Profiles

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PROFILES                                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   PROFILE     │ CHAIN ID │ USE CASE              │ SAFETY                  │
│   ────────────┼──────────┼───────────────────────┼─────────────────────────│
│   dev         │ 1337     │ Local development     │ Unsafe defaults OK      │
│   testnet     │ 5        │ Public testing        │ Test MBO only           │
│   mainnet     │ 1        │ Production            │ Real MBO at risk        │
│                                                                             │
│   PROFILE SELECTION                                                         │
│   ═════════════════                                                         │
│                                                                             │
│   # Initialize with profile                                                │
│   $ mbongo config init --profile mainnet                                   │
│                                                                             │
│   # Switch profile                                                         │
│   $ mbongo config set network.profile testnet                              │
│                                                                             │
│   ⚠️  WARNING: Never mix profiles. Use separate data directories.          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Security Implications

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║   ⚠️  MISCONFIGURATION RISKS                                                ║
║                                                                             ║
║   CRITICAL                                                                  ║
║   ────────                                                                  ║
║   • Wrong chain_id → Replay attacks possible                               ║
║   • RPC exposed to 0.0.0.0 → Remote exploitation                           ║
║   • Unencrypted keys → Theft of funds                                      ║
║   • Wrong validator key → Slashing                                         ║
║                                                                             ║
║   HIGH                                                                      ║
║   ────                                                                      ║
║   • Incorrect bootnodes → Network isolation                                ║
║   • Bad gas config → Transaction failures                                  ║
║   • Logging too verbose → Disk exhaustion                                  ║
║                                                                             ║
║   MEDIUM                                                                    ║
║   ──────                                                                    ║
║   • Suboptimal peer limits → Poor performance                              ║
║   • Wrong compute limits → Missed PoUW rewards                             ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

---

## 2. Command Structure

### 2.1 Syntax

```
mbongo config <command> [subcommand] [flags]
```

### 2.2 Subcommands

| Command | Description | Modifies Config |
|---------|-------------|-----------------|
| `show` | Display current configuration | No |
| `init` | Initialize configuration files | Yes |
| `set` | Set a configuration value | Yes |
| `get` | Get a configuration value | No |
| `reset` | Reset to defaults | Yes |
| `network` | Network configuration | Yes |
| `rpc` | RPC server configuration | Yes |
| `keys` | Key path configuration | Yes |
| `staking` | PoS staking configuration | Yes |
| `pouw` | PoUW compute configuration | Yes |
| `logging` | Logging configuration | Yes |

---

## 3. Detailed Command Documentation

### 3.1 `mbongo config show`

**Description:** Display the current configuration.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--config-dir` | `-c` | No | `~/.mbongo` | Config directory |
| `--output` | `-o` | No | `yaml` | Output format (yaml, json, toml) |
| `--section` | `-s` | No | `all` | Show specific section |
| `--redact` | | No | `true` | Hide sensitive values |

**Examples:**

```bash
# Show all config
mbongo config show

# Show specific section
mbongo config show --section network

# JSON output
mbongo config show --output json

# Show without redaction (careful!)
mbongo config show --redact=false
```

**Output:**

```yaml
# Mbongo Chain Configuration
# Profile: mainnet

node:
  id: "16Uiu2HAmXyz..."
  data_dir: "/var/lib/mbongo"

network:
  chain_id: 1
  profile: "mainnet"
  listen_addr: "/ip4/0.0.0.0/tcp/30303"
  bootnodes:
    - "/ip4/1.2.3.4/tcp/30303/p2p/16Uiu2HAm..."

rpc:
  enabled: true
  addr: "127.0.0.1"
  port: 8545
  cors: ["http://localhost:*"]

logging:
  level: "info"
  format: "text"

staking:
  enabled: true
  validator_key: "[REDACTED]"

pouw:
  enabled: false
```

**Error Cases:**

| Error | Exit Code | Cause |
|-------|-----------|-------|
| `ConfigNotFound` | 2 | No config files exist |
| `ParseError` | 2 | Malformed config file |

---

### 3.2 `mbongo config init`

**Description:** Initialize configuration files with sensible defaults.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--config-dir` | `-c` | No | `~/.mbongo` | Config directory |
| `--profile` | `-p` | No | `mainnet` | Profile (dev, testnet, mainnet) |
| `--force` | `-f` | No | `false` | Overwrite existing config |
| `--interactive` | `-i` | No | `false` | Interactive setup wizard |

**Examples:**

```bash
# Basic initialization
mbongo config init

# Testnet profile
mbongo config init --profile testnet

# Interactive wizard
mbongo config init --interactive

# Force overwrite
mbongo config init --force
```

**Output:**

```
Initializing Mbongo configuration...

Profile: mainnet
Directory: /home/user/.mbongo/config

Creating configuration files:
  ✓ node.yaml
  ✓ network.yaml
  ✓ rpc.yaml
  ✓ logging.yaml
  ✓ staking.yaml
  ✓ pouw.yaml

Configuration initialized successfully.

Next steps:
  1. Review configuration: mbongo config show
  2. Set validator key: mbongo config keys --validator-key <path>
  3. Start node: mbongo node start
```

**Error Cases:**

| Error | Exit Code | Cause |
|-------|-----------|-------|
| `ConfigExists` | 9 | Config already exists (use --force) |
| `PermissionDenied` | 4 | Cannot write to directory |
| `InvalidProfile` | 1 | Unknown profile name |

---

### 3.3 `mbongo config set`

**Description:** Set a configuration value.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `<key>` | | Yes | — | Configuration key (dot notation) |
| `<value>` | | Yes | — | Value to set |
| `--config-dir` | `-c` | No | `~/.mbongo` | Config directory |

**Examples:**

```bash
# Set single value
mbongo config set network.chain_id 1

# Set string value
mbongo config set rpc.addr "127.0.0.1"

# Set boolean
mbongo config set rpc.enabled true

# Set nested value
mbongo config set logging.modules.consensus "debug"

# Set array (JSON format)
mbongo config set network.bootnodes '["node1", "node2"]'
```

**Output:**

```
Configuration updated:
  network.chain_id: 1

Restart node for changes to take effect.
```

**Error Cases:**

| Error | Exit Code | Cause |
|-------|-----------|-------|
| `InvalidKey` | 1 | Unknown configuration key |
| `InvalidValue` | 1 | Value type mismatch |
| `ValidationFailed` | 1 | Value out of allowed range |

---

### 3.4 `mbongo config get`

**Description:** Get a configuration value.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `<key>` | | Yes | — | Configuration key |
| `--output` | `-o` | No | `raw` | Output format |

**Examples:**

```bash
mbongo config get network.chain_id
# Output: 1

mbongo config get rpc.port
# Output: 8545

mbongo config get logging --output json
# Output: {"level": "info", "format": "text", ...}
```

---

### 3.5 `mbongo config reset`

**Description:** Reset configuration to defaults.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--section` | `-s` | No | `all` | Section to reset |
| `--profile` | `-p` | No | (current) | Target profile |
| `--yes` | `-y` | No | `false` | Skip confirmation |

**Examples:**

```bash
# Reset all
mbongo config reset

# Reset specific section
mbongo config reset --section logging

# Non-interactive
mbongo config reset --section rpc --yes
```

---

### 3.6 `mbongo config network`

**Description:** Configure network settings.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--chain-id` | | No | — | Set chain ID |
| `--listen` | | No | — | P2P listen address |
| `--bootnodes` | | No | — | Bootstrap nodes |
| `--max-peers` | | No | `50` | Maximum peers |
| `--show` | | No | `false` | Show current settings |

**Examples:**

```bash
# Show network config
mbongo config network --show

# Set chain ID
mbongo config network --chain-id 1

# Set bootnodes
mbongo config network --bootnodes "/ip4/1.2.3.4/tcp/30303/p2p/..."

# Set max peers
mbongo config network --max-peers 100
```

**Output (--show):**

```
Network Configuration
────────────────────────────────────────────────────────────────────
  Chain ID:      1 (mainnet)
  Listen:        /ip4/0.0.0.0/tcp/30303
  Max Peers:     50
  Bootnodes:     3 configured
────────────────────────────────────────────────────────────────────
```

**Error Cases:**

| Error | Exit Code | Cause |
|-------|-----------|-------|
| `InvalidChainId` | 1 | Chain ID must be positive integer |
| `InvalidMultiaddr` | 1 | Malformed listen address |

---

### 3.7 `mbongo config rpc`

**Description:** Configure RPC server settings.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--enable` | | No | — | Enable RPC |
| `--disable` | | No | — | Disable RPC |
| `--addr` | | No | `127.0.0.1` | Bind address |
| `--port` | | No | `8545` | Port number |
| `--cors` | | No | — | CORS origins |
| `--methods` | | No | — | Allowed methods |
| `--show` | | No | `false` | Show current settings |

**Examples:**

```bash
# Show RPC config
mbongo config rpc --show

# Enable with defaults
mbongo config rpc --enable

# Custom port
mbongo config rpc --enable --port 8546

# Set CORS
mbongo config rpc --cors "http://localhost:3000,https://app.example.com"

# Restrict methods
mbongo config rpc --methods "eth_blockNumber,eth_getBalance"
```

**Output (--show):**

```
RPC Configuration
────────────────────────────────────────────────────────────────────
  Enabled:       true
  Address:       127.0.0.1:8545
  CORS:          http://localhost:*
  Rate Limit:    1000 req/min
  Methods:       all
  WebSocket:     enabled (ws://127.0.0.1:8546)
────────────────────────────────────────────────────────────────────
```

---

### 3.8 `mbongo config keys`

**Description:** Configure key paths and references.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--keystore` | | No | — | Keystore directory |
| `--validator-key` | | No | — | Validator key path |
| `--node-key` | | No | — | Node identity key |
| `--show` | | No | `false` | Show current settings |

**Examples:**

```bash
# Show key config
mbongo config keys --show

# Set keystore path
mbongo config keys --keystore /secure/keystore

# Set validator key
mbongo config keys --validator-key /secure/validator.json
```

**Output (--show):**

```
Keys Configuration
────────────────────────────────────────────────────────────────────
  Keystore:       /home/user/.mbongo/keystore
  Validator Key:  /home/user/.mbongo/keystore/validator.json
  Node Key:       /home/user/.mbongo/keystore/node.key
  Session Keys:   0 configured
────────────────────────────────────────────────────────────────────
```

---

### 3.9 `mbongo config staking`

**Description:** Configure PoS staking settings.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--enable` | | No | — | Enable staking module |
| `--disable` | | No | — | Disable staking module |
| `--validator-key` | | No | — | Validator key path |
| `--fee-recipient` | | No | — | Fee recipient address |
| `--graffiti` | | No | — | Block graffiti |
| `--show` | | No | `false` | Show current settings |

**Examples:**

```bash
# Show staking config
mbongo config staking --show

# Enable as validator
mbongo config staking --enable --validator-key /path/to/key.json

# Set fee recipient
mbongo config staking --fee-recipient 0x742d35Cc...

# Set graffiti
mbongo config staking --graffiti "MyValidator"
```

**Output (--show):**

```
Staking Configuration
────────────────────────────────────────────────────────────────────
  Enabled:         true
  Mode:            Validator
  Validator Key:   /secure/validator.json
  Fee Recipient:   0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7
  Graffiti:        "MbongoValidator"
  Slashing DB:     /var/lib/mbongo/slashing_protection.sqlite
────────────────────────────────────────────────────────────────────
```

---

### 3.10 `mbongo config pouw`

**Description:** Configure PoUW compute provider settings.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--enable` | | No | — | Enable PoUW module |
| `--disable` | | No | — | Disable PoUW module |
| `--gpu-device` | | No | `0` | GPU device ID(s) |
| `--max-tasks` | | No | `10` | Max concurrent tasks |
| `--memory-limit` | | No | `80%` | GPU memory limit |
| `--provider-key` | | No | — | Provider key path |
| `--show` | | No | `false` | Show current settings |

**Examples:**

```bash
# Show PoUW config
mbongo config pouw --show

# Enable with defaults
mbongo config pouw --enable

# Multiple GPUs
mbongo config pouw --enable --gpu-device "0,1,2,3"

# Set limits
mbongo config pouw --max-tasks 20 --memory-limit "90%"
```

**Output (--show):**

```
PoUW Configuration
────────────────────────────────────────────────────────────────────
  Enabled:         true
  GPU Devices:     0, 1 (2 total)
  Max Tasks:       10
  Memory Limit:    80%
  Provider Key:    /secure/provider.json
  Task Types:      inference, rendering, zk-proof
────────────────────────────────────────────────────────────────────
```

---

### 3.11 `mbongo config logging`

**Description:** Configure logging settings.

**Flags:**

| Flag | Short | Required | Default | Description |
|------|-------|----------|---------|-------------|
| `--level` | `-l` | No | `info` | Log level |
| `--format` | | No | `text` | Format (text, json) |
| `--file` | | No | — | Log file path |
| `--module` | | No | — | Module-specific level |
| `--show` | | No | `false` | Show current settings |

**Examples:**

```bash
# Show logging config
mbongo config logging --show

# Set global level
mbongo config logging --level debug

# JSON format
mbongo config logging --format json

# Module-specific
mbongo config logging --module "consensus=debug,network=warn"

# Log to file
mbongo config logging --file /var/log/mbongo/node.log
```

---

## 4. Configuration File Layout

### 4.1 Directory Structure

```
~/.mbongo/
├── config/
│   ├── node.yaml          # Core node settings
│   ├── network.yaml       # Network configuration
│   ├── rpc.yaml           # RPC server settings
│   ├── logging.yaml       # Logging configuration
│   ├── staking.yaml       # PoS staking settings
│   └── pouw.yaml          # PoUW compute settings
├── keystore/              # Encrypted keys
├── data/                  # Blockchain data
└── logs/                  # Log files
```

### 4.2 Complete Configuration Reference

#### `node.yaml`

```yaml
# Node Configuration
# Profile: mainnet

node:
  # Unique node identifier (auto-generated)
  id: "16Uiu2HAmXyz..."
  
  # Data directory for blockchain storage
  data_dir: "/var/lib/mbongo"
  
  # Node name (for identification)
  name: "my-validator-node"
  
  # Node role: full, validator, compute, hybrid
  role: "validator"

# Database settings
database:
  # Backend: rocksdb, leveldb
  backend: "rocksdb"
  
  # Cache size in MB
  cache_size: 512
  
  # Enable compression
  compression: true

# Mempool settings
mempool:
  # Maximum transactions
  max_txs: 10000
  
  # Transaction TTL (seconds)
  ttl: 3600
  
  # Rebroadcast interval
  rebroadcast: 60
```

#### `network.yaml`

```yaml
# Network Configuration

network:
  # Chain identifier (CRITICAL - must match network)
  # mainnet: 1, testnet: 5, devnet: 1337
  chain_id: 1
  
  # Network profile
  profile: "mainnet"
  
  # P2P listen address
  listen_addr: "/ip4/0.0.0.0/tcp/30303"
  
  # External address (for NAT traversal)
  external_addr: null  # auto-detect
  
  # Bootstrap nodes
  bootnodes:
    - "/ip4/1.2.3.4/tcp/30303/p2p/16Uiu2HAm..."
    - "/ip4/5.6.7.8/tcp/30303/p2p/16Uiu2HAm..."
  
  # Peer limits
  max_peers: 50
  min_peers: 10
  
  # Reserved peers (always connect)
  reserved_peers: []
  
  # Gossip settings
  gossip:
    mesh_n: 8
    mesh_n_low: 6
    mesh_n_high: 12
    heartbeat_interval: 1000  # ms
```

#### `rpc.yaml`

```yaml
# RPC Configuration

rpc:
  # Enable RPC server
  enabled: true
  
  # HTTP settings
  http:
    addr: "127.0.0.1"
    port: 8545
    cors: ["http://localhost:*"]
    max_connections: 100
  
  # WebSocket settings
  ws:
    enabled: true
    addr: "127.0.0.1"
    port: 8546
    max_connections: 50
  
  # Rate limiting
  rate_limit:
    enabled: true
    requests_per_minute: 1000
    burst: 100
  
  # Method filtering
  methods:
    # Allowed method namespaces
    namespaces: ["eth", "net", "web3", "mbongo"]
    # Explicitly disabled methods
    disabled: ["eth_sendTransaction"]  # Use personal_ or external signer
```

#### `logging.yaml`

```yaml
# Logging Configuration

logging:
  # Global log level: error, warn, info, debug, trace
  level: "info"
  
  # Output format: text, json
  format: "text"
  
  # Output targets
  output:
    console: true
    file:
      enabled: true
      path: "/var/log/mbongo/node.log"
      rotation:
        max_size: "100MB"
        max_age: "7d"
        max_backups: 5
  
  # Module-specific levels
  modules:
    consensus: "info"
    network: "warn"
    rpc: "info"
    execution: "info"
    pouw: "debug"
```

#### `staking.yaml`

```yaml
# Staking (PoS) Configuration

staking:
  # Enable validator mode
  enabled: true
  
  # Validator key (path to encrypted keystore)
  validator_key: "/secure/keystore/validator.json"
  
  # Fee recipient address
  fee_recipient: "0x742d35Cc6634C0532925a3b844Bc9e7595f8bDe7"
  
  # Block graffiti (max 32 bytes)
  graffiti: "MbongoValidator"
  
  # Slashing protection database
  slashing_protection_db: "/var/lib/mbongo/slashing_protection.sqlite"
  
  # Attestation settings
  attestation:
    # Delay before attesting (ms)
    delay: 500
    # Retry on failure
    max_retries: 3
  
  # Builder API (MEV) - optional
  builder:
    enabled: false
    endpoint: null
```

#### `pouw.yaml`

```yaml
# PoUW (Proof-of-Useful-Work) Configuration

pouw:
  # Enable GPU compute provider
  enabled: true
  
  # Provider key (path to encrypted keystore)
  provider_key: "/secure/keystore/provider.json"
  
  # GPU configuration
  gpu:
    # Device IDs (comma-separated for multiple)
    devices: [0, 1]
    # Memory limit per device (percentage or absolute)
    memory_limit: "80%"
    # Power limit (watts, null for no limit)
    power_limit: null
  
  # Task settings
  tasks:
    # Maximum concurrent tasks
    max_concurrent: 10
    # Task types to accept
    types: ["inference", "rendering", "zk-proof", "training"]
    # Minimum reward threshold (MBO)
    min_reward: "0.001"
  
  # Compute limits
  compute_limits:
    # Max FLOPS per task
    max_flops: 1000000000000
    # Max memory per task
    max_memory: "8GB"
    # Max execution time (seconds)
    max_time: 3600
  
  # Receipt submission
  receipts:
    # Batch size
    batch_size: 10
    # Submit interval (seconds)
    interval: 5
```

### 4.3 Gas Configuration

```yaml
# In node.yaml

gas:
  # Price strategy: static, oracle, adaptive
  price_strategy: "adaptive"
  
  # Static price (if strategy is static)
  static_price: "20gwei"
  
  # Minimum gas price
  min_price: "1gwei"
  
  # Maximum gas price
  max_price: "1000gwei"
  
  # Priority fee defaults
  priority_fee:
    low: "1gwei"
    medium: "2gwei"
    high: "5gwei"
```

---

## 5. Best Practices & Security

### 5.1 Protect Configuration Files

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         FILE PERMISSIONS                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   RECOMMENDED PERMISSIONS                                                   │
│   ═══════════════════════                                                   │
│                                                                             │
│   # Set ownership                                                          │
│   $ sudo chown -R mbongo:mbongo ~/.mbongo                                  │
│                                                                             │
│   # Config files: readable by owner only                                   │
│   $ chmod 600 ~/.mbongo/config/*.yaml                                      │
│                                                                             │
│   # Keystore: very restrictive                                             │
│   $ chmod 700 ~/.mbongo/keystore                                           │
│   $ chmod 600 ~/.mbongo/keystore/*                                         │
│                                                                             │
│   # Data directory                                                         │
│   $ chmod 750 ~/.mbongo/data                                               │
│                                                                             │
│   VERIFICATION                                                              │
│   ════════════                                                              │
│   $ ls -la ~/.mbongo/config/                                               │
│   -rw------- 1 mbongo mbongo 1234 Nov 27 10:00 node.yaml                   │
│   -rw------- 1 mbongo mbongo  567 Nov 27 10:00 staking.yaml                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Never Store Private Keys Unencrypted

```
╔═════════════════════════════════════════════════════════════════════════════╗
║                                                                             ║
║   ⚠️  CRITICAL: KEY STORAGE RULES                                           ║
║                                                                             ║
║   ✗ NEVER store raw private keys in config files                           ║
║   ✗ NEVER use environment variables for private keys                       ║
║   ✗ NEVER commit keys to version control                                   ║
║                                                                             ║
║   ✓ Always use encrypted keystore files                                    ║
║   ✓ Reference keys by PATH, not content                                    ║
║   ✓ Use hardware security modules (HSM) for production                     ║
║                                                                             ║
║   CORRECT (reference path):                                                 ║
║   validator_key: "/secure/keystore/validator.json"                         ║
║                                                                             ║
║   WRONG (raw key):                                                          ║
║   validator_key: "0xac0974bec39a17e36ba4a6b4d238ff944..."                  ║
║                                                                             ║
╚═════════════════════════════════════════════════════════════════════════════╝
```

### 5.3 Validate Chain ID

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CHAIN ID VALIDATION                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   BEFORE STARTING PRODUCTION NODE                                           │
│   ═══════════════════════════════                                           │
│                                                                             │
│   # Always verify chain_id matches your intended network                   │
│   $ mbongo config get network.chain_id                                     │
│                                                                             │
│   Expected values:                                                         │
│   • mainnet: 1                                                             │
│   • testnet: 5                                                             │
│   • devnet: 1337                                                           │
│                                                                             │
│   AUTOMATIC VALIDATION                                                      │
│   ════════════════════                                                      │
│                                                                             │
│   # Enable chain_id validation on startup                                  │
│   $ mbongo config set node.validate_chain_id true                          │
│                                                                             │
│   Node will refuse to start if:                                            │
│   • chain_id doesn't match connected peers                                 │
│   • chain_id doesn't match genesis block                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.4 Use Profiles to Separate Networks

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PROFILE SEPARATION                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   RECOMMENDED SETUP                                                         │
│   ═════════════════                                                         │
│                                                                             │
│   # Mainnet (production)                                                   │
│   ~/.mbongo-mainnet/                                                       │
│   $ mbongo --data-dir ~/.mbongo-mainnet node start                         │
│                                                                             │
│   # Testnet (testing)                                                      │
│   ~/.mbongo-testnet/                                                       │
│   $ mbongo --data-dir ~/.mbongo-testnet node start                         │
│                                                                             │
│   # Devnet (development)                                                   │
│   ~/.mbongo-dev/                                                           │
│   $ mbongo --data-dir ~/.mbongo-dev node start                             │
│                                                                             │
│   USE ALIASES                                                               │
│   ═══════════                                                               │
│                                                                             │
│   # Add to ~/.bashrc                                                       │
│   alias mbongo-main='mbongo --data-dir ~/.mbongo-mainnet'                  │
│   alias mbongo-test='mbongo --data-dir ~/.mbongo-testnet'                  │
│   alias mbongo-dev='mbongo --data-dir ~/.mbongo-dev'                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. ASCII Diagrams

### 6.1 Configuration Initialization Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONFIG INITIALIZATION                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   $ mbongo config init --profile mainnet                                   │
│         │                                                                   │
│         ▼                                                                   │
│   ┌───────────────┐    ┌───────────────┐    ┌───────────────┐              │
│   │ Check Exists  │───▶│ Load Profile  │───▶│ Create Dir    │              │
│   │               │    │ Defaults      │    │ Structure     │              │
│   └───────────────┘    └───────────────┘    └───────────────┘              │
│         │                                          │                        │
│         │ exists?                                  │                        │
│         ▼                                          ▼                        │
│   ┌───────────────┐                        ┌───────────────┐              │
│   │ --force?      │                        │ Generate      │              │
│   │ Abort/Overwrite                        │ Node ID       │              │
│   └───────────────┘                        └───────────────┘              │
│                                                   │                        │
│                                                   ▼                        │
│   ┌─────────────────────────────────────────────────────────────────────┐ │
│   │                                                                     │ │
│   │   Write Configuration Files                                         │ │
│   │                                                                     │ │
│   │   config/                                                           │ │
│   │   ├── node.yaml      ← node.id, data_dir, role                     │ │
│   │   ├── network.yaml   ← chain_id, bootnodes, peers                  │ │
│   │   ├── rpc.yaml       ← ports, cors, rate_limit                     │ │
│   │   ├── logging.yaml   ← level, format, modules                      │ │
│   │   ├── staking.yaml   ← enabled=false (manual setup)                │ │
│   │   └── pouw.yaml      ← enabled=false (manual setup)                │ │
│   │                                                                     │ │
│   └─────────────────────────────────────────────────────────────────────┘ │
│                                                   │                        │
│                                                   ▼                        │
│                                          ┌───────────────┐                │
│                                          │ SUCCESS       │                │
│                                          │ Ready to run  │                │
│                                          └───────────────┘                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Module Configuration Loading

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONFIG LOADING FLOW                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   $ mbongo node start                                                      │
│         │                                                                   │
│         ▼                                                                   │
│   ┌───────────────────────────────────────────────────────────────────┐   │
│   │                     CONFIG LOADER                                  │   │
│   │                                                                     │   │
│   │   1. CLI flags (highest priority)                                  │   │
│   │   2. Environment variables                                         │   │
│   │   3. Config files                                                  │   │
│   │   4. Built-in defaults (lowest priority)                           │   │
│   │                                                                     │   │
│   └───────────────────────────────────────────────────────────────────┘   │
│         │                                                                   │
│         ▼                                                                   │
│   ┌─────────────────────────────────────────────────────────────────────┐ │
│   │                                                                     │ │
│   │   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │ │
│   │   │ Network │ │   RPC   │ │ Staking │ │  PoUW   │ │ Logging │     │ │
│   │   │ Module  │ │ Module  │ │ Module  │ │ Module  │ │ Module  │     │ │
│   │   └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘     │ │
│   │        │           │           │           │           │           │ │
│   │        ▼           ▼           ▼           ▼           ▼           │ │
│   │   ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐     │ │
│   │   │network. │ │ rpc.    │ │staking. │ │ pouw.   │ │logging. │     │ │
│   │   │yaml     │ │ yaml    │ │yaml     │ │ yaml    │ │yaml     │     │ │
│   │   └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘     │ │
│   │                                                                     │ │
│   └─────────────────────────────────────────────────────────────────────┘ │
│         │                                                                   │
│         ▼                                                                   │
│   ┌───────────────┐                                                        │
│   │ VALIDATION    │──▶ Errors? ──▶ Exit with details                      │
│   │ (schema check)│                                                        │
│   └───────────────┘                                                        │
│         │                                                                   │
│         ▼                                                                   │
│   ┌───────────────┐                                                        │
│   │ NODE STARTS   │                                                        │
│   └───────────────┘                                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 PoS/PoUW Inter-Module Dependencies

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         MODULE DEPENDENCIES                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                           ┌─────────────────┐                              │
│                           │   node.yaml     │                              │
│                           │   (core)        │                              │
│                           └────────┬────────┘                              │
│                                    │                                        │
│                    ┌───────────────┼───────────────┐                       │
│                    │               │               │                       │
│                    ▼               ▼               ▼                       │
│           ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                 │
│           │ network.yaml│ │ logging.yaml│ │  rpc.yaml   │                 │
│           └──────┬──────┘ └─────────────┘ └──────┬──────┘                 │
│                  │                               │                         │
│                  │  chain_id, peers              │  endpoints              │
│                  │                               │                         │
│           ┌──────┴───────────────────────────────┴──────┐                 │
│           │                                             │                 │
│           ▼                                             ▼                 │
│   ┌─────────────────┐                         ┌─────────────────┐        │
│   │  staking.yaml   │◀────── depends ────────▶│   pouw.yaml     │        │
│   │     (PoS)       │                         │    (PoUW)       │        │
│   └────────┬────────┘                         └────────┬────────┘        │
│            │                                           │                  │
│            │                                           │                  │
│            │         SHARED DEPENDENCIES               │                  │
│            │         ════════════════════              │                  │
│            │                                           │                  │
│            │  • network.chain_id (both must match)     │                  │
│            │  • Keys from keystore                     │                  │
│            │  • Gas pricing (fee model)                │                  │
│            │  • Consensus timing                       │                  │
│            │                                           │                  │
│            ▼                                           ▼                  │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                                                                 │   │
│   │                      CONSENSUS ENGINE                           │   │
│   │                                                                 │   │
│   │   ┌───────────────────────┐    ┌───────────────────────┐       │   │
│   │   │ PoS Validator         │    │ PoUW Compute Provider │       │   │
│   │   │ (50% block reward)    │    │ (50% block reward)    │       │   │
│   │   └───────────────────────┘    └───────────────────────┘       │   │
│   │                                                                 │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Cross-Links

### Related Documentation

| Document | Description |
|----------|-------------|
| [cli_overview.md](./cli_overview.md) | CLI overview |
| [cli_node.md](./cli_node.md) | Node commands |
| [node_architecture.md](./node_architecture.md) | Node internals |
| [governance_model.md](./governance_model.md) | Governance |
| [fee_model.md](./fee_model.md) | Fee structure |
| [staking_model.md](./staking_model.md) | Staking |
| [economic_security.md](./economic_security.md) | Security |

### Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONFIG COMMANDS QUICK REFERENCE                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   BASICS                             MODULE CONFIG                          │
│   ──────                             ─────────────                          │
│   mbongo config show                 mbongo config network --show           │
│   mbongo config init                 mbongo config rpc --show               │
│   mbongo config set <key> <val>      mbongo config staking --show           │
│   mbongo config get <key>            mbongo config pouw --show              │
│   mbongo config reset                mbongo config logging --show           │
│                                                                             │
│   KEY PATHS                                                                 │
│   ─────────                                                                 │
│   network.chain_id        Node identity and network                        │
│   rpc.http.port           RPC server port                                  │
│   staking.validator_key   Validator key path                               │
│   pouw.gpu.devices        GPU device IDs                                   │
│   logging.level           Log verbosity                                    │
│                                                                             │
│   PROFILES                                                                  │
│   ────────                                                                  │
│   --profile dev           Local development (chain_id: 1337)               │
│   --profile testnet       Public testnet (chain_id: 5)                     │
│   --profile mainnet       Production (chain_id: 1)                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides the complete reference for `mbongo config` commands. For general CLI information, see [cli_overview.md](./cli_overview.md).*

