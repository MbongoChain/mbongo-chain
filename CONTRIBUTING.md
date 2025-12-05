# Contributing to Mbongo Chain

Thank you for your interest in contributing to Mbongo Chain! We're building the future of decentralized, verifiable AI compute, and we're excited to have you join us.

**Our Mission**: Democratize access to GPU compute through a decentralized, trustless network that makes AI infrastructure affordable and censorship-resistant.

Whether you're a Rust developer, blockchain expert, documentation writer, or community builder, there's a place for you in the Mbongo Chain ecosystem.

---

## Table of Contents

1. [Getting Started](#getting-started)
2. [Areas Needing Contribution](#areas-needing-contribution)
3. [Development Setup](#development-setup)
4. [Code Style Guidelines](#code-style-guidelines)
5. [Pull Request Process](#pull-request-process)
6. [Communication Channels](#communication-channels)
7. [Code of Conduct](#code-of-conduct)
8. [Recognition](#recognition)

---

## Getting Started

### Prerequisites

Before contributing, familiarize yourself with:

1. **Project Vision**: Read [README.md](./README.md) for project overview
2. **Architecture**: Review [docs/architecture_overview.md](./docs/architecture_overview.md)
3. **Core Concepts**:
   - [PoX Consensus](./docs/pox_formula.md) - Hybrid Proof of Stake + Proof of Useful Work
   - [Verification Strategy](./docs/verification_strategy.md) - Multi-layer compute verification
   - [Sybil Resistance](./docs/sybil_resistance.md) - Anti-attack mechanisms

### Quick Start

```bash
# 1. Fork the repository on GitHub
# Click "Fork" at https://github.com/mbongo-chain/mbongo-chain

# 2. Clone your fork
git clone https://github.com/YOUR-USERNAME/mbongo-chain.git
cd mbongo-chain

# 3. Add upstream remote
git remote add upstream https://github.com/mbongo-chain/mbongo-chain.git

# 4. Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# 5. Install development tools
rustup component add rustfmt clippy

# 6. Build the project
cargo build

# 7. Run tests
cargo test --all

# 8. You're ready to contribute!
```

---

## Areas Needing Contribution

We welcome contributions in multiple areas. Choose what aligns with your skills and interests!

### 🦀 Core Protocol (Rust)

**Priority**: High
**Difficulty**: Advanced
**Skills**: Rust, blockchain, distributed systems

**Open Tasks:**
- Implement PoX consensus engine (`mbongo-consensus` crate)
- AIDA regulator logic for dynamic coefficient adjustment
- Validator selection algorithm (weighted random)
- Block production and finality gadget
- State transition functions

**How to Start:**
1. Review [docs/consensus_mechanics.md](./docs/consensus_mechanics.md)
2. Check GitHub issues tagged [`core-protocol`](https://github.com/mbongo-chain/mbongo-chain/labels/core-protocol)
3. Comment on an issue to claim it
4. Submit PR with implementation

---

### 🔗 Networking (P2P)

**Priority**: High
**Difficulty**: Intermediate-Advanced
**Skills**: libp2p, networking, Rust

**Open Tasks:**
- Implement P2P networking layer (`mbongo-network` crate)
- Block propagation protocol
- Validator discovery and peer exchange
- Gossip protocol for transactions
- Network telemetry and monitoring

**How to Start:**
1. Review libp2p documentation
2. Check issues tagged [`networking`](https://github.com/mbongo-chain/mbongo-chain/labels/networking)
3. Set up local testnet (2-3 nodes) for testing

---

### ✅ Verification System

**Priority**: Medium
**Difficulty**: Advanced
**Skills**: Cryptography, TEE (SGX/SEV), ZK proofs

**Open Tasks:**
- Redundant execution framework (Phase 1)
- Fraud proof submission and arbitration
- TEE attestation verification (Phase 2)
- ZK-ML proof generation and verification (Phase 3)
- GPU fingerprinting implementation

**How to Start:**
1. Review [docs/verification_strategy.md](./docs/verification_strategy.md)
2. Check issues tagged [`verification`](https://github.com/mbongo-chain/mbongo-chain/labels/verification)
3. Phase 1 (redundant execution) is highest priority

---

### 🖥️ Compute Runtime

**Priority**: Medium
**Difficulty**: Intermediate
**Skills**: Docker, GPU programming, resource metering

**Open Tasks:**
- Task execution engine (`mbongo-compute` crate)
- GPU resource metering (CUDA/ROCm)
- Docker/WASM isolation layer
- Job scheduling and prioritization
- Result storage integration (IPFS/Arweave)

**How to Start:**
1. Review [docs/compute_provider_setup.md](./docs/compute_provider_setup.md)
2. Test local GPU workload execution
3. Check issues tagged [`compute-runtime`](https://github.com/mbongo-chain/mbongo-chain/labels/compute-runtime)

---

### 📚 Documentation

**Priority**: High
**Difficulty**: Beginner-Intermediate
**Skills**: Technical writing, Markdown

**Open Tasks:**
- API documentation (REST/WebSocket/gRPC)
- Tutorial: "Build your first AI app on Mbongo"
- Validator setup guides (cloud providers: AWS, GCP, Hetzner)
- Troubleshooting guide
- Video tutorials (YouTube)
- Translate documentation (non-English)

**How to Start:**
1. Browse [docs/](./docs) folder
2. Identify gaps or unclear sections
3. Submit PR with improvements
4. Check issues tagged [`documentation`](https://github.com/mbongo-chain/mbongo-chain/labels/documentation)

---

### 🛠️ Developer Tools

**Priority**: Medium
**Difficulty**: Intermediate
**Skills**: CLI, SDK development, JavaScript/Python

**Open Tasks:**
- CLI improvements (`mbongo-cli`)
- JavaScript SDK (`@mbongo/sdk-js`)
- Python SDK (`mbongo-py`)
- Monitoring dashboard (Grafana, Prometheus)
- Block explorer (web interface)

**How to Start:**
1. Use existing tools and document pain points
2. Check issues tagged [`tooling`](https://github.com/mbongo-chain/mbongo-chain/labels/tooling)
3. Propose new tools in GitHub Discussions

---

### 🧪 Testing

**Priority**: Medium
**Difficulty**: Intermediate
**Skills**: Testing, fuzzing, Rust

**Open Tasks:**
- Unit tests for all crates (target: >80% coverage)
- Integration tests (multi-node scenarios)
- Fuzzing (cargo-fuzz)
- Benchmarking (criterion)
- Load testing (simulate 1000+ validators)

**How to Start:**
1. Run `cargo test --all` and review coverage
2. Add tests for untested modules
3. Check issues tagged [`testing`](https://github.com/mbongo-chain/mbongo-chain/labels/testing)

---

### 🎨 Community

**Priority**: Medium
**Difficulty**: Beginner
**Skills**: Community management, content creation

**Open Tasks:**
- Write blog posts (Medium, dev.to)
- Create video tutorials
- Answer questions on Discord/forums
- Organize community calls
- Design graphics (logos, infographics)

**How to Start:**
1. Join Discord server
2. Introduce yourself in #introductions
3. Ask how you can help in #community

---

## Development Setup

### Environment Requirements

```yaml
Operating System: Linux, macOS, Windows (WSL2)
Rust: 1.75 or higher
RAM: 16GB+ recommended
Disk: 50GB+ for blockchain data
GPU: Optional (for compute provider testing)
```

### Recommended IDE Setup

**Visual Studio Code:**
```bash
# Install recommended extensions
code --install-extension rust-lang.rust-analyzer
code --install-extension tamasfe.even-better-toml
code --install-extension serayuzgur.crates
```

**Rust-Analyzer Settings:**
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.rustfmt.overrideCommand": ["rustfmt"],
  "editor.formatOnSave": true
}
```

### Building from Source

```bash
# Debug build (fast compilation, slower runtime)
cargo build

# Release build (slow compilation, fast runtime)
cargo build --release

# Build specific crate
cargo build -p mbongo-consensus

# Build with all features
cargo build --all-features

# Build documentation
cargo doc --no-deps --open
```

### Running Tests

```bash
# Run all tests
cargo test --all

# Run tests for specific crate
cargo test -p mbongo-core

# Run with output
cargo test -- --nocapture

# Run ignored tests (slow/integration)
cargo test -- --ignored

# Run benchmarks
cargo bench
```

### Development Workflow

```bash
# 1. Create feature branch
git checkout -b feature/your-feature-name

# 2. Make changes, commit often
git add .
git commit -m "feat: add validator selection algorithm"

# 3. Run checks before pushing
cargo fmt --all -- --check  # Format check
cargo clippy --all -- -D warnings  # Lint
cargo test --all  # Tests

# 4. Push to your fork
git push origin feature/your-feature-name

# 5. Open pull request on GitHub
```

---

## Code Style Guidelines

We follow Rust community standards with some project-specific conventions.

### Rust Formatting

**Use `rustfmt` for automatic formatting:**

```bash
# Format all code
cargo fmt --all

# Check formatting without modifying
cargo fmt --all -- --check
```

**Configuration**: See [rustfmt.toml](./rustfmt.toml) for project settings.

**Key Conventions:**
- Max line width: 100 characters
- Indentation: 4 spaces
- Trailing commas: Always in multiline
- Imports: Group by std, external, internal

**Example:**
```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::types::{Block, Transaction};
use crate::utils::hash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validator {
    pub id: ValidatorId,
    pub stake: u64,
    pub reputation: f64,
}

impl Validator {
    pub fn new(id: ValidatorId, stake: u64) -> Self {
        Self {
            id,
            stake,
            reputation: 1.0,
        }
    }

    pub fn calculate_weight(&self, poc_score: u64) -> f64 {
        let stake_component = self.stake as f64 * self.reputation;
        let work_component = (poc_score as f64).sqrt();
        stake_component + work_component
    }
}
```

---

### Linting with Clippy

**Run Clippy on all code:**

```bash
# Standard lints
cargo clippy --all

# Treat warnings as errors (CI requirement)
cargo clippy --all -- -D warnings

# Fix auto-fixable lints
cargo clippy --fix --all
```

**Clippy Categories We Enforce:**
- `clippy::all` - All default lints
- `clippy::pedantic` - Extra pedantic lints
- `clippy::nursery` - Experimental lints (selective)

**Allowed Lints (Project-Specific):**
```rust
// In rare cases, you can allow specific lints with justification
#[allow(clippy::too_many_arguments)]  // Justified: FFI boundary
pub fn complex_function(a: u64, b: u64, c: u64, d: u64, e: u64) {
    // ...
}
```

---

### Naming Conventions

**Crates:**
- Format: `mbongo-{module}`
- Example: `mbongo-consensus`, `mbongo-network`

**Files:**
- Snake case: `validator_selection.rs`
- Module files: `mod.rs` or `{module_name}.rs`

**Types:**
- Structs: PascalCase (`Validator`, `BlockHeader`)
- Enums: PascalCase (`ConsensusState`, `ValidationError`)
- Traits: PascalCase (`Verifiable`, `ConsensusEngine`)

**Functions/Variables:**
- Snake case: `calculate_weight`, `validator_set`
- Constants: SCREAMING_SNAKE_CASE (`MAX_VALIDATORS`, `BLOCK_TIME`)

**Example:**
```rust
pub const MAX_VALIDATORS: usize = 1000;

pub struct ValidatorSet {
    validators: Vec<Validator>,
}

impl ValidatorSet {
    pub fn select_random(&self, count: usize) -> Vec<Validator> {
        // Implementation
    }
}

pub trait ConsensusEngine {
    fn produce_block(&mut self) -> Result<Block, ConsensusError>;
}
```

---

### Documentation

**Document all public APIs with Rustdoc:**

```rust
/// Calculates the total weight of a validator for block production.
///
/// The weight combines stake-based security with proof of compute contributions,
/// using the PoX formula:
///
/// ```text
/// total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)
/// ```
///
/// # Arguments
///
/// * `stake_weight` - The validator's stake-adjusted weight in MBO
/// * `poc_score` - The proof of compute score (cumulative work)
/// * `c_sr` - AIDA coefficient for stake rewards (0.8 - 1.2)
/// * `c_nl` - AIDA coefficient for network load (0.8 - 1.2)
///
/// # Returns
///
/// Returns the total weight as a `f64`, used for weighted validator selection.
///
/// # Examples
///
/// ```
/// use mbongo_consensus::calculate_total_weight;
///
/// let weight = calculate_total_weight(10_000.0, 50_000, 1.0, 1.0);
/// assert!(weight > 10_000.0);
/// ```
pub fn calculate_total_weight(
    stake_weight: f64,
    poc_score: u64,
    c_sr: f64,
    c_nl: f64,
) -> f64 {
    let stake_component = stake_weight * c_sr;
    let work_component = (poc_score as f64).sqrt() * c_nl;
    stake_component + work_component
}
```

**Documentation Standards:**
- All public functions: Summary + arguments + returns + example
- All public structs: Purpose + usage notes
- All public traits: Contract + implementation requirements
- Complex private functions: Summary (optional)

---

### Error Handling

**Use `Result<T, E>` for fallible operations:**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid block hash: expected {expected}, got {actual}")]
    InvalidBlockHash { expected: Hash, actual: Hash },

    #[error("Insufficient stake: required {required}, have {actual}")]
    InsufficientStake { required: u64, actual: u64 },

    #[error("Validator not found: {0}")]
    ValidatorNotFound(ValidatorId),

    #[error("Consensus error: {0}")]
    ConsensusError(#[from] ConsensusError),
}

pub fn validate_block(block: &Block) -> Result<(), ValidationError> {
    if !verify_hash(block) {
        return Err(ValidationError::InvalidBlockHash {
            expected: calculate_expected_hash(block),
            actual: block.hash,
        });
    }
    Ok(())
}
```

**Avoid `unwrap()` and `expect()` in production code:**
- Use `?` operator for propagating errors
- Use `match` or `if let` for handling
- Only use `unwrap()` in tests or when mathematically impossible to fail

---

### Commit Message Guidelines

We follow [Conventional Commits](https://www.conventionalcommits.org/) for clear history.

**Format:**
```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Formatting (no code change)
- `refactor`: Code restructuring (no behavior change)
- `perf`: Performance improvement
- `test`: Adding/updating tests
- `chore`: Build, CI, dependencies

**Examples:**

```bash
# Feature
git commit -m "feat(consensus): implement PoX validator selection algorithm"

# Bug fix
git commit -m "fix(network): resolve peer discovery race condition"

# Documentation
git commit -m "docs(api): add OpenAPI spec for compute endpoints"

# Breaking change
git commit -m "feat(consensus)!: change block time from 12s to 6s

BREAKING CHANGE: Block time reduced to 6 seconds. Validators must upgrade."
```

**Scope Examples:**
- `consensus` - Consensus engine
- `network` - P2P networking
- `compute` - Compute runtime
- `cli` - Command-line interface
- `docs` - Documentation
- `ci` - Continuous integration

---

## Pull Request Process

### Before Opening a PR

1. **Ensure your code passes all checks:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all -- -D warnings
   cargo test --all
   cargo build --release
   ```

2. **Update documentation:**
   - Add/update Rustdoc comments
   - Update relevant docs/*.md files
   - Add entry to CHANGELOG.md (if applicable)

3. **Write/update tests:**
   - Unit tests for new functions
   - Integration tests for new features
   - Ensure tests are passing

4. **Rebase on latest main:**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

---

### Opening a Pull Request

1. **Push to your fork:**
   ```bash
   git push origin feature/your-feature-name
   ```

2. **Create PR on GitHub:**
   - Go to https://github.com/mbongo-chain/mbongo-chain
   - Click "New Pull Request"
   - Select your fork and branch

3. **Fill out PR template:**

   ```markdown
   ## Description
   Brief description of changes (1-2 sentences)

   ## Type of Change
   - [ ] Bug fix (non-breaking change which fixes an issue)
   - [ ] New feature (non-breaking change which adds functionality)
   - [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
   - [ ] Documentation update

   ## Changes Made
   - Implemented validator selection algorithm using weighted random sampling
   - Added unit tests for edge cases (empty validator set, single validator)
   - Updated docs/consensus_mechanics.md with selection algorithm explanation

   ## Testing
   - [ ] Unit tests added/updated
   - [ ] Integration tests added/updated
   - [ ] Manual testing completed
   - [ ] All tests passing locally

   ## Checklist
   - [ ] Code follows project style guidelines (rustfmt, clippy)
   - [ ] Self-review completed
   - [ ] Documentation updated (Rustdoc, docs/*.md)
   - [ ] No new warnings introduced
   - [ ] CHANGELOG.md updated (if applicable)

   ## Related Issues
   Closes #123
   ```

4. **Request review:**
   - Tag relevant maintainers
   - Link related issues
   - Provide context if needed

---

### PR Review Process

**What Reviewers Look For:**
1. **Correctness**: Does the code do what it claims?
2. **Tests**: Are there adequate tests? Do they pass?
3. **Style**: Does it follow project conventions?
4. **Documentation**: Is it well-documented?
5. **Performance**: Any obvious inefficiencies?
6. **Security**: Any vulnerabilities introduced?

**Responding to Feedback:**
- Be open to suggestions
- Ask questions if unclear
- Make requested changes promptly
- Push updates to the same branch (PR auto-updates)
- Re-request review after addressing comments

**Approval & Merge:**
- Requires **2 approvals** from maintainers
- CI must pass (GitHub Actions)
- Maintainer will merge (squash or rebase merge)
- Your contribution is now part of Mbongo Chain!

---

### Branch Naming

Use descriptive branch names:

**Format:** `<type>/<short-description>`

**Examples:**
- `feature/pox-consensus-engine`
- `fix/network-peer-discovery`
- `docs/api-reference-update`
- `refactor/storage-layer`
- `test/integration-validator-selection`

---

## Communication Channels

### GitHub

**Primary platform for code-related discussions:**

- **Issues**: Bug reports, feature requests
  - https://github.com/mbongo-chain/mbongo-chain/issues
- **Discussions**: Questions, ideas, RFCs
  - https://github.com/mbongo-chain/mbongo-chain/discussions
- **Pull Requests**: Code reviews
  - https://github.com/mbongo-chain/mbongo-chain/pulls

**Creating a Good Issue:**
```markdown
**Title**: Clear, concise description

**Description**:
- What is the issue/feature?
- Why is it needed?
- How should it work?

**Environment** (for bugs):
- OS: Ubuntu 22.04
- Rust version: 1.75
- Commit: abc123

**Steps to Reproduce** (for bugs):
1. Run `mbongo-node --dev`
2. Submit transaction with X
3. Observe error Y

**Expected Behavior**:
Transaction should succeed.

**Actual Behavior**:
Error: "Invalid signature"

**Logs**:
```
ERROR: Invalid signature for transaction 0x...
```
```

---

### Discord (Coming Soon)

**Real-time chat and community:**

- **#general**: General discussion
- **#development**: Dev questions and help
- **#consensus**: Consensus mechanism discussions
- **#verification**: Verification and security
- **#support**: User support
- **#announcements**: Official updates

Join: https://discord.gg/mbongo-chain

---

### Community Calls

**Monthly developer calls:**
- **When**: First Tuesday of each month, 16:00 UTC
- **Where**: Zoom (link in Discord #announcements)
- **What**: Roadmap updates, technical discussions, Q&A

**Agendas posted in advance**: https://github.com/mbongo-chain/mbongo-chain/discussions

---

## Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inclusive environment for all contributors, regardless of:
- Experience level
- Gender identity and expression
- Sexual orientation
- Disability
- Personal appearance
- Body size
- Race, ethnicity, or nationality
- Age
- Religion (or lack thereof)

### Expected Behavior

✅ **Do:**
- Be respectful and considerate
- Welcome newcomers and help them get started
- Give and receive constructive feedback gracefully
- Focus on what's best for the community and project
- Show empathy toward other community members

❌ **Don't:**
- Use sexualized language or imagery
- Make personal attacks or insults
- Troll, harass, or engage in inflammatory behavior
- Publish others' private information without permission
- Conduct yourself unprofessionally

### Enforcement

Violations will be addressed by project maintainers:

1. **First offense**: Private warning
2. **Second offense**: Temporary ban (1-4 weeks)
3. **Third offense**: Permanent ban

**Report violations**: email conduct@mbongochain.com or DM a maintainer on Discord.

### Attribution

This Code of Conduct is adapted from the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/), version 2.1.

---

## Recognition

We value all contributions and recognize our contributors!

### Hall of Fame

**Top contributors are featured:**
- In README.md (Contributors section)
- On our website (coming soon)
- In release announcements
- With special Discord roles

### Contribution Types We Recognize

Not just code! We celebrate:
- 💻 **Code**: Features, bug fixes, refactoring
- 📚 **Documentation**: Guides, tutorials, API docs
- 🧪 **Testing**: Writing tests, finding bugs
- 🎨 **Design**: UI/UX, graphics, branding
- 🌍 **Translation**: Documentation in other languages
- 🗣️ **Community**: Answering questions, onboarding newcomers
- 💡 **Ideas**: Feature proposals, architecture suggestions

### Rewards (Future)

**Planned contributor rewards:**
- Early access to testnet/mainnet tokens
- NFT badges for significant contributions
- Governance voting rights in Mbongo DAO
- Swag (t-shirts, stickers, hoodies)
- Conference tickets and travel stipends

*Details TBD as project matures.*

---

## Getting Help

**Stuck? Have questions?**

1. **Check Documentation**: Browse [docs/](./docs) folder
2. **Search Issues**: Someone may have asked before
3. **Ask on Discord**: #development channel
4. **Open Discussion**: https://github.com/mbongo-chain/mbongo-chain/discussions

**Maintainers are here to help!** Don't hesitate to ask questions.

---

## License

By contributing to Mbongo Chain, you agree that your contributions will be licensed under the [Apache License 2.0](./LICENSE).

Your contributions will be attributed to you in git history and release notes.

---

## Thank You!

Every contribution, no matter how small, helps build the decentralized AI infrastructure of the future. We're grateful for your time, energy, and expertise.

**Let's build something incredible together.**

---

**Links:**
- Website: https://mbongochain.com (coming soon)
- GitHub: https://github.com/mbongo-chain/mbongo-chain
- Discord: https://discord.gg/mbongo-chain (coming soon)
- Twitter: [@mbongo_chain](https://twitter.com/mbongo_chain) (coming soon)

---

**Questions?** Reach out to the core team:
- Email: contributors@mbongochain.com
- GitHub Discussions: https://github.com/mbongo-chain/mbongo-chain/discussions
