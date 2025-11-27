<!-- Verified against tokenomics.md -->
# Vesting & Unlock Model

> **Document Type:** Vesting Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Global Principles](#1-global-principles)
2. [Vesting Categories](#2-vesting-categories)
3. [Vesting Summary Table](#3-vesting-summary-table)
4. [Vesting Timeline](#4-vesting-timeline)
5. [Unlock Security Rules](#5-unlock-security-rules)
6. [Rationale](#6-rationale)
7. [For Participants](#7-for-participants)
8. [References](#8-references)

---

## 1. Global Principles

### 1.1 Supply Foundation

```
┌─────────────────────────────────────────────────────────────┐
│                 VESTING FOUNDATION PRINCIPLES               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   FIXED SUPPLY                                             │
│   ────────────                                              │
│   Total Supply:       31,536,000 MBO                       │
│   Inflation:          0% (permanently)                     │
│   Minting:            Impossible after genesis             │
│                                                             │
│   All vested tokens come from the fixed supply.            │
│   No new tokens can ever be created.                       │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Core Principles

| Principle | Description |
|-----------|-------------|
| **Transparency** | All vesting schedules are public and immutable |
| **Determinism** | Unlock timing is predictable and calculable |
| **Verifiability** | All unlocks are recorded on-chain and auditable |
| **Enforcement** | Smart contracts enforce schedules without exception |

### 1.3 Vesting Contract Guarantees

```
┌─────────────────────────────────────────────────────────────┐
│              VESTING CONTRACT GUARANTEES                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ✓ Schedules are immutable once deployed                  │
│   ✓ No admin keys can accelerate unlocks                   │
│   ✓ Cliff periods are strictly enforced                    │
│   ✓ Linear unlocks occur automatically                     │
│   ✓ Milestone approvals require multi-sig                  │
│   ✓ All state changes emit verifiable events               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Vesting Categories

### 2.1 Early Contributors

```
┌─────────────────────────────────────────────────────────────┐
│         EARLY CONTRIBUTORS (5% = 1,576,800 MBO)             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure:        4-year vesting with 1-year cliff       │
│   Cliff Duration:   12 months                              │
│   Vesting Duration: 48 months total                        │
│   Unlock Method:    Linear monthly (post-cliff)            │
│                                                             │
│   Schedule:                                                 │
│   ─────────                                                 │
│   Months 0-11:      0 MBO unlocked (cliff)                 │
│   Month 12:         394,200 MBO unlocked (25% at cliff)    │
│   Months 13-48:     32,850 MBO/month                       │
│   Month 48:         1,576,800 MBO total (100%)             │
│                                                             │
│   Beneficiaries:                                            │
│   • Founding team members                                  │
│   • Early advisors                                         │
│   • Pre-launch contributors                                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Foundation & Operations

```
┌─────────────────────────────────────────────────────────────┐
│         FOUNDATION & OPERATIONS (10% = 3,153,600 MBO)       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure:        4-year linear vesting                  │
│   Cliff Duration:   None                                   │
│   Vesting Duration: 48 months                              │
│   Unlock Method:    Linear monthly from day 1              │
│                                                             │
│   Schedule:                                                 │
│   ─────────                                                 │
│   Month 1:          65,700 MBO unlocked                    │
│   Month 12:         788,400 MBO cumulative (25%)           │
│   Month 24:         1,576,800 MBO cumulative (50%)         │
│   Month 36:         2,365,200 MBO cumulative (75%)         │
│   Month 48:         3,153,600 MBO cumulative (100%)        │
│                                                             │
│   Usage:                                                    │
│   • Protocol development                                   │
│   • Security audits                                        │
│   • Legal & compliance                                     │
│   • Operational expenses                                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.3 Ecosystem Grants

```
┌─────────────────────────────────────────────────────────────┐
│         ECOSYSTEM GRANTS (15% = 4,730,400 MBO)              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure:        Milestone-based unlocks                │
│   Cliff Duration:   Varies by grant                        │
│   Vesting Duration: Varies by grant                        │
│   Unlock Method:    Multi-sig approval per milestone       │
│                                                             │
│   Standard Grant Structure:                                 │
│   ─────────────────────────                                 │
│   Milestone 0:      10% on approval                        │
│   Milestone 1:      30% on first deliverable               │
│   Milestone 2:      30% on second deliverable              │
│   Milestone 3:      30% on completion                      │
│                                                             │
│   Review Process:                                           │
│   1. Grant application submitted                           │
│   2. Committee review (2-4 weeks)                          │
│   3. Milestones defined and agreed                         │
│   4. Initial payment on approval                           │
│   5. Milestone verification by reviewers                   │
│   6. Multi-sig release upon approval                       │
│                                                             │
│   Grant Categories:                                         │
│   • Infrastructure (tooling, SDKs)                         │
│   • Applications (DeFi, utilities)                         │
│   • Research (academic, security)                          │
│   • Education (tutorials, documentation)                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.4 Community Incentives

```
┌─────────────────────────────────────────────────────────────┐
│         COMMUNITY INCENTIVES (10% = 3,153,600 MBO)          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure:        Epoch streaming                        │
│   Cliff Duration:   None                                   │
│   Vesting Duration: Continuous (5-10 years estimated)      │
│   Unlock Method:    Per-epoch distribution                 │
│                                                             │
│   Distribution Mechanism:                                   │
│   ─────────────────────────                                 │
│   • Fixed amount allocated per epoch                       │
│   • Rate adjustable via governance                         │
│   • Unused allocation rolls to next epoch                  │
│   • No individual cliff or vesting                         │
│                                                             │
│   Epoch Duration:   ~1,000 blocks (~33 minutes)            │
│   Estimated Rate:   ~500-1,000 MBO per epoch (initial)     │
│                                                             │
│   Programs:                                                 │
│   • Liquidity mining                                       │
│   • Staking bonuses                                        │
│   • Bug bounties                                           │
│   • Ambassador rewards                                     │
│   • Governance participation                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.5 Developers (from Grants)

```
┌─────────────────────────────────────────────────────────────┐
│         DEVELOPER UNLOCKS (subset of Grants)                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure:        Performance-based with multi-sig       │
│   Cliff Duration:   None (milestone-triggered)             │
│   Vesting Duration: Per-grant agreement                    │
│   Unlock Method:    Multi-sig approval after review        │
│                                                             │
│   Approval Process:                                         │
│   ──────────────────                                        │
│   1. Developer submits deliverable                         │
│   2. Technical review (1-2 weeks)                          │
│   3. Quality assessment                                    │
│   4. Multi-sig signers approve (3-of-5)                    │
│   5. Funds released to developer                           │
│                                                             │
│   Multi-Sig Requirements:                                   │
│   • 3-of-5 signatures required                             │
│   • Signers are independent reviewers                      │
│   • 48-hour timelock on execution                          │
│   • All approvals logged on-chain                          │
│                                                             │
│   Performance Criteria:                                     │
│   • Code quality and testing                               │
│   • Documentation completeness                             │
│   • Security audit (if applicable)                         │
│   • Community feedback                                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.6 PoS & PoUW Rewards

```
┌─────────────────────────────────────────────────────────────┐
│         PoS & PoUW REWARDS (60% = 18,921,600 MBO)           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure:        Immediate block-level unlock           │
│   Cliff Duration:   None                                   │
│   Vesting Duration: None                                   │
│   Unlock Method:    Per-block emission (non-vested)        │
│                                                             │
│   Distribution:                                             │
│   ─────────────                                             │
│   PoS Validators:   40% of supply (12,614,400 MBO)         │
│   PoUW Providers:   20% of supply (6,307,200 MBO)          │
│                                                             │
│   Unlock Timing:                                            │
│   • Rewards unlock at block finalization                   │
│   • No vesting or lockup period                            │
│   • Immediately transferable                               │
│   • Subject to halving every 5 years                       │
│                                                             │
│   Per-Block (Year 1):                                       │
│   • Total: ~0.1 MBO                                        │
│   • PoS: ~0.05 MBO (50% of block reward)                   │
│   • PoUW: ~0.05 MBO (50% of block reward)                  │
│                                                             │
│   Rationale for Non-Vesting:                                │
│   • Validators need liquidity for operations               │
│   • GPU providers have hardware costs                      │
│   • Rewards compensate for active participation            │
│   • Staked tokens already provide lockup                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. Vesting Summary Table

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                    VESTING SUMMARY TABLE                                                │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                         │
│   CATEGORY               │  %   │    TOTAL MBO    │  CLIFF   │  VESTING  │  UNLOCK METHOD              │
│   ───────────────────────┼──────┼─────────────────┼──────────┼───────────┼─────────────────────────────│
│   Early Contributors     │   5% │     1,576,800   │ 12 mo    │ 48 mo     │ Linear monthly (post-cliff) │
│   Foundation & Ops       │  10% │     3,153,600   │ None     │ 48 mo     │ Linear monthly              │
│   Ecosystem Grants       │  15% │     4,730,400   │ Varies   │ Varies    │ Milestone + multi-sig       │
│   Community Incentives   │  10% │     3,153,600   │ None     │ Streaming │ Per-epoch distribution      │
│   PoS Validators         │  40% │    12,614,400   │ None     │ None      │ Per-block (immediate)       │
│   PoUW Providers         │  20% │     6,307,200   │ None     │ None      │ Per-block (immediate)       │
│   ───────────────────────┼──────┼─────────────────┼──────────┼───────────┼─────────────────────────────│
│   TOTAL                  │ 100% │    31,536,000   │          │           │                             │
│                                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Vesting Timeline

### 4.1 48-Month Vesting Schedule

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                               VESTING TIMELINE (0 → 48 MONTHS)                                          │
├─────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                                         │
│   Month    0    6    12   18   24   30   36   42   48                                                  │
│            │    │    │    │    │    │    │    │    │                                                   │
│            ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼                                                   │
│                                                                                                         │
│   EARLY    ░░░░░░░░░░░▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓█                                        │
│   CONTRIB  │←─ CLIFF ─→│←──────── LINEAR UNLOCK ──────────→│100%                                       │
│            │    0%     │25%      50%      75%              │                                           │
│                                                                                                         │
│   FOUND-   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓█                                           │
│   ATION    │←───────────── LINEAR UNLOCK ─────────────────→│100%                                       │
│            │12.5%    25%      50%      75%                 │                                           │
│                                                                                                         │
│   GRANTS   ◆───────◆───────────◆───────────◆───────────────◆                                           │
│            │M0     │M1         │M2         │M3             │                                           │
│            │10%    │40%        │70%        │100%           │ (milestone markers)                       │
│                                                                                                         │
│   COMMUN-  ════════════════════════════════════════════════════════▶                                   │
│   ITY      │←─────────── CONTINUOUS STREAMING ───────────────────→│                                    │
│            │ (rate adjustable via governance)                     │                                    │
│                                                                                                         │
│   PoS/PoUW ████████████████████████████████████████████████████████                                    │
│   REWARDS  │←─────────── IMMEDIATE PER-BLOCK ────────────────────→│                                    │
│            │ (no vesting, subject to halving)                     │                                    │
│                                                                                                         │
│   Legend:                                                                                               │
│   ░░░  Cliff period (locked)                                                                           │
│   ▓▓▓  Linear vesting (unlocking)                                                                      │
│   ███  Fully unlocked / immediate                                                                      │
│   ◆    Milestone unlock point                                                                          │
│   ═══  Streaming distribution                                                                          │
│                                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Monthly Unlock Amounts

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                          MONTHLY UNLOCK SCHEDULE                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MONTH │ EARLY CONTRIB │ FOUNDATION  │  GRANTS*  │ COMMUNITY** │ TOTAL VESTED         │
│   ──────┼───────────────┼─────────────┼───────────┼─────────────┼──────────────────────│
│      1  │         0     │    65,700   │  Variable │   ~263,000  │      ~329,000        │
│      6  │         0     │   394,200   │  Variable │ ~1,578,000  │    ~1,972,000        │
│     12  │   394,200     │   788,400   │  Variable │ ~3,153,000  │    ~4,336,000        │
│     18  │   591,300     │ 1,182,600   │  Variable │ ~4,730,000  │    ~6,504,000        │
│     24  │   788,400     │ 1,576,800   │  Variable │ ~6,307,000  │    ~8,672,000        │
│     30  │   985,500     │ 1,971,000   │  Variable │ ~7,884,000  │   ~10,840,000        │
│     36  │ 1,182,600     │ 2,365,200   │  Variable │ ~9,461,000  │   ~13,009,000        │
│     42  │ 1,379,700     │ 2,759,400   │  Variable │ ~11,038,000 │   ~15,177,000        │
│     48  │ 1,576,800     │ 3,153,600   │  Variable │ ~12,614,000 │   ~17,344,000        │
│                                                                                         │
│   * Grants depend on milestone completion timing                                       │
│   ** Community includes PoS/PoUW block rewards (estimated)                             │
│                                                                                         │
│   Note: PoS and PoUW rewards are immediate, shown in Community column for total        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Cumulative Release by Year

```
┌─────────────────────────────────────────────────────────────┐
│              CUMULATIVE VESTED RELEASE BY YEAR              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   YEAR │ EARLY   │ FOUND.  │ GRANTS │ COMMUN. │ REWARDS*   │
│   ─────┼─────────┼─────────┼────────┼─────────┼────────────│
│     1  │    0%   │   25%   │  ~20%  │  ~20%   │   ~25%     │
│     2  │   25%   │   50%   │  ~50%  │  ~40%   │   ~50%     │
│     3  │   50%   │   75%   │  ~75%  │  ~55%   │   ~62%     │
│     4  │   75%   │  100%   │  ~90%  │  ~65%   │   ~72%     │
│     5  │  100%   │  100%   │ ~100%  │  ~75%   │   ~80%     │
│                                                             │
│   * Block rewards subject to halving at Year 5             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Unlock Security Rules

### 5.1 Enforcement Mechanisms

```
┌─────────────────────────────────────────────────────────────┐
│              UNLOCK SECURITY ENFORCEMENT                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   RULE 1: NO MANUAL OVERRIDES                              │
│   ───────────────────────────                               │
│   • Vesting contracts have no admin override function      │
│   • No "emergency unlock" capability                       │
│   • Schedule parameters immutable after deployment         │
│                                                             │
│   RULE 2: NO DISCRETIONARY UNLOCKS                         │
│   ────────────────────────────────                          │
│   • All unlocks follow pre-defined rules                   │
│   • No individual can authorize early release              │
│   • Time-based unlocks are automatic                       │
│                                                             │
│   RULE 3: MULTI-SIG FOR GRANTS                             │
│   ────────────────────────────                              │
│   • 3-of-5 signature requirement                           │
│   • 48-hour timelock on execution                          │
│   • Signers publicly identified                            │
│   • Rotation process defined                               │
│                                                             │
│   RULE 4: AUTOMATED SCHEDULE ENFORCEMENT                   │
│   ──────────────────────────────────────                    │
│   • Smart contracts execute unlocks automatically          │
│   • Block timestamp determines eligibility                 │
│   • No human intervention required                         │
│                                                             │
│   RULE 5: GOVERNANCE UPGRADE PATH                          │
│   ───────────────────────────────                           │
│   • Contract upgrades require governance vote              │
│   • 7-day voting period minimum                            │
│   • 66% supermajority required                             │
│   • Changes cannot accelerate existing vests               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 Security Guarantees

| Guarantee | Implementation |
|-----------|----------------|
| **Immutability** | Vesting parameters set at contract deployment |
| **Transparency** | All schedules readable on-chain |
| **Auditability** | Events emitted for every unlock |
| **Decentralization** | Multi-sig prevents single-point control |
| **Timelock** | Delays prevent rushed decisions |

### 5.3 Multi-Sig Configuration

```
┌─────────────────────────────────────────────────────────────┐
│              MULTI-SIG CONFIGURATION                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Grant Release Multi-Sig:                                  │
│   ────────────────────────                                  │
│   Threshold:     3-of-5 signers                            │
│   Timelock:      48 hours                                  │
│   Signers:       Independent reviewers                     │
│                                                             │
│   Foundation Multi-Sig:                                     │
│   ─────────────────────                                     │
│   Threshold:     4-of-7 signers                            │
│   Timelock:      72 hours                                  │
│   Signers:       Foundation council members                │
│                                                             │
│   Emergency Multi-Sig (pause only):                        │
│   ───────────────────────────────                           │
│   Threshold:     2-of-3 signers                            │
│   Capability:    Pause distributions only                  │
│   Resume:        Requires full governance vote             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Rationale

### 6.1 Why Contributors Have Cliffs

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: CONTRIBUTOR CLIFF PERIOD                 │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Purpose of 1-Year Cliff:                                  │
│   ────────────────────────                                  │
│                                                             │
│   1. LONG-TERM ALIGNMENT                                   │
│      Contributors must remain committed for 12 months      │
│      before receiving any tokens.                          │
│                                                             │
│   2. PREVENT EARLY DUMPING                                 │
│      No tokens available during critical early growth      │
│      phase when liquidity is low.                          │
│                                                             │
│   3. PROVE VALUE                                           │
│      Contributors demonstrate value over time before       │
│      compensation is released.                             │
│                                                             │
│   4. MARKET STABILITY                                      │
│      Reduces supply pressure during network launch         │
│      and price discovery period.                           │
│                                                             │
│   5. INDUSTRY STANDARD                                     │
│      Aligns with established practices in tech and         │
│      crypto for contributor compensation.                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Why Grants Are Milestone-Based

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: MILESTONE-BASED GRANTS                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Benefits of Milestone Structure:                          │
│   ────────────────────────────────                          │
│                                                             │
│   1. ACCOUNTABILITY                                        │
│      Funds released only after verified deliverables.      │
│      No payment for incomplete or abandoned projects.      │
│                                                             │
│   2. REDUCED RISK                                          │
│      Protocol limits exposure by funding incrementally.    │
│      Can discontinue grants that underperform.             │
│                                                             │
│   3. QUALITY ASSURANCE                                     │
│      Review process ensures deliverables meet standards.   │
│      Multi-sig prevents unilateral approvals.              │
│                                                             │
│   4. ALIGNED INCENTIVES                                    │
│      Grantees motivated to complete each milestone.        │
│      Ongoing relationship rather than one-time payment.    │
│                                                             │
│   5. TRANSPARENT PROGRESS                                  │
│      Community can track grant progress via milestones.    │
│      Public record of ecosystem development.               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.3 Why Rewards Are Non-Vested

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: IMMEDIATE REWARD UNLOCKS                 │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Why PoS and PoUW Rewards Have No Vesting:                │
│   ──────────────────────────────────────────                │
│                                                             │
│   1. OPERATIONAL COSTS                                     │
│      Validators and GPU providers have ongoing expenses.   │
│      Hardware, electricity, bandwidth require payment.     │
│                                                             │
│   2. ACTIVE PARTICIPATION                                  │
│      Rewards compensate for current work, not past.        │
│      Participants are continuously securing the network.   │
│                                                             │
│   3. STAKING IS LOCKUP                                     │
│      Staked tokens already have unbonding period.          │
│      Additional vesting would double-penalize stakers.     │
│                                                             │
│   4. MARKET LIQUIDITY                                      │
│      Some reward circulation is healthy for markets.       │
│      Enables price discovery and trading activity.         │
│                                                             │
│   5. COMPETITIVE POSITIONING                               │
│      Other networks offer immediate rewards.               │
│      Vesting would reduce validator attractiveness.        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.4 How Vesting Supports Sustainability

```
┌─────────────────────────────────────────────────────────────┐
│         VESTING & LONG-TERM SUSTAINABILITY                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   DECENTRALIZATION                                         │
│   ────────────────                                          │
│   • Gradual release prevents supply concentration          │
│   • Multiple unlock schedules diversify sell pressure      │
│   • No single entity can dump large amounts                │
│                                                             │
│   PRICE STABILITY                                          │
│   ───────────────                                           │
│   • Predictable unlocks allow market preparation           │
│   • Linear vesting smooths supply increases                │
│   • Cliffs prevent early liquidity shocks                  │
│                                                             │
│   LONG-TERM COMMITMENT                                     │
│   ────────────────────                                      │
│   • Multi-year vesting aligns incentives                   │
│   • Contributors invested in network success               │
│   • Foundation has runway for sustained development        │
│                                                             │
│   TRUST & TRANSPARENCY                                     │
│   ────────────────────                                      │
│   • Verifiable schedules build community trust             │
│   • No surprises or hidden unlocks                         │
│   • Clear expectations for all participants                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. For Participants

### 7.1 Contributors

**When You Unlock MBO:**
- **Months 0-11:** No tokens available (cliff period)
- **Month 12:** 25% (394,200 MBO) unlocks at cliff end
- **Months 13-48:** ~32,850 MBO unlocks monthly
- **Month 48:** 100% fully vested

**How to Claim:**
1. Connect wallet to vesting portal
2. View available balance
3. Claim unlocked tokens
4. Tokens transfer to wallet immediately

**Important Notes:**
- Unclaimed tokens remain in vesting contract (secure)
- No deadline to claim (claim anytime after unlock)
- Gas fees apply to claim transactions

---

### 7.2 Developers (Grant Recipients)

**How Unlocks Are Approved:**
1. Submit deliverable with documentation
2. Assigned reviewers assess quality
3. Review period: 1-2 weeks
4. If approved: Multi-sig signs release
5. 48-hour timelock executes
6. Funds transfer to developer wallet

**Milestone Requirements:**
- Clear deliverable description
- Acceptance criteria defined upfront
- Technical review for code submissions
- Documentation for user-facing features

**Dispute Process:**
- Appeal rejected milestones within 14 days
- Independent arbitration available
- Final decision by grants committee

---

### 7.3 Grant Recipients

**Milestone Validation Process:**

```
┌─────────────────────────────────────────────────────────────┐
│              MILESTONE VALIDATION FLOW                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   1. SUBMISSION                                            │
│      └─▶ Upload deliverables to grants portal              │
│                                                             │
│   2. REVIEW ASSIGNMENT                                     │
│      └─▶ 2 reviewers assigned within 48 hours              │
│                                                             │
│   3. TECHNICAL REVIEW                                      │
│      └─▶ Reviewers assess against criteria (1-2 weeks)     │
│                                                             │
│   4. APPROVAL/REJECTION                                    │
│      └─▶ Both reviewers must approve                       │
│      └─▶ Rejection includes feedback                       │
│                                                             │
│   5. MULTI-SIG RELEASE                                     │
│      └─▶ 3-of-5 signers approve transaction                │
│                                                             │
│   6. TIMELOCK EXECUTION                                    │
│      └─▶ 48-hour delay before funds release                │
│                                                             │
│   7. FUNDS TRANSFER                                        │
│      └─▶ MBO sent to grant recipient wallet                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

### 7.4 Community Participants

**How Incentives Flow Per Epoch:**

- **Epoch Duration:** ~1,000 blocks (~33 minutes)
- **Distribution:** Automatic at epoch boundary
- **Eligibility:** Based on program participation
- **Claiming:** Varies by program (some auto-distribute)

**Active Programs:**
| Program | Distribution | Requirements |
|---------|--------------|--------------|
| Liquidity Mining | Per-epoch | Provide liquidity |
| Staking Bonus | Per-epoch | Stake MBO |
| Bug Bounty | On approval | Report valid bugs |
| Ambassador | Monthly | Complete tasks |

**Tracking Rewards:**
- View earned rewards in dashboard
- Historical rewards on-chain
- Projected rewards based on current participation

---

### 7.5 Validators & GPU Providers

**Instant Emission Explanation:**

```
┌─────────────────────────────────────────────────────────────┐
│              INSTANT REWARD MECHANISM                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   VALIDATORS (PoS):                                        │
│   ─────────────────                                         │
│   • Rewards calculated per finalized block                 │
│   • 50% of block reward distributed to PoS                 │
│   • Proposer receives larger share                         │
│   • Attesters share remainder by stake weight              │
│   • Rewards immediately transferable                       │
│                                                             │
│   GPU PROVIDERS (PoUW):                                    │
│   ─────────────────────                                     │
│   • Rewards per verified compute receipt                   │
│   • 50% of block reward distributed to PoUW                │
│   • Share proportional to work units                       │
│   • Rewards immediately transferable                       │
│                                                             │
│   NO VESTING BECAUSE:                                      │
│   ────────────────────                                      │
│   • Compensates active, ongoing participation              │
│   • Covers operational costs (hardware, electricity)       │
│   • Staked tokens already provide economic commitment      │
│   • Industry standard for network rewards                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. References

### Related Documentation

| Document | Description |
|----------|-------------|
| [token_intro.md](./token_intro.md) | MBO token introduction and overview |
| [token_distribution.md](./token_distribution.md) | Allocation breakdown and rationale |
| [monetary_policy.md](./monetary_policy.md) | Fixed supply and halving schedule |
| [reward_mechanics.md](./reward_mechanics.md) | Reward calculation details |
| [governance.md](./governance.md) | Governance processes |

### Contract Addresses

| Contract | Purpose | Address |
|----------|---------|---------|
| Contributor Vesting | Early contributor unlocks | `TBD at launch` |
| Foundation Vesting | Foundation allocation | `TBD at launch` |
| Grant Treasury | Ecosystem grants | `TBD at launch` |
| Community Treasury | Incentive programs | `TBD at launch` |

### Audit Reports

- Vesting Contract Audit: `[Link TBD]`
- Multi-Sig Audit: `[Link TBD]`
- Economic Model Review: `[Link TBD]`

---

*This document defines the official vesting and unlock model for MBO. All schedules are enforced by smart contracts and verifiable on-chain.*

