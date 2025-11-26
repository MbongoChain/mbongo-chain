# Mbongo Chain — Documentation Index

> **Document Type:** Master Index  
> **Last Updated:** November 2025  
> **Status:** Canonical Reference

---

## 1. Purpose

This file is the **master index** of all technical documentation for Mbongo Chain.

It provides a complete navigation map to help developers locate documentation across:
- Architecture and system design
- Consensus and execution
- Networking and synchronization
- GPU compute and PoUW
- Validation and security
- Node types and operations
- Developer guides and workflows

**Use this document as your starting point when exploring the codebase.**

---

## 2. High-Level Overview

| Document | Description |
|----------|-------------|
| [architecture_overview.md](./architecture_overview.md) | Master index with concise architecture summary |
| [full_system_overview.md](./full_system_overview.md) | Comprehensive system architecture (12 sections) |
| [architecture_master_overview.md](./architecture_master_overview.md) | Detailed layer-by-layer breakdown with ASCII diagrams |

---

## 3. Core Components

### Networking

| Document | Description |
|----------|-------------|
| [networking_overview.md](./networking_overview.md) | P2P layer: libp2p transport, gossip protocol, peer discovery |
| [network_sync.md](./network_sync.md) | Chain synchronization: full sync, fast sync, snap sync |
| [network_messages.md](./network_messages.md) | Message types: control, data, sync, consensus |

### Mempool

| Document | Description |
|----------|-------------|
| [mempool_overview.md](./mempool_overview.md) | Transaction pool: validation, prioritization, eviction |
| [mempool_policy.md](./mempool_policy.md) | Fee policy, queue limits, replacement rules |

### Consensus

| Document | Description |
|----------|-------------|
| [consensus_overview.md](./consensus_overview.md) | Hybrid PoS + PoUW: leader election, fork choice, finality |
| [consensus_pos.md](./consensus_pos.md) | Proof-of-Stake: staking, validator set, slashing |
| [consensus_pouw.md](./consensus_pouw.md) | Proof-of-Useful-Work: scoring, integration, incentives |

### Execution

| Document | Description |
|----------|-------------|
| [execution_engine_overview.md](./execution_engine_overview.md) | Runtime: transaction pipeline, state machine, determinism |
| [state_machine.md](./state_machine.md) | State transitions: invariants, gas rules, diff model |
| [transaction_lifecycle.md](./transaction_lifecycle.md) | Tx flow: receive → validate → execute → commit |

### Compute (PoUW)

| Document | Description |
|----------|-------------|
| [compute_engine_overview.md](./compute_engine_overview.md) | GPU coordination: task lifecycle, verification, rewards |
| [compute_providers.md](./compute_providers.md) | Provider setup: hardware, determinism, slashing |
| [compute_marketplace.md](./compute_marketplace.md) | Task submission, pricing, reputation system |

---

## 4. Validation & Security

### Block Validation

| Document | Description |
|----------|-------------|
| [block_validation_pipeline.md](./block_validation_pipeline.md) | Block verification: header, body, state root checks |
| [block_execution.md](./block_execution.md) | Block processing: execution flow, partial failures |

### Sync Validation

| Document | Description |
|----------|-------------|
| [sync_validation.md](./sync_validation.md) | Sync verification: checkpoint proofs, state verification |
| [chain_selection.md](./chain_selection.md) | Fork choice: heaviest chain, reorg rules |

### State Validation

| Document | Description |
|----------|-------------|
| [state_machine_validation.md](./state_machine_validation.md) | State verification: invariants, transition rules |
| [merkle_proofs.md](./merkle_proofs.md) | Proof generation and verification |

### Consensus Validation

| Document | Description |
|----------|-------------|
| [consensus_validation.md](./consensus_validation.md) | Consensus checks: proposer eligibility, attestations |
| [slashing_conditions.md](./slashing_conditions.md) | Slashing: equivocation, invalid blocks, compute fraud |

### Guardian & Compute Validation

| Document | Description |
|----------|-------------|
| [guardian_status.md](./guardian_status.md) | Guardian node: status tracking, compute coordination |
| [compute_verification.md](./compute_verification.md) | Result verification: replicated, probabilistic, ZK |

### Security

| Document | Description |
|----------|-------------|
| [security_model.md](./security_model.md) | Threat analysis: consensus, execution, network, storage |
| [error_categories.md](./error_categories.md) | Error taxonomy: validation, runtime, system errors |
| [attack_vectors.md](./attack_vectors.md) | Known attacks and mitigations |

---

## 5. Node Architecture

### Node Types

| Document | Description |
|----------|-------------|
| [node_architecture.md](./node_architecture.md) | Node types: Full, Validator, Guardian, Light |
| [node_full.md](./node_full.md) | Full node: validation, relay, sync serving |
| [node_validator.md](./node_validator.md) | Validator node: block production, staking |
| [node_guardian.md](./node_guardian.md) | Guardian node: GPU coordination, PoUW verification |
| [node_light.md](./node_light.md) | Light node: header verification, proofs [FUTURE] |

### Hardware Requirements

| Node Type | CPU | RAM | Storage | Network | GPU |
|-----------|-----|-----|---------|---------|-----|
| Full | 4+ cores | 16 GB | 500 GB SSD | 100 Mbps | — |
| Validator | 8+ cores | 32 GB | 1 TB NVMe | 1 Gbps | — |
| Guardian | 16+ cores | 64 GB | 2 TB NVMe | 10 Gbps | A100/H100 |
| Light | 2 cores | 4 GB | 10 GB | 10 Mbps | — |

### Node Roles Matrix

| Capability | Full | Validator | Guardian | Light |
|------------|:----:|:---------:|:--------:|:-----:|
| Full validation | ✓ | ✓ | ✓ | — |
| Block production | — | ✓ | ✓ | — |
| Staking | — | ✓ | ✓ | — |
| GPU compute | — | — | ✓ | — |
| PoUW scoring | — | — | ✓ | — |
| Serve sync | ✓ | ✓ | ✓ | — |
| Header proofs | ✓ | ✓ | ✓ | ✓ |

---

## 6. Developer Documentation

### Getting Started

| Document | Description |
|----------|-------------|
| [getting_started.md](./getting_started.md) | Quick start: clone, build, run |
| [developer_introduction.md](./developer_introduction.md) | Project overview for new contributors |
| [developer_environment.md](./developer_environment.md) | IDE setup, tooling, extensions |

### Development Workflow

| Document | Description |
|----------|-------------|
| [developer_workflow.md](./developer_workflow.md) | Daily workflow: branches, commits, testing |
| [contributing_workflow.md](./contributing_workflow.md) | Contribution process: PRs, reviews, merging |
| [code_style_guide.md](./code_style_guide.md) | Rust style: formatting, naming, patterns |
| [testing_guide.md](./testing_guide.md) | Testing: unit, integration, benchmarks |

### Troubleshooting

| Error | Solution |
|-------|----------|
| `cargo build` fails with missing deps | Run `cargo update` and ensure Rust 1.70+ |
| Node won't sync | Check peer connectivity, firewall rules |
| State root mismatch | Verify determinism, check for non-deterministic code |
| Transaction rejected (nonce) | Fetch current nonce from state before sending |
| Out of gas | Increase gas limit or optimize transaction |
| Signature verification failed | Verify key format and signing algorithm |
| Peer connection refused | Check port forwarding and libp2p config |
| Checkpoint verification failed | Re-download checkpoint, verify hash |
| PoUW receipt rejected | Ensure deterministic execution settings |
| Database corruption | Restore from checkpoint, rebuild indexes |

### Quick Reference

| Topic | Location |
|-------|----------|
| CLI commands | `cli/README.md` |
| Configuration | `node/config/` |
| RPC API | [rpc_api.md](./rpc_api.md) |
| Error codes | [error_categories.md](./error_categories.md) |
| Gas costs | [gas_model.md](./gas_model.md) |

---

## 7. Roadmaps & Future Extensions

### Development Roadmap

| Document | Description |
|----------|-------------|
| [roadmap.md](./roadmap.md) | Development timeline: phases, milestones |
| [changelog.md](./changelog.md) | Version history and release notes |

### Future Features

| Feature | Status | Document |
|---------|--------|----------|
| **Parallel Execution** | Design | [parallel_execution.md](./parallel_execution.md) |
| **ZK State Proofs** | Research | [zk_future.md](./zk_future.md) |
| **GPU Marketplace** | Planned | [compute_marketplace.md](./compute_marketplace.md) |
| **WASM VM** | Planned | [vm_design.md](./vm_design.md) |
| **Cross-Chain Bridges** | Research | [bridges.md](./bridges.md) |

### Future Vision

**Parallel Execution**  
Transaction dependency analysis enabling concurrent execution of non-conflicting transactions. Target: 10x throughput with deterministic merge ordering.

**ZK Integration**  
Zero-knowledge proofs for succinct state transition verification. Light clients sync via ZK proofs. Cross-chain bridges use ZK for trust-minimized verification.

**GPU Marketplace**  
Global compute marketplace where external chains submit tasks. Features: price discovery, reputation, cross-chain settlement.

**VM Design**  
Deterministic WASM engine with instruction-level gas metering, sandboxed execution, and native PoUW integration for compute-heavy contracts.

---

## 8. Recommended Reading Order

| # | Document | Purpose |
|---|----------|---------|
| 1 | [getting_started.md](./getting_started.md) | Set up environment |
| 2 | [developer_introduction.md](./developer_introduction.md) | Understand project |
| 3 | [architecture_overview.md](./architecture_overview.md) | Get big picture |
| 4 | [full_system_overview.md](./full_system_overview.md) | Deep architecture dive |
| 5 | [execution_engine_overview.md](./execution_engine_overview.md) | Learn runtime |
| 6 | [consensus_overview.md](./consensus_overview.md) | Understand consensus |
| 7 | [compute_engine_overview.md](./compute_engine_overview.md) | Explore PoUW |
| 8 | [networking_overview.md](./networking_overview.md) | Study P2P |
| 9 | [node_architecture.md](./node_architecture.md) | Review node types |
| 10 | [security_model.md](./security_model.md) | Understand security |

**Task-Specific Paths:**

| Task | Start With |
|------|------------|
| Consensus work | `consensus_overview.md` → `consensus_validation.md` |
| Runtime development | `execution_engine_overview.md` → `state_machine.md` |
| GPU provider setup | `compute_engine_overview.md` → `compute_providers.md` |
| Node operations | `node_architecture.md` → `node_guardian.md` |
| Security audit | `security_model.md` → `attack_vectors.md` |

---

## 9. Tags & Keywords

**Architecture:**
`architecture` `layers` `components` `system-design` `overview` `diagram`

**Consensus:**
`consensus` `pos` `proof-of-stake` `pouw` `proof-of-useful-work` `leader-election` `fork-choice` `finality` `slashing`

**Execution:**
`execution` `runtime` `state-machine` `stf` `transaction` `determinism` `gas` `metering`

**Compute:**
`compute` `gpu` `pouw` `task` `verification` `provider` `marketplace` `reward`

**Networking:**
`network` `p2p` `libp2p` `gossip` `sync` `peer` `discovery` `message`

**Storage:**
`storage` `state` `trie` `merkle` `blocks` `checkpoint` `index` `rocksdb`

**Validation:**
`validation` `verification` `proof` `security` `slashing` `error` `invariant`

**Nodes:**
`node` `full-node` `validator` `guardian` `light-client` `hardware`

**Development:**
`developer` `getting-started` `workflow` `testing` `contributing` `rust`

**Future:**
`roadmap` `parallel` `zk` `zero-knowledge` `wasm` `vm` `marketplace` `bridge`

---

## Document Status Legend

| Status | Meaning |
|--------|---------|
| ✅ Complete | Document exists and is current |
| 🚧 Draft | Document exists, needs review |
| 📋 Planned | Document planned, not yet written |
| 🔮 Future | Feature/doc for future versions |

---

## Quick Navigation

```
docs/
├── Index & Overview
│   ├── final_doc_index.md ........... This file (master index)
│   ├── architecture_overview.md ..... Concise architecture summary
│   ├── full_system_overview.md ...... Comprehensive system docs
│   └── architecture_master_overview.md Layer-by-layer breakdown
│
├── Core Components
│   ├── consensus_overview.md ........ PoS + PoUW consensus
│   ├── execution_engine_overview.md . Runtime and state machine
│   ├── compute_engine_overview.md ... GPU coordination and PoUW
│   ├── networking_overview.md ....... P2P and sync protocols
│   └── mempool_overview.md .......... Transaction pool
│
├── Validation & Security
│   ├── block_validation_pipeline.md . Block verification
│   ├── state_machine_validation.md .. State transition rules
│   └── security_model.md ............ Threat analysis
│
├── Node Architecture
│   ├── node_architecture.md ......... Node types overview
│   └── node_guardian.md ............. Guardian node details
│
└── Developer Guides
    ├── getting_started.md ........... Quick start
    ├── developer_introduction.md .... Project overview
    └── contributing_workflow.md ..... Contribution process
```

---

*This document is the master index for all Mbongo Chain documentation. Keep it updated as new documents are added.*

