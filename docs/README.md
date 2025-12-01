# Mbongo Chain Documentation

> **Complete technical documentation for the Mbongo Chain blockchain platform**

Welcome to the Mbongo Chain documentation hub. This directory contains comprehensive technical specifications, guides, and reference materials for developers, validators, and contributors.

---

## Quick Navigation

| Category | Description |
|----------|-------------|
| [Getting Started](#getting-started) | Quick start guides and onboarding |
| [Core Concepts](#core-concepts) | Fundamental blockchain concepts |
| [Architecture](#architecture) | System design and architecture |
| [Consensus](#consensus) | PoX consensus mechanism |
| [Economics](#economics-tokenomics) | Token economics and incentives |
| [Operations](#operations-node-setup) | Node setup and operations |
| [Development](#development) | Developer guides and SDKs |
| [APIs](#apis-cli) | API references and CLI docs |
| [Security](#security) | Security model and best practices |

---

## Getting Started

Start here if you're new to Mbongo Chain:

- [**Vision & Overview**](vision.md) - Project vision and mission
- [**Whitepaper**](mbongo_whitepaper.md) - High-level technical overview
- [**Getting Started**](getting_started.md) - Quick start guide
- [**Developer Onboarding**](onboarding_dev.md) - First steps for developers
- [**FAQ**](faq.md) - Frequently asked questions
- [**Glossary**](glossary.md) - Terminology and definitions
- [**Roadmap**](roadmap.md) - Development roadmap

---

## Core Concepts

### Consensus Mechanism

Mbongo Chain uses **PoX** (Proof-of-Everything), a hybrid consensus combining Proof of Stake, Proof of Useful Work, and Proof of Compute:

- [**Consensus Master Overview**](consensus_master_overview.md) - Complete consensus specification
- [**PoC Consensus Mechanics**](poc_consensus_mechanics.md) - Detailed PoC scoring and mechanics
- [**PoX Formula**](pox_formula.md) - Mathematical specification of PoX consensus with AIDA regulation

### Verification & Security

- [**Verification Strategy**](verification_strategy.md) - Multi-layer compute verification (Redundant/TEE/ZK-ML)
- [**Sybil Resistance**](sybil_resistance.md) - Sybil attack prevention mechanisms
- [**Economic Security**](economic_security.md) - Complete economic security model

### Market Positioning

- [**Competitive Analysis**](competitive_analysis.md) - Comparison vs Render, Akash, io.net, Gensyn, Bittensor
- [**Target Market**](target_market.md) - Market analysis and customer personas

---

## Architecture

### System Design

- [**Architecture Master Overview**](architecture_master_overview.md) - **[PRIMARY]** Canonical architecture reference
- [**Full System Overview**](full_system_overview.md) - Complete system overview
- [**Node Architecture**](node_architecture.md) - Node design and components
- [**Runtime Architecture**](runtime_architecture.md) - Runtime execution architecture

### Core Components

- [**Execution Engine**](execution_engine_overview.md) - Transaction execution and state transitions
- [**Compute Engine**](compute_engine_overview.md) - GPU compute execution runtime
- [**Mempool**](mempool_overview.md) - Transaction pool design and management
- [**State Machine**](state_machine_validation.md) - State machine validation and transitions

### Data Flow & Validation

- [**Block Validation Pipeline**](block_validation_pipeline.md) - Complete block validation flow
- [**Sync Validation**](sync_validation.md) - Chain synchronization validation

---

## Consensus

### Core Mechanics

- [**Consensus Master Overview**](consensus_master_overview.md) - **[PRIMARY]** Complete consensus specification
- [**PoC Consensus Mechanics**](poc_consensus_mechanics.md) - **[PRIMARY]** Detailed PoC scoring mechanics
- [**PoX Formula**](pox_formula.md) - Mathematical formula with AIDA regulation

### Validation & Integrity

- [**Consensus Validation**](consensus_validation.md) - Consensus validation procedures
- [**Consensus Integrity Checks**](consensus_integrity_checks.md) - Integrity verification mechanisms

---

## Economics & Tokenomics

### Token Model

- [**Token Introduction**](token_intro.md) - MBO token overview
- [**Token Distribution**](token_distribution.md) - Distribution schedule and allocations
- [**Supply Schedule**](supply_schedule.md) - Emission schedule and inflation
- [**Monetary Policy**](monetary_policy.md) - Monetary policy rules

### Economic Design

- [**Economic Security**](economic_security.md) - Economic security model
- [**Incentive Design**](incentive_design.md) - Incentive structures for all participants
- [**Staking Model**](staking_model.md) - Staking mechanics and rewards
- [**Fee Model**](fee_model.md) - Transaction and compute fee structures
- [**Reward Mechanics**](reward_mechanics.md) - Reward distribution algorithms

### Value & Utility

- [**Utility Value**](utility_value.md) - Token utility analysis
- [**Compute Value**](compute_value.md) - Compute value calculation and pricing
- [**Vesting Model**](vesting_model.md) - Vesting schedules for stakeholders

### Governance

- [**Governance Model**](governance_model.md) - DAO governance mechanisms

### Economic Summary

- [**Economic Summary**](economic_summary.md) - High-level economic overview

---

## Operations & Node Setup

### Validator Operations

- [**Validator Setup**](validator_setup.md) - Complete validator setup guide
- [**Setup Validation**](setup_validation.md) - Validate your node setup
- [**Guardian Status**](guardian_status.md) - Guardian node operations

### Compute Provider Operations

- [**Compute Provider Setup**](compute_provider_setup.md) - GPU compute provider setup guide

### Full Node Operations

- [**Full Node Setup**](full_node_setup.md) - Full node installation and configuration
- [**Node Setup Overview**](node_setup_overview.md) - General node setup summary

---

## Development

### Developer Guides

- [**Developer Guide**](developer_guide.md) - Getting started with development
- [**Developer Introduction**](developer_introduction.md) - Introduction for developers
- [**Developer Environment**](developer_environment.md) - Development environment setup
- [**Developer Workflow**](developer_workflow.md) - Development workflow and best practices
- [**Contributing Workflow**](contributing_workflow.md) - How to contribute to the project

### SDKs

- [**Rust SDK Overview**](rust_sdk_overview.md) - Rust SDK reference and examples
- [**TypeScript SDK Overview**](ts_sdk_overview.md) - TypeScript SDK reference and examples

### Advanced Topics

- [**Oracle Model**](oracle_model.md) - Oracle design and implementation

---

## APIs & CLI

### Command Line Interface

- [**CLI Overview**](cli_overview.md) - Complete CLI command reference
- [**CLI Node Commands**](cli_node.md) - Node management commands
- [**CLI Wallet Commands**](cli_wallet.md) - Wallet management commands
- [**CLI Configuration**](cli_config.md) - CLI configuration options

### APIs

- [**RPC Overview**](rpc_overview.md) - JSON-RPC API reference
- [**OpenAPI Reference**](openapi_reference.md) - OpenAPI/REST API specification

---

## Security

### Threat Prevention

- [**Sybil Resistance**](sybil_resistance.md) - Multi-layer Sybil attack prevention
- [**Verification Strategy**](verification_strategy.md) - Multi-layer compute verification
- [**Economic Security**](economic_security.md) - Economic attack resistance

### Validation & Integrity

- [**Consensus Integrity Checks**](consensus_integrity_checks.md) - Consensus integrity verification
- [**Block Validation Pipeline**](block_validation_pipeline.md) - Block validation security
- [**State Machine Validation**](state_machine_validation.md) - State transition validation

---

## Meta Documentation

### Project Information

- [**Vision**](vision.md) - Project vision and goals
- [**Whitepaper**](mbongo_whitepaper.md) - Technical whitepaper
- [**Roadmap**](roadmap.md) - Development roadmap and milestones
- [**INDEX**](INDEX.md) - Hierarchical documentation index

### Validation & Status

- [**Spec Validation Summary**](spec_validation_summary.md) - Specification validation status
- [**Final Documentation Index**](final_doc_index.md) - Complete documentation listing

---

## Documentation Standards

All documentation in this repository follows these standards:

### File Naming Convention

- Use lowercase with underscores: `my_document.md`
- Use descriptive names: `validator_setup.md` not `setup.md`
- Add `_overview` suffix for high-level docs: `consensus_overview.md`

### Document Structure

Each document should include:

1. **Title** - Clear, descriptive title
2. **Document metadata** - Version, date, status (where applicable)
3. **Table of Contents** - For documents > 50 lines
4. **Introduction** - Purpose and scope
5. **Main content** - Well-structured with headers
6. **Examples** - Code examples and diagrams where applicable
7. **References** - Links to related documents

### Content Guidelines

- **Language**: All documentation must be in English
- **Clarity**: Write for technical readers but avoid unnecessary jargon
- **Completeness**: Include all necessary context
- **Accuracy**: Keep technical specifications precise
- **Examples**: Include practical examples and code samples
- **Diagrams**: Use ASCII diagrams for architecture visualization

---

## Contributing to Documentation

To contribute to or improve the documentation:

1. Read [CONTRIBUTING.md](../CONTRIBUTING.md) in the root directory
2. Follow the documentation standards above
3. Ensure all content is in English
4. Test code examples before submission
5. Update this README.md if adding new documents
6. Submit a pull request with clear description

---

## Documentation Hierarchy

For a complete hierarchical view of all documentation, see [INDEX.md](INDEX.md).

---

## Getting Help

- **GitHub Issues**: https://github.com/mbongo-chain/mbongo-chain/issues
- **GitHub Discussions**: https://github.com/mbongo-chain/mbongo-chain/discussions
- **Discord**: Coming soon

---

## License

All documentation is licensed under [MIT License](../LICENSE).

---

**Last Updated**: December 2025
**Documentation Version**: 1.0.0
