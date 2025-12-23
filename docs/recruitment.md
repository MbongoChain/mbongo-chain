# Contributing Roles & Skill Areas

This document defines the contribution roles and skill requirements for Mbongo Chain development.

## Overview

**If you don't know Rust, you can still contribute meaningfully.**

Rust is required ONLY for core protocol work (Circle 1). All other contribution areas accept other languages and skill sets.

## Contribution Circles

### Circle 1: Core Rust Team

**Scope**: Low-level blockchain primitives, consensus, and state management.

**What they work on:**
- Core protocol implementation (`mbongo-core` crate)
- Consensus engine (`mbongo-consensus` crate)
- State machine and execution engine
- Cryptographic primitives and verification
- Block and transaction processing
- Storage abstractions

**Required skills:**
- **Language**: Rust (required)
- **Domain**: Blockchain fundamentals, distributed systems, cryptography
- **Experience**: Advanced Rust proficiency, systems programming background

**What they must NOT touch:**
- API layer (Circle 2 responsibility)
- Tooling and SDKs (Circle 2 responsibility)
- Documentation beyond technical specifications (Circle 3 responsibility)
- Infrastructure and deployment (Circle 3 responsibility)

**Current Phase 2 focus:**
- TEE integration and attestation
- P2P networking implementation
- State persistence and Merkle tree integration
- Consensus finality mechanisms

---

### Circle 2: Technical Contributors

**Scope**: Higher-layer technical work including APIs, networking interfaces, tooling, and client libraries.

**What they work on:**
- REST API and WebSocket servers
- JSON-RPC implementation
- Client SDKs (JavaScript/TypeScript, Python, Rust)
- CLI tools and developer utilities
- Network protocol handlers
- Testing frameworks and integration tests

**Required skills:**
- **Languages**: JavaScript/TypeScript, Python, Rust (depending on area), or relevant web/networking languages
- **Domain**: Web APIs, networking protocols, developer tooling, software engineering
- **Experience**: Intermediate to advanced in chosen language stack

**What they must NOT touch:**
- Core protocol logic (Circle 1 responsibility)
- Consensus algorithms (Circle 1 responsibility)
- Cryptographic primitives (Circle 1 responsibility)
- State machine implementation (Circle 1 responsibility)

**Current Phase 2 focus:**
- API endpoint implementations
- WebSocket subscription system
- TypeScript and Python SDKs
- Development tooling and local testnet setup

---

### Circle 3: Specialists

**Scope**: Domain expertise areas that support the project without requiring Rust proficiency.

**What they work on:**

**AI/ML Specialists:**
- Compute task verification strategies
- Model optimization for on-chain execution
- ZK-ML proof research and implementation
- GPU compute patterns and best practices

**Security Specialists:**
- Security audits and vulnerability assessments
- Threat modeling
- Penetration testing
- Security documentation and guidelines

**Economics/Tokenomics Specialists:**
- Economic model analysis and validation
- Token distribution strategies
- Incentive mechanism design
- Game theory analysis

**Infrastructure/DevOps Specialists:**
- CI/CD pipeline configuration
- Cloud deployment automation
- Monitoring and observability
- Container orchestration (Docker, Kubernetes)

**Documentation Specialists:**
- Technical documentation writing
- API documentation
- Tutorial creation
- Translation and localization

**Required skills:**
- **Domain expertise**: Deep knowledge in AI/ML, security, economics, DevOps, or technical writing
- **Languages**: Domain-appropriate (Python for ML, YAML/configs for infra, Markdown for docs, etc.)
- **Experience**: Professional-level expertise in chosen specialty

**What they must NOT touch:**
- Core protocol implementation (Circle 1 responsibility)
- API and networking code (Circle 2 responsibility)
- Unless they also have Circle 1 or Circle 2 qualifications

**Current Phase 2 focus:**
- TEE security analysis and attestation verification
- API documentation and developer guides
- Infrastructure automation for testnet deployment
- Economic model validation for Phase 2 features

---

## Cross-Circle Collaboration

Contributors may belong to multiple circles if they have the required skills.

Examples:
- A Rust developer with security expertise (Circle 1 + Security Specialist)
- A TypeScript developer who writes documentation (Circle 2 + Documentation Specialist)
- An AI researcher who implements ML verification (Circle 3 AI Specialist + potential Circle 1 contribution if Rust is learned)

## Getting Started

1. **Identify your circle(s)** based on skills and interests
2. **Review relevant documentation**:
   - Circle 1: `docs/consensus_mechanics.md`, `docs/transaction_structure.md`
   - Circle 2: API specs, SDK documentation
   - Circle 3: Domain-specific docs in `docs/`
3. **Check GitHub issues** tagged with your circle's scope
4. **Open a discussion** if you're unsure which circle fits your contribution
5. **Submit contributions** following the [Contributing Guide](../CONTRIBUTING.md)

## Boundaries and Restrictions

**Strict boundaries exist to maintain code quality and security:**

- Circle 1 work requires Rust expertise. Contributions without Rust proficiency will be rejected.
- Circle 2 work must not modify core protocol logic. All protocol changes must go through Circle 1 review.
- Circle 3 specialists must coordinate with Circle 1/2 when their work intersects with code areas.

**Violations of these boundaries will result in PR closure and redirection to the appropriate circle.**

---

## Current Development Phase

**Phase 2 is active** (January 2026+). All contributions must:
- Target the `dev` branch
- Be labeled `phase-2`
- Align with Phase 2 scope (see [README.md](../README.md))

Phase 1 (Foundation) is complete and frozen. No Phase 1 contributions will be accepted.

