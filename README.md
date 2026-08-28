# Mbongo Chain

[![CI](https://github.com/MbongoChain/mbongo-chain/actions/workflows/ci.yml/badge.svg)](https://github.com/MbongoChain/mbongo-chain/actions/workflows/ci.yml)

**A deterministic verification layer for off-chain AI inference receipts.**

Mbongo Chain verifies cryptographic receipts from off-chain AI inference. It does not execute AI models on-chain. Validators verify receipts deterministically and settle economic outcomes. Execution is off-chain; the chain provides trust and settlement.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

---

## Current Status

**Protocol:** v0.3 (receipt anchoring, RFC 0002) — tag `v0.3-devnet-stable` pending release

**Branch:** All development targets `dev`. PRs must target `dev`.

> v0.3 is network-incompatible with v0.2 by design: transaction encoding,
> storage schema, and P2P protocol strings all changed. v0.3 devnets start
> from a fresh genesis; see
> [PROTOCOL_LOCK_v0.3.md](./docs/specs/PROTOCOL_LOCK_v0.3.md) for the
> migration procedure. This is a devnet release — not mainnet-ready.

### Implemented Now

- Block and transaction data structures (SCALE-encoded, BLAKE3 hashing)
- Typed transaction payloads (`TransactionPayload`, explicit codec indexes)
- **Receipt anchoring (RFC 0002):** `AnchorReceipt` transactions validated
  under a normative deterministic rule order and committed atomically with
  block state; global `task_id` uniqueness; canonical receipt bytes in a
  dedicated RocksDB column family (schema v2)
- Account model (balance, nonce)
- Transfer execution and validation (signature, nonce, balance, replay protection)
- Persistent storage (RocksDB, atomic `WriteBatch`, schema versioning + migration)
- Multi-node devnet: 1 producer + N followers over libp2p (`/mbongo-sync/2`)
- Block sync: bootstrap from genesis, height-based request/response, block announcement
- Timed block production (`--producer`, `--block-time`)
- JSON-RPC 2.0 and REST API
- Deterministic replay harness and devnet convergence harness (with receipt traffic)

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

## Technology Stack (v0.2-devnet-stable)

Core Language:
- Rust (stable toolchain)

Networking:
- libp2p (gossipsub, request/response)

Storage:
- RocksDB (persistent state)
- Atomic WriteBatch

Serialization:
- SCALE encoding

Cryptography:
- Ed25519 signatures
- BLAKE3 hashing

APIs:
- JSON-RPC 2.0
- REST API

Testing:
- Deterministic replay harness
- Devnet convergence harness

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

## Dockerised Devnet (3 nodes)

A reproducible 1-producer + 2-follower devnet that boots from a clean
checkout with one command, on any machine that runs Docker. It is the
cross-platform counterpart of the PowerShell devnet under `scripts/devnet/`,
which remains the tool for long-running Windows soaks.

### Prerequisites

- Docker Engine 24+ (Docker Desktop on Windows/macOS)
- Docker Compose v2+ (`docker compose version`)
- GNU Make, only if you want the `make` shortcuts

### Boot it

```bash
make devnet-up
```

Or, without Make (identical behaviour — the target only delegates):

```bash
./scripts/devnet/docker-devnet.sh up
```

That command builds the devnet image, starts the three nodes on a dedicated
Compose network, waits until each one reports healthy, then runs the
`convergence_probe` binary against all three. It exits `0` only if the nodes
agree on the same height and tip hash **and** the chain produced a new block
while the probe was watching. On failure it prints container state, health
status and recent per-node logs, then exits non-zero.

### Tear it down

```bash
make devnet-down            # or ./scripts/devnet/docker-devnet.sh down
```

This removes the containers, the network and any volumes. It is safe to run
when nothing is up, and safe to run twice. Node state lives in the container
writable layer, so every boot starts from a fresh genesis.

### Configuration

Three layers, applied in order:

| File | Versioned | Purpose |
|------|-----------|---------|
| `.env.base` | yes | Deterministic defaults. A fresh checkout boots with this file alone. |
| `.env.local` | **no** (gitignored) | Your personal overrides. Optional — never required. |
| `.env.ci` | yes | Deterministic overrides for automated runs (`DEVNET_ENV=ci`). |

To override something locally, create `.env.local` with only the keys you
want to change — never edit `.env.base`:

```bash
echo "MBONGO_HOST_RPC_PORT=31944" > .env.local
echo "MBONGO_BLOCK_TIME=2"       >> .env.local
```

To use the CI layer instead:

```bash
DEVNET_ENV=ci ./scripts/devnet/docker-devnet.sh up
```

### Exposed host ports

Only one: the producer JSON-RPC, published on **loopback only** at
`127.0.0.1:${MBONGO_HOST_RPC_PORT}` (default `29944`, chosen to avoid the
operational devnet on 9944-9946 and the in-process harness on 19944-19946).
Followers and the REST APIs are reachable only from inside the Compose
network. Set `MBONGO_HOST_RPC_PORT=0` to let Docker pick a free port.

```bash
curl -X POST -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"get_block_height","id":1}' \
  http://127.0.0.1:29944/rpc
```

### About `0.0.0.0`

Inside the containers the node is started with `--rpc-host 0.0.0.0` and
`--rest-host 0.0.0.0` so the services can reach each other across the
Compose network. **This is a devnet-only setting, not a production
default.** The node itself still binds `127.0.0.1` when those flags are not
passed, and the RPC surface has no authentication — never expose it on an
untrusted network.

### One source of truth for convergence

The verdict comes from `convergence_probe`
([crates/mbongo-node/src/bin/convergence_probe.rs](./crates/mbongo-node/src/bin/convergence_probe.rs)),
which shares its height/tip-hash rules with `devnet_harness` through
`mbongo_node::convergence`. Nothing in the Compose file, the healthchecks
or the Makefile re-implements that comparison. The container healthchecks
only answer a narrower question — "does this node serve JSON-RPC yet?" — via
the `ping` method.

---

## Documentation

| Document | Purpose |
|----------|---------|
| [DEVNET_STABILITY_REPORT.md](./docs/DEVNET_STABILITY_REPORT.md) | Freeze documentation, test matrix |
| [DEV_ONBOARDING.md](./docs/DEV_ONBOARDING.md) | Quick start, CLI reference, devnet commands |
| [ARCHITECTURE_OVERVIEW_FOR_NEW_DEVS.md](./docs/ARCHITECTURE_OVERVIEW_FOR_NEW_DEVS.md) | Layer separation and block flow |
| [PROTOCOL_LOCK_v0.3.md](./docs/specs/PROTOCOL_LOCK_v0.3.md) | Current frozen surfaces, migration, versioning rules |
| [PROTOCOL_LOCK_v0.2.md](./docs/specs/PROTOCOL_LOCK_v0.2.md) | Superseded v0.2 lock (historical) |
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
| **v0.2** | Devnet stable | Multi-node devnet, single producer, block sync. **SUPERSEDED by v0.3.** |
| **v0.3** | Receipt anchoring devnet | RFC 0002 implemented: typed transaction payloads, `AnchorReceipt` consensus rules, receipts column family, protocol string bump. Network-incompatible with v0.2; fresh genesis. **FROZEN** (see [PROTOCOL_LOCK_v0.3.md](./docs/specs/PROTOCOL_LOCK_v0.3.md)). |
| **v0.4+** | Compute verification expansion | Receipt RPC activation, challenge mechanism, PoS minimal, SDK. |
| **v1.0** | Verified inference primitive | Receipt verification live. No on-chain AI execution. |
| **v2+** | Optional PoUW | On-chain execution as opt-in extension. PoUW, TEE, ZK-ML are **future** — not current. |

---

## License

Apache License 2.0 — see [LICENSE](./LICENSE).
