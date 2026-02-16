# Contribution Tiers — Devnet Stable

**Status:** ACTIVE
**Applies to:** `v0.2-devnet-stable` and all work on `dev` branch
**Last updated:** 2026-02-16

---

## What Is Locked vs What Contributors May Change

The protocol lock ([PROTOCOL_LOCK_v0.2.md](specs/PROTOCOL_LOCK_v0.2.md)) divides the codebase into frozen surfaces and open surfaces. This document maps those boundaries to concrete contributor tiers so every PR author knows the rules before they open a PR.

| | Locked (RFC required) | Open (no RFC) |
|-|----------------------|---------------|
| **Block/tx SCALE encoding** | Field set, field order, codec | — |
| **Hashing** | BLAKE3 inputs, Merkle scheme, display format | — |
| **`apply_block` rules** | All five validity rules, atomic `write_batch` | — |
| **Storage trait semantics** | `get_block_by_height`, `get_latest_height`, batch atomicity | Internal refactors preserving semantics |
| **P2P wire formats** | `SyncRequest`, `SyncResponse`, `SyncNotification`, `BlockNotifyAck`, frame encoding, protocol strings, `MAX_RANGE` | — |
| **RPC v0.1** | Method names, param types, return types, error codes | New non-conflicting endpoints (requires spec addendum) |
| **Everything else** | — | Docs, tooling, CI, logging, metrics, SDK, CLI flags, test harnesses, internal refactors |

If your change touches anything in the "Locked" column, stop and file an RFC first. See [RFC_PROCESS.md](RFC_PROCESS.md).

---

## Tier Definitions

### Tier 0 — Core

**Scope:** Protocol, storage, network, `apply_block`.

| Crate / area | Examples |
|--------------|----------|
| `mbongo-core` | `Block`, `Transaction`, `Hash`, `Address`, SCALE derives, `compute_transactions_root` |
| `mbongo-storage` | `Storage` trait, `RocksDbStorage`, `MemoryStorage`, `write_batch`, key schemas |
| `mbongo-network` | `p2p.rs`, `p2p_protocol.rs`, `SyncCodec`, `BlockNotifyCodec`, protocol strings |
| `mbongo-node/backend.rs` | `apply_block`, `produce_block`, `ensure_genesis`, block validity checks |
| `mbongo-node/sync_service.rs` | Inbound sync request handling, response construction |

**Rules:**
- RFC required for any change to a locked surface.
- At least **one Core Maintainer** approval on every PR (author cannot self-approve).
- Two Core Maintainer approvals if the PR touches multiple locked surfaces.
- PR description must reference the specific locked surface and link the RFC if applicable.

### Tier 1 — Devnet

**Scope:** Node orchestration, harnesses, metrics, logging — anything that affects runtime behaviour but does NOT alter locked protocol surfaces.

| Crate / area | Examples |
|--------------|----------|
| `mbongo-node/main.rs` | Sync orchestrator, CLI arg additions, task wiring |
| `mbongo-node/mempool.rs` | Eviction policy, ordering heuristics (not validation rules) |
| `mbongo-node/bin/devnet_harness.rs` | Convergence harness, replay harness |
| Logging | Log levels, log format, `RUST_LOG` defaults |
| Metrics | Prometheus endpoints, counter/gauge additions |
| Node configuration | New CLI flags that do not alter protocol behaviour |

**Rules:**
- No RFC required (unless the change accidentally touches a locked surface).
- At least **one reviewer** approval (Core Maintainer or experienced contributor).
- Must not break existing devnet harness (`cargo run --bin devnet_harness` must pass).
- Must not introduce new clippy warnings.

### Tier 2 — Tooling

**Scope:** SDK, CLI tools, explorer, documentation, CI, scripts.

| Area | Examples |
|------|----------|
| Documentation | All files under `docs/`, README, architecture guides |
| CI / CD | `.github/workflows/`, `scripts/` |
| SDK | `mbongo-wallet`, client libraries, examples |
| CLI tools | Non-node binaries, utility scripts |
| Explorer / UI | Front-end code (when it exists) |
| Tests | New test cases that do not modify production code |

**Rules:**
- No RFC required.
- At least **one reviewer** approval (any contributor with merge rights).
- Documentation PRs should verify that relative links resolve.
- CI changes must not weaken the existing gate (fmt + clippy + test must remain mandatory).

---

## PR Rules (All Tiers)

### Branch target

All PRs target `dev`. The `main` branch is protected and only receives audited milestone merges.

### CI gate

Every PR must pass before merge:

```
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

No exceptions. A red CI blocks merge regardless of tier.

### Protocol lock enforcement

- Any PR that modifies a file in Tier 0 scope must include a statement in the PR description: either "No locked surfaces affected" with justification, or a link to the governing RFC.
- PRs without this statement that touch Tier 0 files will be sent back for revision.
- Reviewers must verify the statement before approving.

### Approval requirements summary

| Tier | Minimum approvals | Who may approve |
|------|-------------------|-----------------|
| Tier 0 — Core | 1 Core Maintainer (2 if multi-surface) | Core Maintainers only |
| Tier 1 — Devnet | 1 reviewer | Core Maintainer or experienced contributor |
| Tier 2 — Tooling | 1 reviewer | Any contributor with merge rights |

### Commit conventions

Follow [Conventional Commits](https://www.conventionalcommits.org/) as defined in [CONTRIBUTING.md](CONTRIBUTING.md). Scope must reflect the tier: `core`, `storage`, `network` for Tier 0; `node`, `harness`, `metrics` for Tier 1; `docs`, `ci`, `sdk`, `cli` for Tier 2.

---

## Related Documents

| Document | Purpose |
|----------|---------|
| [PROTOCOL_LOCK_v0.2.md](specs/PROTOCOL_LOCK_v0.2.md) | Defines exactly which surfaces are frozen |
| [RFC_PROCESS.md](RFC_PROCESS.md) | How to propose a change to a locked surface |
| [DEV_ONBOARDING.md](DEV_ONBOARDING.md) | Build, test, and run instructions for new contributors |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Branching strategy, commit conventions, general PR rules |
