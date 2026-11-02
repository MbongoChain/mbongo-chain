<!-- Verified against tokenomics.md -->
# Mbongo Chain — Governance Model

> **Document Type:** Governance Specification  
> **Last Updated:** November 2025  
> **Status:** ASPIRATIONAL

> [!WARNING]
> **This document describes an on-chain governance mechanism that does not
> exist.** There is no stake-weighted voting, no proposal or treasury system,
> and no PoS or PoUW voting cohort. MBO is not issued.
>
> Repository and protocol changes are today decided by
> [`RFC_PROCESS.md`](RFC_PROCESS.md) and
> [`CONTRIBUTION_TIERS.md`](CONTRIBUTION_TIERS.md). Those documents govern a
> different thing — an off-chain maintainer process — rather than replacing
> the design below. Nothing here currently controls consensus rules, block
> validation, or protocol version upgrades, despite the present tense used
> throughout.

---

## Table of Contents

1. [Governance Scope](#1-governance-scope)
2. [Governance Participants](#2-governance-participants)
3. [Governance Powers Breakdown](#3-governance-powers-breakdown)
4. [Proposal Lifecycle](#4-proposal-lifecycle)
5. [Proposal Categories](#5-proposal-categories)
6. [Voting Rules](#6-voting-rules)
7. [Anti-Manipulation Guarantees](#7-anti-manipulation-guarantees)
8. [Post-Vote Execution Rules](#8-post-vote-execution-rules)
9. [Future Governance Extensions](#9-future-governance-extensions)

---

## 1. Governance Scope

### 1.1 What Governance Controls

Mbongo Chain governance empowers the community to make decisions on protocol evolution while maintaining security and decentralization guarantees.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE SCOPE — CONTROLLABLE                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PROTOCOL UPGRADES                                                                     │
│   ─────────────────                                                                     │
│   • Consensus rule modifications (PoS + PoUW parameters)                               │
│   • Block validation logic updates                                                     │
│   • Network protocol version upgrades                                                  │
│   • State machine execution changes                                                    │
│   • Cryptographic algorithm migrations                                                 │
│                                                                                         │
│   ECONOMIC PARAMETERS                                                                   │
│   ────────────────────                                                                  │
│   • Gas pricing model adjustments                                                      │
│   • Base fee algorithm tuning                                                          │
│   • PoUW compute pricing multipliers                                                   │
│   • AIDA-regulated parameters (within safe bounds)                                     │
│   • Slashing penalty percentages                                                       │
│   • Minimum stake requirements                                                         │
│                                                                                         │
│   Note: PoS/PoUW reward split (50/50) adjustable within DAO-approved                   │
│   safe range (40%–60%) as per tokenomics.md                                            │
│                                                                                         │
│   NEW MODULES & FEATURES                                                                │
│   ──────────────────────                                                                │
│   • Runtime module additions                                                           │
│   • Compute engine extensions                                                          │
│   • Storage layer improvements                                                         │
│   • Networking protocol enhancements                                                   │
│   • Smart contract VM integration (future)                                             │
│                                                                                         │
│   ECOSYSTEM DECISIONS                                                                   │
│   ───────────────────                                                                   │
│   • Grant allocations from ecosystem fund                                              │
│   • Community incentive program adjustments                                            │
│   • Documentation and branding updates                                                 │
│   • Partnership approvals                                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 What Governance Does NOT Control

Certain foundational properties are **immutable** and cannot be modified by governance:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE SCOPE — IMMUTABLE                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   LEDGER HISTORY                                                                        │
│   ──────────────                                                                        │
│   ✗ Cannot revert finalized blocks                                                     │
│   ✗ Cannot modify historical transactions                                              │
│   ✗ Cannot alter past state roots                                                      │
│   ✗ Cannot invalidate completed compute receipts                                       │
│   ✗ Cannot change historical validator/provider rewards                                │
│                                                                                         │
│   EXECUTION DETERMINISM                                                                 │
│   ─────────────────────                                                                 │
│   ✗ Cannot introduce non-deterministic execution                                       │
│   ✗ Cannot break replay-ability guarantees                                             │
│   ✗ Cannot allow undefined behavior in state transitions                               │
│   ✗ Cannot bypass validation rules retroactively                                       │
│                                                                                         │
│   MONETARY INVARIANTS                                                                   │
│   ────────────────────                                                                  │
│   ✗ Cannot increase total supply beyond 31,536,000 MBO                                 │
│   ✗ Cannot create inflationary mechanisms                                              │
│   ✗ Cannot modify halving schedule                                                     │
│   ✗ Cannot unlock vested tokens early                                                  │
│   ✗ Cannot confiscate user funds                                                       │
│                                                                                         │
│   CRYPTOGRAPHIC FOUNDATIONS                                                             │
│   ─────────────────────────                                                             │
│   ✗ Cannot weaken signature schemes                                                    │
│   ✗ Cannot remove slashing evidence requirements                                       │
│   ✗ Cannot disable fraud proofs                                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.3 Governance Boundaries Summary

| Category | Governance Control | Rationale |
|----------|-------------------|-----------|
| **Protocol Rules** | ✓ Modifiable | Allows evolution |
| **Economic Parameters** | ✓ Modifiable (bounded) | Balances flexibility with stability |
| **New Features** | ✓ Modifiable | Enables innovation |
| **Historical State** | ✗ Immutable | Preserves integrity |
| **Total Supply** | ✗ Immutable | Sound money guarantee |
| **Execution Determinism** | ✗ Immutable | Verifiability requirement |

---

## 2. Governance Participants

### 2.1 PoS Validators & Delegators

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoS VALIDATORS & DELEGATORS                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE IN GOVERNANCE                                                                    │
│   ──────────────────                                                                    │
│   Validators and their delegators form the primary governance constituency.            │
│   Their voting power is proportional to staked MBO tokens.                             │
│                                                                                         │
│   VOTING MECHANISM                                                                      │
│   ────────────────                                                                      │
│   • Vote weight = staked MBO at snapshot block                                         │
│   • Validators vote on behalf of delegators (default)                                  │
│   • Delegators can override validator vote                                             │
│   • Delegation does not transfer governance rights                                     │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Review and analyze proposals                                                       │
│   • Vote on protocol upgrades                                                          │
│   • Participate in security discussions                                                │
│   • Signal community sentiment                                                         │
│                                                                                         │
│   ELIGIBILITY                                                                           │
│   ───────────                                                                           │
│   • Validators: Active in current epoch                                                │
│   • Delegators: Staked before snapshot block                                           │
│   • Minimum delegation: 100 MBO                                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 PoUW Compute Providers

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoUW COMPUTE PROVIDERS                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE IN GOVERNANCE                                                                    │
│   ──────────────────                                                                    │
│   Compute providers contribute non-financial governance weight based on their          │
│   verified compute performance and reputation scores.                                  │
│                                                                                         │
│   VOTING MECHANISM                                                                      │
│   ────────────────                                                                      │
│   • Vote weight = PoUW reputation score (non-financial)                                │
│   • Score derived from: task completion rate, verification pass rate, uptime          │
│   • Prevents purchased voting power                                                    │
│   • Ensures compute contributors have voice                                            │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Review compute-related proposals                                                   │
│   • Vote on PoUW parameter changes                                                     │
│   • Provide technical feedback on compute features                                     │
│   • Report compute marketplace issues                                                  │
│                                                                                         │
│   ELIGIBILITY                                                                           │
│   ───────────                                                                           │
│   • Registered compute provider                                                        │
│   • Minimum 30-day active history                                                      │
│   • Reputation score ≥ 0.5 (scale 0-1)                                                 │
│   • No active slashing events                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 MBO Token Holders (Non-Stakers)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBO TOKEN HOLDERS (NON-STAKERS)                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE IN GOVERNANCE                                                                    │
│   ──────────────────                                                                    │
│   Non-staking token holders can participate in governance for non-critical             │
│   (Tier 3) proposals to ensure broad community representation.                         │
│                                                                                         │
│   VOTING MECHANISM                                                                      │
│   ────────────────                                                                      │
│   • Vote weight: 1 MBO = 1 vote                                                        │
│   • Eligible only for Tier 3 (social/community) proposals                              │
│   • Balance snapshot at proposal creation block                                        │
│   • Tokens must be held (not locked in DeFi protocols)                                 │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Vote on community initiatives                                                      │
│   • Participate in social signaling                                                    │
│   • Provide feedback on ecosystem direction                                            │
│                                                                                         │
│   ELIGIBILITY                                                                           │
│   ───────────                                                                           │
│   • Hold ≥ 10 MBO at snapshot                                                          │
│   • Address not flagged for governance abuse                                           │
│                                                                                         │
│   LIMITATIONS                                                                           │
│   ───────────                                                                           │
│   • Cannot vote on Tier 1 (critical) proposals                                         │
│   • Cannot vote on Tier 2 (network parameter) proposals                                │
│   • Lower voting influence compared to stakers                                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Mbongo Foundation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBONGO FOUNDATION (MINIMAL POWERS)                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE IN GOVERNANCE                                                                    │
│   ──────────────────                                                                    │
│   The Foundation serves as a temporary steward during early network growth.            │
│   Powers are minimal and time-limited (10-year Founder Council oversight per           │
│   tokenomics.md).                                                                       │
│                                                                                         │
│   POWERS (STRICTLY LIMITED)                                                             │
│   ─────────────────────────                                                             │
│   ✓ Security hotfix veto (emergency only)                                              │
│   ✓ Execute Tier 3 proposals via multi-sig                                             │
│   ✓ Propose (but not unilaterally pass) protocol changes                               │
│   ✓ Coordinate emergency response                                                      │
│                                                                                         │
│   CANNOT DO                                                                             │
│   ──────────                                                                            │
│   ✗ Override community vote results                                                    │
│   ✗ Modify tokenomics unilaterally                                                     │
│   ✗ Access user funds                                                                  │
│   ✗ Pause network operations (except critical security)                                │
│   ✗ Change consensus rules without vote                                                │
│                                                                                         │
│   ACCOUNTABILITY                                                                        │
│   ──────────────                                                                        │
│   • All Foundation actions logged on-chain                                             │
│   • Quarterly transparency reports required                                            │
│   • Community can vote to remove Foundation powers                                     │
│   • Powers automatically reduce over time                                              │
│                                                                                         │
│   SUNSET TIMELINE                                                                       │
│   ────────────────                                                                      │
│   Year 0-5:   Full stewardship role                                                    │
│   Year 5-10:  Reduced powers (no veto except security)                                 │
│   Year 10+:   Advisory only, no governance powers                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.5 External Reviewers (Security Researchers)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EXTERNAL REVIEWERS                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE IN GOVERNANCE                                                                    │
│   ──────────────────                                                                    │
│   Security researchers and auditors provide expert review of proposals                 │
│   without direct voting power.                                                         │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Technical review of Tier 1 proposals                                               │
│   • Security audits before major upgrades                                              │
│   • Vulnerability disclosure coordination                                              │
│   • Independent verification of implementations                                        │
│                                                                                         │
│   INFLUENCE                                                                             │
│   ─────────                                                                             │
│   • Advisory opinions published on-chain                                               │
│   • Security concerns can trigger extended review period                               │
│   • Verified auditors can flag proposals for additional scrutiny                       │
│   • Bug bounty reporters gain reputation weight                                        │
│                                                                                         │
│   ELIGIBILITY                                                                           │
│   ───────────                                                                           │
│   • Registered through Foundation verification                                         │
│   • Proven security research track record                                              │
│   • No conflicts of interest                                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Governance Powers Breakdown

### 3.1 Power Distribution Matrix

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                            GOVERNANCE POWERS BREAKDOWN                                                          │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                                                 │
│   PARTICIPANT             │ VOTING POWER                    │ RESPONSIBILITIES                   │ RISKS IF COMPROMISED        │
│   ────────────────────────┼─────────────────────────────────┼────────────────────────────────────┼─────────────────────────────│
│                           │                                 │                                    │                             │
│   PoS Validators &        │ Stake-denominated               │ • Vote on all proposal tiers       │ • Cartel formation          │
│   Delegators              │ (1 staked MBO = 1 vote)         │ • Review protocol changes          │ • Vote buying               │
│                           │                                 │ • Signal upgrade readiness         │ • Coordinated attacks       │
│                           │                                 │ • Participate in discussions       │ • Stake concentration       │
│                           │                                 │                                    │                             │
│   PoUW Compute            │ PoUW performance reputation     │ • Vote on compute-related props    │ • Sybil provider farms      │
│   Providers               │ (non-financial score)           │ • Technical feedback               │ • Reputation gaming         │
│                           │ Score range: 0.0 - 1.0          │ • Report marketplace issues        │ • Collusion attacks         │
│                           │                                 │ • Validate compute parameters      │                             │
│                           │                                 │                                    │                             │
│   MBO Token Holders       │ 1 MBO = 1 vote                  │ • Vote on Tier 3 proposals only    │ • Whale manipulation        │
│   (Non-Stakers)           │ (Tier 3 only)                   │ • Community signaling              │ • Flash loan attacks        │
│                           │                                 │ • Ecosystem feedback               │ • Last-minute accumulation  │
│                           │                                 │                                    │                             │
│   Mbongo Foundation       │ Security veto only              │ • Emergency hotfix veto            │ • Centralization risk       │
│                           │ (no positive voting power)      │ • Execute Tier 3 multi-sig         │ • Key compromise            │
│                           │                                 │ • Coordinate security response     │ • Mission drift             │
│                           │                                 │                                    │                             │
│   External Reviewers      │ Advisory only                   │ • Security audits                  │ • False security signals    │
│                           │ (no voting power)               │ • Technical review                 │ • Conflict of interest      │
│                           │                                 │ • Vulnerability disclosure         │ • Bribery                   │
│                           │                                 │                                    │                             │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Voting Weight Formulas

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VOTING WEIGHT CALCULATIONS                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VALIDATOR/DELEGATOR WEIGHT                                                            │
│   ──────────────────────────                                                            │
│   Weight(v) = Staked_MBO(v) at snapshot_block                                          │
│                                                                                         │
│   Where:                                                                                │
│   • Staked_MBO includes self-stake + delegations                                       │
│   • Delegators can override: Delegator_Vote overrides Validator_Vote                   │
│   • Snapshot taken at proposal creation block                                          │
│                                                                                         │
│                                                                                         │
│   COMPUTE PROVIDER WEIGHT                                                               │
│   ───────────────────────                                                               │
│   Weight(p) = Reputation_Score(p) × Provider_Multiplier                                │
│                                                                                         │
│   Reputation_Score = (0.4 × Success_Rate)                                              │
│                    + (0.3 × Verification_Pass_Rate)                                    │
│                    + (0.2 × Uptime)                                                    │
│                    + (0.1 × Longevity)                                                 │
│                                                                                         │
│   Provider_Multiplier = 0.3 (caps provider influence)                                  │
│                                                                                         │
│                                                                                         │
│   TOKEN HOLDER WEIGHT (Tier 3 only)                                                    │
│   ─────────────────────────────────                                                     │
│   Weight(h) = MBO_Balance(h) at snapshot_block                                         │
│                                                                                         │
│   • Simple 1:1 ratio                                                                   │
│   • No multipliers                                                                     │
│   • Capped at 1% of total votes per address (anti-whale)                               │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Tier Eligibility by Participant

| Participant | Tier 1 (Critical) | Tier 2 (Parameters) | Tier 3 (Community) |
|-------------|-------------------|---------------------|-------------------|
| **Validators** | ✓ Full weight | ✓ Full weight | ✓ Full weight |
| **Delegators** | ✓ Full weight | ✓ Full weight | ✓ Full weight |
| **Compute Providers** | ✓ Reputation weight | ✓ Reputation weight | ✓ Reputation weight |
| **Token Holders** | ✗ Not eligible | ✗ Not eligible | ✓ 1:1 weight |
| **Foundation** | ✓ Veto only | ✗ No power | ✓ Execute only |
| **Reviewers** | Advisory | Advisory | Advisory |

---

## 4. Proposal Lifecycle

### 4.1 Complete Lifecycle Flow

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PROPOSAL LIFECYCLE                                              │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 1: DRAFT
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   Author                                                                             │
  │      │                                                                               │
  │      │  1. Create proposal document                                                  │
  │      │  2. Define objectives and rationale                                           │
  │      │  3. Specify implementation details                                            │
  │      │  4. Submit to governance forum                                                │
  │      │                                                                               │
  │      ▼                                                                               │
  │   ┌────────────────────────┐                                                         │
  │   │   DRAFT PROPOSAL       │                                                         │
  │   │   • Title              │                                                         │
  │   │   • Summary            │                                                         │
  │   │   • Motivation         │                                                         │
  │   │   • Specification      │                                                         │
  │   │   • Tier classification│                                                         │
  │   └────────────────────────┘                                                         │
  │                                                                                      │
  │   Duration: Unlimited (author-controlled)                                            │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 2: REVIEW
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   Community & Reviewers                                                              │
  │      │                                                                               │
  │      │  1. Technical review by experts                                               │
  │      │  2. Security audit (Tier 1 required)                                          │
  │      │  3. Community discussion period                                               │
  │      │  4. Author responds to feedback                                               │
  │      │  5. Proposal refinement                                                       │
  │      │                                                                               │
  │      ▼                                                                               │
  │   ┌────────────────────────┐       ┌────────────────────────┐                        │
  │   │   REVIEW PERIOD        │       │   OUTPUTS              │                        │
  │   │                        │       │                        │                        │
  │   │   Tier 1: 14 days min  │  ───▶ │   • Audit report       │                        │
  │   │   Tier 2: 7 days min   │       │   • Community feedback │                        │
  │   │   Tier 3: 3 days min   │       │   • Final proposal     │                        │
  │   └────────────────────────┘       └────────────────────────┘                        │
  │                                                                                      │
  │   Gate: Proposal must receive ≥5% of eligible voters' interest to proceed           │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 3: STAKING SNAPSHOT
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   Protocol                                                                           │
  │      │                                                                               │
  │      │  1. Lock proposal on-chain                                                    │
  │      │  2. Record staking snapshot at block N                                        │
  │      │  3. Calculate eligible voters                                                 │
  │      │  4. Freeze voting weights                                                     │
  │      │                                                                               │
  │      ▼                                                                               │
  │   ┌────────────────────────────────────────────────────────────────────────────────┐ │
  │   │                           SNAPSHOT BLOCK (N)                                   │ │
  │   │                                                                                │ │
  │   │   Captured:                                                                    │ │
  │   │   • All validator stakes                                                       │ │
  │   │   • All delegator positions                                                    │ │
  │   │   • Compute provider reputation scores                                         │ │
  │   │   • Token holder balances (Tier 3)                                             │ │
  │   │                                                                                │ │
  │   │   Post-snapshot stake changes do NOT affect voting weight                      │ │
  │   └────────────────────────────────────────────────────────────────────────────────┘ │
  │                                                                                      │
  │   Duration: Instantaneous (single block)                                             │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 4: ON-CHAIN VOTE
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   Eligible Voters                                                                    │
  │      │                                                                               │
  │      │  1. Cast vote (YES / NO / ABSTAIN)                                            │
  │      │  2. Vote recorded on-chain with signature                                     │
  │      │  3. Votes immutable after submission                                          │
  │      │  4. Running tally visible to all                                              │
  │      │                                                                               │
  │      ▼                                                                               │
  │   ┌────────────────────────┐                                                         │
  │   │   VOTING WINDOW        │                                                         │
  │   │                        │                                                         │
  │   │   Tier 1: 5 epochs     │   Quorum: 70%                                           │
  │   │   Tier 2: 3 epochs     │   Quorum: 55%                                           │
  │   │   Tier 3: 2 epochs     │   Quorum: 25%                                           │
  │   │                        │                                                         │
  │   │   1 epoch ≈ 1 week     │                                                         │
  │   └────────────────────────┘                                                         │
  │                                                                                      │
  │   Possible Outcomes:                                                                 │
  │   • PASSED: Quorum met + YES > NO                                                    │
  │   • FAILED: Quorum met + NO ≥ YES                                                    │
  │   • EXPIRED: Quorum not met                                                          │
  │   • VETOED: Foundation security veto (Tier 1 only)                                   │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 5: EXECUTION
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   If PASSED:                                                                         │
  │      │                                                                               │
  │      │  Tier 1 & 2:                                                                  │
  │      │  1. Execution delay period begins                                             │
  │      │  2. Network upgrade scheduled                                                 │
  │      │  3. Validators signal readiness                                               │
  │      │  4. Auto-execute at target block                                              │
  │      │                                                                               │
  │      │  Tier 3:                                                                      │
  │      │  1. Foundation multi-sig prepares execution                                   │
  │      │  2. 48-hour timelock                                                          │
  │      │  3. Multi-sig execution                                                       │
  │      │                                                                               │
  │      ▼                                                                               │
  │   ┌────────────────────────┐                                                         │
  │   │   EXECUTION DELAY      │                                                         │
  │   │                        │                                                         │
  │   │   Tier 1: 14 days      │   Allows rollback if critical issue found              │
  │   │   Tier 2: 7 days       │                                                         │
  │   │   Tier 3: 48 hours     │                                                         │
  │   └────────────────────────┘                                                         │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STAGE 6: FINALITY
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   Network                                                                            │
  │      │                                                                               │
  │      │  1. Change activated on network                                               │
  │      │  2. All nodes running new rules                                               │
  │      │  3. Proposal marked EXECUTED                                                  │
  │      │  4. Historical record preserved                                               │
  │      │                                                                               │
  │      ▼                                                                               │
  │   ┌────────────────────────────────────────────────────────────────────────────────┐ │
  │   │                              PROPOSAL FINALIZED                                │ │
  │   │                                                                                │ │
  │   │   Status: EXECUTED                                                             │ │
  │   │   Block: #xxxxxxxx                                                             │ │
  │   │   Changes: Active on network                                                   │ │
  │   │   Record: Permanent on-chain                                                   │ │
  │   └────────────────────────────────────────────────────────────────────────────────┘ │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Stage Duration Summary

| Stage | Tier 1 | Tier 2 | Tier 3 |
|-------|--------|--------|--------|
| **Draft** | Unlimited | Unlimited | Unlimited |
| **Review** | ≥14 days | ≥7 days | ≥3 days |
| **Snapshot** | 1 block | 1 block | 1 block |
| **Voting** | 5 epochs (~5 weeks) | 3 epochs (~3 weeks) | 2 epochs (~2 weeks) |
| **Execution Delay** | 14 days | 7 days | 48 hours |
| **Total Minimum** | ~9 weeks | ~5 weeks | ~3 weeks |

---

## 5. Proposal Categories

### 5.1 Three-Tier System Overview

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PROPOSAL TIER SYSTEM                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                            TIER 1: CRITICAL                                      │  │
│   │                                                                                  │  │
│   │   Scope:                                                                         │  │
│   │   • Consensus rule modifications                                                │  │
│   │   • Block validation logic changes                                              │  │
│   │   • Cryptographic algorithm updates                                             │  │
│   │   • State machine execution changes                                             │  │
│   │   • Slashing condition modifications                                            │  │
│   │   • Major security patches                                                      │  │
│   │                                                                                  │  │
│   │   Requirements:                                                                  │  │
│   │   • Quorum: 70% of eligible voting weight                                       │  │
│   │   • Approval: Supermajority (66.7% YES)                                         │  │
│   │   • Security audit: Required                                                    │  │
│   │   • Foundation veto: Available                                                  │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                            TIER 2: NETWORK PARAMETERS                            │  │
│   │                                                                                  │  │
│   │   Scope:                                                                         │  │
│   │   • Gas pricing model adjustments                                               │  │
│   │   • Fee parameter tuning                                                        │  │
│   │   • PoUW compute weight adjustments                                             │  │
│   │   • AIDA parameter bounds                                                       │  │
│   │   • Minimum stake requirements                                                  │  │
│   │   • Provider capacity limits                                                    │  │
│   │                                                                                  │  │
│   │   Requirements:                                                                  │  │
│   │   • Quorum: 55% of eligible voting weight                                       │  │
│   │   • Approval: Simple majority (>50% YES)                                        │  │
│   │   • Security audit: Recommended                                                 │  │
│   │   • Foundation veto: Not available                                              │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                            TIER 3: SOCIAL/COMMUNITY                              │  │
│   │                                                                                  │  │
│   │   Scope:                                                                         │  │
│   │   • Documentation updates                                                       │  │
│   │   • Grant allocations                                                           │  │
│   │   • Community program funding                                                   │  │
│   │   • Branding decisions                                                          │  │
│   │   • Partnership approvals                                                       │  │
│   │   • Ambassador programs                                                         │  │
│   │                                                                                  │  │
│   │   Requirements:                                                                  │  │
│   │   • Quorum: 25% of eligible voting weight                                       │  │
│   │   • Approval: Simple majority (>50% YES)                                        │  │
│   │   • Security audit: Not required                                                │  │
│   │   • Foundation veto: Not available                                              │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Tier Comparison Table

| Attribute | Tier 1 (Critical) | Tier 2 (Parameters) | Tier 3 (Community) |
|-----------|-------------------|---------------------|-------------------|
| **Quorum** | 70% | 55% | 25% |
| **Approval Threshold** | 66.7% (⅔) | >50% | >50% |
| **Review Period** | ≥14 days | ≥7 days | ≥3 days |
| **Voting Window** | 5 epochs | 3 epochs | 2 epochs |
| **Execution Delay** | 14 days | 7 days | 48 hours |
| **Security Audit** | Required | Recommended | Not required |
| **Foundation Veto** | Yes | No | No |
| **Token Holders Vote** | No | No | Yes |
| **Auto-Execution** | Yes | Yes | No (multi-sig) |

### 5.3 Example Proposals by Tier

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EXAMPLE PROPOSALS                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TIER 1 EXAMPLES                                                                       │
│   ───────────────                                                                       │
│   • MIP-001: Upgrade block finality mechanism                                          │
│   • MIP-005: Change signature scheme to BLS aggregation                                │
│   • MIP-012: Modify PoUW verification algorithm                                        │
│   • MIP-018: Update slashing conditions for validators                                 │
│                                                                                         │
│   TIER 2 EXAMPLES                                                                       │
│   ───────────────                                                                       │
│   • MIP-002: Adjust base fee algorithm parameters                                      │
│   • MIP-007: Increase minimum validator stake to 15,000 MBO                            │
│   • MIP-015: Update compute pricing multipliers                                        │
│   • MIP-021: Modify AIDA burn rate bounds (10%-25%)                                    │
│                                                                                         │
│   TIER 3 EXAMPLES                                                                       │
│   ───────────────                                                                       │
│   • MIP-003: Fund developer tooling grant (500,000 MBO)                                │
│   • MIP-009: Launch ambassador program in Asia                                         │
│   • MIP-016: Update documentation repository structure                                 │
│   • MIP-022: Approve partnership with Compute Provider Alliance                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Voting Rules

### 6.1 Quorum Thresholds

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         QUORUM REQUIREMENTS                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TIER 1 (CRITICAL)                                                                     │
│   ─────────────────                                                                     │
│   Quorum = 70% of total eligible voting weight                                         │
│                                                                                         │
│   Eligible = Validator_Stakes + Delegator_Stakes + Provider_Reputation_Weight          │
│                                                                                         │
│   Example:                                                                              │
│   Total eligible weight: 10,000,000                                                    │
│   Required quorum: 7,000,000 (70%)                                                     │
│                                                                                         │
│                                                                                         │
│   TIER 2 (PARAMETERS)                                                                   │
│   ───────────────────                                                                   │
│   Quorum = 55% of total eligible voting weight                                         │
│                                                                                         │
│   Example:                                                                              │
│   Total eligible weight: 10,000,000                                                    │
│   Required quorum: 5,500,000 (55%)                                                     │
│                                                                                         │
│                                                                                         │
│   TIER 3 (COMMUNITY)                                                                    │
│   ──────────────────                                                                    │
│   Quorum = 25% of total eligible voting weight                                         │
│                                                                                         │
│   Eligible = Above + Token_Holder_Balances (capped)                                    │
│                                                                                         │
│   Example:                                                                              │
│   Total eligible weight: 15,000,000                                                    │
│   Required quorum: 3,750,000 (25%)                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Supermajority Requirements

| Tier | Approval Threshold | Rationale |
|------|-------------------|-----------|
| **Tier 1** | 66.7% (⅔ supermajority) | High bar for critical changes |
| **Tier 2** | >50% (simple majority) | Reasonable bar for parameters |
| **Tier 3** | >50% (simple majority) | Low friction for community |

```
APPROVAL CALCULATION:

Tier 1:
  approval_rate = YES_weight / (YES_weight + NO_weight)
  PASSED if: quorum_met AND approval_rate >= 0.667

Tier 2 & 3:
  approval_rate = YES_weight / (YES_weight + NO_weight)
  PASSED if: quorum_met AND approval_rate > 0.50
```

### 6.3 Abstention Handling

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ABSTENTION RULES                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ABSTAIN VOTE EFFECTS                                                                  │
│   ────────────────────                                                                  │
│   1. Counts toward quorum (shows participation)                                        │
│   2. Does NOT count in YES/NO ratio                                                    │
│   3. Signals neutral position on proposal                                              │
│                                                                                         │
│   FORMULA                                                                               │
│   ───────                                                                               │
│   Quorum_Weight = YES_weight + NO_weight + ABSTAIN_weight                              │
│   Approval_Rate = YES_weight / (YES_weight + NO_weight)                                │
│                                                                                         │
│   Note: ABSTAIN is excluded from approval calculation                                  │
│                                                                                         │
│   EXAMPLE                                                                               │
│   ───────                                                                               │
│   Tier 2 proposal with 10,000,000 eligible weight:                                     │
│   • Required quorum: 5,500,000 (55%)                                                   │
│   • Votes: YES=3,000,000, NO=1,500,000, ABSTAIN=2,000,000                              │
│   • Total participation: 6,500,000 (65%) ✓ Quorum met                                  │
│   • Approval: 3,000,000 / 4,500,000 = 66.7% ✓ Passed                                   │
│                                                                                         │
│   IMPLICIT ABSTAIN                                                                      │
│   ────────────────                                                                      │
│   Eligible voters who do not vote are NOT counted as abstaining.                       │
│   Only explicit ABSTAIN votes count toward quorum.                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.4 Multi-Epoch Voting Windows

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VOTING WINDOW STRUCTURE                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   EPOCH DEFINITION                                                                      │
│   ────────────────                                                                      │
│   1 Epoch = ~50,400 blocks = ~7 days (at 1-second blocks)                              │
│                                                                                         │
│                                                                                         │
│   TIER 1 (5 EPOCHS)                                                                     │
│   ─────────────────                                                                     │
│   ┌────────┬────────┬────────┬────────┬────────┐                                       │
│   │Epoch 1 │Epoch 2 │Epoch 3 │Epoch 4 │Epoch 5 │                                       │
│   │  7d    │  7d    │  7d    │  7d    │  7d    │                                       │
│   └────────┴────────┴────────┴────────┴────────┘                                       │
│   │◀──────────── 35 days voting ────────────▶│                                         │
│                                                                                         │
│                                                                                         │
│   TIER 2 (3 EPOCHS)                                                                     │
│   ─────────────────                                                                     │
│   ┌────────┬────────┬────────┐                                                         │
│   │Epoch 1 │Epoch 2 │Epoch 3 │                                                         │
│   │  7d    │  7d    │  7d    │                                                         │
│   └────────┴────────┴────────┘                                                         │
│   │◀───── 21 days voting ────▶│                                                        │
│                                                                                         │
│                                                                                         │
│   TIER 3 (2 EPOCHS)                                                                     │
│   ─────────────────                                                                     │
│   ┌────────┬────────┐                                                                  │
│   │Epoch 1 │Epoch 2 │                                                                  │
│   │  7d    │  7d    │                                                                  │
│   └────────┴────────┘                                                                  │
│   │◀─ 14 days voting ─▶│                                                               │
│                                                                                         │
│                                                                                         │
│   VOTE MODIFICATION                                                                     │
│   ─────────────────                                                                     │
│   • Votes can be changed until voting window closes                                    │
│   • Latest vote overwrites previous                                                    │
│   • Final tally at window close block                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Anti-Manipulation Guarantees

### 7.1 Core Protections

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ANTI-MANIPULATION GUARANTEES                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ✓ NO DISCRETIONARY OVERRIDES                                                         │
│   ────────────────────────────                                                          │
│   • Governance rules are protocol-enforced                                             │
│   • No admin keys can bypass vote requirements                                         │
│   • All thresholds immutable within tier definition                                    │
│   • Emergency changes still require vote (expedited process)                           │
│                                                                                         │
│   ✓ NO UNILATERAL FOUNDATION CHANGES                                                   │
│   ──────────────────────────────────                                                    │
│   • Foundation cannot pass proposals alone                                             │
│   • Veto power limited to security emergencies only                                    │
│   • All Foundation actions require on-chain justification                              │
│   • Community can override Foundation veto (90% vote)                                  │
│                                                                                         │
│   ✓ STAKE SNAPSHOT PREVENTS LAST-MINUTE ATTACKS                                        │
│   ──────────────────────────────────────────────                                        │
│   • Voting weight frozen at proposal creation block                                    │
│   • Post-snapshot stake changes have no effect                                         │
│   • Prevents whale accumulation during voting                                          │
│   • Prevents flash loan governance attacks                                             │
│                                                                                         │
│   ✓ COMPUTE REPUTATION PREVENTS SYBIL ATTACKS                                          │
│   ───────────────────────────────────────────                                           │
│   • Provider voting requires 30+ days history                                          │
│   • Reputation score based on verified work                                            │
│   • Creating fake providers is economically expensive                                  │
│   • Statistical detection of coordinated behavior                                      │
│                                                                                         │
│   ✓ ALL VOTES STORED ON-CHAIN                                                          │
│   ───────────────────────────                                                           │
│   • Complete voting history preserved                                                  │
│   • Votes signed by voter's key                                                        │
│   • Public audit trail for all decisions                                               │
│   • Cannot claim vote was altered                                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Attack Mitigation Matrix

| Attack Vector | Protection Mechanism | Effectiveness |
|---------------|---------------------|---------------|
| **Whale Manipulation** | Stake snapshot, 1% cap for Tier 3 | High |
| **Flash Loan Attack** | Snapshot before vote, no unstaked voting | Very High |
| **Sybil Providers** | Reputation requirement, stake collateral | High |
| **Vote Buying** | On-chain transparency, reputation damage | Medium |
| **Foundation Capture** | Limited powers, sunset timeline, override | High |
| **Last-Minute Swing** | Snapshot freeze, multi-epoch voting | Very High |
| **Proposal Spam** | Interest threshold, deposit requirement | High |

### 7.3 Transparency Requirements

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TRANSPARENCY REQUIREMENTS                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ON-CHAIN RECORDS                                                                      │
│   ────────────────                                                                      │
│   • Proposal full text and metadata                                                    │
│   • All votes with voter identity                                                      │
│   • Snapshot block and weights                                                         │
│   • Execution transactions                                                             │
│   • Foundation actions with justification                                              │
│                                                                                         │
│   PUBLIC DASHBOARDS                                                                     │
│   ─────────────────                                                                     │
│   • Real-time voting progress                                                          │
│   • Voter participation rates                                                          │
│   • Historical proposal outcomes                                                       │
│   • Foundation action log                                                              │
│                                                                                         │
│   AUDIT TRAIL                                                                           │
│   ───────────                                                                           │
│   • Cryptographic proof of vote integrity                                              │
│   • Merkle proofs for snapshot data                                                    │
│   • Execution verification receipts                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Post-Vote Execution Rules

### 8.1 Tier 1 & Tier 2: Automatic Network Execution

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         AUTOMATIC EXECUTION (TIER 1 & 2)                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   EXECUTION PIPELINE                                                                    │
│   ──────────────────                                                                    │
│                                                                                         │
│   Vote Passed                                                                           │
│        │                                                                                │
│        ▼                                                                                │
│   ┌──────────────────┐                                                                 │
│   │ DELAY PERIOD     │   Tier 1: 14 days                                               │
│   │                  │   Tier 2: 7 days                                                │
│   │ Purpose:         │                                                                 │
│   │ • Final review   │                                                                 │
│   │ • Rollback       │                                                                 │
│   │   opportunity    │                                                                 │
│   └────────┬─────────┘                                                                 │
│            │                                                                            │
│            ▼                                                                            │
│   ┌──────────────────┐                                                                 │
│   │ VALIDATOR        │   Validators signal upgrade readiness                           │
│   │ SIGNALING        │   Required: 80% of validators ready                             │
│   │                  │   Automatic delay if threshold not met                          │
│   └────────┬─────────┘                                                                 │
│            │                                                                            │
│            ▼                                                                            │
│   ┌──────────────────┐                                                                 │
│   │ SCHEDULED        │   Protocol schedules execution at target block                  │
│   │ EXECUTION        │   No human intervention required                                │
│   │                  │   Deterministic activation                                      │
│   └────────┬─────────┘                                                                 │
│            │                                                                            │
│            ▼                                                                            │
│   ┌──────────────────┐                                                                 │
│   │ NETWORK          │   All nodes apply new rules                                     │
│   │ ACTIVATION       │   Consensus enforces upgrade                                    │
│   │                  │   Proposal marked EXECUTED                                      │
│   └──────────────────┘                                                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Tier 3: Foundation Multi-Sig Execution

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MULTI-SIG EXECUTION (TIER 3)                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   EXECUTION PIPELINE                                                                    │
│   ──────────────────                                                                    │
│                                                                                         │
│   Vote Passed                                                                           │
│        │                                                                                │
│        ▼                                                                                │
│   ┌──────────────────┐                                                                 │
│   │ FOUNDATION       │   Foundation prepares execution transaction                     │
│   │ PREPARATION      │   • Verify vote passed correctly                                │
│   │                  │   • Prepare fund transfers / actions                            │
│   └────────┬─────────┘                                                                 │
│            │                                                                            │
│            ▼                                                                            │
│   ┌──────────────────┐                                                                 │
│   │ MULTI-SIG        │   Threshold: 4-of-7 Foundation signers                          │
│   │ APPROVAL         │   48-hour timelock before execution                             │
│   │                  │   All signatures recorded on-chain                              │
│   └────────┬─────────┘                                                                 │
│            │                                                                            │
│            ▼                                                                            │
│   ┌──────────────────┐                                                                 │
│   │ TIMELOCK         │   Public waiting period                                         │
│   │ 48 HOURS         │   Allows community review                                       │
│   │                  │   Cancel possible if fraud detected                             │
│   └────────┬─────────┘                                                                 │
│            │                                                                            │
│            ▼                                                                            │
│   ┌──────────────────┐                                                                 │
│   │ EXECUTION        │   Transaction executed automatically                            │
│   │                  │   Funds transferred / action completed                          │
│   │                  │   Proposal marked EXECUTED                                      │
│   └──────────────────┘                                                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.3 Execution Delays Summary

| Tier | Delay Purpose | Duration | Rollback Possible |
|------|---------------|----------|-------------------|
| **Tier 1** | Critical review, emergency response | 14 days | Yes (new vote) |
| **Tier 2** | Parameter validation, testing | 7 days | Yes (new vote) |
| **Tier 3** | Community verification | 48 hours | Yes (multi-sig cancel) |

### 8.4 Rollback Procedures

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ROLLBACK PROCEDURES                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DURING DELAY PERIOD                                                                   │
│   ───────────────────                                                                   │
│   • Emergency proposal can be submitted                                                │
│   • Expedited 48-hour vote for critical issues                                         │
│   • 80% supermajority required to cancel                                               │
│   • Original proposer can voluntarily withdraw                                         │
│                                                                                         │
│   AFTER EXECUTION (Tier 1 & 2)                                                         │
│   ────────────────────────────                                                          │
│   • Revert requires new proposal                                                       │
│   • Same tier requirements as original                                                 │
│   • Cannot alter historical state (only future rules)                                  │
│                                                                                         │
│   AFTER EXECUTION (Tier 3)                                                             │
│   ────────────────────────                                                              │
│   • Fund transfers are final                                                           │
│   • New proposal required for corrections                                              │
│   • Multi-sig cannot unilaterally reverse                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Future Governance Extensions

### 9.1 Treasury DAO

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TREASURY DAO [FUTURE]                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   OVERVIEW                                                                              │
│   ────────                                                                              │
│   Decentralized management of community grant funds, removing Foundation               │
│   as intermediary for fund allocation.                                                 │
│                                                                                         │
│   STRUCTURE                                                                             │
│   ─────────                                                                             │
│   • Community-elected council (7-11 members)                                           │
│   • Rotating terms (1 year)                                                            │
│   • Multi-sig treasury (5-of-9)                                                        │
│   • Public spending records                                                            │
│                                                                                         │
│   POWERS                                                                                │
│   ──────                                                                                │
│   • Approve grants up to 50,000 MBO without vote                                       │
│   • Propose larger allocations to governance                                           │
│   • Set grant program parameters                                                       │
│   • Review and audit grant recipients                                                  │
│                                                                                         │
│   TIMELINE                                                                              │
│   ────────                                                                              │
│   Target: Year 2-3 of mainnet                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Governance Fraud Slashing

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE FRAUD SLASHING [FUTURE]                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SLASHABLE OFFENSES                                                                    │
│   ──────────────────                                                                    │
│   • Vote manipulation (provable double-voting)                                         │
│   • Proposal fraud (false information)                                                 │
│   • Bribery acceptance (on-chain evidence)                                             │
│   • Coordinated manipulation (statistical detection)                                   │
│                                                                                         │
│   ENFORCEMENT                                                                           │
│   ───────────                                                                           │
│   • Fraud proofs submitted on-chain                                                    │
│   • Review period for accused party                                                    │
│   • Governance vote to confirm slash                                                   │
│   • Slashed stake redistributed to reporters                                           │
│                                                                                         │
│   PENALTIES                                                                             │
│   ─────────                                                                             │
│   • 10-50% stake slash depending on severity                                           │
│   • Governance participation ban (temporary)                                           │
│   • Reputation score reset for providers                                               │
│                                                                                         │
│   TIMELINE                                                                              │
│   ────────                                                                              │
│   Target: Year 3-4 of mainnet                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.3 Compute-Governance Hybrid Weighting

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE-GOVERNANCE HYBRID [RESEARCH]                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CONCEPT                                                                               │
│   ───────                                                                               │
│   Integrate PoUW compute contributions more deeply into governance weight,             │
│   similar to how consensus uses 50/50 PoS/PoUW split.                                  │
│                                                                                         │
│   RESEARCH QUESTIONS                                                                    │
│   ──────────────────                                                                    │
│   • How to prevent compute-washing for governance power?                               │
│   • Optimal balance between stake and compute weight?                                  │
│   • Time-weighting of compute contributions?                                           │
│   • Protection against rented compute attacks?                                         │
│                                                                                         │
│   POTENTIAL MODEL                                                                       │
│   ───────────────                                                                       │
│   Governance_Weight = (α × Stake) + (β × Compute_Reputation)                           │
│                                                                                         │
│   Where α + β = 1, initial research suggests α=0.7, β=0.3                              │
│                                                                                         │
│   TIMELINE                                                                              │
│   ────────                                                                              │
│   Research phase: Year 2-3                                                             │
│   Implementation: TBD based on research                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.4 ZK-Proof Based Identity and Voting Security

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ZK GOVERNANCE SECURITY [FUTURE]                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PRIVATE VOTING                                                                        │
│   ──────────────                                                                        │
│   • Votes encrypted until tally                                                        │
│   • ZK proof of eligibility without revealing identity                                 │
│   • Prevents vote-buying verification                                                  │
│   • Reduces social pressure on voters                                                  │
│                                                                                         │
│   SYBIL-RESISTANT IDENTITY                                                             │
│   ────────────────────────                                                              │
│   • ZK proof of unique human (optional)                                                │
│   • Privacy-preserving reputation scores                                               │
│   • Cross-chain identity verification                                                  │
│   • Quadratic voting with ZK eligibility                                               │
│                                                                                         │
│   TECHNICAL APPROACH                                                                    │
│   ──────────────────                                                                    │
│   • Commit-reveal scheme with ZK proofs                                                │
│   • SNARK-based vote aggregation                                                       │
│   • Homomorphic encryption for tallying                                                │
│   • Nullifier-based double-vote prevention                                             │
│                                                                                         │
│   TIMELINE                                                                              │
│   ────────                                                                              │
│   Research: Year 2-3                                                                   │
│   Pilot: Year 4+                                                                       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.5 Roadmap Summary

| Extension | Status | Target Timeline |
|-----------|--------|-----------------|
| **Treasury DAO** | Planned | Year 2-3 |
| **Governance Fraud Slashing** | Planned | Year 3-4 |
| **Compute-Governance Hybrid** | Research | Year 2-3 (research) |
| **ZK Voting Security** | Research | Year 4+ |
| **Cross-Chain Governance** | Exploration | Year 5+ |

---

## Appendix: Governance Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE QUICK REFERENCE                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TIER SUMMARY                                                                          │
│   ────────────                                                                          │
│   Tier 1 (Critical):    70% quorum, 66.7% approval, 5 epochs, 14-day delay             │
│   Tier 2 (Parameters):  55% quorum, >50% approval, 3 epochs, 7-day delay               │
│   Tier 3 (Community):   25% quorum, >50% approval, 2 epochs, 48-hour delay             │
│                                                                                         │
│   VOTING POWER                                                                          │
│   ────────────                                                                          │
│   Validators/Delegators: Stake-denominated (all tiers)                                 │
│   Compute Providers:     Reputation-based (all tiers)                                  │
│   Token Holders:         1:1 MBO (Tier 3 only)                                         │
│   Foundation:            Veto only (Tier 1 security)                                   │
│                                                                                         │
│   KEY PROTECTIONS                                                                       │
│   ───────────────                                                                       │
│   • Stake snapshot at proposal creation                                                │
│   • Multi-epoch voting windows                                                         │
│   • Execution delays for review                                                        │
│   • All votes on-chain                                                                 │
│   • No discretionary overrides                                                         │
│                                                                                         │
│   IMMUTABLE PROPERTIES                                                                  │
│   ────────────────────                                                                  │
│   • Total supply: 31,536,000 MBO                                                       │
│   • Halving schedule                                                                   │
│   • Historical ledger state                                                            │
│   • Execution determinism                                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [token_distribution.md](./token_distribution.md) | Allocation breakdown |
| [consensus_master_overview.md](./consensus_master_overview.md) | Consensus specification |
| [vesting_model.md](./vesting_model.md) | Token unlock schedules |

---

*This document defines the official governance model for Mbongo Chain. All governance operations are enforced by the protocol and recorded on-chain.*

