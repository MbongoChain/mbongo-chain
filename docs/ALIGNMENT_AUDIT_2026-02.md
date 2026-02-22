# Vision v1 Alignment Audit — February 2026

**Audit date:** 2026-02-17
**Audited against:** [VISION_v1.md](VISION_v1.md), [COMPUTE_INTERFACE_v0.1.md](specs/COMPUTE_INTERFACE_v0.1.md), [economic_summary.md](economic_summary.md), [economic_security.md](economic_security.md)
**Scope:** All open GitHub issues, milestones, labels, and tokenomics documents

---

## Executive Summary

Vision v1 defines Mbongo as a **deterministic verification layer for off-chain AI inference receipts**. It explicitly excludes on-chain execution, GPU scheduling, PoUW, and marketplace semantics in v1.

The current issue tracker and tokenomics documents were written before this strategic narrowing. They assume a full PoUW execution layer, on-chain GPU scheduling, and a 50/50 PoS/PoUW reward split — all of which contradict Vision v1.

This audit identifies every misalignment and proposes concrete remediation.

---

## 1. Issues That Contradict Vision v1

| # | Title | Contradiction | Recommendation |
|---|-------|---------------|----------------|
| #52 | Phase 2: AI / GPU Compute Exploration | Body says "Prototype GPU / AI compute integration ideas". Vision v1 explicitly states Mbongo is "not a cloud GPU network" and does not execute AI models. Prototyping GPU runtime work implies on-chain or node-side execution. | **Rewrite.** Rescope to "Compute Receipt Format Exploration" — prototyping receipt structures, deterministic verification rules, and off-chain executor SDK integration. Remove all GPU runtime language. |

No other open issues directly implement PoUW execution, on-chain AI, or staking-assumes-compute. The Phase 1 issues (#3, #5, #6, #7) are foundation work and are Vision-neutral.

---

## 2. Issues That Are Unclear or Ambiguous

| # | Title | Ambiguity | Recommendation |
|---|-------|-----------|----------------|
| #52 | Phase 2: AI / GPU Compute Exploration | Labels include `ai-compute` ("AI / GPU / compute work"). The label description implies hands-on GPU work, not verification design. Scope says "Experiments" with "Python, CUDA, C++" — all execution-side languages, not verification-side. | **Rewrite** issue body and **rename label** `ai-compute` to `compute-verification` with description "Compute receipt verification and SDK". |
| #50 | Phase 2: Developer Tooling & CLI Improvements | Broad umbrella. Not contradictory but has no connection to Vision v1 compute priorities. Should be scoped to v1 deliverables. | **Update** body to list concrete v1 tooling: receipt submission CLI, task status query, explorer receipt view. |
| #57 | TypeScript SDK & API Wrapper Exploration | Well-scoped and Vision-aligned. However, it references "fetching chain state" and "submitting a mock transaction" generically. Should mention compute receipt types from COMPUTE_INTERFACE_v0.1. | **Minor update.** Add a note that the SDK should include typed interfaces for `ComputeTask` and `ComputeReceipt` as defined in the compute interface spec. |

---

## 3. Issue Categorization

### v1 Core (allowed now)

| # | Title | Rationale |
|---|-------|-----------|
| #3 | Implement Account Model | Foundation. Required for all settlement. Phase 1 — already implemented but issue still open. |
| #5 | Implement Signature Scheme | Foundation. Required for receipt signature verification. Phase 1 — already implemented. |
| #6 | Implement RocksDB Integration | Foundation. Required for receipt persistence. Phase 1 — already implemented. |
| #7 | Implement State Trie | Foundation. Required for state proofs. Phase 1 — already implemented. |

### v1 Ecosystem (SDK / Explorer / Monitoring)

| # | Title | Rationale |
|---|-------|-----------|
| #57 | TypeScript SDK & API Wrapper | Directly supports v1 deliverable: "SDK for submitting tasks and retrieving receipts." |
| #50 | Developer Tooling & CLI Improvements | Supports v1 deliverable: tooling for receipt submission and task lifecycle. |
| #53 | Infrastructure & Testnet Automation | Supports devnet operations. Vision-neutral infrastructure. |

### Deferred to v2+

| # | Title | Rationale |
|---|-------|-----------|
| #52 | AI / GPU Compute Exploration | GPU runtime prototyping is v2+ scope. Must be rewritten if kept open. |

### Must Be Closed

| # | Title | Rationale |
|---|-------|-----------|
| #3 | Implement Account Model | Already implemented and shipped in Phase 1. Issue is stale. |
| #5 | Implement Signature Scheme | Already implemented and shipped in Phase 1. Issue is stale. |
| #6 | Implement RocksDB Integration | Already implemented and shipped in Phase 1. Issue is stale. |
| #7 | Implement State Trie | Phase 1 used a flat height-indexed storage model, not a Merkle Patricia Trie. The state trie was descoped. Close as `wontfix` or convert to a future v0.3 RFC candidate if state proofs become a v1 requirement. |

---

## 4. Tokenomics Compatibility Analysis

### Source documents

- `docs/economic_summary.md` — primary tokenomics reference
- `docs/economic_security.md` — security model

### Findings

| Tokenomics assumption | Vision v1 reality | Aligned? | Action required |
|-----------------------|-------------------|----------|-----------------|
| **50/50 PoS/PoUW reward split** | v1 has no PoUW. Validators verify receipts; they do not execute compute. There is no "useful work" to reward in the PoUW sense. | **NO** | Reward split must be revised. v1 model: 100% PoS rewards for validators. Compute providers are paid by task submitters via receipt settlement, not by block rewards. PoUW share deferred to v2+. |
| **"Compute providers earn 50% of block rewards"** | v1 compute providers are off-chain executors. They earn fees from task submitters, not block rewards. Block rewards go to validators only. | **NO** | Rewrite "Participants & Incentives" section. Compute providers earn task fees, not block reward share. |
| **"PoUW: Validators earn rewards by executing real AI/ML workloads"** | Vision v1 explicitly: "No Proof of Useful Work. Validators do not execute AI workloads." | **NO** | Remove PoUW references from v1-scoped sections. Preserve as "v2+ optional extension" language. |
| **50,000 MBO validator minimum stake** | Not contradicted by Vision v1. PoS is planned for v0.3+. Stake amounts are economic parameters, not protocol-locked. | Yes | No action. Placeholder — will be finalized in staking RFC. |
| **1,000 MBO fixed slashing for invalid compute** | v1 defines receipt verification but the slashing mechanism is TBD per COMPUTE_INTERFACE_v0.1 section 5. The 1,000 MBO figure is premature. | **Premature** | Mark as placeholder. Must go through RFC before activation. Add note referencing COMPUTE_INTERFACE_v0.1 economic placeholders. |
| **"Compute gas: PoUW task payments (GPU/TPU/CPU/FPGA/ASIC)"** | v1 does not meter GPU compute. Task fees are paid for receipt verification, not for compute gas. | **NO** | Rewrite gas model. v1 has transaction gas only. Compute fees are separate from gas — they are task-level payments settled on receipt verification. |
| **Block time: 1 second** | Current devnet uses configurable block time (default 5s, harness uses 1s). Not contradicted, but 1s is aspirational, not implemented. | Neutral | No action. Aspirational parameter. |
| **Halving every 5 years** | Not contradicted. Emission schedule is future economic policy. | Yes | No action. |
| **Base fees burned** | Not contradicted. Fee burning is compatible with verification-only model. | Yes | No action. |

### Summary

The tokenomics documents assume a **full PoUW execution chain** where validators run GPU workloads and split block rewards 50/50 between stakers and compute workers. Vision v1 is a **verification-only chain** where validators verify receipts and compute happens off-chain. The entire reward and participant model needs a v1-specific revision.

---

## 5. Structured Remediation Plan

### 5.1 Issues to Close

| # | Title | Reason |
|---|-------|--------|
| #3 | Implement Account Model | Implemented. Stale. |
| #5 | Implement Signature Scheme | Implemented. Stale. |
| #6 | Implement RocksDB Integration | Implemented. Stale. |
| #7 | Implement State Trie | Descoped. Close as `wontfix` or convert to RFC candidate. |

### 5.2 Issues to Rewrite

| # | Current title | Proposed title | Key changes |
|---|---------------|----------------|-------------|
| #52 | Phase 2: AI / GPU Compute Exploration | Compute Receipt Verification — Research & Prototyping | Remove GPU/CUDA/C++ language. Scope to: receipt format prototyping, deterministic verification rule design, off-chain executor mock, SDK integration proof-of-concept. Reference COMPUTE_INTERFACE_v0.1. Add label `compute-verification`, remove `ai-compute`. |
| #50 | Phase 2: Developer Tooling & CLI Improvements | Developer Tooling — v1 Verification Layer | Add concrete v1 scope: receipt submission CLI tool, task lifecycle query commands, explorer receipt visualization. Reference VISION_v1 and COMPUTE_INTERFACE_v0.1. |
| #57 | TypeScript SDK & API Wrapper Exploration | TypeScript SDK — v1 Receipt Submission & Query | Add typed interfaces for `ComputeTask`, `ComputeReceipt`, `ComputeStatus` from COMPUTE_INTERFACE_v0.1. Add reserved RPC methods as stub types. |

### 5.3 New Issues to Create

| Title | Milestone | Labels | Description |
|-------|-----------|--------|-------------|
| Tokenomics v1 Revision: Remove PoUW from v1 reward model | v0.3 — Minimal PoS | `docs`, `rfc-required`, `tier-0` | Revise economic_summary.md and economic_security.md. Replace 50/50 PoS/PoUW split with 100% PoS for v1. Move PoUW rewards to v2+ section. Align with VISION_v1 and COMPUTE_INTERFACE_v0.1 economic placeholders. |
| RFC: Staking & Validator Set for v0.3 | v0.3 — Minimal PoS | `rfc-required`, `tier-0`, `mbongo-consensus` | Define minimal PoS: stake registration, validator set selection, slashing for equivocation. No PoUW. No compute execution. Reference VISION_v1 section 4. |
| RFC: ComputeReceipt On-Chain Format for v0.4 | v0.4 — Receipt Standardization | `rfc-required`, `tier-0`, `mbongo-core` | Formalize SCALE encoding for ComputeTask and ComputeReceipt. Define block body extension. Define receipt verification rules. Reference COMPUTE_INTERFACE_v0.1 section 2. |
| Implement Reserved Compute RPC Stubs (return -32601) | v0.3 — Minimal PoS | `tier-1`, `mbongo-network` | Add the 5 reserved compute RPC method names from COMPUTE_INTERFACE_v0.1 section 3. All return "Method not found". Prevents name collisions in SDK development. |
| Explorer MVP: Block & Transaction Viewer | SDK v0.1 | `tier-2`, `tooling`, `good-first-issue` | Web-based block explorer showing blocks, transactions, accounts. Foundation for future receipt visualization. |
| Devnet Smoke Test CI Job | v0.3 — Minimal PoS | `tier-1`, `infra`, `testing` | Run `devnet_harness` as a CI job on `dev` branch pushes. Catches convergence regressions automatically. |
| README Tokenomics Section Alignment | v0.3 — Minimal PoS | `docs`, `tier-2` | Update README.md sections "Key Features" and "Tokenomics Summary" to remove PoUW-as-v1 language. Align with VISION_v1 framing: verification layer, not execution layer. |

### 5.4 Proposed Milestone Structure

| Milestone | Scope | Status |
|-----------|-------|--------|
| **Phase 1 — Foundation** (existing, #1) | Block, tx, account, crypto, storage | Close. All work done. |
| **v0.2 — Devnet Stable** (new) | P2P, sync, block announce, timed production, producer enforcement, replay | Close on tag. All work done. |
| **v0.3 — Minimal PoS** (new) | Staking registration, validator set, slashing for equivocation, reserved compute RPC stubs, tokenomics v1 revision | Next active milestone. |
| **v0.4 — Receipt Standardization** (new) | ComputeReceipt on-chain format, challenge mechanism RFC, receipt verification rules | After v0.3. |
| **SDK v0.1** (new) | TypeScript SDK, CLI receipt tools, explorer MVP | Parallel with v0.3/v0.4. No protocol dependency. |
| **v1.0 — Verified Inference** (new) | Receipt verification live, economic parameters activated, SDK stable, initial integrators | Target milestone. |

### 5.5 Recommended Labels

| Label | Description | Applies to |
|-------|-------------|------------|
| `tier-0` | Protocol / storage / network / apply_block (Core Maintainer approval required) | RFC-gated protocol changes |
| `tier-1` | Node orchestration, harnesses, metrics, logging (non-breaking) | Devnet, CI, observability |
| `tier-2` | SDK, CLI, explorer, docs, CI (no protocol impact) | Ecosystem and tooling |
| `rfc-required` | Change touches a locked surface; RFC must be linked | Any Tier 0 protocol change |
| `compute-verification` | Compute receipt verification and SDK (replaces `ai-compute` for v1 scope) | Receipt design, verification rules, SDK typed interfaces |
| `good-first-issue` | (exists) | Explorer MVP, docs alignment, CLI tools |
| `v1-aligned` | Confirmed aligned with VISION_v1 scope | Triage label for new issues |
| `v2-deferred` | Explicitly deferred to v2+ per Vision v1 | PoUW execution, GPU runtime, marketplace |

---

## 6. Critical Misalignments Requiring Immediate Attention

**Priority 1 — Tokenomics documents contradict Vision v1.**
`economic_summary.md` and `economic_security.md` describe a 50/50 PoS/PoUW split where validators execute GPU workloads. Vision v1 says validators verify receipts, not execute compute. README "Key Features" section also describes PoUW as a current feature. All three documents need revision. This is the highest-impact misalignment because it shapes external perception of the project.

**Priority 2 — Phase 1 milestone still open with 4 completed issues.**
Issues #3, #5, #6, #7 are all implemented and shipped. The milestone should be closed to reflect reality. Leaving it open implies foundation work is incomplete.

**Priority 3 — No milestones exist beyond Phase 1.**
The v0.2 devnet is tagged and stable but has no corresponding milestone. Future work (v0.3 PoS, v0.4 receipts, SDK) has no tracking structure. Creating the proposed milestones enables planning visibility.

**Priority 4 — Issue #52 actively invites GPU runtime work.**
The issue body says "Prototype GPU / AI compute integration ideas" with "CUDA, C++" as skills. This directly contradicts Vision v1's "not a cloud GPU network" position. If a contributor claims this issue as-is, they will produce work that must be rejected.

---

*This audit is a point-in-time assessment. It does not modify any code, issues, or milestones. All recommendations require manual execution by a Core Maintainer.*
