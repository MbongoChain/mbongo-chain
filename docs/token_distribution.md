<!-- Verified against tokenomics.md -->
# Token Distribution & Allocation Model

> **Document Type:** Distribution Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Total Supply Reminder](#1-total-supply-reminder)
2. [Master Allocation Table](#2-master-allocation-table)
3. [Vesting Schedules](#3-vesting-schedules)
4. [Supply Release Chart](#4-supply-release-chart)
5. [Allocation Rationale](#5-allocation-rationale)
6. [Security Implications](#6-security-implications)
7. [For Participants](#7-for-participants)

---

## 1. Total Supply Reminder

```
┌─────────────────────────────────────────────────────────────┐
│                    MBO SUPPLY PARAMETERS                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Maximum Supply:         31,536,000 MBO                   │
│   Inflation Rate:         0% (permanently)                 │
│   Supply Cap:             Immutable (protocol-enforced)    │
│   Halving Schedule:       Every 5 years (block rewards)    │
│                                                             │
│   All allocations sum to exactly 31,536,000 MBO.           │
│   No additional tokens can ever be created.                │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

> **Reference:** See `monetary_policy.md` for halving mechanics and emission schedule.

---

## 2. Master Allocation Table

### 2.1 Allocation Summary

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           MBO ALLOCATION BREAKDOWN                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Category                        │   %    │      MBO Amount │ Vesting                 │
│   ────────────────────────────────┼────────┼─────────────────┼─────────────────────────│
│   PoS Validators & Delegators     │  40%   │    12,614,400   │ Per-block unlock        │
│   PoUW Compute Providers          │  20%   │     6,307,200   │ Per-block unlock        │
│   Ecosystem Grants & Developers   │  15%   │     4,730,400   │ Milestone-based         │
│   Foundation & Operations         │  10%   │     3,153,600   │ 4-year vesting          │
│   Community & Incentives          │  10%   │     3,153,600   │ Epoch streaming         │
│   Early Contributors              │   5%   │     1,576,800   │ 4-year, 1-year cliff    │
│   ────────────────────────────────┼────────┼─────────────────┼─────────────────────────│
│   TOTAL                           │ 100%   │    31,536,000   │                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Detailed Allocation Table

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           DETAILED ALLOCATION                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CATEGORY                │    %   │       MBO      │ PURPOSE                          │
│   ────────────────────────┼────────┼────────────────┼──────────────────────────────────│
│                                                                                         │
│   PoS VALIDATORS &        │  40%   │  12,614,400    │ Staking rewards distributed      │
│   DELEGATORS              │        │                │ per block to secure the network  │
│                           │        │                │ and validate transactions.       │
│                                                                                         │
│   PoUW COMPUTE            │  20%   │   6,307,200    │ GPU work rewards for verified    │
│   PROVIDERS               │        │                │ compute tasks, incentivizing     │
│                           │        │                │ useful work contribution.        │
│                                                                                         │
│   ECOSYSTEM GRANTS &      │  15%   │   4,730,400    │ Funding for developers building  │
│   DEVELOPERS              │        │                │ applications, tools, and         │
│                           │        │                │ infrastructure on Mbongo Chain.  │
│                                                                                         │
│   FOUNDATION &            │  10%   │   3,153,600    │ Long-term operational runway     │
│   OPERATIONS              │        │                │ for protocol development,        │
│                           │        │                │ research, and maintenance.       │
│                                                                                         │
│   COMMUNITY &             │  10%   │   3,153,600    │ Incentive programs, airdrops,    │
│   INCENTIVES              │        │                │ bounties, and community          │
│                           │        │                │ engagement initiatives.          │
│                                                                                         │
│   EARLY CONTRIBUTORS      │   5%   │   1,576,800    │ Compensation for founding team   │
│                           │        │                │ and early supporters with        │
│                           │        │                │ long-term vesting alignment.     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Vesting Schedules

### 3.1 Vesting Overview

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           VESTING SCHEDULES                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CATEGORY                │ VESTING TYPE      │ CLIFF   │ DURATION │ UNLOCK RATE       │
│   ────────────────────────┼───────────────────┼─────────┼──────────┼───────────────────│
│   PoS Rewards             │ Per-block         │ None    │ Ongoing  │ Each block        │
│   PoUW Rewards            │ Per-block         │ None    │ Ongoing  │ Each block        │
│   Ecosystem Grants        │ Milestone-based   │ Varies  │ Varies   │ Per milestone     │
│   Foundation              │ Linear            │ None    │ 4 years  │ Monthly           │
│   Community Incentives    │ Epoch streaming   │ None    │ Ongoing  │ Per epoch         │
│   Early Contributors      │ Linear + Cliff    │ 1 year  │ 4 years  │ Monthly (post-cliff)│
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Early Contributors Vesting

```
┌─────────────────────────────────────────────────────────────┐
│         EARLY CONTRIBUTORS VESTING (5% = 1,576,800 MBO)     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure: 4-year linear vesting with 1-year cliff       │
│                                                             │
│   Year 0:     0 MBO unlocked (cliff period)                │
│   Year 1:     394,200 MBO unlocked (25% at cliff)          │
│   Year 2:     788,400 MBO cumulative (50%)                 │
│   Year 3:   1,182,600 MBO cumulative (75%)                 │
│   Year 4:   1,576,800 MBO cumulative (100%)                │
│                                                             │
│   Monthly unlock (post-cliff): ~32,850 MBO/month           │
│                                                             │
│   Cliff Rationale:                                          │
│   • Ensures long-term commitment                           │
│   • Aligns contributor incentives with network success     │
│   • Prevents early dumping                                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.3 Foundation Vesting

```
┌─────────────────────────────────────────────────────────────┐
│         FOUNDATION VESTING (10% = 3,153,600 MBO)            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure: 4-year linear vesting, no cliff               │
│                                                             │
│   Year 1:     788,400 MBO unlocked (25%)                   │
│   Year 2:   1,576,800 MBO cumulative (50%)                 │
│   Year 3:   2,365,200 MBO cumulative (75%)                 │
│   Year 4:   3,153,600 MBO cumulative (100%)                │
│                                                             │
│   Monthly unlock: ~65,700 MBO/month                        │
│                                                             │
│   Purpose:                                                  │
│   • Ongoing protocol development                           │
│   • Security audits and research                           │
│   • Operational expenses                                   │
│   • Emergency reserves                                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.4 Community Incentives Streaming

```
┌─────────────────────────────────────────────────────────────┐
│       COMMUNITY INCENTIVES (10% = 3,153,600 MBO)            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure: Epoch-based streaming                         │
│                                                             │
│   • Distributed continuously per epoch                     │
│   • Rate determined by governance                          │
│   • Adjustable based on program needs                      │
│                                                             │
│   Distribution Channels:                                    │
│   • Liquidity incentives                                   │
│   • Participation rewards                                  │
│   • Bug bounties                                           │
│   • Community competitions                                 │
│   • Ambassador programs                                    │
│                                                             │
│   Estimated Duration: 5-10 years                           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.5 Ecosystem Grants

```
┌─────────────────────────────────────────────────────────────┐
│         ECOSYSTEM GRANTS (15% = 4,730,400 MBO)              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure: Milestone-based unlock                        │
│                                                             │
│   Grant Process:                                            │
│   1. Application submitted                                 │
│   2. Review and approval                                   │
│   3. Milestones defined                                    │
│   4. Funds released per milestone completion               │
│                                                             │
│   Typical Grant Structure:                                  │
│   • 10% on approval                                        │
│   • 30% on first milestone                                 │
│   • 30% on second milestone                                │
│   • 30% on completion                                      │
│                                                             │
│   Categories:                                               │
│   • Infrastructure & tooling                               │
│   • DeFi applications                                      │
│   • Developer SDKs                                         │
│   • Research grants                                        │
│   • Educational content                                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.6 Block Rewards (PoS & PoUW)

```
┌─────────────────────────────────────────────────────────────┐
│       BLOCK REWARDS (60% = 18,921,600 MBO combined)         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Structure: Per-block unlock (no vesting)                 │
│                                                             │
│   PoS Validators (40%):   12,614,400 MBO                   │
│   PoUW Providers (20%):    6,307,200 MBO                   │
│                                                             │
│   Distribution:                                             │
│   • Unlocked immediately upon block finalization           │
│   • No lockup or vesting period                            │
│   • Subject to halving every 5 years                       │
│                                                             │
│   Emission Rate (Year 1):                                   │
│   • ~31,536,000 blocks per year                            │
│   • ~0.1 MBO per block (total)                             │
│   • ~0.05 MBO to PoS per block (50%)                       │
│   • ~0.05 MBO to PoUW per block (50%)                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Supply Release Chart

### 4.1 10-Year Release Schedule

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           SUPPLY RELEASE CHART (Years 0-10)                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   % Released                                                                            │
│   100% ┤                                                    ════════════════ Total     │
│        │                                              ══════                            │
│    90% ┤                                        ══════                                  │
│        │                                  ══════        ─────────────────── Block      │
│    80% ┤                            ══════                                  Rewards    │
│        │                      ══════            ─────────                   (PoS+PoUW) │
│    70% ┤                ══════            ─────                                        │
│        │          ══════            ─────         ················· Foundation        │
│    60% ┤    ══════            ─────                                                    │
│        │════            ─────                ················                          │
│    50% ┤          ─────                ················                                │
│        │    ─────                ················         ▪▪▪▪▪▪▪▪▪▪▪▪▪ Early         │
│    40% ┤─────                ················                           Contributors  │
│        │              ················                          ▪▪▪▪▪▪▪               │
│    30% ┤        ················                          ▪▪▪▪▪▪                       │
│        │  ················                          ▪▪▪▪▪▪      ○○○○○○○○○ Community   │
│    20% ┤················                      ▪▪▪▪▪▪      ○○○○○○                       │
│        │                                ▪▪▪▪▪▪      ○○○○○○        ++++++++ Grants     │
│    10% ┤                          ▪▪▪▪▪▪      ○○○○○○        ++++++                     │
│        │                    ▪▪▪▪▪▪      ○○○○○○        ++++++                           │
│     0% ┼──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┤        │
│        0      1      2      3      4      5      6      7      8      9     10        │
│                                     Years                                              │
│                                                                                         │
│   Legend:                                                                               │
│   ════  Total Supply Released                                                          │
│   ────  Block Rewards (PoS + PoUW)                                                     │
│   ····  Foundation (4-year vest)                                                       │
│   ▪▪▪▪  Early Contributors (4-year vest, 1-year cliff)                                │
│   ○○○○  Community Incentives (streaming)                                               │
│   ++++  Ecosystem Grants (milestone-based)                                             │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Release Milestones

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                           RELEASE MILESTONES                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   YEAR │ BLOCK REWARDS │ FOUNDATION │ CONTRIBUTORS │ COMMUNITY │ GRANTS │ TOTAL %     │
│   ─────┼───────────────┼────────────┼──────────────┼───────────┼────────┼─────────────│
│     0  │      0%       │     0%     │      0%      │     0%    │   0%   │     0%      │
│     1  │     25%       │    25%     │      0%      │    20%    │  15%   │    ~22%     │
│     2  │     50%       │    50%     │     25%      │    40%    │  30%   │    ~43%     │
│     3  │     62%       │    75%     │     50%      │    55%    │  45%   │    ~58%     │
│     4  │     72%       │   100%     │     75%      │    65%    │  60%   │    ~72%     │
│     5  │     80%       │   100%     │    100%      │    75%    │  75%   │    ~82%     │
│     6  │     85%       │   100%     │    100%      │    82%    │  85%   │    ~87%     │
│     7  │     89%       │   100%     │    100%      │    88%    │  90%   │    ~91%     │
│     8  │     92%       │   100%     │    100%      │    92%    │  95%   │    ~94%     │
│     9  │     95%       │   100%     │    100%      │    96%    │  98%   │    ~96%     │
│    10  │     97%       │   100%     │    100%      │   100%    │ 100%   │    ~98%     │
│                                                                                         │
│   Note: Percentages are approximate due to halving and variable grant timing.          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Allocation Rationale

### 5.1 Why Validators Receive 40%

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: PoS VALIDATORS (40%)                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Primary Network Security                                  │
│   ────────────────────────                                  │
│   Validators are the backbone of consensus. A substantial  │
│   allocation ensures:                                       │
│                                                             │
│   • Strong economic incentive to stake and secure network  │
│   • Competitive yields attract capital commitment          │
│   • Decentralization through distributed validator set     │
│   • Long-term alignment with network success               │
│                                                             │
│   Comparative Analysis:                                     │
│   • Ethereum: ~4-5% APY for stakers                        │
│   • Cosmos: ~15-20% APY for stakers                        │
│   • Mbongo: Designed for competitive, sustainable yields   │
│                                                             │
│   The 40% allocation provides runway for decades of        │
│   validator rewards while maintaining scarcity.            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 Why GPU Compute Receives 20%

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: PoUW COMPUTE PROVIDERS (20%)             │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Compute-First Blockchain                                  │
│   ────────────────────────                                  │
│   Mbongo Chain's core innovation is useful computation.    │
│   The 20% allocation ensures:                               │
│                                                             │
│   • GPU providers are economically incentivized            │
│   • Compute marketplace has sustainable reward pool        │
│   • Hardware investments are recoverable                   │
│   • Network utility grows through real computation         │
│                                                             │
│   Why 20% (not higher):                                     │
│   • PoUW supplements PoS, not replaces it                  │
│   • Security primarily from stake, utility from compute    │
│   • 50/50 split balances security vs utility incentives    │
│                                                             │
│   This allocation positions Mbongo as a leader in the      │
│   emerging compute-enabled blockchain space.               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.3 Why Grants Exist

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: ECOSYSTEM GRANTS (15%)                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Ecosystem Growth                                          │
│   ────────────────                                          │
│   Networks succeed through vibrant ecosystems. Grants      │
│   enable:                                                   │
│                                                             │
│   • Developer onboarding and tooling                       │
│   • Application diversity and innovation                   │
│   • Infrastructure development                             │
│   • Research and academic partnerships                     │
│                                                             │
│   Milestone-Based Structure:                                │
│   • Prevents misuse of funds                               │
│   • Ensures deliverables before payment                    │
│   • Aligns grant recipients with ecosystem success         │
│                                                             │
│   15% provides substantial runway for multi-year           │
│   ecosystem development without compromising other         │
│   allocations.                                              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.4 Why Foundation Gets Long-Term Runway

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: FOUNDATION (10%)                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Protocol Stewardship                                      │
│   ────────────────────                                      │
│   The Foundation maintains the protocol through:           │
│                                                             │
│   • Core development and upgrades                          │
│   • Security audits and bug bounties                       │
│   • Legal and compliance operations                        │
│   • Community coordination                                 │
│   • Emergency response capability                          │
│                                                             │
│   4-Year Vesting:                                           │
│   • Ensures long-term commitment                           │
│   • Prevents foundation dumping                            │
│   • Aligns foundation with network success                 │
│   • Provides predictable operational runway                │
│                                                             │
│   10% is conservative compared to many projects,           │
│   reflecting commitment to decentralization.               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.5 Why Community Incentives Matter

```
┌─────────────────────────────────────────────────────────────┐
│         RATIONALE: COMMUNITY INCENTIVES (10%)               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Network Effects                                           │
│   ───────────────                                           │
│   Community drives adoption. Incentives enable:            │
│                                                             │
│   • User acquisition and retention                         │
│   • Liquidity bootstrapping                                │
│   • Ambassador and evangelist programs                     │
│   • Bug bounties and security research                     │
│   • Governance participation rewards                       │
│                                                             │
│   Epoch Streaming:                                          │
│   • Continuous distribution maintains engagement           │
│   • Governance can adjust rates as needed                  │
│   • No large unlocks that destabilize market               │
│                                                             │
│   Community incentives are the "marketing budget" of       │
│   a decentralized network.                                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Security Implications

### 6.1 Transparency Guarantees

```
┌─────────────────────────────────────────────────────────────┐
│              DISTRIBUTION SECURITY GUARANTEES               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ✓ NO INSIDER MANIPULATION                                │
│     • All allocations defined at genesis                   │
│     • No hidden wallets or reserves                        │
│     • Governance cannot create new tokens                  │
│                                                             │
│   ✓ NO HIDDEN SUPPLY                                       │
│     • Total supply verifiable on-chain                     │
│     • All allocation addresses public                      │
│     • Vesting contracts auditable                          │
│                                                             │
│   ✓ FULLY TRANSPARENT UNLOCKS                              │
│     • Vesting schedules encoded in contracts               │
│     • Unlock events visible to all                         │
│     • No discretionary releases                            │
│                                                             │
│   ✓ VERIFIABLE ON-CHAIN                                    │
│     • Supply cap enforced at protocol level                │
│     • Distribution tracked in state                        │
│     • Anyone can audit allocations                         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 6.2 Anti-Manipulation Measures

| Measure | Implementation |
|---------|----------------|
| **Fixed Supply** | Protocol rejects any transaction that would exceed 31,536,000 MBO |
| **Vesting Enforcement** | Smart contracts enforce unlock schedules, no overrides |
| **Public Addresses** | All allocation wallets published and monitored |
| **Audit Trail** | Every token transfer recorded on-chain |
| **Governance Limits** | Governance cannot modify supply or allocation percentages |

### 6.3 Verification Methods

```
┌─────────────────────────────────────────────────────────────┐
│              HOW TO VERIFY DISTRIBUTION                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   1. TOTAL SUPPLY CHECK                                    │
│      Query: total_supply()                                 │
│      Expected: ≤ 31,536,000 MBO                            │
│                                                             │
│   2. ALLOCATION ADDRESS BALANCES                           │
│      Foundation: 0x... (published)                         │
│      Grants: 0x... (published)                             │
│      Community: 0x... (published)                          │
│      Contributors: 0x... (published)                       │
│                                                             │
│   3. VESTING CONTRACT STATE                                │
│      Query vesting contracts for:                          │
│      • Total allocated                                     │
│      • Amount unlocked                                     │
│      • Remaining locked                                    │
│      • Next unlock timestamp                               │
│                                                             │
│   4. BLOCK REWARD EMISSIONS                                │
│      Sum all block rewards issued                          │
│      Compare to emission schedule                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. For Participants

### 7.1 Validators

**How You Earn MBO:**
- Stake MBO to participate in consensus
- Earn 50% of block rewards proportional to stake
- Receive proposer bonuses when producing blocks
- Collect delegation commissions from delegators

**How MBO Unlocks:**
- Rewards unlock immediately upon block finalization
- No vesting or lockup on earned rewards
- Staked MBO has unbonding period (e.g., 21 days)

**Optimization:**
- Maximize stake to increase reward share
- Maintain high uptime for full reward eligibility
- Set competitive commission to attract delegations

---

### 7.2 GPU Providers

**How You Earn MBO:**
- Register as a compute provider
- Complete assigned compute tasks
- Receive 50% of block rewards proportional to verified work

**How MBO Unlocks:**
- Rewards unlock immediately upon receipt verification
- No vesting or lockup on earned rewards
- Provider stake has unbonding period if staked

**Optimization:**
- Invest in capable hardware for higher-tier tasks
- Maintain high success rate for priority assignment
- Ensure deterministic execution environment

---

### 7.3 Developers

**How You Earn MBO:**
- Apply for ecosystem grants
- Build applications, tools, or infrastructure
- Receive milestone-based payments

**How MBO Unlocks:**
- Unlocked upon milestone completion and verification
- Typical structure: 10% on approval, remainder on milestones
- No additional vesting after milestone payment

**Optimization:**
- Define clear, achievable milestones
- Demonstrate value to ecosystem
- Engage with community for feedback

---

### 7.4 Community Participants

**How You Earn MBO:**
- Participate in incentive programs
- Contribute to bounties and competitions
- Engage as ambassador or educator

**How MBO Unlocks:**
- Streamed per epoch based on program rules
- Varies by specific incentive program
- Some programs may have task-based instant unlock

**Optimization:**
- Stay active in community channels
- Participate early in programs
- Build reputation for larger opportunities

---

### 7.5 Ecosystem Contributors

**How You Earn MBO:**
- Early contributors receive vested allocation
- Foundation team receives operational allocation
- Advisors receive milestone-based allocation

**How MBO Unlocks:**
- Early contributors: 4-year vesting with 1-year cliff
- Foundation: 4-year linear vesting
- Terms vary by contributor agreement

**Note:**
- All contributor allocations are publicly disclosed
- Vesting enforced by smart contracts
- No acceleration clauses or early unlocks

---

## 8. Related Documentation

| Document | Description |
|----------|-------------|
| `token_intro.md` | MBO token introduction |
| `monetary_policy.md` | Fixed supply and halving |
| `reward_mechanics.md` | Reward calculation details |
| `vesting_contracts.md` | Vesting contract specifications |
| `governance.md` | Governance participation |

---

*This document defines the official MBO token distribution. All allocations are immutable and verifiable on-chain.*

