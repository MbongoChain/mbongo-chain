# Mbongo Chain

**A compute-native Layer 1 blockchain powered by Proof of X (PoX) consensus**

Mbongo Chain is a Rust-native, compute-first blockchain that combines Proof of Stake (PoS) with Proof of Useful Work (PoUW) to create a decentralized GPU compute network. By rewarding validators for both staking tokens and performing verifiable AI/ML computations, Mbongo Chain enables affordable, trustless compute while maintaining strong economic security.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)
[![Documentation](https://img.shields.io/badge/docs-latest-brightgreen.svg)](./docs)

---

## Vision

**Democratize access to GPU compute through decentralized, verifiable infrastructure.**

The AI revolution is bottlenecked by expensive, centralized compute infrastructure. Mbongo Chain solves this by:

- **Reducing costs** by 30-50% compared to AWS/GCP through decentralized GPU markets
- **Ensuring trust** via cryptographic verification of compute results
- **Preventing centralization** through diminishing returns and adaptive economics
- **Enabling Web3-native AI** with on-chain verification and DAO-friendly governance

---

## Key Features

### 🔗 Proof of X (PoX) Consensus

Mbongo Chain introduces **PoX consensus**, a novel hybrid mechanism that combines:

- **Proof of Stake (PoS)**: Economic security through token staking (MBO)
- **Proof of Useful Work (PoUW)**: Validators earn rewards by executing real AI/ML workloads
- **Proof of Compute (PoC)**: Scoring system for computational contributions

**Formula:**
```
total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)
```

Where `C_SR` and `C_NL` are dynamically adjusted by AIDA to maintain network balance.

### 🤖 AIDA Regulator

**Adaptive Intelligence for Dynamic Adjustment** (AIDA) is an on-chain regulator that:

- Monitors stake-to-work ratio in real-time
- Dynamically adjusts consensus coefficients (C_SR, C_NL) to maintain 50/50 balance
- Prevents stake or compute dominance through automatic rebalancing
- Ensures long-term decentralization and economic sustainability

### ✅ Multi-Layer Verification

Mbongo Chain ensures compute correctness through a progressive verification strategy:

**Phase 1 (Current)**: Redundant Execution
- 3 randomly selected validators verify each task
- 2/3 majority consensus required
- Optimistic fraud proofs with 100-block challenge period

**Phase 2 (Q2 2025)**: Trusted Execution Environments (TEE)
- Intel SGX / AMD SEV support
- Reduced redundancy (1 TEE + 2 standard validators)
- Remote attestation verification

**Phase 3 (Q4 2025)**: Zero-Knowledge Machine Learning (ZK-ML)
- Cryptographic proofs of correct computation
- O(1) verification time
- Privacy-preserving AI execution

### 🎯 Anti-Centralization Design

- **Square root function** on PoC scores creates diminishing returns
- Large providers earn 3.16× more by splitting into 10 nodes vs 1 large node
- AIDA automatically adjusts incentives if centralization is detected
- No whale dominance: built-in economic pressure for decentralization

### 💰 Dual Token Economy

- **MBO**: Native utility token for staking, compute payments, and governance
- **Compute Credits**: Off-chain credits for seamless developer experience (optional)
- Flexible payment: MBO, stablecoins (USDC/DAI), or fiat conversion

---

## Architecture Overview

Mbongo Chain is built entirely in **Rust** for performance, safety, and WebAssembly compatibility.

### System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Mbongo Chain                            │
├─────────────────────────────────────────────────────────────┤
│  Application Layer                                          │
│  ├─ Compute Marketplace (task submission/matching)          │
│  ├─ Staking Interface (validator registration)              │
│  └─ Governance (DAO voting, parameter updates)              │
├─────────────────────────────────────────────────────────────┤
│  Consensus Layer                                            │
│  ├─ PoX Engine (stake + work weight calculation)            │
│  ├─ AIDA Regulator (coefficient adjustment)                 │
│  ├─ Block Production (weighted validator selection)         │
│  └─ Finality Gadget (BFT finalization)                      │
├─────────────────────────────────────────────────────────────┤
│  Verification Layer                                         │
│  ├─ Redundant Execution (Phase 1)                           │
│  ├─ Fraud Proof System (challenge/arbitration)              │
│  ├─ TEE Attestation (Phase 2)                               │
│  └─ ZK Proof Verification (Phase 3)                         │
├─────────────────────────────────────────────────────────────┤
│  Execution Layer                                            │
│  ├─ Compute Runtime (Docker/WASM isolation)                 │
│  ├─ Resource Metering (CPU/GPU/RAM tracking)                │
│  └─ Result Storage (IPFS/Arweave integration)               │
├─────────────────────────────────────────────────────────────┤
│  Network Layer                                              │
│  ├─ P2P Networking (libp2p)                                 │
│  ├─ Block Propagation                                       │
│  └─ Validator Discovery                                     │
├─────────────────────────────────────────────────────────────┤
│  Storage Layer                                              │
│  ├─ State Database (RocksDB)                                │
│  ├─ Block Storage                                           │
│  └─ Merkle Tree (state commitments)                         │
└─────────────────────────────────────────────────────────────┘
```

### Rust Crates Structure

```
mbongo-chain/
├── crates/
│   ├── mbongo-core/              # Core blockchain primitives
│   │   ├── types/                # Block, transaction, account types
│   │   ├── crypto/               # Cryptographic functions (hashing, signing)
│   │   └── storage/              # State management and persistence
│   │
│   ├── mbongo-consensus/         # PoX consensus implementation
│   │   ├── pox/                  # PoX engine (stake + work weight)
│   │   ├── aida/                 # AIDA regulator logic
│   │   ├── selection/            # Validator selection algorithm
│   │   └── finality/             # BFT finality gadget
│   │
│   ├── mbongo-verification/      # Compute verification layer
│   │   ├── redundant/            # Redundant execution (Phase 1)
│   │   ├── fraud_proofs/         # Optimistic fraud proof system
│   │   ├── tee/                  # TEE attestation (Phase 2)
│   │   └── zk/                   # ZK-ML proof verification (Phase 3)
│   │
│   ├── mbongo-compute/           # Compute execution runtime
│   │   ├── executor/             # Task execution engine
│   │   ├── metering/             # Resource usage tracking
│   │   ├── scheduler/            # Task scheduling and prioritization
│   │   └── isolation/            # Sandbox (Docker/WASM)
│   │
│   ├── mbongo-network/           # P2P networking
│   │   ├── p2p/                  # libp2p integration
│   │   ├── rpc/                  # JSON-RPC API server
│   │   └── sync/                 # Block synchronization
│   │
│   ├── mbongo-runtime/           # Smart contract execution
│   │   ├── wasm/                 # WebAssembly VM
│   │   ├── precompiles/          # Native precompiled contracts
│   │   └── gas/                  # Gas metering
│   │
│   ├── mbongo-api/               # External APIs
│   │   ├── rest/                 # REST API for compute jobs
│   │   ├── ws/                   # WebSocket subscriptions
│   │   └── sdk/                  # Client libraries (Rust, JS, Python)
│   │
│   ├── mbongo-wallet/            # Wallet and key management
│   │   ├── keystore/             # Encrypted key storage
│   │   ├── signing/              # Transaction signing
│   │   └── cli/                  # CLI wallet interface
│   │
│   └── mbongo-node/              # Full node binary
│       ├── config/               # Node configuration
│       ├── telemetry/            # Metrics and monitoring
│       └── main.rs               # Node entry point
│
├── docs/                         # Documentation
├── scripts/                      # Build and deployment scripts
├── tests/                        # Integration tests
└── Cargo.toml                    # Workspace manifest
```

**Key Dependencies:**
- **substrate-primitives**: Core blockchain types (adapted)
- **libp2p**: P2P networking
- **tokio**: Async runtime
- **serde**: Serialization/deserialization
- **rocksdb**: Database backend
- **ed25519-dalek**: Cryptographic signing
- **blake3**: Fast hashing
- **wasmtime**: WebAssembly runtime

---

## Quick Start

### Prerequisites

- **Rust** 1.75 or higher ([install via rustup](https://rustup.rs/))
- **Git**
- **Docker** (optional, for compute execution)
- **16GB+ RAM** recommended for full node

### Installation

```bash
# Clone the repository
git clone https://github.com/mbongo-chain/mbongo-chain.git
cd mbongo-chain

# Build the project (release mode)
cargo build --release

# Run tests
cargo test --all

# Build documentation
cargo doc --no-deps --open
```

### Running a Full Node

```bash
# Run a development node (single validator, instant finality)
./target/release/mbongo-node --dev

# Run a node connected to testnet
./target/release/mbongo-node \
  --chain testnet \
  --bootnodes /ip4/35.123.45.67/tcp/30333/p2p/12D3KooWExample

# Run a validator node
./target/release/mbongo-node \
  --chain mainnet \
  --validator \
  --name "My Validator" \
  --rpc-port 9933 \
  --ws-port 9944
```

### Running a Compute Provider

```bash
# Register as a compute provider
mbongo-cli provider register \
  --gpu nvidia-rtx-4090 \
  --stake 10000 \
  --commission 5

# Start providing compute
./target/release/mbongo-node \
  --chain mainnet \
  --provider \
  --compute-threads 4 \
  --max-gpu-memory 24GB
```

### Submitting a Compute Job (API Example)

```bash
# Using the REST API
curl -X POST http://localhost:9933/compute/submit \
  -H "Content-Type: application/json" \
  -d '{
    "model": "meta-llama/Llama-2-70b-chat-hf",
    "input": "Explain quantum computing in simple terms",
    "max_tokens": 500,
    "payment": {
      "amount": "100",
      "currency": "MBO"
    }
  }'

# Response
{
  "job_id": "0x1234...5678",
  "status": "pending",
  "estimated_completion": "30s",
  "cost": "100 MBO"
}
```

For detailed setup instructions, see:
- [Full Node Setup Guide](./docs/full_node_setup.md)
- [Validator Setup Guide](./docs/validator_setup.md)
- [Compute Provider Setup Guide](./docs/compute_provider_setup.md)

---

## Documentation

Comprehensive documentation is available in the [`docs/`](./docs) directory:

### Core Documentation
- **[Consensus Mechanics](./docs/consensus_mechanics.md)**: Complete PoUW consensus specification
- **[PoX Formula](./docs/pox_formula.md)**: Mathematical formula for validator weight calculation
- **[Verification Strategy](./docs/verification_strategy.md)**: Multi-layer compute verification approach
- **[AIDA Specification](./docs/aida_specification.md)**: Adaptive regulator design (coming soon)
- **[Economic Model](./docs/economic_model.md)**: Tokenomics and incentive structure (coming soon)

### Setup Guides
- **[Full Node Setup](./docs/full_node_setup.md)**: Running a full node
- **[Validator Setup](./docs/validator_setup.md)**: Becoming a validator
- **[Compute Provider Setup](./docs/compute_provider_setup.md)**: Providing GPU compute

### Business & Strategy
- **[Target Market Analysis](./docs/target_market.md)**: Customer segments and competitive positioning
- **[Roadmap](./docs/roadmap.md)**: Development phases and milestones (coming soon)

### API & Development
- **[API Reference](./docs/api_reference.md)**: REST and WebSocket API documentation (coming soon)
- **[SDK Documentation](./docs/sdk.md)**: Client library usage (coming soon)

---

## Tokenomics Summary

### MBO Token

**Symbol**: MBO (Mbongo)
**Max Supply**: 31,536,000 MBO (one token per second per year)
**Decimals**: 18

### Distribution

| Allocation | Amount | Percentage | Vesting |
|------------|--------|------------|---------|
| **Validators & Compute Providers** | 15,768,000 MBO | 50% | Ongoing emissions (10 years) |
| **Community & Ecosystem** | 6,307,200 MBO | 20% | 4 years linear |
| **Team & Advisors** | 4,730,400 MBO | 15% | 4 years, 1-year cliff |
| **Investors** | 3,153,600 MBO | 10% | 3 years, 6-month cliff |
| **Treasury & DAO** | 1,576,800 MBO | 5% | Governance-controlled |

### Emission Schedule

```
Year 1:  3,153,600 MBO (10% of max supply)
Year 2:  2,522,880 MBO (20% reduction)
Year 3:  2,018,304 MBO (20% reduction)
...
Year 10: ~315,360 MBO (final emissions)
```

**Block Rewards** (decreasing over time):
- Year 1: ~6 MBO per block (6-second blocks)
- Year 5: ~2.5 MBO per block
- Year 10: ~0.6 MBO per block

**Reward Split**:
- 70% to validators (stake + compute)
- 20% to compute providers (pure compute)
- 10% to treasury (governance + development)

For detailed tokenomics, see [Economic Model](./docs/economic_model.md) (coming soon).

---

## Roadmap

### Phase 1: Foundation (Q1-Q2 2025) ✅

**Status**: In Development

- [x] Core blockchain implementation (Rust)
- [x] PoX consensus engine
- [x] AIDA regulator (basic)
- [x] Redundant execution verification
- [x] Optimistic fraud proofs
- [ ] Testnet launch
- [ ] Validator onboarding (50+ validators)

**Deliverables**:
- Testnet with 50+ validators
- Redundant verification (3 validators per task)
- Basic compute marketplace
- CLI tools and documentation

---

### Phase 2: Scaling (Q3-Q4 2025)

**Status**: Planned

- [ ] TEE integration (Intel SGX, AMD SEV)
- [ ] Hybrid verification (1 TEE + 2 standard)
- [ ] Compute provider SDK (Rust, JavaScript, Python)
- [ ] REST API and WebSocket support
- [ ] Mainnet candidate launch
- [ ] DAO governance activation

**Deliverables**:
- Mainnet launch with TEE support
- 200+ validators, 100+ compute providers
- Public API for developers
- On-chain governance

---

### Phase 3: Innovation (2026)

**Status**: Research

- [ ] ZK-ML proof generation and verification
- [ ] Privacy-preserving compute
- [ ] Cross-chain bridges (Ethereum, Cosmos, Polkadot)
- [ ] Enterprise-grade SLAs
- [ ] Advanced fraud detection (ML-based)

**Deliverables**:
- Full ZK-ML verification
- Multi-chain compute orchestration
- Enterprise partnerships
- 1,000+ validators globally

---

## Use Cases

### 1. AI Inference as a Service

**Target**: AI startups, SaaS companies

**Problem**: High LLM inference costs ($0.50+/1M tokens on AWS)

**Solution**:
- Run Llama 2 70B for $0.25/1M tokens (50% savings)
- OpenAI-compatible API
- Pay in MBO or stablecoins

**Example**:
```python
import mbongo

client = mbongo.Client(api_key="your_key")

response = client.inference(
    model="meta-llama/Llama-2-70b-chat-hf",
    messages=[
        {"role": "user", "content": "Explain PoX consensus"}
    ],
    max_tokens=500
)

print(response.text)
# Cost: ~0.05 MBO (~$0.10)
```

---

### 2. DAO Governance with Verifiable AI

**Target**: Decentralized Autonomous Organizations

**Problem**: Can't trust centralized AI for governance decisions

**Solution**:
- Cryptographically verified AI analysis
- On-chain result verification
- Fraud-proof guarantees

**Example**:
```solidity
// Smart contract integration
contract ProposalAnalyzer {
    function analyzeProposal(string memory proposalText)
        external returns (bytes32 jobId)
    {
        // Submit to Mbongo Chain
        jobId = mbongoCompute.submit(
            "proposal-analysis",
            proposalText,
            100 * 1e18 // 100 MBO payment
        );

        // Result verified via fraud proofs
        // DAO trusts the analysis
    }
}
```

---

### 3. Indie Developer GPU Access

**Target**: Solo developers, students, hobbyists

**Problem**: Can't afford $1,500+ GPUs or $2+/hour cloud costs

**Solution**:
- Pay-per-use GPU access at $0.50-1/hour
- No upfront investment
- Generous free tier (5 hours/month)

**Example**:
```bash
# Fine-tune a model without owning a GPU
mbongo-cli compute run \
  --image pytorch/pytorch:latest \
  --gpu nvidia-rtx-4090 \
  --script train.py \
  --budget 10 MBO

# Total cost: ~$5 for 10 hours of RTX 4090 time
```

---

## Contributing

We welcome contributions from the community! Mbongo Chain is open-source and community-driven.

### How to Contribute

1. **Fork the repository**
2. **Create a feature branch** (`git checkout -b feature/amazing-feature`)
3. **Commit your changes** (`git commit -m 'Add amazing feature'`)
4. **Push to the branch** (`git push origin feature/amazing-feature`)
5. **Open a Pull Request**

### Development Guidelines

- **Code Style**: Follow Rust standard style (`cargo fmt`)
- **Testing**: Add tests for new features (`cargo test`)
- **Documentation**: Document public APIs with Rustdoc comments
- **Commit Messages**: Use conventional commits (e.g., `feat:`, `fix:`, `docs:`)

### Areas for Contribution

- **Core Development**: Consensus, networking, storage
- **Verification**: TEE integration, ZK proof research
- **Compute Runtime**: GPU scheduling, isolation, metering
- **Tooling**: CLI improvements, monitoring dashboards
- **Documentation**: Tutorials, guides, translations
- **Testing**: Integration tests, fuzzing, benchmarks

### Community

- **Discord**: [discord.gg/mbongo-chain](https://discord.gg/mbongo-chain) (coming soon)
- **Twitter**: [@mbongo_chain](https://twitter.com/mbongo_chain) (coming soon)
- **Forum**: [forum.mbongochain.com](https://forum.mbongochain.com) (coming soon)
- **GitHub Discussions**: [Discussions](https://github.com/mbongo-chain/mbongo-chain/discussions)

---

## Security

### Responsible Disclosure

If you discover a security vulnerability, please email security@mbongochain.com. Do not open public issues for security vulnerabilities.

**We offer bug bounties for critical vulnerabilities:**
- Critical: Up to 50,000 MBO
- High: Up to 20,000 MBO
- Medium: Up to 5,000 MBO
- Low: Up to 1,000 MBO

### Audits

- **Phase 1**: Internal security review (Q1 2025)
- **Phase 2**: External audit by Certik (Q2 2025, planned)
- **Phase 3**: Ongoing bug bounty program (Mainnet launch)

---

## License

This project is licensed under the **MIT License** - see the [LICENSE](./LICENSE) file for details.

```
MIT License

Copyright (c) 2024-2025 Mbongo Chain Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## Acknowledgments

Mbongo Chain is inspired by and builds upon the work of:

- **Substrate** (Parity Technologies) - Blockchain framework architecture
- **Ethereum** - Smart contract design and EVM compatibility goals
- **Filecoin** - Proof of useful work concepts
- **Cosmos** - Inter-blockchain communication
- **Gensyn** - ML verification research
- **Akash Network** - Decentralized compute marketplace

Special thanks to the Rust blockchain community for tooling and support.

---

## Links

- **Website**: [mbongochain.com](https://mbongochain.com) (coming soon)
- **Documentation**: [docs.mbongochain.com](https://docs.mbongochain.com) (coming soon)
- **Block Explorer**: [explorer.mbongochain.com](https://explorer.mbongochain.com) (coming soon)
- **GitHub**: [github.com/mbongo-chain/mbongo-chain](https://github.com/mbongo-chain/mbongo-chain)

---

**Built with ❤️ in Rust for a decentralized AI future.**
