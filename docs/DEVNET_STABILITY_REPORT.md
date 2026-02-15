# Mbongo Chain — Devnet Stability Report

**Version:** v0.2-devnet-stable
**Status:** FROZEN
**Date:** February 2026
**Canonical Protocol Spec:** [PROTOCOL_DEFINITION_v0.1.md](./specs/PROTOCOL_DEFINITION_v0.1.md)

---

## 1. Executive Summary

Phase 2 delivers a fully functional multi-node devnet for Mbongo Chain. This release proves that a producer node can create blocks on a configurable timer, broadcast them to follower nodes over libp2p, and that all nodes converge to the same chain state.

Phase 2 deliverables:

- **Multi-node devnet** with 1 producer and N followers
- **Single producer model** controlled by `--producer` flag
- **libp2p networking** for peer-to-peer block transport (TCP, noise, yamux)
- **Block sync protocol** for bootstrapping from genesis via height-based request/response
- **Block announcement (push)** so new blocks reach followers immediately
- **Timed block production** via configurable `--block-time` interval
- **Producer role enforcement** preventing non-producer nodes from creating blocks
- **Atomic block application** using `WriteBatch` for all-or-nothing state commits
- **Deterministic replay** proven by an independent replay harness
- **Restart convergence** validated by the devnet convergence harness

All protocol behavior aligns with [PROTOCOL_DEFINITION_v0.1.md](./specs/PROTOCOL_DEFINITION_v0.1.md). No protocol changes were introduced.

---

## 2. Architecture Snapshot

### Producer Node

A producer node is started with the `--producer` flag. It is the single authority for block creation in the devnet.

| Capability | Detail |
|---|---|
| Block creation | Timer-triggered via `--block-time` (default: 5 seconds) |
| Broadcasting | New blocks are pushed to all connected peers via libp2p notification protocol |
| Sync serving | Responds to block-by-height requests from followers |
| RPC | Full JSON-RPC 2.0 and REST API |

CLI example:

```
mbongo-node --producer --block-time 5 --rpc-port 9944 --rest-port 8080 --p2p-port 30333 --data-dir data
```

### Follower Node

A follower node connects to the producer (or other full nodes) via `--bootnodes` and syncs the chain.

| Capability | Detail |
|---|---|
| Bootstrap sync | Fetches all blocks from genesis on startup via request/response protocol |
| Live sync | Receives new block announcements in real time |
| Validation | Every received block is validated via `apply_block()` (parent linkage, transactions root, tx validity) |
| Atomic commit | Block state is written atomically via `write_batch` |
| Block production | **Disabled.** Returns error if attempted without `--producer` flag |

CLI example:

```
mbongo-node --bootnodes /ip4/127.0.0.1/tcp/30333 --rpc-port 9945 --rest-port 8081 --p2p-port 30334 --data-dir data_follower
```

---

## 3. Determinism Guarantees

Mbongo Chain guarantees deterministic block processing across all nodes:

| Guarantee | Mechanism |
|---|---|
| Serialization | SCALE encoding (parity-scale-codec) for all on-chain data |
| Hashing | BLAKE3 for block hashes, transaction roots, and state commitments |
| Atomicity | `WriteBatch` ensures block application is all-or-nothing; no partial state |
| Replay verification | The replay harness exports blocks from a producer, replays them on a fresh backend, and asserts identical tip hash |
| Convergence verification | The devnet convergence harness spawns 3 nodes (1 producer + 2 followers) and asserts all reach the same height and tip hash |

The deterministic replay harness (M2.5) independently validates that:

1. Blocks exported via RPC can be deserialized and re-applied in order
2. Parent hash linkage holds across all replayed blocks
3. Transactions root matches for every replayed block
4. The final tip hash on the replay backend matches the original producer

---

## 4. Test Matrix

| Metric | Value |
|---|---|
| **Total passing tests** | 154 |
| **Ignored tests** | 2 (long-running harness integration tests) |
| **Compiler warnings** | 0 |
| **Clippy warnings** | 0 (`-D warnings` enforced) |

### Test Distribution

| Crate | Tests |
|---|---|
| mbongo-core | 41 (including 3 property tests) |
| mbongo-storage | 14 |
| mbongo-network | 16 (9 lib + 7 integration) |
| mbongo-node | 62 unit |
| mbongo-api | 6 (1 lib + 5 integration) |
| mbongo-consensus | 1 (stub) |
| mbongo-compute | 1 (stub) |
| mbongo-runtime | 1 (stub) |
| mbongo-verification | 1 (stub) |
| mbongo-wallet | 1 (stub) |
| Doc tests | 10 |

### Validation Harnesses

| Harness | Milestone | Type | Description |
|---|---|---|---|
| `devnet_harness` | M2.4 | Binary | Spawns 3-node devnet, validates convergence, tests producer and follower restart |
| `replay_harness` | M2.5 | Binary | Exports blocks from producer, replays on fresh backend, asserts identical tip hash |
| `devnet_convergence` | M2.4 | Integration test (`#[ignore]`) | Automated convergence check for CI |
| `deterministic_replay` | M2.5 | Integration test (`#[ignore]`) | Automated replay verification for CI |
| `devnet_test.ps1` | M2.4 | PowerShell script | Builds and runs devnet harness with PASS/FAIL output |
| `replay_test.ps1` | M2.5 | PowerShell script | Builds and runs replay harness with PASS/FAIL output |

---

## 5. What Is Frozen

The following are frozen at v0.2-devnet-stable. Changes require a protocol version bump and explicit approval.

| Component | Spec Reference |
|---|---|
| **Block structure** | `BlockHeader` (parent_hash, state_root, transactions_root, timestamp, height) + `BlockBody` (transactions) |
| **Transaction structure** | Transfer type (sender, receiver, amount, nonce, signature) |
| **RPC v0.1** | `submit_transaction`, `produce_block`, `get_block_height`, `ping`, `get_latest_block_hash`, `get_block_by_height` |
| **P2P sync protocol** | Block request/response by height range over libp2p request-response |
| **Block notification protocol** | Push-based block announcement over libp2p request-response |
| **Storage invariants** | Atomic `WriteBatch`, column family separation, height indexing, parent linkage |
| **Block validity rules** | Parent linkage, height monotonic, deterministic SCALE hash, transactions root, transaction validity |

---

## 6. What Is Explicitly Out of Scope

These are **not** part of v0.2 and will be addressed in future phases:

- Consensus protocol (PoX, PoS, PoUW)
- Fork handling and reorg
- AIDA economic regulator
- Compute marketplace (task submission, GPU coordination)
- TEE integration (Intel SGX, AMD SEV)
- ZK-ML proofs
- Gas model and fee ordering
- Smart contracts (WASM)
- Validator set management
- Slashing and economic penalties
- Transaction gossip protocol
- Peer discovery (mDNS, DHT)
- Mempool eviction policy

---

## 7. Allowed Contributions

The following areas are open for contribution without risk of protocol breakage:

| Area | Examples |
|---|---|
| **SDK** | Rust/JS/Python client libraries, transaction builders |
| **CLI tools** | Wallet CLI, block explorer CLI, diagnostic utilities |
| **Metrics and observability** | Prometheus exporters, Grafana dashboards, structured logging |
| **DevOps** | Docker images, CI/CD pipelines, cloud deployment scripts |
| **Documentation** | Tutorials, API guides, onboarding materials |
| **Testing** | Additional unit tests, integration tests, fuzzing, benchmarks |

All contributions must pass `cargo clippy --workspace -- -D warnings` and `cargo test --workspace`.

---

## 8. Forbidden Modifications

The following **must not** be changed without explicit protocol version bump and approval:

| Area | Reason |
|---|---|
| **Storage layer invariants** | Atomic `WriteBatch`, column families, key encoding — changing these breaks all existing data |
| **Block model** | `BlockHeader` and `BlockBody` fields are frozen; adding/removing fields is a protocol change |
| **Transaction model** | `Transaction` fields and signing payload are frozen |
| **P2P protocol messages** | Sync request/response and block notification message formats are frozen |
| **Fork logic** | No fork choice exists by design; introducing one is a protocol change |
| **Consensus logic** | No consensus exists by design; introducing one is a protocol change |
| **Block validity rules** | Parent linkage, height monotonicity, transactions root, transaction validity rules are frozen |

---

## Milestone Changelog

| Milestone | Description | Tests After |
|---|---|---|
| M2.0 | Block sync protocol (request/response over libp2p) | 131 |
| M2.1 | Block announcement (push-based notification) | 139 |
| M2.2 | Timed block production (configurable interval) | 142 |
| M2.3 | Producer role enforcement (`--producer` flag) | 144 |
| M2.4 | Devnet convergence harness (3-node validation) | 148 |
| M2.5 | Deterministic replay harness (block export/replay) | 154 |
