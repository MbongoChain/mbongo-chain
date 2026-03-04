# Mbongo Chain — Developer Onboarding

**Version:** v0.2-devnet-stable
**Canonical Protocol Spec:** [PROTOCOL_DEFINITION_v0.1.md](./specs/PROTOCOL_DEFINITION_v0.1.md)

---

## Quick Start

### Prerequisites

- **Rust** 1.75+ ([install via rustup](https://rustup.rs/))
- **Git**
- **Windows 10/11** or **Linux** (Windows-first development, Linux supported)

### Build

```bash
cargo build --workspace
```

### Run Tests

```bash
cargo test --workspace
```

Expected result: 154 tests passing, 2 ignored, 0 warnings.

### Run a Producer Node

```bash
cargo run --bin mbongo-node -- --producer --block-time 5 --rpc-port 9944 --rest-port 8080 --p2p-port 30333 --data-dir data_producer
```

The producer will create a new block every 5 seconds and listen for peer connections on port 30333.

### Run a Follower Node

Open a second terminal:

```bash
cargo run --bin mbongo-node -- --bootnodes /ip4/127.0.0.1/tcp/30333 --rpc-port 9945 --rest-port 8081 --p2p-port 30334 --data-dir data_follower
```

The follower will sync from genesis, then receive new blocks in real time.

### Run the Devnet Convergence Harness

```bash
cargo run --bin devnet_harness
```

This spawns 3 nodes (1 producer + 2 followers), waits for blocks, and validates that all nodes converge to the same height and tip hash. It also tests restart scenarios.

### Run the Deterministic Replay Harness

```bash
cargo run --bin replay_harness
```

This spawns a producer, exports blocks via RPC, replays them on a fresh in-memory backend, and asserts that the final tip hash is identical.

---

## Devnet Commands (Windows PowerShell)

### Build and Test

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

### Run Producer + Follower (Two Terminals)

**Terminal 1 — Producer:**

```powershell
cargo run --bin mbongo-node -- --producer --block-time 5 --rpc-port 9944 --rest-port 8080 --p2p-port 30333 --data-dir data_producer
```

**Terminal 2 — Follower:**

```powershell
cargo run --bin mbongo-node -- --bootnodes /ip4/127.0.0.1/tcp/30333 --rpc-port 9945 --rest-port 8081 --p2p-port 30334 --data-dir data_follower
```

### Query a Node via RPC

```powershell
# Get block height
Invoke-RestMethod -Uri http://localhost:9944/rpc -Method POST -ContentType "application/json" -Body '{"jsonrpc":"2.0","method":"get_block_height","params":[],"id":1}'

# Ping
Invoke-RestMethod -Uri http://localhost:9944/rpc -Method POST -ContentType "application/json" -Body '{"jsonrpc":"2.0","method":"ping","params":[],"id":1}'

# Get latest block hash
Invoke-RestMethod -Uri http://localhost:9944/rpc -Method POST -ContentType "application/json" -Body '{"jsonrpc":"2.0","method":"get_latest_block_hash","params":[],"id":1}'

# Get block by height
Invoke-RestMethod -Uri http://localhost:9944/rpc -Method POST -ContentType "application/json" -Body '{"jsonrpc":"2.0","method":"get_block_by_height","params":{"height":1},"id":1}'
```

### Run Validation Scripts

```powershell
# Devnet convergence test
.\scripts\devnet_test.ps1

# Deterministic replay test
.\scripts\replay_test.ps1
```

---

## Project Layout Overview

```
mbongo-chain/
  Cargo.toml              # Workspace manifest
  crates/
    mbongo-core/           # Core blockchain primitives (pure logic)
    mbongo-storage/        # Persistence layer (RocksDB, InMemoryStorage)
    mbongo-network/        # Transport layer (JSON-RPC, REST, libp2p)
    mbongo-node/           # Node orchestration (main binary, harnesses)
    mbongo-api/            # REST API handlers
    mbongo-consensus/      # Consensus (stub — not yet implemented)
    mbongo-compute/        # Compute engine (stub — not yet implemented)
    mbongo-verification/   # Verification layer (stub — not yet implemented)
    mbongo-runtime/        # Smart contract runtime (stub — not yet implemented)
    mbongo-wallet/         # Wallet and key management (stub)
  docs/
    specs/                 # Frozen protocol specs (PDD v0.1, RPC v0.1)
    architecture/          # Architecture documents
  scripts/                 # PowerShell scripts for devnet and replay testing
  sdk/                     # SDK scaffolding
```

### Crate Descriptions

#### mbongo-core

Pure logic crate. Contains block, transaction, and account types. Cryptographic operations (Ed25519, BLAKE3). SCALE encoding derives. No I/O, no network, no filesystem.

#### mbongo-storage

Persistence crate. Defines the `Storage` trait and provides two implementations:

- `RocksDbStorage` — Production storage backed by RocksDB with column families
- `InMemoryStorage` — Test storage using `HashMap` and `RwLock`

All writes use `WriteBatch` for atomic block application.

#### mbongo-network

Transport crate. Contains:

- JSON-RPC 2.0 server (Axum) with method dispatch
- REST API route definitions
- `RpcBackend` trait for backend abstraction
- libp2p P2P protocol definitions (sync request/response, block notification)

#### mbongo-node

Orchestration crate. Wires all components together:

- CLI argument parsing (clap)
- `NodeBackend` with storage, mempool, block production, and `apply_block`
- P2P networking lifecycle (libp2p swarm, sync service)
- Timed block production (tokio interval)
- Block receiver channel for incoming blocks from P2P
- Devnet and replay harness binaries

#### mbongo-consensus (stub)

Placeholder for future PoX consensus. Currently contains a single stub test. No consensus logic exists.

#### mbongo-compute (stub)

Placeholder for future compute marketplace. Currently contains a single stub test.

#### mbongo-verification (stub)

Placeholder for future verification layer (TEE, ZK-ML). Currently contains a single stub test.

---

## Development Rules

### Protocol Integrity

1. **Do not break PDD v0.1.** All changes must be compatible with [PROTOCOL_DEFINITION_v0.1.md](./specs/PROTOCOL_DEFINITION_v0.1.md).
2. **No protocol changes without spec update.** If you need to change block format, transaction format, or P2P messages, update the spec first and get approval.
3. **RPC v0.1 is frozen.** Adding new RPC methods is allowed; changing or removing existing methods requires a version bump.

### Code Quality

4. **All changes must pass `cargo clippy --workspace -- -D warnings`.** Zero warnings policy.
5. **All changes must pass `cargo fmt --all --check`.** Standard Rust formatting.
6. **All changes must pass `cargo test --workspace`.** No regressions allowed.
7. **No `unwrap()` in production paths.** Use proper error handling (`Result`, `?`, `map_err`). `unwrap()` is acceptable in tests only.
8. **No `unsafe` code** without explicit justification and review.

### Layer Discipline

9. **Respect crate boundaries.** See [ARCHITECTURE_GUARDRAILS.md](./ARCHITECTURE_GUARDRAILS.md) for layer rules.
10. **mbongo-core has no I/O dependencies.** It must remain pure logic.
11. **mbongo-storage has no network dependencies.** It handles persistence only.
12. **mbongo-network has no storage mutation.** It forwards to the backend.

### Branching

13. **All Phase 2 development targets the `dev` branch.** The `main` branch is reserved for audited, stable milestones.
14. **Use conventional commit messages.** Format: `type: description` (e.g., `feat:`, `fix:`, `docs:`, `test:`, `refactor:`).

---

## CLI Flags Reference

| Flag | Default | Description |
|---|---|---|
| `--producer` | false | Enable block production on this node |
| `--block-time` | 5 | Block production interval in seconds (producer only) |
| `--rpc-port` | 9944 | JSON-RPC server port |
| `--rest-port` | 8080 | REST API server port |
| `--p2p-port` | 30333 | libp2p listening port |
| `--bootnodes` | (none) | Multiaddr(s) of peers to connect to on startup |
| `--data-dir` | data | Directory for RocksDB storage |
| `--dev` | false | Development mode |
| `--chain` | dev | Chain identifier |
| `--validator` | false | Validator mode (future) |
| `--provider` | false | Compute provider mode (future) |
| `--name` | (none) | Node name |

---

## Key Links

- [Stability Report](./DEVNET_STABILITY_REPORT.md) — Devnet freeze documentation
- [Architecture Overview](./ARCHITECTURE_OVERVIEW_FOR_NEW_DEVS.md) — High-level architecture for new developers
- [Protocol Definition v0.1](./specs/PROTOCOL_DEFINITION_v0.1.md) — Canonical protocol spec
- [RPC Spec v0.1](./specs/rpc_v0.1.md) — JSON-RPC method definitions
- [Architecture Guardrails](./ARCHITECTURE_GUARDRAILS.md) — Layer rules and invariant protections
- [Phase 1 Complete](./PHASE_1_COMPLETE.md) — Phase 1 closure summary
- [Phase 2 Plan](./PHASE_2_PLAN.md) — Phase 2 milestone roadmap
