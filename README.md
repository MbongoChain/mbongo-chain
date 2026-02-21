# Mbongo Chain

**A deterministic verification layer for off-chain AI inference receipts.**

Mbongo Chain verifies cryptographic receipts from off-chain AI inference. It does not execute AI models on-chain. Validators verify receipts deterministically and settle economic outcomes. Execution is off-chain; the chain provides trust and settlement.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

---

## Current Status

**Tag:** `v0.2-devnet-stable`  
**Branch:** All development targets `dev`. PRs must target `dev`.

### Implemented Now

- Block and transaction data structures (SCALE-encoded, BLAKE3 hashing)
- Account model (balance, nonce)
- Transfer execution and validation (signature, nonce, balance, replay protection)
- Persistent storage (RocksDB, atomic `WriteBatch`)
- Multi-node devnet: 1 producer + N followers over libp2p
- Block sync: bootstrap from genesis, height-based request/response, block announcement
- Timed block production (`--producer`, `--block-time`)
- JSON-RPC 2.0 and REST API
- Deterministic replay harness and devnet convergence harness

### Explicitly NOT in Scope for v0.2 / v1

- Proof of Stake, Proof of Useful Work, PoX consensus
- AIDA regulator
- GPU marketplace, compute provider runtime, Docker/WASM execution
- TEE attestation, ZK-ML proofs
- On-chain AI model execution
- Block rewards (no economics in v0.2)
- Smart contracts, gas metering
- REST compute job submission

See [VISION_v1.md](./docs/VISION_v1.md) and [tokenomics.md](./docs/tokenomics.md).

---

## Quick Start (Windows PowerShell)

### Prerequisites

- **Rust** 1.75+ ([install via rustup](https://rustup.rs/))
- **Git**

### Clone, Build, Test

```powershell
git clone https://github.com/MbongoChain/mbongo-chain.git
cd mbongo-chain
git checkout dev

cargo build --workspace
cargo test --workspace
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

### Run Validation Harnesses

```powershell
cargo run -p mbongo-node --bin devnet_harness
cargo run -p mbongo-node --bin replay_harness
```

Or: `.\scripts\devnet_test.ps1` and `.\scripts\replay_test.ps1`

See [DEV_ONBOARDING.md](./docs/DEV_ONBOARDING.md) for full CLI reference.

---

## Documentation

| Document | Purpose |
|----------|---------|
| [DEVNET_STABILITY_REPORT.md](./docs/DEVNET_STABILITY_REPORT.md) | Freeze documentation, test matrix |
| [DEV_ONBOARDING.md](./docs/DEV_ONBOARDING.md) | Quick start, CLI reference, devnet commands |
| [ARCHITECTURE_OVERVIEW_FOR_NEW_DEVS.md](./docs/ARCHITECTURE_OVERVIEW_FOR_NEW_DEVS.md) | Layer separation and block flow |
| [PROTOCOL_LOCK_v0.2.md](./docs/specs/PROTOCOL_LOCK_v0.2.md) | Frozen surfaces, versioning rules |
| [COMPUTE_INTERFACE_v0.1.md](./docs/specs/COMPUTE_INTERFACE_v0.1.md) | Future receipt spec (no implementation in v0.2) |
| [VISION_v1.md](./docs/VISION_v1.md) | Verification layer scope |
| [tokenomics.md](./docs/tokenomics.md) | v1 vs v2+ economics |
| [CONTRIBUTION_TIERS.md](./docs/CONTRIBUTION_TIERS.md) | Tier 0/1/2 change rules |
| [RFC_PROCESS.md](./docs/RFC_PROCESS.md) | How to propose changes to locked surfaces |

---

## Contributing

- **PRs target the `dev` branch.** `main` is reserved for audited milestones.
- **Tier labels:** Changes to locked surfaces (block format, RPC, P2P, storage) require an RFC and version bump. See [CONTRIBUTION_TIERS.md](./docs/CONTRIBUTION_TIERS.md) and [PROTOCOL_LOCK_v0.2.md](./docs/specs/PROTOCOL_LOCK_v0.2.md).
- **Good first issues:** GitHub Issues with labels `tier-2` or `good-first-issue`.

See [CONTRIBUTING.md](./CONTRIBUTING.md).

---

## Roadmap

| Version | Milestone | Scope |
|---------|-----------|-------|
| **v0.2** | Devnet stable | Multi-node devnet, single producer, block sync. **FROZEN.** |
| **v0.3** | PoS minimal + receipt prototype | Stake-weighted validator set. Reserved RPC stubs. Receipt verification prototype. |
| **v0.4+** | Compute verification expansion | Canonical receipt format, challenge mechanism, SDK. |
| **v1.0** | Verified inference primitive | Receipt verification live. No on-chain AI execution. |
| **v2+** | Optional PoUW | On-chain execution as opt-in extension. PoUW, TEE, ZK-ML are **future** — not current. |

---

## License

Apache License 2.0 — see [LICENSE](./LICENSE).
