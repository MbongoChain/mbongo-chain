# Mbongo Chain — Getting Started

Get your development environment ready in 5 minutes.

---

## 1. Introduction

### Purpose of This Guide

This guide walks you through setting up the Mbongo Chain development environment from scratch. By the end, you'll have a fully functional workspace ready for development and contribution.

### What You'll Accomplish

In approximately 5 minutes, you will:

- ✅ Install the Rust toolchain
- ✅ Clone and build the Mbongo Chain workspace
- ✅ Validate code formatting and linting
- ✅ Understand the project structure
- ✅ Be ready to contribute

---

## 2. Requirements

### Rust Toolchain Installation (Windows)

**Option 1: Download Installer**

1. Visit https://rustup.rs
2. Download `rustup-init.exe`
3. Run the installer and follow prompts
4. Restart your terminal

**Option 2: PowerShell Command**

```powershell
# Download and run rustup installer
Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
.\rustup-init.exe -y
Remove-Item rustup-init.exe

# Restart PowerShell after installation
```

**Verify Installation:**

```powershell
rustc --version
# Expected: rustc 1.XX.X (XXXXXXXX YYYY-MM-DD)

cargo --version
# Expected: cargo 1.XX.X (XXXXXXXX YYYY-MM-DD)
```

### Developer Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| **Clippy** | Rust linter | `rustup component add clippy` |
| **Rustfmt** | Code formatter | `rustup component add rustfmt` |

```powershell
# Install both tools
rustup component add clippy rustfmt

# Verify installation
cargo clippy --version
cargo fmt --version
```

### Git

**Install Git for Windows:**

1. Download from https://git-scm.com/download/win
2. Run the installer (use default settings)
3. Restart your terminal

**Verify:**

```powershell
git --version
# Expected: git version 2.XX.X.windows.X
```

### PowerShell 7+ (Recommended)

PowerShell 7 provides better compatibility and features.

```powershell
# Check current version
$PSVersionTable.PSVersion

# Install PowerShell 7 (if needed)
winget install Microsoft.PowerShell
```

### Optional: GPU Drivers for PoUW Testing

For future PoUW compute testing:

| GPU Vendor | Driver | Download |
|------------|--------|----------|
| NVIDIA | CUDA Toolkit | https://developer.nvidia.com/cuda-downloads |
| AMD | ROCm | https://rocm.docs.amd.com |

*Note: GPU support is planned for future releases.*

---

## 3. Repository Setup

### Step 1: Clone the Repository

```powershell
git clone https://github.com/gkalombo21/mbongo-chain.git
```

### Step 2: Enter the Project Directory

```powershell
cd mbongo-chain
```

### Step 3: Build the Workspace

```powershell
cargo build --workspace
```

Expected output:
```
   Compiling crypto v0.1.0 (C:\Dev\mbongo-chain\crypto)
   Compiling network v0.1.0 (C:\Dev\mbongo-chain\network)
   Compiling runtime v0.1.0 (C:\Dev\mbongo-chain\runtime)
   Compiling pow v0.1.0 (C:\Dev\mbongo-chain\pow)
   Compiling node v0.1.0 (C:\Dev\mbongo-chain\node)
   Compiling cli v0.1.0 (C:\Dev\mbongo-chain\cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

### Step 4: Validate Formatting

```powershell
cargo fmt --all -- --check
```

Expected output (if formatted correctly):
```
(no output - all files are formatted)
```

### Step 5: Validate Clippy

```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

Expected output:
```
    Checking crypto v0.1.0
    Checking network v0.1.0
    ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

### Step 6: Run Unit Tests

```powershell
cargo test --workspace
```

*Note: Test coverage is being expanded. Some modules may have placeholder tests.*

---

## 4. First Build (Workspace)

### Build Command

```powershell
cargo build --workspace
```

### Expected Output

A successful build shows all modules compiling:

```
   Compiling crypto v0.1.0
   Compiling network v0.1.0
   Compiling pow v0.1.0
   Compiling runtime v0.1.0
   Compiling node v0.1.0
   Compiling cli v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.32s
```

### Build in Release Mode

For optimized builds:

```powershell
cargo build --workspace --release
```

### Troubleshooting Common Errors

#### Error: Rust Version Mismatch

```
error: package `some-crate` requires rustc 1.70.0 or newer
```

**Fix:**
```powershell
rustup update stable
rustup default stable
```

#### Error: Missing Toolchain

```
error: toolchain 'stable-x86_64-pc-windows-msvc' is not installed
```

**Fix:**
```powershell
rustup install stable
rustup default stable
```

#### Error: Missing MSVC Build Tools

```
error: linker `link.exe` not found
```

**Fix:**
1. Download Visual Studio Build Tools from https://visualstudio.microsoft.com/downloads/
2. Install "Desktop development with C++"
3. Restart your terminal

#### Error: Cargo.lock Conflict

```
error: failed to select a version for `some-crate`
```

**Fix:**
```powershell
cargo update
cargo build --workspace
```

---

## 5. Developer Tools Quick Validation

### Rustfmt (Code Formatter)

**What it checks:**
- Consistent code formatting
- Proper indentation
- Line length limits
- Import ordering

**Command:**
```powershell
cargo fmt --all -- --check
```

**Output if unformatted:**
```
Diff in C:\Dev\mbongo-chain\runtime\src\lib.rs at line 42:
     fn example() {
-        let x=1;
+        let x = 1;
     }
```

**To fix formatting:**
```powershell
cargo fmt --all
```

### Clippy (Linter)

**What it checks:**
- Common programming mistakes
- Unidiomatic Rust patterns
- Performance issues
- Potential bugs
- Code complexity

**Command:**
```powershell
cargo clippy --workspace --all-targets -- -D warnings
```

**Output if issues found:**
```
warning: unused variable: `x`
  --> runtime\src\lib.rs:42:9
   |
42 |     let x = 1;
   |         ^ help: if this is intentional, prefix it with an underscore: `_x`
```

**Common Clippy Fixes:**

| Warning | Fix |
|---------|-----|
| `unused_variable` | Prefix with `_` or remove |
| `needless_return` | Remove explicit `return` |
| `clone_on_copy` | Remove `.clone()` on Copy types |
| `redundant_closure` | Use function reference directly |

---

## 6. Running the Placeholder Node

### Current Status

The Mbongo Chain node and runtime execution engine are **under active development**. Full node functionality will be available in future releases.

### Available Commands (Current)

```powershell
# Show CLI help
cargo run -p cli -- --help

# Show version
cargo run -p cli -- version

# Show workspace info
cargo run -p cli -- info
```

### Future Node Commands (Planned)

Once implemented, the node will support:

```powershell
# Start the node
cargo run -p node -- --config config.toml

# Or using the CLI
cargo run -p cli -- run --data-dir ./data

# Start with specific options
cargo run -p cli -- run --rpc-port 8545 --p2p-port 30303
```

### Development Workflow

While node execution is being developed:

1. **Focus on module development**: Work on individual crates
2. **Run unit tests**: `cargo test -p <module>`
3. **Build and validate**: Use Clippy and Rustfmt
4. **Read architecture docs**: Understand the design before implementing

---

## 7. Project Structure Summary

```
mbongo-chain/
├── cli/       Command-line interface binary
├── crypto/    Cryptographic primitives (hashing, signatures)
├── docs/      Developer documentation
├── network/   P2P networking layer
├── node/      Node orchestration and coordination
├── pow/       Proof of Useful Work verification
├── runtime/   State machine and execution engine
├── scripts/   Development and CI scripts
└── spec/      Protocol specifications
```

### Quick Module Reference

| Module | Purpose | Binary |
|--------|---------|--------|
| `cli` | User commands | ✓ (main.rs) |
| `node` | Orchestration | Library |
| `runtime` | Execution | Library |
| `network` | P2P | Library |
| `crypto` | Cryptography | Library |
| `pow` | Compute verification | Library |

*For detailed module information, see [developer_introduction.md](developer_introduction.md).*

---

## 8. Troubleshooting

### Rust Toolchain Not Installed

**Error:**
```
'rustc' is not recognized as an internal or external command
```

**Fix:**
1. Install Rust from https://rustup.rs
2. Restart your terminal
3. Verify: `rustc --version`

### Cargo Build Fails - Missing Components

**Error:**
```
error: component 'cargo' for target 'x86_64-pc-windows-msvc' is unavailable
```

**Fix:**
```powershell
rustup install stable
rustup default stable
rustup component add cargo
```

### Clippy Not Installed

**Error:**
```
error: no such command: `clippy`
```

**Fix:**
```powershell
rustup component add clippy
```

### Rustfmt Not Installed

**Error:**
```
error: no such command: `fmt`
```

**Fix:**
```powershell
rustup component add rustfmt
```

### Windows PATH Issues

**Error:**
```
'cargo' is not recognized as an internal or external command
```

**Fix:**
1. Open System Properties → Environment Variables
2. Add to PATH: `%USERPROFILE%\.cargo\bin`
3. Restart your terminal

**Or via PowerShell:**
```powershell
$env:PATH += ";$env:USERPROFILE\.cargo\bin"
# To make permanent, add to your PowerShell profile
```

### Git Not Found

**Error:**
```
'git' is not recognized as an internal or external command
```

**Fix:**
1. Download Git from https://git-scm.com/download/win
2. Install with default options
3. Restart your terminal
4. Verify: `git --version`

### Permission Denied Errors

**Error:**
```
error: Permission denied (os error 5)
```

**Fix:**
1. Run PowerShell as Administrator
2. Or change target directory permissions
3. Ensure antivirus isn't blocking cargo

### Disk Space Issues

**Error:**
```
error: failed to write to `target/...`
```

**Fix:**
1. Check available disk space
2. Clean build artifacts: `cargo clean`
3. Rebuild: `cargo build --workspace`

### Network/Download Issues

**Error:**
```
error: failed to download `crate-name`
```

**Fix:**
1. Check internet connection
2. Try again: `cargo build --workspace`
3. If behind proxy, configure cargo:

```powershell
# Create/edit ~/.cargo/config.toml
[http]
proxy = "http://proxy.example.com:8080"
```

---

## 9. Summary

**Congratulations!** You have successfully set up the Mbongo Chain development environment.

### You Are Now Ready To:

- ✅ **Read architecture docs**: Start with [final_architecture_overview.md](final_architecture_overview.md)
- ✅ **Contribute**: Follow the [CONTRIBUTING.md](../CONTRIBUTING.md) guidelines
- ✅ **Run validation scripts**: Use `cargo fmt`, `cargo clippy`, `cargo test`
- ✅ **Explore the codebase**: Each module is well-documented
- ✅ **Prepare for implementation**: The runtime and node are under active development

### Next Steps

1. **Read** [developer_introduction.md](developer_introduction.md) for a deeper overview
2. **Explore** the architecture in [final_architecture_overview.md](final_architecture_overview.md)
3. **Pick an issue** from the issue tracker to start contributing
4. **Join the community** and ask questions

### Quick Command Reference

| Task | Command |
|------|---------|
| Build all | `cargo build --workspace` |
| Run tests | `cargo test --workspace` |
| Format code | `cargo fmt --all` |
| Check format | `cargo fmt --all -- --check` |
| Run Clippy | `cargo clippy --workspace --all-targets -- -D warnings` |
| Clean build | `cargo clean` |
| Update deps | `cargo update` |
| Show CLI help | `cargo run -p cli -- --help` |

---

**Welcome to Mbongo Chain!** 🚀

**Mbongo Chain** — Compute-first blockchain infrastructure for the global future.
