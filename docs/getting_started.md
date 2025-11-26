# Getting Started with Mbongo Chain

Welcome! This guide will help you set up and run Mbongo Chain in just a few minutes.

---

## 1. Overview

**Mbongo Chain** is a Rust-native Layer 1 blockchain powered by Proof of Stake and Proof of Useful Work. It is designed for high-performance compute markets, decentralized GPU coordination, and secure execution.

---

## 2. System Requirements

Before you begin, ensure you have:

- **Operating System:** Windows 10+, macOS 10.15+, or Linux
- **Rust:** Stable toolchain (1.70+)
- **Git:** Any recent version

---

## 3. Install Rust

Open a terminal and run:

```shell
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

On Windows, download and run the installer from: https://rustup.rs

After installation, restart your terminal and verify:

```shell
rustc --version
cargo --version
```

---

## 4. Clone the Repository

```shell
git clone https://github.com/gkalombo21/mbongo-chain.git
cd mbongo-chain
```

---

## 5. Build the Project

Build all modules:

```shell
cargo build --workspace
```

This compiles the entire Mbongo Chain workspace.

---

## 6. Run the Node

Start the node in development mode:

```shell
cargo run -p node
```

The node will initialize and begin processing.

---

## 7. Run the CLI

View available CLI commands:

```shell
cargo run -p cli -- help
```

Example commands:

```shell
cargo run -p cli -- version
cargo run -p cli -- info
```

---

## 8. Run Tests

Run all tests across the workspace:

```shell
cargo test --workspace
```

All tests should pass. If any fail, check the [Troubleshooting](developer_guide.md#10-troubleshooting) section.

---

## 9. Folder Structure

```
mbongo-chain/
├── cli/        → Command-line interface
├── crypto/     → Cryptographic primitives
├── network/    → P2P networking
├── node/       → Full node implementation
├── pow/        → Proof of Useful Work
├── runtime/    → Execution engine
├── docs/       → Documentation
├── spec/       → Protocol specifications
└── scripts/    → Automation tools
```

---

## 10. Where to Go Next

Now that you're set up, explore these resources:

| Resource | Description |
|----------|-------------|
| [Developer Guide](developer_guide.md) | In-depth development instructions |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | How to contribute to the project |
| [README.md](../README.md) | Project overview and features |

### Security Contact

Found a vulnerability? **Do not open a public issue.**

Report privately to: **security@mbongo.money**

---

**Welcome to Mbongo Chain!** 🚀

