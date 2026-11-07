<!-- Verified against tokenomics.md -->
# Mbongo Chain — Incentive Design

> **HISTORICAL, NON-NORMATIVE.** Banner added 2026-11-07; the text below it is unchanged.
>
> - **Nothing here is implemented.** The runtime has no issuance, supply cap, burn, fees,
>   treasury, staking, rewards or vesting.
> - **No parameter here is adopted:** not the 31,536,000 MBO figure, the emission schedule,
>   the block time, the allocation percentages or the reward splits.
> - **PoUW is obsolete.** Validators executing workloads, and PoUW rewards or allocations,
>   are withdrawn by [`VISION_v2.md`](VISION_v2.md) P1. Provider assignment and settlement
>   need their own RFCs (VISION_v2 §7).
> - **Where every monetary question stands:** RFC 0004 §12, where no decision is made. The
>   inconsistencies between these documents are listed in RFC 0004 §13:
>   [`rfcs/0004-mbo-monetary-decision-framework.md`](rfcs/0004-mbo-monetary-decision-framework.md).

> **Document Type:** Economic Design Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of Incentive Design](#1-purpose-of-incentive-design)
2. [Participants & Their Incentives](#2-participants--their-incentives)
3. [Core Incentive Principles](#3-core-incentive-principles)
4. [Reward Model Overview](#4-reward-model-overview)
5. [Reward Allocation Breakdown](#5-reward-allocation-breakdown)
6. [Fee Model Interaction](#6-fee-model-interaction)
7. [Slashing & Penalties](#7-slashing--penalties)
8. [Long-term Sustainability](#8-long-term-sustainability)
9. [Economic Safety Guarantees](#9-economic-safety-guarantees)

---

## 1. Purpose of Incentive Design

### 1.1 Why Incentives Exist in Mbongo Chain

Mbongo Chain's incentive system is the economic engine that aligns the behavior of diverse participants toward common network goals. Unlike centralized systems where compliance is enforced through authority, decentralized networks rely on carefully designed economic incentives to ensure participants act honestly and contribute productively.

The incentive design serves as the foundation for:

- **Protocol Security**: Validators stake MBO tokens, creating economic skin-in-the-game that makes attacks prohibitively expensive
- **Resource Allocation**: Compute providers are compensated for contributing GPU cycles, ensuring the network has sufficient computational throughput
- **Network Growth**: Developers and community members are rewarded for building, maintaining, and expanding the ecosystem
- **Long-term Alignment**: All participants benefit from network success, creating positive-sum dynamics

### 1.2 Design Goals

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         INCENTIVE DESIGN GOALS                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                              SECURITY                                            │  │
│   │                                                                                  │  │
│   │   • Make attacks economically irrational                                        │  │
│   │   • Reward honest behavior, punish malicious actions                            │  │
│   │   • Ensure validators have substantial stake at risk                            │  │
│   │   • Prevent consensus manipulation through economic barriers                    │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           DECENTRALIZATION                                       │  │
│   │                                                                                  │  │
│   │   • Prevent stake concentration in few hands                                    │  │
│   │   • Enable small validators to participate profitably                           │  │
│   │   • Distribute compute rewards broadly across providers                         │  │
│   │   • Ensure no single entity controls network operation                          │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                          COMPUTE EFFICIENCY                                      │  │
│   │                                                                                  │  │
│   │   • Incentivize GPU providers to offer competitive pricing                      │  │
│   │   • Reward high-quality, verified compute work                                  │  │
│   │   • Encourage hardware investment and upgrades                                  │  │
│   │   • Minimize idle compute capacity across the network                           │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                        LONG-TERM SUSTAINABILITY                                  │  │
│   │                                                                                  │  │
│   │   • Fixed supply creates predictable economics                                  │  │
│   │   • Halving schedule ensures multi-decade reward runway                         │  │
│   │   • Fee burning maintains token value as rewards decrease                       │  │
│   │   • No inflation means existing holders are not diluted                         │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                              FAIRNESS                                            │  │
│   │                                                                                  │  │
│   │   • Equal rules for all participants                                            │  │
│   │   • No privileged actors or hidden advantages                                   │  │
│   │   • Transparent on-chain reward distribution                                    │  │
│   │   • Merit-based rewards (stake weight, compute quality)                         │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Participants & Their Incentives

### 2.1 PoS Validators

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoS VALIDATORS                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE                                                                                  │
│   ────                                                                                  │
│   Validators are the backbone of network security. They stake MBO tokens as            │
│   collateral, produce blocks, attest to chain validity, and maintain consensus.        │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Run validator node infrastructure (24/7 uptime)                                    │
│   • Propose and validate blocks                                                        │
│   • Participate in consensus voting                                                    │
│   • Verify PoUW compute receipts                                                       │
│   • Maintain protocol rule compliance                                                  │
│                                                                                         │
│   INCENTIVES                                                                            │
│   ──────────                                                                            │
│   • 50% of block rewards distributed to PoS pool                                       │
│   • Proposer bonus for producing valid blocks                                          │
│   • Transaction fee share                                                              │
│   • Delegation commissions (set by validator)                                          │
│                                                                                         │
│   RISKS                                                                                 │
│   ─────                                                                                 │
│   • Stake slashed for downtime or malicious behavior                                   │
│   • Opportunity cost of locked stake                                                   │
│   • Infrastructure and operational costs                                               │
│                                                                                         │
│   ECONOMIC ALIGNMENT                                                                    │
│   ───────────────────                                                                   │
│   Validators profit when:                                                              │
│   • Network is secure and thriving                                                     │
│   • MBO token maintains or increases value                                             │
│   • Transaction volume grows                                                           │
│   • They maintain high uptime and correctness                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Delegators

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DELEGATORS                                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE                                                                                  │
│   ────                                                                                  │
│   Delegators contribute to network security by staking MBO with validators,            │
│   without running infrastructure themselves. They provide passive security             │
│   capital and help decentralize stake distribution.                                    │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Choose reliable validators to delegate to                                          │
│   • Monitor validator performance                                                      │
│   • Re-delegate if validator underperforms                                             │
│   • Participate in governance (optional)                                               │
│                                                                                         │
│   INCENTIVES                                                                            │
│   ──────────                                                                            │
│   • Share of validator's PoS rewards (after commission)                                │
│   • Passive income on staked MBO                                                       │
│   • No infrastructure costs                                                            │
│   • Governance voting power                                                            │
│                                                                                         │
│   RISKS                                                                                 │
│   ─────                                                                                 │
│   • Stake slashed if chosen validator misbehaves                                       │
│   • Unbonding period (tokens locked during withdrawal)                                 │
│   • Validator commission rates can change                                              │
│                                                                                         │
│   ECONOMIC ALIGNMENT                                                                    │
│   ───────────────────                                                                   │
│   Delegators profit when:                                                              │
│   • Chosen validators perform well                                                     │
│   • Network security remains strong                                                    │
│   • MBO token appreciates in value                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 PoUW Compute Providers

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoUW COMPUTE PROVIDERS                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE                                                                                  │
│   ────                                                                                  │
│   Compute providers contribute GPU cycles to the network's decentralized compute       │
│   marketplace. They execute AI inference, training tasks, validation acceleration,     │
│   and other useful workloads that earn PoUW (Proof-of-Useful-Work) rewards.           │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   • Run GPU infrastructure (availability commitment)                                   │
│   • Execute assigned compute tasks                                                     │
│   • Produce valid compute receipts                                                     │
│   • Maintain deterministic execution environment                                       │
│   • Meet quality and timing requirements                                               │
│                                                                                         │
│   INCENTIVES                                                                            │
│   ──────────                                                                            │
│   • 50% of block rewards distributed to PoUW pool                                      │
│   • Rewards proportional to verified work units                                        │
│   • Compute fees from task requesters                                                  │
│   • Reputation score improvement for consistent performance                            │
│                                                                                         │
│   RISKS                                                                                 │
│   ─────                                                                                 │
│   • Slashed for invalid compute results                                                │
│   • Hardware investment and electricity costs                                          │
│   • Task availability may vary                                                         │
│   • Reputation damage for poor performance                                             │
│                                                                                         │
│   ECONOMIC ALIGNMENT                                                                    │
│   ───────────────────                                                                   │
│   Providers profit when:                                                               │
│   • Compute demand grows (AI, ML, rendering)                                           │
│   • They maintain high task completion rates                                           │
│   • Network attracts more compute requesters                                           │
│   • MBO value supports hardware investment                                             │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Developers & Community

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DEVELOPERS & COMMUNITY                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ROLE                                                                                  │
│   ────                                                                                  │
│   Developers build applications, tools, and infrastructure on Mbongo Chain.            │
│   Community members contribute through education, governance participation,            │
│   bug reporting, and ecosystem growth activities.                                      │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│   Developers:                                                                          │
│   • Build dApps and tooling                                                            │
│   • Contribute to core protocol                                                        │
│   • Create documentation and tutorials                                                 │
│   • Report and fix bugs                                                                │
│                                                                                         │
│   Community:                                                                            │
│   • Participate in governance                                                          │
│   • Spread awareness and adoption                                                      │
│   • Provide feedback and testing                                                       │
│   • Support new users                                                                  │
│                                                                                         │
│   INCENTIVES                                                                            │
│   ──────────                                                                            │
│   • Ecosystem grants (15% of total supply allocated)                                   │
│   • Bug bounty rewards                                                                 │
│   • Community incentive programs (10% of total supply)                                 │
│   • Developer milestone payments                                                       │
│   • Ambassador and education rewards                                                   │
│                                                                                         │
│   ECONOMIC ALIGNMENT                                                                    │
│   ───────────────────                                                                   │
│   Developers/Community profit when:                                                    │
│   • Ecosystem grows and attracts users                                                 │
│   • Grant programs are well-funded                                                     │
│   • MBO token appreciates with adoption                                                │
│   • Network becomes industry-standard for compute                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.5 Participant Summary Matrix

| Participant | Primary Contribution | Primary Reward | Risk Exposure | Alignment Mechanism |
|-------------|---------------------|----------------|---------------|---------------------|
| **Validators** | Block production, consensus | 50% PoS rewards | Stake slash, ops cost | Stake at risk |
| **Delegators** | Security capital | Reward share | Slash (via validator) | Validator selection |
| **Compute Providers** | GPU cycles | 50% PoUW rewards | Invalid work slash | Work verification |
| **Developers** | Code, tools, apps | Grants, bounties | Milestone failure | Grant milestones |
| **Community** | Growth, governance | Incentive programs | Time investment | Token appreciation |

---

## 3. Core Incentive Principles

### 3.1 Fixed Supply Discipline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FIXED SUPPLY DISCIPLINE                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TOTAL SUPPLY: 31,536,000 MBO                                                         │
│   ══════════════════════════                                                            │
│                                                                                         │
│   This number is IMMUTABLE. It cannot be changed by:                                   │
│   • Governance vote                                                                    │
│   • Foundation decision                                                                │
│   • Protocol upgrade                                                                   │
│   • Emergency action                                                                   │
│                                                                                         │
│   WHY THIS MATTERS                                                                      │
│   ────────────────                                                                      │
│   • Creates predictable scarcity                                                       │
│   • Prevents inflation tax on holders                                                  │
│   • Enables long-term economic planning                                                │
│   • Builds trust through mathematical guarantees                                       │
│                                                                                         │
│   SUPPLY ORIGIN                                                                         │
│   ─────────────                                                                         │
│   31,536,000 = seconds in a year (365.25 × 24 × 60 × 60)                               │
│                                                                                         │
│   Symbolizes: "One MBO per second of opportunity"                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Halving Every 5 Years

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         HALVING SCHEDULE                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MECHANISM                                                                             │
│   ─────────                                                                             │
│   Block rewards are reduced by 50% every 5 years (157,680,000 blocks at 1s/block)     │
│                                                                                         │
│   SCHEDULE                                                                              │
│   ────────                                                                              │
│   ┌────────────┬─────────────────┬──────────────────┐                                  │
│   │   Period   │  Block Reward   │  Annual Emission │                                  │
│   ├────────────┼─────────────────┼──────────────────┤                                  │
│   │  Year 1-5  │    0.1 MBO      │  ~3,153,600 MBO  │                                  │
│   │  Year 6-10 │    0.05 MBO     │  ~1,576,800 MBO  │                                  │
│   │  Year 11-15│    0.025 MBO    │   ~788,400 MBO   │                                  │
│   │  Year 16-20│    0.0125 MBO   │   ~394,200 MBO   │                                  │
│   │  Year 21+  │    Continues... │   Decreasing...  │                                  │
│   └────────────┴─────────────────┴──────────────────┘                                  │
│                                                                                         │
│   WHY HALVING CREATES VALUE                                                             │
│   ─────────────────────────                                                             │
│   • Predictable scarcity increases perceived value                                     │
│   • Early participants rewarded for bootstrapping risk                                 │
│   • Forces network to become self-sustaining via fees                                  │
│   • Creates multi-decade reward runway                                                 │
│                                                                                         │
│   SECURITY PRESERVATION                                                                 │
│   ─────────────────────                                                                 │
│   As block rewards decrease, transaction fees grow to compensate,                      │
│   maintaining validator and provider incentives.                                       │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Deterministic Issuance

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DETERMINISTIC ISSUANCE                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PRINCIPLE                                                                             │
│   ─────────                                                                             │
│   Every MBO that will ever exist is created according to a fixed, predetermined        │
│   schedule. No human decision affects token creation.                                  │
│                                                                                         │
│   ISSUANCE FORMULA                                                                      │
│   ────────────────                                                                      │
│   block_reward(height) = INITIAL_REWARD × (0.5 ^ (height / HALVING_INTERVAL))         │
│                                                                                         │
│   Where:                                                                                │
│   • INITIAL_REWARD = 0.1 MBO                                                           │
│   • HALVING_INTERVAL = 157,680,000 blocks (5 years)                                    │
│                                                                                         │
│   GUARANTEES                                                                            │
│   ──────────                                                                            │
│   ✓ Anyone can independently calculate total supply at any block                       │
│   ✓ No surprises or unexpected inflation events                                        │
│   ✓ Full auditability of monetary policy                                               │
│   ✓ Identical result on every node                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Full Transparency

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TRANSPARENCY                                                    │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ALL REWARDS ON-CHAIN                                                                  │
│   ────────────────────                                                                  │
│   • Every block reward is recorded in block metadata                                   │
│   • Every recipient address is publicly visible                                        │
│   • Distribution calculations are deterministic                                        │
│   • Anyone can verify reward correctness                                               │
│                                                                                         │
│   VISIBLE INFORMATION                                                                   │
│   ───────────────────                                                                   │
│   • Block-by-block reward amounts                                                      │
│   • PoS/PoUW split per block                                                           │
│   • Individual validator/provider payouts                                              │
│   • Slashing events and amounts                                                        │
│   • Fee burn amounts                                                                   │
│   • Vesting unlock events                                                              │
│                                                                                         │
│   AUDIT TOOLS                                                                           │
│   ───────────                                                                           │
│   • Block explorer with reward tracking                                                │
│   • Supply verification scripts                                                        │
│   • Distribution analysis dashboards                                                   │
│   • Independent verification nodes                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.5 Market Neutrality

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MARKET NEUTRALITY                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   NO DISCRETIONARY ALLOCATION                                                           │
│   ───────────────────────────                                                           │
│   • Rewards follow algorithmic rules only                                              │
│   • No entity can direct rewards to specific addresses                                 │
│   • No favored validators or providers                                                 │
│   • Market competition determines who earns                                            │
│                                                                                         │
│   PROTECTED BY                                                                          │
│   ────────────                                                                          │
│   • Protocol-enforced distribution rules                                               │
│   • Immutable reward formulas                                                          │
│   • No admin keys for reward allocation                                                │
│   • Governance cannot target specific participants                                     │
│                                                                                         │
│   FAIR COMPETITION                                                                      │
│   ────────────────                                                                      │
│   PoS: Stake more, perform better → earn more                                          │
│   PoUW: Complete more work, higher quality → earn more                                 │
│                                                                                         │
│   Same rules apply to:                                                                 │
│   • Foundation-operated validators (if any)                                            │
│   • Early contributors                                                                 │
│   • New participants                                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Reward Model Overview

### 4.1 High-Level Reward Flow

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         REWARD MODEL OVERVIEW                                           │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  ╔═══════════════════════════════════════════════════════════════════════════════════════╗
  ║                            NETWORK ACTIVITY                                           ║
  ╚═══════════════════════════════════════════════════════════════════════════════════════╝
                                       │
           ┌───────────────────────────┼───────────────────────────┐
           │                           │                           │
           ▼                           ▼                           ▼
  ┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
  │    STAKING      │        │    COMPUTE      │        │      FEES       │
  │                 │        │                 │        │                 │
  │  MBO locked in  │        │  GPU cycles     │        │  Transaction +  │
  │  validator set  │        │  contributed    │        │  Compute fees   │
  └────────┬────────┘        └────────┬────────┘        └────────┬────────┘
           │                          │                          │
           ▼                          ▼                          ▼
  ┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
  │    SECURITY     │        │   THROUGHPUT    │        │   BURNING +     │
  │                 │        │                 │        │ REDISTRIBUTION  │
  │  • Consensus    │        │  • AI inference │        │                 │
  │  • Block valid. │        │  • Training     │        │  • Base fee     │
  │  • Attack cost  │        │  • Validation   │        │    burned       │
  └────────┬────────┘        └────────┬────────┘        │  • Priority fee │
           │                          │                 │    to proposer  │
           │                          │                 └────────┬────────┘
           │                          │                          │
           └──────────────┬───────────┴──────────────────────────┘
                          │
                          ▼
  ╔═══════════════════════════════════════════════════════════════════════════════════════╗
  ║                              BLOCK REWARD POOL                                        ║
  ║                                                                                       ║
  ║                        0.1 MBO per block (Year 1-5)                                   ║
  ╚═══════════════════════════════════════════════════════════════════════════════════════╝
                          │
           ┌──────────────┴──────────────┐
           │                             │
           ▼                             ▼
  ┌─────────────────────────┐  ┌─────────────────────────┐
  │                         │  │                         │
  │   PoS POOL (50%)        │  │   PoUW POOL (50%)       │
  │                         │  │                         │
  │   0.05 MBO/block        │  │   0.05 MBO/block        │
  │                         │  │                         │
  │   ┌───────────────┐     │  │   ┌───────────────┐     │
  │   │ Block Proposer│     │  │   │ GPU Provider 1│     │
  │   │ + Attesters   │     │  │   │ + Provider 2  │     │
  │   │ + Delegators  │     │  │   │ + Provider N  │     │
  │   └───────────────┘     │  │   └───────────────┘     │
  │                         │  │                         │
  │   Distribution:         │  │   Distribution:         │
  │   • Stake weight        │  │   • Work units          │
  │   • Performance score   │  │   • Quality score       │
  └─────────────────────────┘  └─────────────────────────┘
           │                             │
           └──────────────┬──────────────┘
                          │
                          ▼
  ╔═══════════════════════════════════════════════════════════════════════════════════════╗
  ║                         PARTICIPANT WALLETS                                           ║
  ║                                                                                       ║
  ║   Validators │ Delegators │ Compute Providers                                         ║
  ╚═══════════════════════════════════════════════════════════════════════════════════════╝
```

### 4.2 Value Flow Summary

| Input | Mechanism | Output | Beneficiary |
|-------|-----------|--------|-------------|
| **Staked MBO** | Security collateral | Block rewards | Validators + Delegators |
| **GPU Cycles** | Useful work | Compute rewards | Providers |
| **Transaction Fees** | Execution cost | Burn + Proposer tip | Token holders + Proposer |
| **Compute Fees** | Task cost | Provider payment | Providers |

---

## 5. Reward Allocation Breakdown

### 5.1 Canonical Allocation Values

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         REWARD ALLOCATION (CANONICAL)                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   BLOCK REWARD SPLIT                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ┌──────────────────────────┐    ┌──────────────────────────┐                 │  │
│   │   │                          │    │                          │                 │  │
│   │   │     PoS VALIDATORS       │    │    PoUW COMPUTE          │                 │  │
│   │   │        50%               │    │       50%                │                 │  │
│   │   │                          │    │                          │                 │  │
│   │   │  • Block proposer        │    │  • GPU providers         │                 │  │
│   │   │  • Attesters             │    │  • Task executors        │                 │  │
│   │   │  • Delegators (indirect) │    │  • Verification nodes    │                 │  │
│   │   │                          │    │                          │                 │  │
│   │   └──────────────────────────┘    └──────────────────────────┘                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   FEE HANDLING                                                                          │
│   ════════════                                                                          │
│   • Base fee: BURNED (removed from circulation)                                        │
│   • Priority fee: Paid to block proposer                                               │
│   • Compute fee: Paid directly to providers                                            │
│                                                                                         │
│   TREASURY ALLOCATION                                                                   │
│   ═══════════════════                                                                   │
│   • Treasury receives: 0% of block rewards                                             │
│   • No dilution mechanism                                                              │
│   • Slashed tokens → redistributed (not treasury)                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Allocation Table

| Category | Percentage | Paid To | Trigger | Notes |
|----------|------------|---------|---------|-------|
| **PoS Block Rewards** | 50% | Validators + Delegators | Every finalized block | Split by stake weight × performance |
| **PoUW Block Rewards** | 50% | Compute Providers | Every finalized block | Split by verified work units |
| **Base Fees** | 100% burned | No one (supply reduction) | Every transaction | Deflationary mechanism |
| **Priority Fees** | 100% | Block Proposer | User-set priority | Incentivizes inclusion |
| **Compute Fees** | 100% | Task Provider | Task completion | Direct payment |
| **Slashed Stake** | 100% | Reward pool / Burned | Slashing event | Depends on offense type |
| **Treasury Share** | 0% | N/A | N/A | No ongoing dilution |

### 5.3 Per-Block Distribution Example (Year 1)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EXAMPLE: SINGLE BLOCK DISTRIBUTION                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Block Reward: 0.1 MBO                                                                │
│   Fees Collected: 0.02 MBO (0.015 base + 0.005 priority)                               │
│                                                                                         │
│   DISTRIBUTION                                                                          │
│   ────────────                                                                          │
│   ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│   │   Component           │   Amount      │   Recipient                              │ │
│   ├───────────────────────┼───────────────┼──────────────────────────────────────────┤ │
│   │   PoS Pool            │   0.05 MBO    │   Validators (stake-weighted)            │ │
│   │   PoUW Pool           │   0.05 MBO    │   Compute providers (work-weighted)      │ │
│   │   Base Fee (burned)   │   0.015 MBO   │   Removed from circulation               │ │
│   │   Priority Fee        │   0.005 MBO   │   Block proposer                         │ │
│   ├───────────────────────┼───────────────┼──────────────────────────────────────────┤ │
│   │   Total Distributed   │   0.105 MBO   │                                          │ │
│   │   Total Burned        │   0.015 MBO   │                                          │ │
│   └──────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                         │
│   NET SUPPLY CHANGE: +0.085 MBO (reward - burn)                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Fee Model Interaction

### 6.1 Fee Categories

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FEE MODEL                                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           STATE FEES (Execution Gas)                             │  │
│   ├─────────────────────────────────────────────────────────────────────────────────┤  │
│   │                                                                                  │  │
│   │   Purpose: Pay for state machine execution                                      │  │
│   │                                                                                  │  │
│   │   Components:                                                                    │  │
│   │   • Base computation cost                                                       │  │
│   │   • State read/write operations                                                 │  │
│   │   • Memory allocation                                                           │  │
│   │   • Signature verification                                                      │  │
│   │                                                                                  │  │
│   │   Pricing: Dynamic (AIDA-regulated within bounds)                               │  │
│   │                                                                                  │  │
│   │   Destination: BASE → Burned | PRIORITY → Proposer                              │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           COMPUTE FEES (PoUW Cycles)                             │  │
│   ├─────────────────────────────────────────────────────────────────────────────────┤  │
│   │                                                                                  │  │
│   │   Purpose: Pay for GPU compute tasks                                            │  │
│   │                                                                                  │  │
│   │   Components:                                                                    │  │
│   │   • GPU time                                                                    │  │
│   │   • Memory bandwidth                                                            │  │
│   │   • Task complexity                                                             │  │
│   │   • Verification overhead                                                       │  │
│   │                                                                                  │  │
│   │   Pricing: Market-determined with AIDA price discovery                          │  │
│   │                                                                                  │  │
│   │   Destination: 100% → Compute Provider                                          │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                           PRIORITY FEES                                          │  │
│   ├─────────────────────────────────────────────────────────────────────────────────┤  │
│   │                                                                                  │  │
│   │   Purpose: Incentivize faster transaction inclusion                             │  │
│   │                                                                                  │  │
│   │   Mechanism:                                                                     │  │
│   │   • User sets priority fee above base                                           │  │
│   │   • Higher priority = earlier inclusion                                         │  │
│   │   • Market competition for block space                                          │  │
│   │                                                                                  │  │
│   │   Pricing: User-determined (market auction)                                     │  │
│   │                                                                                  │  │
│   │   Destination: 100% → Block Proposer                                            │  │
│   │                                                                                  │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Deterministic Burn Mechanism

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         BURN MECHANISM                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   BASE FEE BURN                                                                         │
│   ─────────────                                                                         │
│   100% of base fees are permanently destroyed (sent to null address)                   │
│                                                                                         │
│   FORMULA                                                                               │
│   ───────                                                                               │
│   burned_per_block = sum(tx_gas × base_fee_per_gas) for all transactions              │
│                                                                                         │
│   WHY BURN                                                                              │
│   ────────                                                                              │
│   • Creates deflationary pressure                                                      │
│   • Aligns user interests with token value                                             │
│   • Prevents fee extraction by insiders                                                │
│   • Makes high-activity periods benefit all holders                                    │
│                                                                                         │
│   BURN VISIBILITY                                                                       │
│   ───────────────                                                                       │
│   • Burn amount recorded in every block header                                         │
│   • Cumulative burn trackable via block explorer                                       │
│   • Affects circulating supply calculations                                            │
│                                                                                         │
│   EQUILIBRIUM DYNAMICS                                                                  │
│   ────────────────────                                                                  │
│   High Activity → High Burns → Reduced Supply → Higher Scarcity                        │
│                                                                                         │
│   Long-term: Burns may exceed emissions, creating net deflation                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Fee Flow Diagram

```
  ┌─────────────────────────────────────────────────────────────────────────────────────┐
  │                              TRANSACTION                                            │
  │                                                                                     │
  │   User pays: gas_used × (base_fee + priority_fee) + compute_fee                    │
  └─────────────────────────────────────────────────────────────────────────────────────┘
                                       │
           ┌───────────────────────────┼───────────────────────────┐
           │                           │                           │
           ▼                           ▼                           ▼
  ┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
  │   BASE FEE      │        │  PRIORITY FEE   │        │  COMPUTE FEE    │
  │                 │        │                 │        │                 │
  │   Burned        │        │   To Proposer   │        │   To Provider   │
  │   (destroyed)   │        │   (validator)   │        │   (GPU node)    │
  └─────────────────┘        └─────────────────┘        └─────────────────┘
           │                           │                           │
           ▼                           ▼                           ▼
  ┌─────────────────┐        ┌─────────────────┐        ┌─────────────────┐
  │   NULL ADDRESS  │        │   PROPOSER      │        │   PROVIDER      │
  │                 │        │   WALLET        │        │   WALLET        │
  │   Supply ↓      │        │                 │        │                 │
  └─────────────────┘        └─────────────────┘        └─────────────────┘
```

---

## 7. Slashing & Penalties

### 7.1 Slashing Overview

Slashing is the mechanism that enforces honest behavior by confiscating stake from misbehaving participants. It creates economic consequences for attacks, making the network costly to compromise.

### 7.2 Severity Ladder

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING SEVERITY LADDER                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SEVERITY        OFFENSE                              PENALTY              DESTINATION │
│   ═══════════════════════════════════════════════════════════════════════════════════  │
│                                                                                         │
│   ▲                                                                                     │
│   │  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │  │                                                                             │   │
│   │  │   LEVEL 5: CRITICAL                                                         │   │
│   │  │   ──────────────────                                                        │   │
│   │  │   • Coordinated attack on consensus                                         │   │
│   │  │   • Finality reversion attempt                                              │   │
│   │  │                                                                             │   │
│   │  │   Penalty: 100% stake slash + permanent ban                                 │   │
│   │  │   Destination: 50% burned, 50% to reporter                                  │   │
│   │  │                                                                             │   │
│   │  └─────────────────────────────────────────────────────────────────────────────┘   │
│   │                                                                                     │
│   │  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │  │                                                                             │   │
│   │  │   LEVEL 4: SEVERE                                                           │   │
│   │  │   ───────────────                                                           │   │
│   │  │   • Double signing (conflicting blocks)                                     │   │
│   │  │   • Equivocation attack                                                     │   │
│   │  │                                                                             │   │
│   │  │   Penalty: 33% stake slash + 30-day jail                                    │   │
│   │  │   Destination: 50% burned, 50% to reporter                                  │   │
│   │  │                                                                             │   │
│   │  └─────────────────────────────────────────────────────────────────────────────┘   │
│   │                                                                                     │
│   │  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │  │                                                                             │   │
│   S  │   LEVEL 3: MODERATE                                                         │   │
│   E  │   ────────────────                                                          │   │
│   V  │   • Invalid compute result (intentional)                                    │   │
│   E  │   • Repeated invalid attestations                                           │   │
│   R  │                                                                             │   │
│   I  │   Penalty: 10% stake/collateral slash                                       │   │
│   T  │   Destination: Redistributed to reward pool                                 │   │
│   Y  │                                                                             │   │
│   │  └─────────────────────────────────────────────────────────────────────────────┘   │
│   │                                                                                     │
│   │  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │  │                                                                             │   │
│   │  │   LEVEL 2: MINOR                                                            │   │
│   │  │   ─────────────                                                             │   │
│   │  │   • Extended downtime (>24 hours)                                           │   │
│   │  │   • Missed attestation duties                                               │   │
│   │  │                                                                             │   │
│   │  │   Penalty: 1% stake slash                                                   │   │
│   │  │   Destination: Redistributed to active validators                           │   │
│   │  │                                                                             │   │
│   │  └─────────────────────────────────────────────────────────────────────────────┘   │
│   │                                                                                     │
│   │  ┌─────────────────────────────────────────────────────────────────────────────┐   │
│   │  │                                                                             │   │
│   │  │   LEVEL 1: WARNING                                                          │   │
│   │  │   ────────────────                                                          │   │
│   │  │   • Brief downtime (<4 hours)                                               │   │
│   │  │   • Single missed block                                                     │   │
│   │  │                                                                             │   │
│   │  │   Penalty: Reward withholding only (no slash)                               │   │
│   │  │   Destination: N/A                                                          │   │
│   │  │                                                                             │   │
│   ▼  └─────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 Slashing Categories

#### 7.3.1 Validator Downtime Slashing (Mild)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DOWNTIME SLASHING                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TRIGGER                                                                               │
│   ───────                                                                               │
│   Validator fails to participate in consensus for extended period                      │
│                                                                                         │
│   THRESHOLDS                                                                            │
│   ──────────                                                                            │
│   • <4 hours:   Warning only, reward withholding                                       │
│   • 4-24 hours: 0.1% slash, temporary jail (1 hour)                                    │
│   • >24 hours:  1% slash, extended jail (24 hours)                                     │
│   • >7 days:    5% slash, forced unbonding                                             │
│                                                                                         │
│   RATIONALE                                                                             │
│   ─────────                                                                             │
│   • Mild penalties encourage reliability without destroying validators                 │
│   • Accidental downtime should not be catastrophic                                     │
│   • Repeated offenses escalate penalties                                               │
│                                                                                         │
│   DESTINATION                                                                           │
│   ───────────                                                                           │
│   Slashed MBO redistributed to active validators in same epoch                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 7.3.2 Malicious Signing Slashing (Severe)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MALICIOUS SIGNING SLASHING                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TRIGGER                                                                               │
│   ───────                                                                               │
│   Validator signs conflicting blocks or attestations (equivocation)                    │
│                                                                                         │
│   DETECTION                                                                             │
│   ─────────                                                                             │
│   • Two signatures from same validator on conflicting data                             │
│   • Cryptographic proof of misbehavior                                                 │
│   • Anyone can submit evidence                                                         │
│                                                                                         │
│   PENALTY                                                                               │
│   ───────                                                                               │
│   • 33% of stake slashed                                                               │
│   • Validator jailed for 30 days                                                       │
│   • Reputation permanently marked                                                      │
│                                                                                         │
│   RATIONALE                                                                             │
│   ─────────                                                                             │
│   • Double signing is a serious attack on consensus                                    │
│   • Severe penalty makes attack economically irrational                                │
│   • Reporter reward incentivizes monitoring                                            │
│                                                                                         │
│   DESTINATION                                                                           │
│   ───────────                                                                           │
│   • 50% of slashed amount: Burned                                                      │
│   • 50% of slashed amount: Paid to evidence submitter                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

#### 7.3.3 PoUW Invalid Compute Slashing (Fixed Penalty)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         INVALID COMPUTE SLASHING                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TRIGGER                                                                               │
│   ───────                                                                               │
│   Compute provider submits invalid or fraudulent compute receipt                       │
│                                                                                         │
│   DETECTION                                                                             │
│   ─────────                                                                             │
│   • Verification nodes re-execute and find mismatch                                    │
│   • Statistical sampling detects anomalies                                             │
│   • Cross-provider result comparison                                                   │
│                                                                                         │
│   PENALTY                                                                               │
│   ───────                                                                               │
│   • Fixed: 1,000 MBO per invalid receipt                                               │
│   • Provider collateral confiscated                                                    │
│   • Reputation score severely reduced                                                  │
│   • Repeat offenses: Permanent ban                                                     │
│                                                                                         │
│   RATIONALE                                                                             │
│   ─────────                                                                             │
│   • Fixed penalty ensures consistent deterrent                                         │
│   • Collateral requirement prevents spam providers                                     │
│   • Reputation system excludes bad actors long-term                                    │
│                                                                                         │
│   DESTINATION                                                                           │
│   ───────────                                                                           │
│   • 100% redistributed to PoUW reward pool                                             │
│   • Benefits honest providers                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.4 Slashed Token Flow

```
  ┌─────────────────────────────────────────────────────────────────────────────────────┐
  │                           SLASHING EVENT                                            │
  │                                                                                     │
  │   Validator/Provider commits offense → Evidence submitted → Slash executed         │
  └─────────────────────────────────────────────────────────────────────────────────────┘
                                       │
                                       ▼
                         ┌─────────────────────────┐
                         │    SLASHED MBO POOL     │
                         │                         │
                         │   (Confiscated tokens)  │
                         └────────────┬────────────┘
                                      │
           ┌──────────────────────────┼──────────────────────────┐
           │                          │                          │
           ▼                          ▼                          ▼
  ┌─────────────────┐       ┌─────────────────┐       ┌─────────────────┐
  │     BURNED      │       │    REPORTER     │       │  REWARD POOL    │
  │                 │       │                 │       │                 │
  │  For severe     │       │  Evidence       │       │  Minor offenses │
  │  offenses       │       │  submitter      │       │  redistributed  │
  │  (50%)          │       │  reward (50%)   │       │  to active      │
  │                 │       │                 │       │  participants   │
  └─────────────────┘       └─────────────────┘       └─────────────────┘
```

---

## 8. Long-term Sustainability

### 8.1 Why 31.5M Supply + Halving is Sufficient

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         LONG-TERM SUPPLY SUSTAINABILITY                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   EMISSION CURVE (50-YEAR PROJECTION)                                                   │
│   ───────────────────────────────────                                                   │
│                                                                                         │
│   Supply                                                                                │
│   (MBO)                                                                                 │
│     │                                                                                   │
│   31.5M│ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─  (cap)          │
│     │                                            ●───────────────────●                  │
│   30M │                                   ●──────┘                                      │
│     │                             ●───────┘                                             │
│   25M │                      ●────┘                                                     │
│     │                  ●─────┘                                                          │
│   20M │            ●───┘                                                                │
│     │         ●───┘                                                                     │
│   15M │     ●──┘                                                                        │
│     │   ●──┘                                                                            │
│   10M │ ●─┘                                                                             │
│     │●┘                                                                                 │
│    5M │                                                                                 │
│     │                                                                                   │
│     └───┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬────▶ Years              │
│         5    10    15    20    25    30    35    40    45    50                         │
│                                                                                         │
│   KEY INSIGHT                                                                           │
│   ───────────                                                                           │
│   Supply approaches cap asymptotically. By year 20, ~99% is issued.                    │
│   Remaining emissions create minimal dilution while maintaining                        │
│   reward incentives.                                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Security Budget Transition

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY BUDGET EVOLUTION                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PHASE 1: Block Reward Dominant (Year 1-10)                                           │
│   ──────────────────────────────────────────                                            │
│   • Block rewards provide majority of security budget                                  │
│   • Fees are supplementary                                                             │
│   • Network bootstrapping period                                                       │
│                                                                                         │
│   Security Budget = Block_Rewards (high) + Fees (low)                                  │
│                                                                                         │
│                                                                                         │
│   PHASE 2: Transition (Year 10-20)                                                     │
│   ────────────────────────────────                                                      │
│   • Block rewards decreasing due to halving                                            │
│   • Transaction volume growing                                                         │
│   • Fees becoming significant                                                          │
│                                                                                         │
│   Security Budget = Block_Rewards (medium) + Fees (medium)                             │
│                                                                                         │
│                                                                                         │
│   PHASE 3: Fee Dominant (Year 20+)                                                     │
│   ────────────────────────────────                                                      │
│   • Block rewards minimal                                                              │
│   • Network at scale with high transaction volume                                      │
│   • Fees sustain security budget                                                       │
│                                                                                         │
│   Security Budget = Block_Rewards (low) + Fees (high)                                  │
│                                                                                         │
│                                                                                         │
│   EQUILIBRIUM                                                                           │
│   ───────────                                                                           │
│   If fee revenue < required security budget:                                           │
│   • Base fee increases automatically                                                   │
│   • Usage cost rises                                                                   │
│   • Equilibrium restored                                                               │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.3 Why 50/50 PoS/PoUW Prevents Centralization

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DECENTRALIZATION DYNAMICS                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   BALANCED INCENTIVES                                                                   │
│   ───────────────────                                                                   │
│   • PoS alone → Capital concentration (rich get richer)                                │
│   • PoUW alone → Hardware concentration (big farms dominate)                           │
│   • 50/50 split → Diverse participation paths                                          │
│                                                                                         │
│   PARTICIPANT DIVERSITY                                                                 │
│   ────────────────────                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Capital-rich, compute-poor → Focus on PoS staking                            │  │
│   │   Capital-poor, compute-rich → Focus on PoUW provision                         │  │
│   │   Balanced participants → Participate in both                                   │  │
│   │                                                                                 │  │
│   │   Result: No single resource type dominates                                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   GEOGRAPHIC DISTRIBUTION                                                               │
│   ───────────────────────                                                               │
│   • PoS: Low hardware requirements → Global participation                              │
│   • PoUW: Compute can be anywhere with electricity                                     │
│   • Combined: No single jurisdiction controls network                                  │
│                                                                                         │
│   ATTACK COST                                                                           │
│   ───────────                                                                           │
│   Attacker needs to control BOTH:                                                      │
│   • Majority of stake (expensive capital)                                              │
│   • Majority of compute (expensive hardware)                                           │
│                                                                                         │
│   50/50 split doubles attack complexity vs single-mechanism chains                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.4 Fee Burning Creates Equilibrium

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FEE BURN EQUILIBRIUM                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEFLATIONARY PRESSURE                                                                 │
│   ─────────────────────                                                                 │
│   • High network usage → High fees burned                                              │
│   • Burned tokens removed permanently                                                  │
│   • Circulating supply decreases                                                       │
│   • Scarcity increases                                                                 │
│                                                                                         │
│   EQUILIBRIUM DYNAMICS                                                                  │
│   ────────────────────                                                                  │
│                                                                                         │
│   If network thrives:                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   High Usage → High Burns → Lower Supply → Higher Value → Attracts Validators   │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   If network struggles:                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   Low Usage → Low Burns → Stable Supply → Lower Inflation → Preserved Value     │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   LONG-TERM PROJECTION                                                                  │
│   ────────────────────                                                                  │
│   As block rewards approach zero:                                                      │
│   • Burns may exceed new issuance                                                      │
│   • Net supply becomes deflationary                                                    │
│   • Token value supported by scarcity                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Economic Safety Guarantees

### 9.1 Immutable Economic Properties

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ECONOMIC SAFETY GUARANTEES                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                         NO HIDDEN MINTING                                        ║  │
│   ╠═════════════════════════════════════════════════════════════════════════════════╣  │
│   ║                                                                                  ║  │
│   ║   • Total supply capped at 31,536,000 MBO                                       ║  │
│   ║   • No admin function to create tokens                                          ║  │
│   ║   • No governance pathway to increase supply                                    ║  │
│   ║   • Issuance formula hardcoded in consensus rules                               ║  │
│   ║   • Any node can verify total supply at any time                                ║  │
│   ║                                                                                  ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                       NO DISCRETIONARY UNLOCKS                                   ║  │
│   ╠═════════════════════════════════════════════════════════════════════════════════╣  │
│   ║                                                                                  ║  │
│   ║   • All vesting schedules deterministic                                         ║  │
│   ║   • No entity can accelerate unlocks                                            ║  │
│   ║   • Foundation cannot access locked tokens early                                ║  │
│   ║   • Unlock events recorded on-chain                                             ║  │
│   ║   • Multi-sig cannot override vesting rules                                     ║  │
│   ║                                                                                  ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                         NO PRIVILEGED ACTORS                                     ║  │
│   ╠═════════════════════════════════════════════════════════════════════════════════╣  │
│   ║                                                                                  ║  │
│   ║   • Same reward rules apply to all participants                                 ║  │
│   ║   • Foundation validators follow same rules                                     ║  │
│   ║   • No preferential transaction ordering                                        ║  │
│   ║   • No reward boosts for insiders                                               ║  │
│   ║   • Early contributors vest same as others                                      ║  │
│   ║                                                                                  ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                   DETERMINISTIC SLASHING ENFORCEMENT                             ║  │
│   ╠═════════════════════════════════════════════════════════════════════════════════╣  │
│   ║                                                                                  ║  │
│   ║   • Slashing conditions defined in protocol                                     ║  │
│   ║   • Evidence-based (cryptographic proofs)                                       ║  │
│   ║   • Automatic execution (no human judgment)                                     ║  │
│   ║   • Penalty amounts fixed by severity                                           ║  │
│   ║   • Appeals process limited to evidence validity                                ║  │
│   ║                                                                                  ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
│   ╔═════════════════════════════════════════════════════════════════════════════════╗  │
│   ║                     GOVERNANCE CANNOT MINT TOKENS                                ║  │
│   ╠═════════════════════════════════════════════════════════════════════════════════╣  │
│   ║                                                                                  ║  │
│   ║   • Supply cap is outside governance scope                                      ║  │
│   ║   • No proposal type for increasing supply                                      ║  │
│   ║   • Consensus rules reject mint transactions                                    ║  │
│   ║   • Even 100% vote cannot create new MBO                                        ║  │
│   ║   • This is a constitutional constraint                                         ║  │
│   ║                                                                                  ║  │
│   ╚═════════════════════════════════════════════════════════════════════════════════╝  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Safety Guarantee Summary Table

| Guarantee | Enforcement Mechanism | Verification Method |
|-----------|----------------------|---------------------|
| **No Hidden Minting** | Hardcoded supply cap | Compare total_supply to 31,536,000 |
| **No Discretionary Unlocks** | Time-locked smart contracts | Verify unlock timestamps on-chain |
| **No Privileged Actors** | Protocol-level equality | Audit reward distribution code |
| **Deterministic Slashing** | Evidence-based automation | Review slashing transaction proofs |
| **Governance Cannot Mint** | Consensus rule rejection | Attempt mint → observe rejection |

### 9.3 Economic Attack Resistance

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ATTACK RESISTANCE                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   INFLATION ATTACK                                                                      │
│   ────────────────                                                                      │
│   Threat: Attacker tries to create tokens from nothing                                 │
│   Protection: Supply cap enforced at consensus level                                   │
│   Result: Invalid blocks rejected by all honest nodes                                  │
│                                                                                         │
│   REWARD MANIPULATION                                                                   │
│   ───────────────────                                                                   │
│   Threat: Attacker tries to claim more than fair share                                 │
│   Protection: Deterministic distribution formulas                                      │
│   Result: Invalid reward claims rejected by validation                                 │
│                                                                                         │
│   GOVERNANCE CAPTURE                                                                    │
│   ──────────────────                                                                    │
│   Threat: Attacker controls governance to change economics                             │
│   Protection: Supply/vesting outside governance scope                                  │
│   Result: Cannot vote to mint or unlock early                                          │
│                                                                                         │
│   SLASHING ABUSE                                                                        │
│   ──────────────                                                                        │
│   Threat: Attacker submits false slashing evidence                                     │
│   Protection: Cryptographic evidence requirement                                       │
│   Result: False evidence rejected, submitter penalized                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Appendix: Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         INCENTIVE DESIGN QUICK REFERENCE                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CANONICAL VALUES                                                                      │
│   ────────────────                                                                      │
│   Total Supply:        31,536,000 MBO (fixed)                                          │
│   Block Reward:        0.1 MBO (Year 1-5)                                              │
│   PoS/PoUW Split:      50% / 50%                                                       │
│   Halving Interval:    5 years (157,680,000 blocks)                                    │
│   Block Time:          1 second                                                        │
│                                                                                         │
│   PARTICIPANTS                                                                          │
│   ────────────                                                                          │
│   Validators:          Block production, 50% of rewards                                │
│   Delegators:          Passive staking, share of validator rewards                     │
│   Compute Providers:   GPU cycles, 50% of rewards                                      │
│   Developers:          Ecosystem grants (15% allocation)                               │
│                                                                                         │
│   FEE HANDLING                                                                          │
│   ────────────                                                                          │
│   Base Fee:            100% burned                                                     │
│   Priority Fee:        100% to proposer                                                │
│   Compute Fee:         100% to provider                                                │
│                                                                                         │
│   SLASHING (MAX)                                                                        │
│   ──────────────                                                                        │
│   Downtime:            1-5% stake                                                      │
│   Double Sign:         33% stake                                                       │
│   Invalid Compute:     1,000 MBO fixed                                                 │
│   Consensus Attack:    100% stake + ban                                                │
│                                                                                         │
│   SAFETY GUARANTEES                                                                     │
│   ─────────────────                                                                     │
│   ✓ No hidden minting                                                                  │
│   ✓ No discretionary unlocks                                                           │
│   ✓ No privileged actors                                                               │
│   ✓ Deterministic slashing                                                             │
│   ✓ Governance cannot mint                                                             │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [token_distribution.md](./token_distribution.md) | Allocation breakdown |
| [reward_mechanics.md](./reward_mechanics.md) | Detailed reward calculations |
| [vesting_model.md](./vesting_model.md) | Token unlock schedules |
| [governance_model.md](./governance_model.md) | Governance rules |

---

*This document defines the incentive design principles for Mbongo Chain. All incentive mechanisms are enforced by the protocol and recorded on-chain.*

