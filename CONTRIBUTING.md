# Contributing to Mbongo Chain

This document defines contribution rules for Mbongo Chain. All contributors must comply with these requirements.

**Canonical references:**
- [PROTOCOL_LOCK_v0.2.md](./docs/specs/PROTOCOL_LOCK_v0.2.md) — Frozen surfaces and forbidden changes
- [CONTRIBUTION_TIERS.md](./docs/CONTRIBUTION_TIERS.md) — Tier 0/1/2 change rules
- [RFC_PROCESS.md](./docs/RFC_PROCESS.md) — How to propose changes to locked surfaces

---

## Development Philosophy

Mbongo Chain is a deterministic verification layer for off-chain AI inference receipts. The protocol does not execute AI models on-chain. Validators verify receipts deterministically and settle economic outcomes.

**Principles:**
- **Determinism is mandatory.** Given the same inputs and chain state, every node must reach the same result. No non-deterministic behavior in protocol-critical paths.
- **Protocol stability.** v0.2-devnet-stable is frozen. Changes to locked surfaces require an RFC and version bump.
- **Layer discipline.** Respect crate boundaries. See [ARCHITECTURE_GUARDRAILS.md](./docs/ARCHITECTURE_GUARDRAILS.md).

---

## Contribution Scope

### Locked Surfaces (RFC Required)

The following require an RFC before any change:

| Surface | Examples |
|---------|----------|
| Block/tx SCALE encoding | Field set, field order, codec |
| Hashing | BLAKE3 inputs, Merkle scheme, display format |
| `apply_block` rules | All five validity rules, atomic `write_batch` |
| Storage trait semantics | `get_block_by_height`, `get_latest_height`, batch atomicity |
| P2P wire formats | SyncRequest, SyncResponse, SyncNotification, protocol strings |
| RPC v0.1 | Method names, param types, return types, error codes |

If your change touches a locked surface, file an RFC in `docs/rfcs/` and obtain approval before implementing. See [RFC_PROCESS.md](./docs/RFC_PROCESS.md).

### Open Surfaces (No RFC)

Docs, tooling, CI, logging, metrics, SDK, CLI flags, test harnesses, internal refactors that preserve locked semantics.

---

## How to Contribute

### Bugs

1. Search existing issues. Open a new issue if none exists.
2. Include: reproduction steps, environment (OS, Rust version), expected vs actual behavior, logs.
3. Submit a PR targeting `dev` with a fix. Reference the issue.

### Features

1. **Locked surface:** File an RFC first. Do not implement until RFC is accepted.
2. **Open surface:** Open an issue for discussion. Implement and submit PR targeting `dev`.

### Documentation

1. Identify gaps in [docs/](./docs).
2. Submit PR with changes. Ensure links are valid and formatting is correct.

### Tests

1. Add unit tests for new logic. Add integration tests for new features.
2. Ensure `cargo test --workspace` passes. No regressions.

---

## Branching Model

- **`main`:** Reserved for audited, stable milestones. Protected. Not a development branch.
- **`dev`:** Active development. **All PRs must target `dev`.** No exceptions.

Rebase on `dev` before opening a PR:

```powershell
git fetch origin
git rebase origin/dev
```

---

## Rust Toolchain

- **Rust:** 1.75 or higher
- **Components:** `rustfmt`, `clippy`

```powershell
rustup update stable
rustup component add rustfmt clippy
```

### Build and Test

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

All checks must pass before opening a PR.

---

## Code Requirements

### Determinism

Protocol-critical code must be deterministic. Prohibited in `apply_block`, block production, and state transitions:

- Random number generation (unless explicitly specified in protocol)
- System time for logic decisions (timestamp in block header is allowed)
- Non-deterministic floating point
- Unordered iteration over `HashMap`/`HashSet` where order affects output

### Formatting and Linting

- `cargo fmt --all` — Format before commit
- `cargo clippy --workspace -- -D warnings` — Zero warnings policy

### Error Handling

- Use `Result<T, E>` for fallible operations. Use `?` for propagation.
- No `unwrap()` or `expect()` in production paths. Tests only.
- No `unsafe` without explicit justification and review.

### Commit Messages

Conventional commits: `type(scope): subject`

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `test`: Tests
- `refactor`: Code restructuring
- `chore`: Build, CI, dependencies

---

## Licensing

By contributing, you agree that your contributions are licensed under the [Apache License 2.0](./LICENSE).

---

## Security Disclosure

Report security vulnerabilities to **info@mbongochain.org**. Do not open public issues for security vulnerabilities.

Include: description of the vulnerability, steps to reproduce, potential impact, suggested fix (if any).

---

## Tier Labels

PRs may be labeled by change scope:

- **tier-0:** Core protocol, storage, network, `apply_block`. RFC required. Core Maintainer approval.
- **tier-1:** Node orchestration, harnesses, metrics. No RFC. Reviewer approval. Must not break devnet harness.
- **tier-2:** Docs, tooling, CI, SDK. No RFC. Standard review.

See [CONTRIBUTION_TIERS.md](./docs/CONTRIBUTION_TIERS.md) for full rules.

---

## Links

- [README](./README.md)
- [DEV_ONBOARDING.md](./docs/DEV_ONBOARDING.md) — Quick start, CLI reference
- [VISION_v1.md](./docs/VISION_v1.md) — Verification layer scope
- [PROTOCOL_LOCK_v0.2.md](./docs/specs/PROTOCOL_LOCK_v0.2.md)
- [RFC_PROCESS.md](./docs/RFC_PROCESS.md)
- [CONTRIBUTION_TIERS.md](./docs/CONTRIBUTION_TIERS.md)
