# Mbongo Chain - AI Assistant Context

This file provides context about the Mbongo Chain project for AI coding assistants (Claude, GitHub Copilot, etc.) to better understand the codebase and assist developers.

---

## Project Overview

**Name**: Mbongo Chain
**Type**: Layer 1 Blockchain
**Language**: Rust
**Status**: Pre-testnet (Development Phase)
**Target**: Decentralized GPU compute for AI/ML workloads

**Core Mission**: Democratize access to GPU compute through a decentralized, verifiable network that makes AI infrastructure 30-50% cheaper than AWS/GCP while maintaining trustless verification.

---

## What Makes This Project Unique

### 1. PoX Consensus (Proof of X)

**Hybrid mechanism combining:**
- **Proof of Stake (PoS)**: Economic security through MBO token staking
- **Proof of Useful Work (PoUW)**: Validators earn rewards by executing real AI/ML workloads
- **Proof of Compute (PoC)**: Scoring system for computational contributions

**Formula:**
```
total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)
```

Where C_SR and C_NL are dynamically adjusted by AIDA.

### 2. AIDA Regulator

**Adaptive Intelligence for Dynamic Adjustment** - an on-chain economic regulator that:
- Monitors stake-to-work ratio in real-time
- Dynamically adjusts consensus coefficients (C_SR, C_NL) between 0.8-1.2
- Maintains 50/50 balance between stake and compute work
- Prevents centralization automatically (no manual governance needed)

### 3. Multi-Layer Verification

**Progressive security strategy:**
- **Phase 1** (Current): Redundant execution (3 validators verify each task)
- **Phase 2** (Q2 2025): TEE attestation (Intel SGX, AMD SEV)
- **Phase 3** (Q4 2025): ZK-ML proofs (zero-knowledge verification)

Plus optimistic fraud proofs with 100-block challenge period.

### 4. Anti-Centralization Design

**Square root function on PoC scores** creates diminishing returns:
- Large providers earn 3.16× more by splitting into 10 nodes vs 1 large node
- Economic incentive for decentralization built into the protocol

---

## Tech Stack

### Core Blockchain

```yaml
Language: Rust (1.75+)
Consensus: PoX (custom, Proof of Stake + Proof of Useful Work)
Block Time: 6 seconds
Finality: BFT (Byzantine Fault Tolerant)
State Model: Account-based (like Ethereum)
VM: WebAssembly (WASM) for smart contracts
```

### Key Dependencies

```toml
# Async runtime
tokio = "1.35"

# Serialization
serde = { version = "1.0", features = ["derive"] }

# Cryptography
ed25519-dalek = "2.1"  # Signing
blake3 = "1.5"          # Hashing

# Networking
libp2p = "0.53"         # P2P networking

# Storage
rocksdb = "0.22"        # Database

# WASM runtime
wasmtime = "16.0"       # Smart contracts

# Error handling
thiserror = "1.0"

# Testing
criterion = "0.5"       # Benchmarking
```

### Crate Structure

```
mbongo-chain/
├── crates/
│   ├── mbongo-core/          # Blockchain primitives (types, crypto, storage)
│   ├── mbongo-consensus/     # PoX consensus engine + AIDA regulator
│   ├── mbongo-verification/  # Compute verification (redundant/TEE/ZK)
│   ├── mbongo-compute/       # GPU task execution runtime
│   ├── mbongo-network/       # P2P networking (libp2p)
│   ├── mbongo-runtime/       # WASM VM for smart contracts
│   ├── mbongo-api/           # REST/WebSocket APIs
│   ├── mbongo-wallet/        # Key management and signing
│   └── mbongo-node/          # Full node binary
```

---

## Key Documentation Files

### Essential Reading (in order of importance)

1. **README.md** - Project overview, quick start, features
2. **docs/pox_formula.md** - Complete PoX consensus mathematics
3. **docs/verification_strategy.md** - Multi-layer verification approach
4. **docs/consensus_mechanics.md** - PoUW consensus specification
5. **docs/sybil_resistance.md** - Anti-Sybil attack mechanisms
6. **docs/target_market.md** - Customer segments and use cases
7. **docs/competitive_analysis.md** - vs Render/Akash/io.net/Gensyn
8. **CONTRIBUTING.md** - Code style, PR process, development workflow

### Architecture Documentation

- **docs/architecture_overview.md** - System architecture
- **docs/node_architecture.md** - Node design
- **docs/runtime_architecture.md** - WASM runtime

### Setup Guides

- **docs/validator_setup.md** - Becoming a validator
- **docs/compute_provider_setup.md** - Providing GPU compute
- **docs/full_node_setup.md** - Running a full node

### Economic Model

- **docs/tokenomics.md** - Token distribution, emissions (coming soon)
- **docs/economic_model.md** - Incentive design (coming soon)

---

## Coding Conventions

### Rust Style

**Follow standard Rust conventions:**
- Use `rustfmt` for formatting (see `rustfmt.toml`)
- Use `clippy` for linting (`cargo clippy --all -- -D warnings`)
- Max line width: 100 characters
- Indentation: 4 spaces

### Naming

```rust
// Structs/Enums/Traits: PascalCase
pub struct Validator { }
pub enum ConsensusState { }
pub trait Verifiable { }

// Functions/Variables: snake_case
fn calculate_total_weight() { }
let validator_set = Vec::new();

// Constants: SCREAMING_SNAKE_CASE
const MAX_VALIDATORS: usize = 1000;
const BLOCK_TIME_SECONDS: u64 = 6;

// Crates: mbongo-{module}
// Examples: mbongo-consensus, mbongo-network
```

### Error Handling

**Always use `Result<T, E>` for fallible operations:**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid block hash: {0}")]
    InvalidBlockHash(Hash),

    #[error("Insufficient stake: required {required}, have {actual}")]
    InsufficientStake { required: u64, actual: u64 },
}

// Use ? operator for propagation
pub fn validate_block(block: &Block) -> Result<(), ValidationError> {
    verify_signature(block)?;
    verify_hash(block)?;
    Ok(())
}

// Avoid unwrap() in production code (tests are okay)
```

### Documentation

**Document all public APIs with Rustdoc:**

```rust
/// Calculates the total weight of a validator for block production.
///
/// Uses the PoX formula:
/// ```text
/// total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)
/// ```
///
/// # Arguments
///
/// * `stake_weight` - Validator's stake-adjusted weight in MBO
/// * `poc_score` - Proof of compute score (cumulative work)
/// * `c_sr` - AIDA coefficient for stake rewards (0.8 - 1.2)
/// * `c_nl` - AIDA coefficient for network load (0.8 - 1.2)
///
/// # Examples
///
/// ```
/// let weight = calculate_total_weight(10_000.0, 50_000, 1.0, 1.0);
/// ```
pub fn calculate_total_weight(
    stake_weight: f64,
    poc_score: u64,
    c_sr: f64,
    c_nl: f64,
) -> f64 {
    // Implementation
}
```

### Commit Messages

**Follow Conventional Commits:**

```
feat(consensus): implement PoX validator selection algorithm
fix(network): resolve peer discovery race condition
docs(api): add OpenAPI spec for compute endpoints
test(verification): add integration tests for fraud proofs
```

---

## Important Constants and Types

### Token

```rust
// Token symbol: MBO (Mbongo)
// Max supply: 31,536,000 MBO (one token per second per year)
// Decimals: 18 (like Ethereum)

pub const TOKEN_SYMBOL: &str = "MBO";
pub const MAX_SUPPLY: u64 = 31_536_000 * 10u64.pow(18);
pub const TOKEN_DECIMALS: u8 = 18;
```

### Consensus Parameters

```rust
// Block time: 6 seconds
pub const BLOCK_TIME_SECONDS: u64 = 6;

// Validator selection: 3 validators per compute task
pub const DEFAULT_VALIDATORS_PER_TASK: usize = 3;

// AIDA coefficients: dynamic between 0.8 and 1.2
pub const AIDA_MIN_COEFFICIENT: f64 = 0.8;
pub const AIDA_MAX_COEFFICIENT: f64 = 1.2;
pub const AIDA_LEARNING_RATE: f64 = 0.01;

// Fraud proof challenge period: 100 blocks (~10 minutes)
pub const FRAUD_PROOF_CHALLENGE_BLOCKS: u64 = 100;

// Minimum stake per validator: 1,000 MBO
pub const MIN_VALIDATOR_STAKE: u64 = 1_000 * 10u64.pow(18);
```

### Common Types

```rust
// Hash type (32 bytes, Blake3)
pub type Hash = [u8; 32];

// Block number
pub type BlockNumber = u64;

// Validator ID
pub type ValidatorId = [u8; 32];

// Timestamp (Unix milliseconds)
pub type Timestamp = u64;

// Balance (in smallest unit, 18 decimals)
pub type Balance = u128;
```

---

## Current Development Status

### Phase 1 (Current) - Foundation

**Status**: In Development (Q1-Q2 2025)

**Completed:**
- [x] Project structure and documentation
- [x] PoX consensus specification (docs/pox_formula.md)
- [x] Verification strategy (docs/verification_strategy.md)
- [x] Sybil resistance design (docs/sybil_resistance.md)
- [x] Economic model design
- [x] Competitive analysis

**In Progress:**
- [ ] Core blockchain types (`mbongo-core`)
- [ ] PoX consensus engine (`mbongo-consensus`)
- [ ] AIDA regulator implementation
- [ ] Redundant verification framework
- [ ] P2P networking (`mbongo-network`)

**Next Steps:**
- [ ] Testnet launch (Q1 2025)
- [ ] Validator onboarding
- [ ] CLI tools and SDK

### Phase 2 (Planned) - TEE Integration

**Timeline**: Q3-Q4 2025

- [ ] Intel SGX attestation
- [ ] AMD SEV support
- [ ] Hybrid verification (1 TEE + 2 standard validators)
- [ ] Mainnet candidate launch

### Phase 3 (Research) - ZK-ML

**Timeline**: 2026+

- [ ] Zero-knowledge ML proofs
- [ ] Privacy-preserving compute
- [ ] Full production mainnet

---

## Common Tasks and Patterns

### Adding a New Feature

1. **Understand the context**: Read relevant docs (e.g., `docs/pox_formula.md` for consensus changes)
2. **Design first**: For complex features, create a design doc in `docs/`
3. **Write tests first**: TDD approach recommended
4. **Implement**: Follow coding conventions
5. **Document**: Rustdoc + update relevant docs/*.md files
6. **Test**: Run `cargo test --all`, `cargo clippy --all`
7. **PR**: Open pull request with clear description

### Working with PoX Consensus

**Key formula to remember:**
```rust
// Total weight = (stake × C_SR) + (√(PoC) × C_NL)
fn calculate_total_weight(stake_weight: f64, poc_score: u64, c_sr: f64, c_nl: f64) -> f64 {
    let stake_component = stake_weight * c_sr;
    let work_component = (poc_score as f64).sqrt() * c_nl;
    stake_component + work_component
}
```

**AIDA adjustment:**
```rust
// C_SR(t+1) = C_SR(t) + α × (target_ratio - current_ratio)
// C_NL(t+1) = 2.0 - C_SR(t+1)
fn adjust_aida_coefficients(
    current_c_sr: f64,
    target_ratio: f64,
    actual_ratio: f64,
    learning_rate: f64,
) -> (f64, f64) {
    let new_c_sr = (current_c_sr + learning_rate * (target_ratio - actual_ratio))
        .clamp(0.8, 1.2);
    let new_c_nl = 2.0 - new_c_sr;
    (new_c_sr, new_c_nl)
}
```

### Working with Verification

**Redundant execution pattern:**
```rust
// 1. Select 3 validators randomly (weighted by stake × reputation)
let validators = select_validators(&validator_set, 3);

// 2. Assign task to all 3
for validator in &validators {
    assign_task(validator, &task);
}

// 3. Collect results
let results = collect_results(&task, &validators).await;

// 4. Check consensus (2/3 agreement)
if results.iter().filter(|r| r.output == majority_output).count() >= 2 {
    accept_result(majority_output);
} else {
    // No consensus - enter arbitration
    arbitrate(&task, &results);
}
```

### Working with Sybil Resistance

**GPU fingerprinting:**
```rust
// Generate unique GPU fingerprint
let fingerprint = GpuFingerprint {
    uuid: gpu.uuid(),                    // NVIDIA GPU-specific UUID
    pci_device_id: gpu.pci_id(),         // PCI device identifier
    performance_hash: benchmark_gpu(),    // Performance characteristics
};

// Check if already registered (prevent duplicates)
if registry.is_registered(&fingerprint.hash()) {
    return Err(Error::GpuAlreadyRegistered);
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pox_weight_calculation() {
        let weight = calculate_total_weight(10_000.0, 50_000, 1.0, 1.0);
        assert!(weight > 10_000.0);
        assert!(weight < 20_000.0);
    }

    #[test]
    fn test_aida_bounds() {
        let (c_sr, c_nl) = adjust_aida_coefficients(1.0, 0.5, 0.7, 0.01);
        assert!(c_sr >= 0.8 && c_sr <= 1.2);
        assert!(c_nl >= 0.8 && c_nl <= 1.2);
        assert!((c_sr + c_nl - 2.0).abs() < 0.001); // Sum = 2.0
    }
}
```

### Integration Tests

```rust
// tests/integration/consensus.rs
#[tokio::test]
async fn test_validator_selection_distribution() {
    let validator_set = create_test_validators(100);
    let selections = (0..1000)
        .map(|_| select_validators(&validator_set, 3))
        .collect::<Vec<_>>();

    // Verify distribution is roughly proportional to stake × reputation
    verify_selection_fairness(&selections, &validator_set);
}
```

---

## Security Considerations

### Critical Security Areas

1. **Consensus Safety**: Never allow validators to control >2/3 of selection
2. **Cryptographic Verification**: All signatures must be verified before trust
3. **Sybil Resistance**: Multi-layer defense (stake + fingerprint + TEE + behavioral)
4. **Slashing**: Ensure economic penalties deter attacks
5. **Fraud Proofs**: Challenge period must be sufficient (100 blocks)

### Security Checklist for PRs

- [ ] No `unwrap()` or `expect()` on untrusted input
- [ ] All user input validated
- [ ] Cryptographic operations use constant-time algorithms
- [ ] No integer overflow/underflow (use `checked_*` methods)
- [ ] Error handling doesn't leak sensitive information
- [ ] Tests cover edge cases and attack vectors

---

## Common Pitfalls to Avoid

### 1. Ignoring the Square Root in PoC

❌ **Wrong:**
```rust
let work_component = poc_score as f64 * c_nl;
```

✅ **Correct:**
```rust
let work_component = (poc_score as f64).sqrt() * c_nl;
```

The square root is critical for anti-centralization (diminishing returns).

### 2. Forgetting AIDA Bounds

❌ **Wrong:**
```rust
let c_sr = current_c_sr + learning_rate * adjustment;
```

✅ **Correct:**
```rust
let c_sr = (current_c_sr + learning_rate * adjustment).clamp(0.8, 1.2);
```

AIDA coefficients must stay within [0.8, 1.2].

### 3. Mixing Token Units

❌ **Wrong:**
```rust
let stake = 1000; // Is this MBO or smallest unit?
```

✅ **Correct:**
```rust
let stake_mbo = 1000u64;
let stake_smallest_unit = stake_mbo * 10u64.pow(TOKEN_DECIMALS);
```

Always be explicit about token units (MBO vs smallest unit).

### 4. Ignoring Fraud Proof Challenge Period

❌ **Wrong:**
```rust
// Immediately finalize result
finalize_result(&result);
```

✅ **Correct:**
```rust
// Lock result, allow 100-block challenge period
lock_result(&result, current_block + FRAUD_PROOF_CHALLENGE_BLOCKS);
```

Results must be challengeable for 100 blocks before finalization.

---

## Useful Commands

### Development

```bash
# Build all crates
cargo build --all

# Build release
cargo build --release

# Run tests
cargo test --all

# Run tests with output
cargo test -- --nocapture

# Format code
cargo fmt --all

# Lint
cargo clippy --all -- -D warnings

# Generate docs
cargo doc --no-deps --open

# Benchmark
cargo bench

# Run specific crate
cargo run -p mbongo-node -- --dev
```

### Git Workflow

```bash
# Create feature branch
git checkout -b feature/pox-consensus-engine

# Commit with conventional commits
git commit -m "feat(consensus): implement PoX validator selection"

# Rebase on main
git fetch upstream
git rebase upstream/main

# Push
git push origin feature/pox-consensus-engine
```

---

## Questions to Ask When Reviewing Code

1. **Is it correct?** Does it implement the spec correctly?
2. **Is it safe?** Any security vulnerabilities?
3. **Is it tested?** Adequate test coverage?
4. **Is it documented?** Rustdoc and docs/*.md updated?
5. **Is it idiomatic Rust?** Follows best practices?
6. **Is it efficient?** Any obvious performance issues?
7. **Does it follow PoX principles?** Maintains decentralization?

---

## Project-Specific Terminology

| Term | Definition |
|------|------------|
| **PoX** | Proof of X - hybrid consensus (PoS + PoUW) |
| **PoUW** | Proof of Useful Work - validators do real compute |
| **PoC** | Proof of Compute - scoring system for work contributions |
| **AIDA** | Adaptive Intelligence for Dynamic Adjustment - economic regulator |
| **C_SR** | AIDA coefficient for Stake Rewards (0.8 - 1.2) |
| **C_NL** | AIDA coefficient for Network Load (0.8 - 1.2) |
| **MBO** | Native token symbol (Mbongo) |
| **Redundant Execution** | 3 validators verify each task (Phase 1) |
| **TEE** | Trusted Execution Environment (SGX/SEV) - Phase 2 |
| **ZK-ML** | Zero-Knowledge Machine Learning proofs - Phase 3 |
| **Fraud Proof** | Challenge mechanism for incorrect results |
| **Sybil Attack** | Single entity pretending to be many validators |
| **GPU Fingerprinting** | Hardware identification to prevent duplicates |

---

## AI Assistant Guidelines

### When Helping with This Project:

1. **Read the relevant docs first**: Don't make assumptions. If working on consensus, read `docs/pox_formula.md`.

2. **Maintain the PoX formula**: Never remove the square root from PoC scores or change AIDA bounds without justification.

3. **Follow Rust conventions**: Use `rustfmt`, `clippy`, proper error handling.

4. **Prioritize security**: This is financial infrastructure. Be conservative.

5. **Keep MBO token consistent**: Always use MBO (not MBG or other variants).

6. **Document changes**: Update both Rustdoc and docs/*.md files.

7. **Write tests**: Include tests with every feature/fix.

8. **Ask for clarification**: If unsure about PoX mechanics, AIDA behavior, or verification strategy, ask before implementing.

### Helpful Prompts:

- "Implement PoX validator selection algorithm based on docs/pox_formula.md"
- "Add fraud proof challenge mechanism per docs/verification_strategy.md"
- "Create GPU fingerprinting module according to docs/sybil_resistance.md"
- "Write integration test for AIDA coefficient adjustment"
- "Update docs/consensus_mechanics.md with new validator rotation logic"

---

## Contact and Resources

- **GitHub**: https://github.com/mbongo-chain/mbongo-chain
- **Documentation**: [docs/](./docs) folder
- **Contributing**: [CONTRIBUTING.md](./CONTRIBUTING.md)
- **Discord**: https://discord.gg/mbongo-chain (coming soon)

---

**Last Updated**: 2025-11-30
**Project Version**: Pre-testnet (v0.1.0-dev)
**Rust Version**: 1.75+

---

*This file is designed to provide context for AI coding assistants. Keep it updated as the project evolves.*
