# Mbongo Chain Developer Guide

Welcome to the official **Mbongo Chain Developer Guide**. This document provides everything you need to build, run, test, and contribute to the Mbongo Chain blockchain.

---

## 1. Introduction

### What is Mbongo Chain?

Mbongo Chain is a **Rust-native, compute-first Layer 1 blockchain** powered by a hybrid **Proof of Stake (PoS) + Proof of Useful Work (PoUW)** consensus mechanism.

Designed for high-performance execution, Mbongo Chain targets:

- AI compute markets
- Decentralized GPU coordination
- Secure, deterministic execution
- Global-scale compute validation

### Architecture Overview

Mbongo Chain follows a **modular monorepo architecture** with strict separation of concerns:

```
┌─────────────────────────────────────────────────┐
│                     CLI                         │
│              (User Interface)                   │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────┐
│                    NODE                         │
│         (Orchestration & Coordination)          │
└──────┬──────────────┬──────────────┬────────────┘
       │              │              │
┌──────▼──────┐ ┌─────▼─────┐ ┌──────▼──────┐
│   RUNTIME   │ │  NETWORK  │ │    POW      │
│ (Execution) │ │   (P2P)   │ │  (Compute)  │
└──────┬──────┘ └───────────┘ └──────┬──────┘
       │                             │
       └──────────┬──────────────────┘
                  │
          ┌───────▼───────┐
          │    CRYPTO     │
          │ (Primitives)  │
          └───────────────┘
```

### Module Dependencies

| Module   | Dependencies                     |
|----------|----------------------------------|
| crypto   | None (standalone)                |
| network  | None (standalone)                |
| pow      | crypto                           |
| runtime  | crypto                           |
| node     | runtime, network, crypto, pow    |
| cli      | node                             |

---

## 2. Repository Structure

```
mbongo-chain/
├── cli/                # Command-line interface
│   └── src/
│       └── main.rs     # CLI entry point
├── crypto/             # Cryptographic primitives
│   └── src/
│       └── lib.rs      # Hashing, signatures, keypairs
├── network/            # P2P networking layer
│   └── src/
│       └── lib.rs      # Peer discovery, message passing
├── node/               # Full node implementation
│   └── src/
│       └── lib.rs      # Node orchestration, block processing
├── pow/                # Proof of Useful Work module
│   └── src/
│       └── lib.rs      # Compute validation, PoUW logic
├── runtime/            # Deterministic execution runtime
│   └── src/
│       └── lib.rs      # State machine, transaction execution
├── docs/               # Developer documentation
├── spec/               # Protocol specifications
├── scripts/            # Automation and tooling
├── .github/            # GitHub workflows and templates
├── Cargo.toml          # Workspace manifest
├── README.md           # Project overview
└── CONTRIBUTING.md     # Contribution guidelines
```

---

## 3. Development Prerequisites

### Required Tools

| Tool       | Purpose                     | Installation                        |
|------------|-----------------------------|-------------------------------------|
| **Rust**   | Programming language        | https://rustup.rs                   |
| **Cargo**  | Package manager (with Rust) | Bundled with Rust                   |
| **Rustfmt**| Code formatter              | `rustup component add rustfmt`      |
| **Clippy** | Linter                      | `rustup component add clippy`       |
| **Git**    | Version control             | https://git-scm.com                 |

### Verify Installation

```shell
rustc --version
cargo --version
rustfmt --version
cargo clippy --version
```

### Recommended Tools

- **Cursor Editor** — AI-assisted development
- **VS Code** with rust-analyzer extension
- **Make** — for running dev commands (optional)

---

## 4. How to Build the Project

### Clone the Repository

```shell
git clone https://github.com/gkalombo21/mbongo-chain.git
cd mbongo-chain
```

### Build All Modules

```shell
cargo build --workspace
```

### Build in Release Mode

```shell
cargo build --workspace --release
```

### Build a Specific Module

```shell
cargo build -p node
cargo build -p runtime
cargo build -p crypto
```

---

## 5. How to Run the Node

### Run the Node (Development)

```shell
cargo run -p node
```

### Run the CLI

```shell
cargo run -p cli
```

### CLI Commands

```shell
cargo run -p cli -- version    # Print version
cargo run -p cli -- info       # Print workspace info
cargo run -p cli -- run        # Start node (future)
```

---

## 6. How to Run Tests

### Run All Tests

```shell
cargo test --workspace
```

### Run Tests for a Specific Module

```shell
cargo test -p node
cargo test -p runtime
cargo test -p crypto
cargo test -p network
cargo test -p pow
cargo test -p cli
```

### Run Tests with Output

```shell
cargo test --workspace -- --nocapture
```

---

## 7. Code Standards

### Formatting (Rustfmt)

All code must be formatted with `rustfmt`:

```shell
cargo fmt --all
```

Check formatting without modifying:

```shell
cargo fmt --all -- --check
```

### Linting (Clippy)

All warnings are treated as errors:

```shell
cargo clippy --workspace --all-targets -- -D warnings
```

### Module Boundaries

Strict dependency rules must be followed:

| Rule | Description |
|------|-------------|
| ✅ | `crypto` and `network` are standalone (no internal dependencies) |
| ✅ | `pow` and `runtime` depend only on `crypto` |
| ✅ | `node` depends on `runtime`, `network`, `crypto`, `pow` |
| ✅ | `cli` depends only on `node` |
| ❌ | Circular dependencies are forbidden |
| ❌ | Modules must not bypass their dependency layer |

### Documentation

- All public functions must have doc comments (`///`)
- All public structs and enums must be documented
- Use `cargo doc --workspace --open` to generate and view docs

---

## 8. How to Use Cursor with This Repository

[Cursor](https://cursor.sh) is an AI-powered code editor that enhances development productivity.

### Setup

1. Install Cursor from https://cursor.sh
2. Open the `mbongo-chain` folder
3. Cursor will automatically detect the Rust workspace

### Using AI Assistance

- Use **Cmd+K** (Mac) or **Ctrl+K** (Windows/Linux) to invoke AI
- Ask questions about the codebase
- Request code generation, refactoring, or explanations

### Agent Roles

The repository is configured with specialized AI agent roles:

| Agent | Responsibility |
|-------|----------------|
| Lead Architect | Architecture enforcement, module boundaries |
| TestSprite | Workspace validation, simulated checks |
| DevOps Agent | CI/CD, automation, scripts |
| Module Agents | Per-module development (node, runtime, etc.) |

### Tips

- Always run `cargo check` before committing
- Use AI to explain unfamiliar Rust patterns
- Let agents validate changes before pushing

---

## 9. How to Submit PRs

### Workflow

1. **Fork** the repository
2. **Create a branch** from `develop`:
   ```shell
   git checkout -b feature/my-feature develop
   ```
3. **Make changes** following code standards
4. **Run checks**:
   ```shell
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
5. **Commit** using Conventional Commits:
   ```
   feat: add new runtime interface
   fix: resolve block validation bug
   docs: update developer guide
   ```
6. **Push** your branch:
   ```shell
   git push origin feature/my-feature
   ```
7. **Open a Pull Request** against `develop`

### PR Checklist

- [ ] Code compiles without errors
- [ ] All tests pass
- [ ] Code is formatted (`cargo fmt`)
- [ ] No Clippy warnings
- [ ] Documentation updated (if applicable)
- [ ] Module boundaries respected
- [ ] Commit messages follow conventions

### Review Process

- PRs require at least one maintainer approval
- CI must pass before merging
- Address all review comments before re-requesting review

---

## 10. Troubleshooting

### Build Errors

**Problem:** `cargo build` fails with missing dependencies

**Solution:**
```shell
cargo update
cargo build --workspace
```

---

**Problem:** Rust version mismatch

**Solution:**
```shell
rustup update stable
rustup default stable
```

---

### Test Failures

**Problem:** Tests fail unexpectedly

**Solution:**
```shell
cargo clean
cargo test --workspace
```

---

### Clippy Warnings

**Problem:** Clippy reports warnings

**Solution:**
```shell
cargo clippy --workspace --fix --allow-dirty
```

---

### Module Boundary Violations

**Problem:** Import error when using another module

**Solution:**
- Check the dependency rules in Section 7
- Ensure `Cargo.toml` has the correct dependencies
- Refactor to respect module boundaries

---

### Formatting Issues

**Problem:** CI fails on formatting check

**Solution:**
```shell
cargo fmt --all
git add -A
git commit --amend --no-edit
```

---

## 11. Contact Information

### Security Vulnerabilities

**Do NOT report security issues in public GitHub issues.**

Report vulnerabilities privately through GitHub private vulnerability reporting, as
[`SECURITY.md`](../.github/SECURITY.md) describes.

We follow coordinated disclosure, and will acknowledge your report within 72 hours.

---

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.
