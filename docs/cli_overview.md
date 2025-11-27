# Mbongo Chain — CLI Overview

> **Document Type:** CLI Reference  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Audience:** Node Operators, Validators, Developers, DevOps Engineers

---

## Table of Contents

1. [Purpose of the CLI](#1-purpose-of-the-cli)
2. [Architecture Overview](#2-architecture-overview)
3. [Command Structure](#3-command-structure)
4. [Conventions](#4-conventions)
5. [Security Constraints](#5-security-constraints)
6. [Examples](#6-examples)
7. [Cross-Links](#7-cross-links)

---

## 1. Purpose of the CLI

### 1.1 What the Mbongo CLI Controls

The `mbongo` CLI is the primary interface for interacting with Mbongo Chain nodes. It provides comprehensive control over node operations, validator management, account handling, staking, networking, consensus monitoring, compute tasks, and developer utilities.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBONGO CLI CAPABILITIES                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   NODE OPERATIONS                           VALIDATOR MANAGEMENT                        │
│   ═══════════════                           ═════════════════════                       │
│   • Start / stop nodes                      • Register validator                        │
│   • Configure runtime parameters            • Update validator metadata                 │
│   • Monitor node health                     • View validator status                     │
│   • Manage peer connections                 • Monitor attestations                      │
│   • Sync state and blocks                   • Track rewards and slashing               │
│                                                                                         │
│   ACCOUNT & KEY MANAGEMENT                  STAKING OPERATIONS                          │
│   ════════════════════════                  ══════════════════                          │
│   • Generate keypairs                       • Stake MBO                                 │
│   • Import / export keys                    • Delegate to validators                    │
│   • Sign transactions                       • Unbond stake                              │
│   • Query balances                          • Withdraw rewards                          │
│   • Manage multiple accounts                • View staking positions                    │
│                                                                                         │
│   NETWORK & CONSENSUS                       COMPUTE (PoUW)                              │
│   ═════════════════════                     ══════════════                              │
│   • View peer list                          • Register compute provider                 │
│   • Monitor gossip                          • Submit compute proofs                     │
│   • Query consensus state                   • View compute receipts                     │
│   • Check finality status                   • Monitor GPU utilization                   │
│   • Inspect block proposals                 • Track compute rewards                     │
│                                                                                         │
│   DEVELOPER TOOLS                           DEBUGGING & DIAGNOSTICS                     │
│   ═══════════════════                       ═══════════════════════                     │
│   • Generate test fixtures                  • Inspect state trie                        │
│   • Benchmark operations                    • Replay transactions                       │
│   • Format and lint checks                  • Export debug dumps                        │
│   • Build and deploy contracts              • Profile performance                       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Node Type Relationships

The CLI interfaces with three primary node types, each with distinct responsibilities:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         NODE TYPE ARCHITECTURE                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                              ┌─────────────────────────────┐                           │
│                              │                             │                           │
│                              │        MBONGO CLI           │                           │
│                              │                             │                           │
│                              └─────────────┬───────────────┘                           │
│                                            │                                           │
│                    ┌───────────────────────┼───────────────────────┐                   │
│                    │                       │                       │                   │
│                    ▼                       ▼                       ▼                   │
│   ┌────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────┐    │
│   │                        │ │                        │ │                        │    │
│   │    VALIDATOR NODE      │ │    COMPUTE NODE        │ │      RPC SERVER        │    │
│   │        (PoS)           │ │       (PoUW)           │ │                        │    │
│   │                        │ │                        │ │                        │    │
│   ├────────────────────────┤ ├────────────────────────┤ ├────────────────────────┤    │
│   │                        │ │                        │ │                        │    │
│   │ • Block proposal       │ │ • GPU task execution   │ │ • External API access  │    │
│   │ • Attestation          │ │ • Compute receipts     │ │ • Query endpoints      │    │
│   │ • Consensus voting     │ │ • Proof generation     │ │ • Transaction submit   │    │
│   │ • State validation     │ │ • Work unit tracking   │ │ • WebSocket streams    │    │
│   │ • Slashing avoidance   │ │ • Attestation signing  │ │ • Health checks        │    │
│   │                        │ │                        │ │                        │    │
│   └────────────────────────┘ └────────────────────────┘ └────────────────────────┘    │
│                                                                                         │
│   RELATIONSHIP MATRIX                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   ┌────────────────────────────────────────────────────────────────────────────────┐   │
│   │                                                                                │   │
│   │   Node Type        │ Consensus │ Compute │ RPC   │ Staking │ CLI Control      │   │
│   │   ─────────────────┼───────────┼─────────┼───────┼─────────┼──────────────────│   │
│   │   Validator Node   │ Yes       │ No      │ Opt   │ Yes     │ Full             │   │
│   │   Compute Node     │ Partial   │ Yes     │ Opt   │ Opt     │ Full             │   │
│   │   RPC Server       │ No        │ No      │ Yes   │ No      │ Query only       │   │
│   │   Full Node        │ Verify    │ No      │ Opt   │ No      │ Full             │   │
│   │                                                                                │   │
│   └────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Architecture Overview

### 2.1 CLI → Runtime → Node API Flow

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMMAND EXECUTION FLOW                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   USER INPUT                                                                            │
│   ══════════                                                                            │
│                                                                                         │
│   $ mbongo validator status --validator-id 0x1234...                                   │
│                                                                                         │
│   ────────────────────────────────────────────────────────────────────────────────────  │
│                                                                                         │
│   EXECUTION PIPELINE                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│   │   PARSER    │    │  VALIDATOR  │    │  EXECUTOR   │    │  FORMATTER  │            │
│   │             │───▶│             │───▶│             │───▶│             │            │
│   │   (clap)    │    │   (args)    │    │   (async)   │    │   (output)  │            │
│   └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘            │
│         │                  │                  │                  │                     │
│         ▼                  ▼                  ▼                  ▼                     │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│   │ Parse CLI   │    │ Validate    │    │ Call Node   │    │ Format      │            │
│   │ arguments   │    │ inputs &    │    │ API or      │    │ response    │            │
│   │ & flags     │    │ permissions │    │ Runtime     │    │ for output  │            │
│   └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘            │
│                                                                                         │
│   ────────────────────────────────────────────────────────────────────────────────────  │
│                                                                                         │
│   INTERNAL ARCHITECTURE                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ┌───────────────────────────────────────────────────────────────────────────┐│  │
│   │   │                           CLI LAYER                                       ││  │
│   │   │                                                                           ││  │
│   │   │   • Argument parsing (clap)                                               ││  │
│   │   │   • Subcommand routing                                                    ││  │
│   │   │   • Input validation                                                      ││  │
│   │   │   • Configuration loading                                                 ││  │
│   │   │                                                                           ││  │
│   │   └───────────────────────────────────┬───────────────────────────────────────┘│  │
│   │                                       │                                         │  │
│   │                                       ▼                                         │  │
│   │   ┌───────────────────────────────────────────────────────────────────────────┐│  │
│   │   │                         RUNTIME LAYER                                     ││  │
│   │   │                                                                           ││  │
│   │   │   • Tokio async runtime                                                   ││  │
│   │   │   • Connection management                                                 ││  │
│   │   │   • Transaction building                                                  ││  │
│   │   │   • Signing operations                                                    ││  │
│   │   │                                                                           ││  │
│   │   └───────────────────────────────────┬───────────────────────────────────────┘│  │
│   │                                       │                                         │  │
│   │                                       ▼                                         │  │
│   │   ┌───────────────────────────────────────────────────────────────────────────┐│  │
│   │   │                         NODE API LAYER                                    ││  │
│   │   │                                                                           ││  │
│   │   │   • JSON-RPC client                                                       ││  │
│   │   │   • IPC socket                                                            ││  │
│   │   │   • Direct state access (local node)                                      ││  │
│   │   │   • WebSocket subscriptions                                               ││  │
│   │   │                                                                           ││  │
│   │   └───────────────────────────────────────────────────────────────────────────┘│  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Command Execution Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EXECUTION PIPELINE STAGES                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   STAGE 1: PARSE                                                                        │
│   ══════════════                                                                        │
│   • Parse command-line arguments using `clap`                                          │
│   • Extract subcommand, flags, and positional args                                     │
│   • Load environment variables and config files                                        │
│   • Merge configuration sources (CLI > ENV > Config > Default)                         │
│                                                                                         │
│   STAGE 2: VALIDATE                                                                     │
│   ═════════════════                                                                     │
│   • Check required arguments are present                                               │
│   • Validate argument formats (addresses, hashes, etc.)                                │
│   • Verify permissions and capabilities                                                │
│   • Check node connectivity (if required)                                              │
│                                                                                         │
│   STAGE 3: EXECUTE                                                                      │
│   ════════════════                                                                      │
│   • Build request/transaction                                                          │
│   • Sign if necessary (using keystore)                                                 │
│   • Send to node via appropriate channel                                               │
│   • Wait for response/confirmation                                                     │
│                                                                                         │
│   STAGE 4: FORMAT                                                                       │
│   ══════════════                                                                        │
│   • Parse response data                                                                │
│   • Apply output format (JSON, table, raw)                                             │
│   • Write to stdout/stderr                                                             │
│   • Return appropriate exit code                                                       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Error Handling Standard

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ERROR HANDLING                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ERROR CATEGORIES                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Category        │ Exit Code │ Description                                     │  │
│   │   ────────────────┼───────────┼─────────────────────────────────────────────────│  │
│   │   Success         │ 0         │ Command completed successfully                  │  │
│   │   InvalidArgs     │ 1         │ Invalid command-line arguments                  │  │
│   │   ConfigError     │ 2         │ Configuration file error                        │  │
│   │   ConnectionError │ 3         │ Cannot connect to node                          │  │
│   │   AuthError       │ 4         │ Authentication/permission denied                │  │
│   │   ExecutionError  │ 5         │ Command execution failed                        │  │
│   │   TimeoutError    │ 6         │ Operation timed out                             │  │
│   │   InternalError   │ 99        │ Unexpected internal error                       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ERROR OUTPUT FORMAT                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   Errors are written to stderr with consistent structure:                              │
│                                                                                         │
│   ```                                                                                   │
│   error[E0003]: connection refused                                                     │
│     --> node connection                                                                │
│     |                                                                                  │
│     = help: ensure the node is running with `mbongo node start`                       │
│     = note: tried to connect to http://127.0.0.1:8545                                 │
│   ```                                                                                   │
│                                                                                         │
│   RUST ERROR HANDLING                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   • Uses `thiserror` for error definitions                                             │
│   • Uses `anyhow` for error propagation                                                │
│   • Context added at each layer                                                        │
│   • Backtrace available with RUST_BACKTRACE=1                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Logging Patterns

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         LOGGING CONFIGURATION                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   LOG LEVELS                                                                            │
│   ══════════                                                                            │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Level   │ Flag        │ Usage                                                 │  │
│   │   ────────┼─────────────┼───────────────────────────────────────────────────────│  │
│   │   error   │ (default)   │ Critical errors only                                  │  │
│   │   warn    │ -v          │ Warnings and errors                                   │  │
│   │   info    │ -vv         │ Informational messages                                │  │
│   │   debug   │ -vvv        │ Debug output                                          │  │
│   │   trace   │ -vvvv       │ Trace-level (very verbose)                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ENVIRONMENT VARIABLES                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   MBONGO_LOG=debug              # Set log level                                        │
│   MBONGO_LOG_FORMAT=json        # Output format (text, json)                           │
│   MBONGO_LOG_FILE=/var/log/mbongo/cli.log  # Log file path                             │
│   RUST_LOG=mbongo=debug,tokio=warn         # Fine-grained control                      │
│                                                                                         │
│   LOG OUTPUT EXAMPLE                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   2025-11-27T10:23:45.123Z INFO  mbongo::cli::node > Starting node...                  │
│   2025-11-27T10:23:45.456Z DEBUG mbongo::runtime   > Loading config from config.toml   │
│   2025-11-27T10:23:46.789Z INFO  mbongo::network   > Connected to 12 peers             │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Command Structure

### 3.1 Global Syntax

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMMAND SYNTAX                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   GENERAL FORMAT                                                                        │
│   ══════════════                                                                        │
│                                                                                         │
│   mbongo <category> <command> [subcommand] [flags] [arguments]                         │
│                                                                                         │
│                                                                                         │
│   STRUCTURE BREAKDOWN                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   mbongo       validator    status      --validator-id    0x1234...            │  │
│   │   ───────      ─────────    ──────      ──────────────    ─────────            │  │
│   │   binary       category     command     flag              argument             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   EXAMPLES                                                                              │
│   ════════                                                                              │
│                                                                                         │
│   # Simple command                                                                     │
│   mbongo node start                                                                    │
│                                                                                         │
│   # With flags                                                                         │
│   mbongo node start --config ./config.toml --data-dir /var/mbongo                      │
│                                                                                         │
│   # With output format                                                                 │
│   mbongo account balance 0x1234... --output json                                       │
│                                                                                         │
│   # With verbosity                                                                     │
│   mbongo network peers -vv --output table                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Command Categories

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMMAND CATEGORIES                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Category   │ Description                    │ Key Commands                    │  │
│   │   ───────────┼────────────────────────────────┼─────────────────────────────────│  │
│   │              │                                │                                 │  │
│   │   node       │ Node lifecycle management      │ start, stop, status, sync,      │  │
│   │              │                                │ config, version, health         │  │
│   │              │                                │                                 │  │
│   │   validator  │ Validator operations           │ register, status, update,       │  │
│   │              │                                │ withdraw, exit, rewards         │  │
│   │              │                                │                                 │  │
│   │   account    │ Account & key management       │ create, import, export, list,   │  │
│   │              │                                │ balance, transfer, sign         │  │
│   │              │                                │                                 │  │
│   │   stake      │ Staking operations             │ deposit, delegate, unbond,      │  │
│   │              │                                │ withdraw, positions, rewards    │  │
│   │              │                                │                                 │  │
│   │   network    │ Network & peer management      │ peers, add-peer, ban-peer,      │  │
│   │              │                                │ gossip, topology, stats         │  │
│   │              │                                │                                 │  │
│   │   consensus  │ Consensus monitoring           │ state, block, finality,         │  │
│   │              │                                │ proposals, votes, epoch         │  │
│   │              │                                │                                 │  │
│   │   compute    │ PoUW compute operations        │ register, submit, receipts,     │  │
│   │              │                                │ tasks, status, rewards          │  │
│   │              │                                │                                 │  │
│   │   tools      │ Utility tools                  │ hash, encode, decode, verify,   │  │
│   │              │                                │ benchmark, convert              │  │
│   │              │                                │                                 │  │
│   │   dev        │ Developer utilities            │ init, build, test, deploy,      │  │
│   │              │                                │ debug, inspect, replay          │  │
│   │              │                                │                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Category Details

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: node                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo node <command>                                                                │
│                                                                                         │
│   Commands:                                                                            │
│   ├── start       Start the node                                                       │
│   ├── stop        Stop the running node                                                │
│   ├── restart     Restart the node                                                     │
│   ├── status      Show node status                                                     │
│   ├── sync        Sync state from network                                              │
│   ├── config      Show/edit configuration                                              │
│   ├── version     Show version information                                             │
│   ├── health      Health check endpoint                                                │
│   └── export      Export node data                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: validator                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo validator <command>                                                           │
│                                                                                         │
│   Commands:                                                                            │
│   ├── register    Register as validator                                                │
│   ├── status      Show validator status                                                │
│   ├── update      Update validator metadata                                            │
│   ├── exit        Begin validator exit                                                 │
│   ├── rewards     View accumulated rewards                                             │
│   ├── withdraw    Withdraw available rewards                                           │
│   ├── slashing    View slashing history                                                │
│   └── attestations View attestation history                                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: account                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo account <command>                                                             │
│                                                                                         │
│   Commands:                                                                            │
│   ├── create      Create new account                                                   │
│   ├── import      Import account from key                                              │
│   ├── export      Export account (encrypted)                                           │
│   ├── list        List all accounts                                                    │
│   ├── balance     Check account balance                                                │
│   ├── transfer    Transfer MBO                                                         │
│   ├── sign        Sign a message                                                       │
│   └── verify      Verify a signature                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: stake                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo stake <command>                                                               │
│                                                                                         │
│   Commands:                                                                            │
│   ├── deposit     Deposit stake                                                        │
│   ├── delegate    Delegate to validator                                                │
│   ├── unbond      Begin unbonding                                                      │
│   ├── withdraw    Withdraw unbonded stake                                              │
│   ├── positions   View staking positions                                               │
│   ├── rewards     View staking rewards                                                 │
│   └── history     View staking history                                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: network                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo network <command>                                                             │
│                                                                                         │
│   Commands:                                                                            │
│   ├── peers       List connected peers                                                 │
│   ├── add-peer    Manually add a peer                                                  │
│   ├── ban-peer    Ban a peer                                                           │
│   ├── gossip      View gossip statistics                                               │
│   ├── topology    Network topology map                                                 │
│   ├── stats       Network statistics                                                   │
│   └── discover    Trigger peer discovery                                               │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: consensus                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo consensus <command>                                                           │
│                                                                                         │
│   Commands:                                                                            │
│   ├── state       Current consensus state                                              │
│   ├── block       Query block by hash/height                                           │
│   ├── finality    Finality status                                                      │
│   ├── proposals   Recent proposals                                                     │
│   ├── votes       Vote statistics                                                      │
│   ├── epoch       Current epoch info                                                   │
│   └── validators  Active validator set                                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: compute                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo compute <command>                                                             │
│                                                                                         │
│   Commands:                                                                            │
│   ├── register    Register as compute provider                                         │
│   ├── submit      Submit compute proof                                                 │
│   ├── receipts    View compute receipts                                                │
│   ├── tasks       List available tasks                                                 │
│   ├── status      Provider status                                                      │
│   ├── rewards     View compute rewards                                                 │
│   └── benchmark   Run GPU benchmark                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: tools                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo tools <command>                                                               │
│                                                                                         │
│   Commands:                                                                            │
│   ├── hash        Compute hash of data                                                 │
│   ├── encode      Encode data (hex, base64, etc.)                                      │
│   ├── decode      Decode data                                                          │
│   ├── verify      Verify signature/proof                                               │
│   ├── benchmark   Run benchmarks                                                       │
│   ├── convert     Unit conversions                                                     │
│   └── generate    Generate test data                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CATEGORY: dev                                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   mbongo dev <command>                                                                 │
│                                                                                         │
│   Commands:                                                                            │
│   ├── init        Initialize development environment                                   │
│   ├── build       Build project                                                        │
│   ├── test        Run tests                                                            │
│   ├── deploy      Deploy contract (future)                                             │
│   ├── debug       Debug transaction                                                    │
│   ├── inspect     Inspect state                                                        │
│   ├── replay      Replay transaction                                                   │
│   └── profile     Profile execution                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Conventions

### 4.1 Flag Conventions

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FLAG CONVENTIONS                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   LONG VS SHORT FLAGS                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Short │ Long              │ Description                                       │  │
│   │   ──────┼───────────────────┼───────────────────────────────────────────────────│  │
│   │   -c    │ --config          │ Configuration file path                           │  │
│   │   -d    │ --data-dir        │ Data directory path                               │  │
│   │   -o    │ --output          │ Output format (json, table, raw)                  │  │
│   │   -v    │ --verbose         │ Increase verbosity (stackable)                    │  │
│   │   -q    │ --quiet           │ Suppress output                                   │  │
│   │   -h    │ --help            │ Show help message                                 │  │
│   │   -V    │ --version         │ Show version                                      │  │
│   │   -y    │ --yes             │ Skip confirmations                                │  │
│   │   -n    │ --dry-run         │ Simulate without executing                        │  │
│   │         │ --rpc-url         │ RPC endpoint URL                                  │  │
│   │         │ --keystore        │ Keystore directory                                │  │
│   │         │ --password-file   │ Password file path                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   REQUIRED VS OPTIONAL                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   Required flags are indicated in help with no default:                                │
│   --validator-id <ID>       Validator ID (required)                                    │
│                                                                                         │
│   Optional flags show defaults:                                                        │
│   --timeout <SECONDS>       Request timeout [default: 30]                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Output Formats

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         OUTPUT FORMATS                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   JSON OUTPUT (--output json)                                                           │
│   ═══════════════════════════                                                           │
│                                                                                         │
│   For programmatic consumption:                                                        │
│                                                                                         │
│   $ mbongo validator status --validator-id 0x1234 --output json                        │
│                                                                                         │
│   {                                                                                     │
│     "validator_id": "0x1234...",                                                       │
│     "status": "active",                                                                │
│     "stake": "50000000000000000000000",                                                │
│     "stake_formatted": "50,000 MBO",                                                   │
│     "uptime": 0.9987,                                                                  │
│     "last_attestation": 12345678,                                                      │
│     "rewards_pending": "125.5 MBO"                                                     │
│   }                                                                                     │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   TABLE OUTPUT (--output table) [default]                                               │
│   ═══════════════════════════════════════                                               │
│                                                                                         │
│   For human readability:                                                               │
│                                                                                         │
│   $ mbongo validator status --validator-id 0x1234 --output table                       │
│                                                                                         │
│   ┌──────────────────────────────────────────────────────────────┐                     │
│   │ Validator Status                                             │                     │
│   ├────────────────────┬─────────────────────────────────────────┤                     │
│   │ Validator ID       │ 0x1234...5678                           │                     │
│   │ Status             │ Active                                  │                     │
│   │ Stake              │ 50,000 MBO                              │                     │
│   │ Uptime             │ 99.87%                                  │                     │
│   │ Last Attestation   │ Block #12,345,678                       │                     │
│   │ Pending Rewards    │ 125.5 MBO                               │                     │
│   └────────────────────┴─────────────────────────────────────────┘                     │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   RAW OUTPUT (--output raw)                                                             │
│   ═════════════════════════                                                             │
│                                                                                         │
│   For minimal processing:                                                              │
│                                                                                         │
│   $ mbongo account balance 0x1234 --output raw                                         │
│   50000000000000000000000                                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Exit Code Standards

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EXIT CODES                                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Code │ Name              │ Description                                        │  │
│   │   ─────┼───────────────────┼────────────────────────────────────────────────────│  │
│   │   0    │ SUCCESS           │ Command completed successfully                     │  │
│   │   1    │ INVALID_ARGS      │ Invalid command-line arguments                     │  │
│   │   2    │ CONFIG_ERROR      │ Configuration file error                           │  │
│   │   3    │ CONNECTION_ERROR  │ Cannot connect to node/RPC                         │  │
│   │   4    │ AUTH_ERROR        │ Authentication failed / permission denied          │  │
│   │   5    │ EXECUTION_ERROR   │ Command execution failed                           │  │
│   │   6    │ TIMEOUT_ERROR     │ Operation timed out                                │  │
│   │   7    │ NOT_FOUND         │ Resource not found                                 │  │
│   │   8    │ INSUFFICIENT      │ Insufficient balance/stake                         │  │
│   │   9    │ ALREADY_EXISTS    │ Resource already exists                            │  │
│   │   10   │ INVALID_STATE     │ Invalid state for operation                        │  │
│   │   99   │ INTERNAL_ERROR    │ Unexpected internal error                          │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   USAGE IN SCRIPTS                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   ```bash                                                                               │
│   mbongo node status || echo "Node not running (exit: $?)"                             │
│   ```                                                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Security Constraints

### 5.1 Private Key Protection

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY: PRIVATE KEYS                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                                                                                 ║  │
│   ║   ⚠️  NEVER EXPOSE PRIVATE KEYS                                                 ║  │
│   ║                                                                                 ║  │
│   ║   The CLI is designed to NEVER:                                                 ║  │
│   ║   • Display private keys in output                                              ║  │
│   ║   • Log private keys (even in debug mode)                                       ║  │
│   ║   • Accept private keys as command-line arguments                               ║  │
│   ║   • Store private keys in plain text                                            ║  │
│   ║                                                                                 ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   KEY STORAGE                                                                           │
│   ═══════════                                                                           │
│                                                                                         │
│   • Keys stored in encrypted keystore                                                  │
│   • Default location: ~/.mbongo/keystore/                                              │
│   • Encryption: scrypt + AES-256-GCM                                                   │
│   • Password required to decrypt                                                       │
│                                                                                         │
│   SAFE PRACTICES                                                                        │
│   ══════════════                                                                        │
│                                                                                         │
│   ✓ Use --password-file instead of typing passwords                                   │
│   ✓ Use environment variables for sensitive values                                    │
│   ✓ Use hardware wallets when possible (future)                                       │
│   ✗ Never pass private keys as CLI arguments                                          │
│   ✗ Never store passwords in shell history                                            │
│                                                                                         │
│   EXPORT WARNING                                                                        │
│   ══════════════                                                                        │
│                                                                                         │
│   $ mbongo account export 0x1234                                                       │
│                                                                                         │
│   ⚠️  WARNING: You are about to export a private key.                                  │
│   This key controls your funds and validator operations.                               │
│   Anyone with this key can steal your assets.                                          │
│                                                                                         │
│   Are you sure? [y/N]                                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Local-Only vs RPC-Enabled Commands

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMMAND ACCESS LEVELS                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   LOCAL-ONLY COMMANDS                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   These commands require direct access to the node:                                    │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Command              │ Reason                                                 │  │
│   │   ─────────────────────┼───────────────────────────────────────────────────────│  │
│   │   node start/stop      │ Process control                                       │  │
│   │   account export       │ Key access                                            │  │
│   │   dev debug/inspect    │ Internal state access                                 │  │
│   │   validator register   │ Requires signing                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   RPC-ENABLED COMMANDS                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   These commands can work over RPC:                                                    │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Command              │ Notes                                                  │  │
│   │   ─────────────────────┼───────────────────────────────────────────────────────│  │
│   │   account balance      │ Read-only query                                       │  │
│   │   consensus block      │ Read-only query                                       │  │
│   │   network peers        │ Read-only query                                       │  │
│   │   validator status     │ Read-only query                                       │  │
│   │   account transfer     │ Requires local keystore for signing                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   RPC SECURITY                                                                          │
│   ════════════                                                                          │
│                                                                                         │
│   • RPC endpoints should be TLS-protected                                              │
│   • Use --rpc-url with https:// in production                                          │
│   • Consider IP whitelisting for RPC access                                            │
│   • Never expose admin RPC endpoints publicly                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.3 CLI Safety Warnings

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SAFETY WARNINGS                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   INTERACTIVE CONFIRMATIONS                                                             │
│   ═════════════════════════                                                             │
│                                                                                         │
│   High-risk operations require confirmation:                                           │
│                                                                                         │
│   $ mbongo validator exit                                                              │
│                                                                                         │
│   ⚠️  WARNING: Validator Exit                                                          │
│   ────────────────────────────────────────────────────────────                         │
│   This will begin the validator exit process.                                          │
│   Your stake will be unbonded over 21 days.                                            │
│   You cannot reverse this action.                                                      │
│                                                                                         │
│   Validator: 0x1234...5678                                                             │
│   Stake:     50,000 MBO                                                                │
│                                                                                         │
│   Type 'EXIT' to confirm:                                                              │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   SKIP CONFIRMATIONS                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   For automation, use --yes (with caution):                                            │
│                                                                                         │
│   $ mbongo stake unbond --amount 1000 --yes                                            │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   DRY RUN                                                                               │
│   ═══════                                                                               │
│                                                                                         │
│   Preview without executing:                                                           │
│                                                                                         │
│   $ mbongo account transfer --to 0x5678 --amount 100 --dry-run                         │
│                                                                                         │
│   [DRY RUN] Transaction preview:                                                       │
│   From:   0x1234...                                                                    │
│   To:     0x5678...                                                                    │
│   Amount: 100 MBO                                                                      │
│   Fee:    ~0.001 MBO                                                                   │
│   (No transaction submitted)                                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Examples

### 6.1 Start Node

```bash
# Start with default configuration
mbongo node start

# Start with custom config
mbongo node start --config /etc/mbongo/config.toml

# Start as validator
mbongo node start --validator --keystore ~/.mbongo/validator

# Start with specific data directory
mbongo node start --data-dir /var/lib/mbongo

# Start with verbose logging
mbongo node start -vvv

# Start with custom RPC port
mbongo node start --rpc-port 8546 --rpc-addr 0.0.0.0
```

### 6.2 Query Block

```bash
# Get latest block
mbongo consensus block latest

# Get block by height
mbongo consensus block --height 12345678

# Get block by hash
mbongo consensus block --hash 0xabc123...

# Get block with full transaction details
mbongo consensus block --height 12345678 --full

# Output as JSON for parsing
mbongo consensus block latest --output json

# Get only the block hash
mbongo consensus block latest --output raw
```

### 6.3 Check Validator Status

```bash
# Check your validator status
mbongo validator status

# Check specific validator
mbongo validator status --validator-id 0x1234...

# Check all validators in active set
mbongo consensus validators --output table

# Check validator rewards
mbongo validator rewards

# Check validator attestation history
mbongo validator attestations --limit 100

# Check slashing history
mbongo validator slashing
```

### 6.4 Submit Compute Proof

```bash
# Register as compute provider
mbongo compute register --gpu-specs "NVIDIA RTX 4090"

# Submit a compute proof
mbongo compute submit \
  --task-id 0xdef456... \
  --result-hash 0x789abc... \
  --work-units 1000000 \
  --proof-file ./proof.bin

# View your compute receipts
mbongo compute receipts --limit 50

# Check compute rewards
mbongo compute rewards

# View available tasks
mbongo compute tasks --status pending

# Run GPU benchmark
mbongo compute benchmark --duration 60
```

### 6.5 Export Account Keys

```bash
# List all accounts
mbongo account list

# Export account (encrypted JSON keystore)
mbongo account export 0x1234... --output ./keystore.json

# Export with specific password file
mbongo account export 0x1234... \
  --output ./keystore.json \
  --password-file ~/.mbongo/password

# Import account from keystore
mbongo account import ./keystore.json

# Create new account
mbongo account create --name "validator-1"

# Check balance
mbongo account balance 0x1234...
```

### 6.6 Additional Common Operations

```bash
# Transfer MBO
mbongo account transfer \
  --to 0x5678... \
  --amount 100 \
  --gas-price auto

# Stake MBO
mbongo stake deposit --amount 50000

# Delegate to validator
mbongo stake delegate \
  --validator 0x9abc... \
  --amount 1000

# Begin unbonding
mbongo stake unbond --amount 500

# Check network peers
mbongo network peers --output table

# Add a peer manually
mbongo network add-peer /ip4/1.2.3.4/tcp/30303/p2p/QmXyz...

# Check node health
mbongo node health --output json
```

---

## 7. Cross-Links

### Related CLI Documentation

| Document | Description | Status |
|----------|-------------|--------|
| **[cli_node.md](./cli_node.md)** | Node management commands | Planned |
| **[cli_network.md](./cli_network.md)** | Network and peer commands | Planned |
| **[cli_validator.md](./cli_validator.md)** | Validator operations | Planned |
| **[cli_keys.md](./cli_keys.md)** | Key and account management | Planned |
| **[cli_debug.md](./cli_debug.md)** | Debugging and diagnostics | Planned |
| **[cli_config.md](./cli_config.md)** | Configuration reference | Planned |

### Architecture Documentation

| Document | Description |
|----------|-------------|
| **[node_architecture.md](./node_architecture.md)** | Node internals |
| **[consensus_master_overview.md](./consensus_master_overview.md)** | Consensus details |
| **[compute_engine_overview.md](./compute_engine_overview.md)** | PoUW compute system |
| **[network_overview.md](./network_overview.md)** | Networking layer |

### Quick Reference Card

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         CLI QUICK REFERENCE                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   COMMON COMMANDS                                                                       │
│   ───────────────                                                                       │
│   mbongo node start              # Start the node                                      │
│   mbongo node status             # Check node status                                   │
│   mbongo validator status        # Check validator status                              │
│   mbongo account balance <addr>  # Check balance                                       │
│   mbongo consensus block latest  # Get latest block                                    │
│   mbongo network peers           # List connected peers                                │
│                                                                                         │
│   GLOBAL FLAGS                                                                          │
│   ────────────                                                                          │
│   -c, --config <FILE>           Configuration file                                     │
│   -d, --data-dir <DIR>          Data directory                                         │
│   -o, --output <FORMAT>         Output format (json, table, raw)                       │
│   -v, --verbose                 Increase verbosity                                     │
│   -q, --quiet                   Suppress output                                        │
│   -h, --help                    Show help                                              │
│   -V, --version                 Show version                                           │
│                                                                                         │
│   ENVIRONMENT VARIABLES                                                                 │
│   ─────────────────────                                                                 │
│   MBONGO_CONFIG                 Config file path                                       │
│   MBONGO_DATA_DIR               Data directory path                                    │
│   MBONGO_RPC_URL                RPC endpoint URL                                       │
│   MBONGO_LOG                    Log level                                              │
│   MBONGO_KEYSTORE               Keystore directory                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

*This document provides the CLI overview for Mbongo Chain. For detailed command references, see the linked documentation.*

