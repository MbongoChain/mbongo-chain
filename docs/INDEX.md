# Mbongo Chain Documentation Index

> **If you need to know what Mbongo Chain currently does, read the authority
> map below and nothing else.** The rest of this file catalogues the whole
> documentation corpus, most of which predates the current protocol and
> describes designs that were never built.

---

## Authority map — what is true today

Each row names the one document that decides its subject. Where another
document disagrees with one of these, this table wins.

| Subject | Authority | Status |
|---|---|---|
| Project vision and scope | [`VISION_v1.md`](VISION_v1.md) | NORMATIVE |
| Frozen protocol surfaces, versioning | [`specs/PROTOCOL_LOCK_v0.3.md`](specs/PROTOCOL_LOCK_v0.3.md) | NORMATIVE |
| JSON-RPC contract | [`specs/rpc_v0.2.md`](specs/rpc_v0.2.md) | NORMATIVE (FROZEN) |
| Receipt v1 structure | [`specs/RECEIPT_SPEC_v0.1.md`](specs/RECEIPT_SPEC_v0.1.md) | NORMATIVE |
| Receipt anchoring consensus rules | [`rfcs/0002-receipt-anchoring-v0.3.md`](rfcs/0002-receipt-anchoring-v0.3.md) | NORMATIVE (Accepted) |
| How protocol changes are proposed and accepted | [`RFC_PROCESS.md`](RFC_PROCESS.md) | NORMATIVE |
| Which changes need an RFC | [`CONTRIBUTION_TIERS.md`](CONTRIBUTION_TIERS.md) | NORMATIVE |
| Contribution workflow | [`../CONTRIBUTING.md`](../CONTRIBUTING.md) | NORMATIVE |
| Receipt anchoring: data model, crypto domains, storage | [`architecture/compute-receipts.md`](architecture/compute-receipts.md) | CURRENT |
| Building, signing, anchoring, verifying receipts | [`development/compute-receipts.md`](development/compute-receipts.md) | CURRENT |
| Devnet topology and infrastructure | [`architecture/devnet-infrastructure.md`](architecture/devnet-infrastructure.md) | CURRENT |
| Running a devnet locally | [`development/devnet.md`](development/devnet.md) | CURRENT |
| Devnet operations | [`runbooks/DEVNET_V0.3_OPERATIONS.md`](runbooks/DEVNET_V0.3_OPERATIONS.md) | CURRENT |
| TypeScript SDK | [`../sdk/typescript/README.md`](../sdk/typescript/README.md) | CURRENT |

The shipped RPC surface is **six JSON-RPC methods**. Receipt anchoring travels
through the generic `submit_transaction`; there is no compute RPC, and no
lookup of a receipt by `task_id`.

---

## Status vocabulary

| Status | Meaning |
|---|---|
| **NORMATIVE** | Defines a contract or process other documents defer to. |
| **CURRENT** | Accurately describes what is implemented or operated today. |
| **HISTORICAL** | Records past work or evidence. Not current authority. |
| **SUPERSEDED** | A newer authoritative document replaces it. |
| **ASPIRATIONAL** | Describes planned or imagined behaviour that is not implemented. |

Only documents named in the authority map above, or explicitly marked in this
file, carry a status. The rest of the corpus is unclassified.

---

## Reading anything else in this directory

Most of `docs/` was written between November 2025 and February 2026, before
the protocol was locked, before RPC v0.2, and before the vision narrowed to a
verification layer for off-chain inference receipts. Those documents were
written in good faith as design material and remain useful for understanding
intent and history.

**They are not authoritative, and several describe systems that do not
exist.** Unless a document appears in the authority map above, or the sections
below mark it CURRENT or NORMATIVE, assume it records a design idea rather
than the running system, and verify against the code, the specs, or the tests
before relying on it.

Four documents are known to contradict the running system and carry a banner
saying so: [`ts_sdk_overview.md`](ts_sdk_overview.md),
[`rpc_overview.md`](rpc_overview.md),
[`openapi_reference.md`](openapi_reference.md) and
[`governance_model.md`](governance_model.md). Others may be wrong without
having been checked yet.

---

## Catalogue

Everything below is a map of the corpus, not a statement about correctness.

### Legend

- **[L1]** - High-level overviews and introductions
- **[L2]** - Detailed specifications and guides
- **[L3]** - Implementation details and advanced topics
- **[PRIMARY]** - Most complete document on the topic within this catalogue
- **[ARCHIVE]** - Older version, kept for reference

`[PRIMARY]` marks depth within the catalogue. It does **not** mean current or
authoritative; the authority map above is the only source of that.

---

## 1. Introduction & Getting Started

### 1.1 Project Overview
```
├── vision.md [L1]
│   └── Project vision, mission, and long-term goals
├── mbongo_whitepaper.md [L1]
│   └── High-level technical whitepaper
├── roadmap.md [L1]
│   └── Development roadmap and milestones
└── faq.md [L1]
    └── Frequently asked questions
```

### 1.2 Onboarding
```
├── getting_started.md [L1]
│   └── Quick start guide for all users
├── onboarding_dev.md [L1]
│   └── Developer onboarding and first steps
└── glossary.md [L1]
    └── Terminology and definitions
```

---

## 2. Core Concepts

### 2.1 Consensus Mechanism
```
├── consensus_master_overview.md [L2] [PRIMARY]
│   └── Complete consensus specification (PoS + PoUW + PoC)
├── poc_consensus_mechanics.md [L2] [PRIMARY]
│   └── Detailed PoC scoring, compute units, reliability, decay
├── pox_formula.md [L2] [PRIMARY]
│   └── Mathematical formula: total_weight = (stake_weight × C_SR) + (√(poc_score) × C_NL)
└── archive/
    ├── consensus_overview.md [L1] [ARCHIVE]
    ├── consensus_validation.md [L2] [ARCHIVE]
    ├── consensus_validation_summary.md [L2] [ARCHIVE]
    ├── consensus_integrity_checks.md [L3] [ARCHIVE]
    └── block_validation_pipeline.md [L3] [ARCHIVE]
```

### 2.2 Verification & Security
```
├── verification_strategy.md [L2] [PRIMARY]
│   ├── Phase 1: Redundant Execution (3 validators)
│   ├── Phase 2: TEE Integration (Intel SGX / AMD SEV)
│   ├── Phase 3: ZK-ML Proofs
│   └── Fraud proofs (100-block challenge period)
├── sybil_resistance.md [L2] [PRIMARY]
│   ├── GPU fingerprinting
│   ├── Minimum stake (1,000 MBO)
│   ├── TEE attestation
│   ├── Behavioral analysis
│   └── Slashing mechanisms
└── economic_security.md [L2] [PRIMARY]
    └── Complete economic security model and attack vectors
```

### 2.3 Market & Competition
```
├── competitive_analysis.md [L2]
│   └── Comparison vs Render, Akash, io.net, Gensyn, Bittensor, RunPod
└── target_market.md [L2]
    └── Market analysis, customer personas, TAM estimation
```

---

## 3. Architecture

### 3.1 System Design
```
├── architecture_master_overview.md [L2] [PRIMARY]
│   ├── Complete system architecture
│   ├── All layer interactions
│   ├── Component relationships
│   └── Data flow diagrams
├── full_system_overview.md [L2]
│   └── End-to-end system overview
├── node_architecture.md [L2]
│   └── Node types: Full, Validator, Guardian, Light
└── runtime_architecture.md [L2]
    └── Runtime execution and WebAssembly VM
```

### 3.2 Core Components
```
├── execution_engine_overview.md [L3]
│   └── Transaction execution and state transitions (S' = F(S,T))
├── transaction_structure.md [L2] [PRIMARY]
│   └── Fundamental transaction schema, serialization, signatures
├── block_structure.md [L2]
│   └── Block header/body schema, roots, and serialization
├── architecture/compute-receipts.md [L2] [PRIMARY]
│   └── Compute receipts, anchoring, cryptographic domains, validation
├── compute_engine_overview.md [L3]
│   └── GPU compute execution runtime
├── mempool_overview.md [L3]
│   └── Transaction pool design, priority queues, eviction
└── state_machine_validation.md [L3]
    └── State machine validation and transition logic
```

### 3.3 Data & Validation
```
└── sync_validation.md [L3]
    └── Chain synchronization and validation
```

---

## 4. Economics & Tokenomics

### 4.1 Token Fundamentals
```
├── token_intro.md [L1]
│   └── MBO token overview and basics
├── token_distribution.md [L2]
│   └── Distribution schedule and allocations
├── supply_schedule.md [L2]
│   └── Emission schedule: 31,536,000 MBO max supply
└── monetary_policy.md [L2]
    └── Inflation, deflation, monetary policy rules
```

### 4.2 Economic Design
```
├── economic_security.md [L2] [PRIMARY]
│   └── Economic attack resistance and game theory
├── incentive_design.md [L2]
│   └── Incentive structures for validators, compute providers, users
├── staking_model.md [L2]
│   └── Staking mechanics, rewards, time multipliers
├── fee_model.md [L2]
│   └── Transaction fees, compute fees, gas model
└── reward_mechanics.md [L2]
    └── Reward distribution algorithms
```

### 4.3 Value & Utility
```
├── utility_value.md [L2]
│   └── Token utility analysis and value capture
├── compute_value.md [L3]
│   └── Compute value calculation: job_value × compute_units × reliability
└── vesting_model.md [L2]
    └── Vesting schedules for team, investors, community
```

### 4.4 Governance
```
├── governance_model.md [L2] [ASPIRATIONAL]
│   └── Stake-weighted on-chain DAO: proposals, voting, treasury.
│       No such mechanism exists. Repository and protocol changes are
│       governed by RFC_PROCESS.md and CONTRIBUTION_TIERS.md.
└── oracle_model.md [L3]
    └── Oracle design for external data feeds
```

### 4.5 Economic Summary
```
└── economic_summary.md [L1]
    └── High-level economic overview
```

---

## 5. Operations & Node Setup

### 5.1 Validator Operations
```
├── validator_setup.md [L2] [PRIMARY]
│   ├── Complete validator setup guide
│   ├── Hardware requirements
│   ├── Installation steps
│   ├── Configuration
│   └── Monitoring
├── setup_validation.md [L2]
│   └── Validate node setup and troubleshooting
└── guardian_status.md [L3]
    └── Guardian node operations and special privileges
```

### 5.2 Compute Provider Operations
```
└── compute_provider_setup.md [L2] [PRIMARY]
    ├── GPU compute provider setup
    ├── Hardware requirements (NVIDIA/AMD)
    ├── Driver installation
    ├── Job acceptance configuration
    └── Performance optimization
```

### 5.3 Full Node Operations
```
├── full_node_setup.md [L2] [PRIMARY]
│   └── Full node installation and configuration
└── node_setup_overview.md [L1]
    └── General node setup summary
```

---

## 6. Development

### 6.1 Developer Guides
```
├── developer_guide.md [L1]
│   └── Getting started with development
├── developer_introduction.md [L1]
│   └── Introduction for developers
├── developer_environment.md [L2]
│   ├── Environment setup (Rust, Node.js, tooling)
│   ├── Build process
│   └── Testing setup
├── developer_workflow.md [L2]
│   └── Development workflow and best practices
└── contributing_workflow.md [L2]
    └── How to contribute code and documentation
```

### 6.2 SDKs & Libraries
```
├── rust_sdk_overview.md [L2]
│   ├── Rust SDK installation
│   ├── Core types and traits
│   ├── Transaction building
│   └── Code examples
├── ts_sdk_overview.md [L2] [ASPIRATIONAL]
│   └── Wallet, ComputeClient, GovernanceClient and providers that
│       @mbongo/sdk does not implement. For the shipped package see
│       ../sdk/typescript/README.md.
└── development/compute-receipts.md [L2] [PRIMARY]
    ├── Receipt primitives and anchoring API
    ├── Nonce, errors and retry semantics
    └── Testing against the shared fixtures
```

---

## 7. APIs & CLI

### 7.1 Command Line Interface
```
├── cli_overview.md [L2] [PRIMARY]
│   └── Complete CLI command reference
├── cli_node.md [L2]
│   └── Node management: start, stop, status, logs
├── cli_wallet.md [L2]
│   └── Wallet management: create, import, send, balance
└── cli_config.md [L2]
    └── Configuration management and environment variables
```

### 7.2 APIs
```
├── specs/rpc_v0.2.md [L2] [PRIMARY]
│   └── The JSON-RPC contract the node actually serves. FROZEN.
│       Six methods, no subscriptions.
├── rpc_overview.md [L2] [ASPIRATIONAL]
│   └── An Ethereum-compatible surface (eth_*, mbongo_* camelCase,
│       WebSocket subscriptions) that the node does not serve.
└── openapi_reference.md [L3] [ASPIRATIONAL]
    └── Eighteen /v1/* REST paths, none of which the node serves.
        A small REST surface does exist; see crates/mbongo-api.
```

---

## 8. Security

### 8.1 Threat Prevention
```
├── sybil_resistance.md [L2] [PRIMARY]
│   └── Multi-layer Sybil attack prevention
├── verification_strategy.md [L2] [PRIMARY]
│   └── Multi-layer compute verification
└── economic_security.md [L2] [PRIMARY]
    └── Economic attack resistance
```

---

## 9. Meta Documentation

### 9.1 Project Information
```
├── INDEX.md [L1] [PRIMARY]
│   └── Authority map and documentation catalogue (this file)
├── README.md [L1] [SUPERSEDED]
│   └── An earlier navigation hub for this directory, superseded by
│       this file. Kept for its category descriptions.
├── vision.md [L1]
│   └── Project vision and goals
├── mbongo_whitepaper.md [L1]
│   └── Technical whitepaper
└── roadmap.md [L1]
    └── Development roadmap
```

### 9.2 Validation & Status
```
├── spec_validation_summary.md [L3]
│   └── Specification validation status
└── final_doc_index.md [L3] [SUPERSEDED]
    └── A third documentation index from November 2025, superseded by
        this file.
```

### 9.3 Archive
```
└── archive/
    ├── consensus_overview.md
    ├── consensus_validation.md
    ├── consensus_validation_summary.md
    ├── consensus_integrity_checks.md
    └── block_validation_pipeline.md
```

---

## Document Statistics

### By Category
```
Introduction & Getting Started     7 documents
Core Concepts                      8 documents
Architecture                       8 documents
Economics & Tokenomics            14 documents
Operations & Node Setup            5 documents
Development                        7 documents
APIs & CLI                         6 documents
Security                           3 documents
Meta Documentation                 5 documents
Archive                            5 documents
─────────────────────────────────────────────
TOTAL                             68 documents
```

### By Level
```
[L1] High-level overviews         18 documents
[L2] Detailed specifications      33 documents
[L3] Implementation details       12 documents
[ARCHIVE] Archived documents       5 documents
```

### Longest documents in the catalogue

This ranks depth of coverage within the 2025 design corpus. It is not a
statement about what is current, and it is not the authority map — that is at
the top of this file. `rpc_overview.md` below is aspirational; `README.md` is
superseded by this file.

```
1.  consensus_master_overview.md
2.  poc_consensus_mechanics.md
3.  pox_formula.md
4.  verification_strategy.md
5.  sybil_resistance.md
6.  economic_security.md
7.  architecture_master_overview.md
8.  validator_setup.md
9.  compute_provider_setup.md
10. full_node_setup.md
11. cli_overview.md
12. rpc_overview.md
13. README.md (this directory)
14. INDEX.md (this file)
```

---

## Documentation Reading Paths

**Path 0 is the only one that traverses current documents.** Paths 1 to 6
below were written for the November 2025 corpus and route through design
material, including documents now known to be aspirational. They are kept
because they show how that material was meant to be read, not because they
describe the running system.

### Path 0: What Mbongo Chain currently is
```
1. ../README.md                            what runs today, how to start it
2. VISION_v1.md                            scope, and what is excluded
3. specs/PROTOCOL_LOCK_v0.3.md             which surfaces are frozen
4. specs/rpc_v0.2.md                       the RPC contract
5. architecture/compute-receipts.md        receipts on chain
6. ../sdk/typescript/README.md             the shipped SDK
7. RFC_PROCESS.md                          how any of it changes
```

### Path 1: New User (Non-Technical)
```
1. vision.md
2. mbongo_whitepaper.md
3. faq.md
4. competitive_analysis.md
5. target_market.md
```

### Path 2: Developer (Getting Started)
```
1. getting_started.md
2. developer_guide.md
3. developer_environment.md
4. rust_sdk_overview.md
5. cli_overview.md
```
For the SDK and the RPC contract this path is superseded by Path 0:
`../sdk/typescript/README.md` and `specs/rpc_v0.2.md`. The documents it
used to name here, `ts_sdk_overview.md` and `rpc_overview.md`, are
aspirational.

### Path 3: Validator (Operations)
```
1. consensus_master_overview.md
2. pox_formula.md
3. economic_security.md
4. validator_setup.md
5. setup_validation.md
6. cli_node.md
```

### Path 4: Compute Provider
```
1. poc_consensus_mechanics.md
2. compute_value.md
3. verification_strategy.md
4. compute_provider_setup.md
5. compute_engine_overview.md
```

### Path 5: Blockchain Architect (Technical Deep Dive)
```
1. architecture_master_overview.md
2. consensus_master_overview.md
3. pox_formula.md
4. verification_strategy.md
5. sybil_resistance.md
6. economic_security.md
7. execution_engine_overview.md
8. compute_engine_overview.md
9. state_machine_validation.md
```

### Path 6: Economist/Tokenomics Analyst
```
1. token_intro.md
2. supply_schedule.md
3. economic_security.md
4. staking_model.md
5. incentive_design.md
6. fee_model.md
7. governance_model.md
8. utility_value.md
```

---

## Cross-References

### Consensus → Economics
```
consensus_master_overview.md ──→ staking_model.md
poc_consensus_mechanics.md   ──→ compute_value.md
pox_formula.md               ──→ reward_mechanics.md
```

### Architecture → Implementation
```
architecture_master_overview.md ──→ execution_engine_overview.md
architecture_master_overview.md ──→ compute_engine_overview.md
node_architecture.md            ──→ validator_setup.md
```

### Security → Operations
```
verification_strategy.md ──→ compute_provider_setup.md
sybil_resistance.md      ──→ validator_setup.md
economic_security.md     ──→ staking_model.md
```

---

## Version Information

- **Documentation Version**: 1.0.0
- **Last Major Update**: December 2025
- **Total Documents**: 68
- **Primary Documents**: 14
- **Archived Documents**: 5

---

## Maintenance Notes

### Archive Policy

Documents are moved to `archive/` when:
1. Replaced by newer, more comprehensive version
2. Content is superseded by canonical document
3. Contains outdated information but kept for reference
4. Still referenced by external sources

Archived documents are NOT deleted to maintain historical context.

### Update Schedule

- **Primary documents**: Review quarterly
- **Technical specifications**: Update with protocol changes
- **Setup guides**: Update with each release
- **API references**: Auto-generate from code

---

## Contributing

To add new documentation:

1. Follow naming conventions in README.md
2. Add entry to this INDEX.md in appropriate section
3. Update README.md navigation links
4. Mark document level: [L1], [L2], or [L3]
5. Mark as [PRIMARY] if canonical for topic
6. Submit PR with documentation updates

---

**For detailed documentation standards and guidelines, see [README.md](README.md)**
