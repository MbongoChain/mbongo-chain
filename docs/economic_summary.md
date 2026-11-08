<!-- Verified against tokenomics.md -->
# Mbongo Chain — Economic Summary

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

> **Document Type:** Executive Economic Overview  
> **Last Updated:** November 2025  
> **Status:** Official Reference  
> **Audience:** Investors, Partners, Researchers, Validators, Builders

---

## Table of Contents

1. [Overview](#1-overview)
2. [Token Utility](#2-token-utility)
3. [Participants & Incentives](#3-participants--incentives)
4. [Monetary Policy](#4-monetary-policy)
5. [Reward Mechanics](#5-reward-mechanics)
6. [Distribution](#6-distribution)
7. [Why This Model Is Sustainable](#7-why-this-model-is-sustainable)
8. [References](#8-references)

---

## 1. Overview

```
╔═════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                         ║
║                         MBONGO CHAIN ECONOMIC MODEL                                     ║
║                                                                                         ║
╠═════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                         ║
║   FIXED SUPPLY                                                                          ║
║   ────────────────────────────────────────────────────────────────────────────────────  ║
║   Total Supply:        31,536,000 MBO                                                   ║
║   Inflation:           0% (fixed forever)                                               ║
║   Minting:             None after genesis emission schedule                             ║
║   Governance:          Cannot increase supply                                           ║
║                                                                                         ║
║   DEFLATIONARY MECHANICS                                                                ║
║   ────────────────────────────────────────────────────────────────────────────────────  ║
║   Base Fees:           100% burned                                                      ║
║   Slashed Stake:       100% burned                                                      ║
║   Net Effect:          Circulating supply decreases over time                           ║
║                                                                                         ║
║   CONSENSUS MODEL                                                                       ║
║   ────────────────────────────────────────────────────────────────────────────────────  ║
║   Block Time:          1 second                                                         ║
║   Blocks Per Year:     31,536,000                                                       ║
║   Reward Split:        50% PoS + 50% PoUW                                               ║
║   Halving:             Every 5 years (157,680,000 blocks)                               ║
║                                                                                         ║
║   INITIAL BLOCK REWARD (Year 1-5)                                                       ║
║   ────────────────────────────────────────────────────────────────────────────────────  ║
║   Total:               0.1 MBO per block                                                ║
║   To PoS:              0.05 MBO (50%)                                                   ║
║   To PoUW:             0.05 MBO (50%)                                                   ║
║                                                                                         ║
╚═════════════════════════════════════════════════════════════════════════════════════════╝
```

### Key Metrics

| Metric | Value |
|--------|-------|
| **Total Supply** | 31,536,000 MBO |
| **Inflation Rate** | 0% |
| **Block Time** | 1 second |
| **Annual Blocks** | 31,536,000 |
| **Initial Block Reward** | 0.1 MBO |
| **Halving Period** | 5 years |
| **PoS Share** | 50% |
| **PoUW Share** | 50% |

---

## 2. Token Utility

### What MBO Is Used For

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBO TOKEN UTILITY                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   GAS FEES                                                                      │  │
│   │   ────────                                                                      │  │
│   │   • Execution gas: Transaction processing                                      │  │
│   │   • Compute gas: PoUW task payments (GPU/TPU/CPU/FPGA/ASIC)                    │  │
│   │   • Storage gas: On-chain data storage                                         │  │
│   │   • Network gas: Message propagation                                           │  │
│   │                                                                                 │  │
│   │   Base fee → BURNED                                                            │  │
│   │   Priority fee → Validator/Provider                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   STAKING                                                                       │  │
│   │   ───────                                                                       │  │
│   │   • Validators: Minimum 50,000 MBO to participate in consensus                 │  │
│   │   • Delegators: Minimum 100 MBO to earn passive rewards                        │  │
│   │   • Security: Stake at risk (slashable for misbehavior)                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   COMPUTE TASKS (PoUW)                                                          │  │
│   │   ────────────────────                                                          │  │
│   │   • Compute providers earn MBO for valid compute receipts                      │  │
│   │   • Users pay MBO for AI/ML, rendering, ZK proofs, and batch jobs              │  │
│   │   • Compute fees support decentralized compute marketplace                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   GOVERNANCE & PROTOCOL UPGRADES                                                │  │
│   │   ──────────────────────────────                                                │  │
│   │   • Voting power from staked MBO                                               │  │
│   │   • Protocol parameter changes                                                 │  │
│   │   • Network upgrades                                                           │  │
│   │   • Note: Governance CANNOT increase supply                                    │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   SLASHING PENALTIES                                                            │  │
│   │   ──────────────────                                                            │  │
│   │   • Double-signing: 5% of stake                                                │  │
│   │   • Downtime: 0.5% of stake                                                    │  │
│   │   • Invalid compute: 1,000 MBO fixed                                           │  │
│   │   • ALL slashed MBO is BURNED (not redistributed)                              │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Participants & Incentives

### Ecosystem Participants

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PARTICIPANTS & INCENTIVES                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VALIDATORS (PoS)                               │ 50% of Block Rewards                │
│   ═══════════════                                │                                     │
│                                                  │                                     │
│   Role:                                          │  Reward Sources:                    │
│   • Stake MBO as collateral                      │  • Block reward share               │
│   • Propose new blocks                           │  • Priority fees                    │
│   • Attest to block validity                     │  • Proposer bonuses                 │
│   • Participate in consensus                     │                                     │
│                                                  │  Risk:                              │
│   Requirements:                                  │  • 5% slash for double-sign         │
│   • Minimum 50,000 MBO stake                     │  • 0.5% slash for downtime          │
│   • 21-day unbonding period                      │                                     │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   COMPUTE PROVIDERS (PoUW)                       │ 50% of Block Rewards                │
│   ════════════════════                           │                                     │
│                                                  │                                     │
│   Role:                                          │  Reward Sources:                    │
│   • Perform useful computation                   │  • Block reward share               │
│   • Submit verifiable receipts                   │  • Compute task fees                │
│   • Contribute to chain security                 │  • Priority fees                    │
│                                                  │                                     │
│   Requirements:                                  │  Risk:                              │
│   • Compute hardware (GPU/TPU/CPU/FPGA/ASIC)     │  • 1,000 MBO per invalid receipt    │
│   • Stake optional (improves priority)           │  • Reputation damage                │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   DELEGATORS                                     │ 20% of PoS Pool                     │
│   ══════════                                     │                                     │
│                                                  │                                     │
│   Role:                                          │  Reward Sources:                    │
│   • Delegate MBO to validators                   │  • Share of validator rewards       │
│   • Support validator infrastructure             │  • Proportional to delegation       │
│   • Participate in security                      │                                     │
│                                                  │  Risk:                              │
│   Requirements:                                  │  • Shared slashing with validator   │
│   • Minimum 100 MBO                              │                                     │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   DEVELOPERS                                     │ Ecosystem Allocation                │
│   ══════════                                     │                                     │
│                                                  │                                     │
│   Incentives:                                    │  Programs:                          │
│   • Ecosystem grants (milestone-based)           │  • Hackathons (quarterly)           │
│   • Bug bounties (up to 100,000 MBO)             │  • Retroactive funding              │
│   • Developer tools bounties                     │  • SDK/tooling grants               │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   COMMUNITY CONTRIBUTORS                         │ Community Allocation                │
│   ══════════════════════                         │                                     │
│                                                  │                                     │
│   Incentives:                                    │  Programs:                          │
│   • Ambassador rewards                           │  • Content creation bounties        │
│   • Documentation contributions                  │  • Regional community building      │
│   • Governance participation                     │  • Education initiatives            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### Summary Table

| Participant | Reward Source | Share | Requirements |
|-------------|---------------|-------|--------------|
| **Validators** | Block rewards + fees | 50% of total (80% of PoS) | 50,000 MBO stake |
| **Compute Providers** | Block rewards + fees | 50% of total | Compute hardware (GPU/TPU/CPU/FPGA/ASIC) |
| **Delegators** | Validator share | 20% of PoS pool | 100 MBO |
| **Developers** | Ecosystem grants | Milestone-based | Build on Mbongo |
| **Community** | Incentives pool | Per-epoch stream | Contribute value |

---

## 4. Monetary Policy

### Fixed Supply, Predictable Issuance

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MONETARY POLICY                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FIXED SUPPLY                                                                          │
│   ════════════                                                                          │
│                                                                                         │
│   • Total: 31,536,000 MBO (immutable)                                                  │
│   • No minting mechanism after emission schedule                                       │
│   • Governance cannot increase supply                                                  │
│   • Mathematical guarantee: Σ(all MBO) ≤ 31,536,000                                    │
│                                                                                         │
│                                                                                         │
│   DETERMINISTIC ISSUANCE (10-Year Schedule)                                             │
│   ═════════════════════════════════════════                                             │
│                                                                                         │
│   Years 1-5:   0.1 MBO/block   →  3,153,600 MBO/year                                   │
│   Years 6-10:  0.05 MBO/block  →  1,576,800 MBO/year                                   │
│   Years 11+:   0.025 MBO/block →  788,400 MBO/year (and halving continues)             │
│                                                                                         │
│                                                                                         │
│   BLOCK REWARD DECAY SCHEDULE                                                           │
│   ═══════════════════════════                                                           │
│                                                                                         │
│   Block                                                                                 │
│   Reward                                                                                │
│   (MBO)                                                                                 │
│                                                                                         │
│   0.100 ┤████████████████████████████████████████                                      │
│         │                                        │                                      │
│   0.050 ┤                                        └────────────────────                  │
│         │                                                            │                  │
│   0.025 ┤                                                            └────────────      │
│         │                                                                        │      │
│   0.012 ┤                                                                        └───   │
│         │                                                                               │
│         └────┬─────────┬─────────┬─────────┬─────────┬─────────┬─────────┬────▶ Years  │
│              5         10        15        20        25        30        35             │
│                                                                                         │
│                                                                                         │
│   ALL BURNS PERMANENTLY REDUCE SUPPLY                                                   │
│   ═══════════════════════════════════                                                   │
│                                                                                         │
│   • Base fees: Burned                                                                  │
│   • Slashed stake: Burned                                                              │
│   • Invalid compute penalties: Burned                                                  │
│   • Net effect: Deflationary over time                                                 │
│                                                                                         │
│                                                                                         │
│   NO DISCRETIONARY MONETARY CHANGES                                                     │
│   ═════════════════════════════════                                                     │
│                                                                                         │
│   ✗ No emergency minting                                                               │
│   ✗ No governance-triggered inflation                                                  │
│   ✗ No Foundation override                                                             │
│   ✗ No manual supply adjustments                                                       │
│                                                                                         │
│   The emission schedule is protocol-enforced and immutable.                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Reward Mechanics

### How Rewards Are Distributed

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         REWARD MECHANICS                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PoS REWARDS (50% of Block Rewards)                                                    │
│   ══════════════════════════════════                                                    │
│                                                                                         │
│   Calculation based on:                                                                │
│   • Stake weight: More stake → larger share                                            │
│   • Performance: Uptime and attestation rate                                           │
│   • Proposer bonus: Extra reward for block proposal                                    │
│                                                                                         │
│   Distribution within PoS pool:                                                        │
│   • 80% to validators                                                                  │
│   • 20% to delegators                                                                  │
│                                                                                         │
│   Formula:                                                                              │
│   validator_reward = (stake / total_stake) × pos_pool × 0.80 × performance             │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   PoUW REWARDS (50% of Block Rewards)                                                   │
│   ═══════════════════════════════════                                                   │
│                                                                                         │
│   Calculation based on:                                                                │
│   • Compute integrity: Valid receipts only                                             │
│   • Work units: Measured compute cycles (normalized across hardware)                   │
│   • Performance: Verification success rate                                             │
│                                                                                         │
│   Formula:                                                                              │
│   provider_reward = (work_units / total_work) × pouw_pool × quality_multiplier         │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   SLASHING PENALTIES                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   ┌───────────────────────────────────────────────────────────────────────────────┐   │
│   │                                                                               │   │
│   │   Offense              │ Penalty           │ Destination                      │   │
│   │   ─────────────────────┼───────────────────┼──────────────────────────────────│   │
│   │   Double-signing       │ 5% of stake       │ BURNED                           │   │
│   │   Downtime (>500 blks) │ 0.5% of stake     │ BURNED                           │   │
│   │   Invalid compute      │ 1,000 MBO fixed   │ BURNED                           │   │
│   │   Repeated offenses    │ Escalating        │ BURNED                           │   │
│   │                                                                               │   │
│   └───────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                         │
│   ─────────────────────────────────────────────────────────────────────────────────────│
│                                                                                         │
│   PRIORITY FEE ROUTING                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   Standard transactions:      Priority fee → Block proposer (validator)                │
│   Compute transactions:       Priority fee → Compute provider                          │
│   Oracle messages:            Priority fee → Attesters                                 │
│                                                                                         │
│   Base fee is ALWAYS burned regardless of transaction type.                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Distribution

### Token Allocation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         TOKEN DISTRIBUTION                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TOTAL SUPPLY: 31,536,000 MBO                                                          │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Category                  │ %     │ MBO Amount   │ Unlock                     │  │
│   │   ──────────────────────────┼───────┼──────────────┼────────────────────────────│  │
│   │   PoS Validators/Delegators │ 40%   │ 12,614,400   │ Per-block emission         │  │
│   │   PoUW Compute Providers    │ 20%   │ 6,307,200    │ Per-block emission         │  │
│   │   Ecosystem Grants          │ 15%   │ 4,730,400    │ Milestone-based            │  │
│   │   Foundation & Operations   │ 10%   │ 3,153,600    │ 4-year linear vesting      │  │
│   │   Community & Incentives    │ 10%   │ 3,153,600    │ Per-epoch stream           │  │
│   │   Early Contributors        │ 5%    │ 1,576,800    │ 4-year, 1-year cliff       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   UNLOCK MECHANISMS                                                                     │
│   ═════════════════                                                                     │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   VALIDATOR SET (60% of total)                                                  │  │
│   │   • PoS + PoUW combined                                                        │  │
│   │   • Unlocked per block as rewards                                              │  │
│   │   • Immediate availability                                                     │  │
│   │                                                                                 │  │
│   │   TEAM/FOUNDATION (10%)                                                         │  │
│   │   • 4-year linear vesting                                                      │  │
│   │   • No cliff (gradual release)                                                 │  │
│   │   • Time-locked smart contract                                                 │  │
│   │                                                                                 │  │
│   │   ECOSYSTEM/COMMUNITY (25%)                                                     │  │
│   │   • Grants: Milestone-based                                                    │  │
│   │   • Community: Per-epoch stream                                                │  │
│   │   • Multi-sig controlled                                                       │  │
│   │                                                                                 │  │
│   │   EARLY CONTRIBUTORS (5%)                                                       │  │
│   │   • 1-year cliff                                                               │  │
│   │   • 4-year total vesting                                                       │  │
│   │   • Linear monthly after cliff                                                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   ALL UNLOCKS ARE LINEAR/NON-MANUAL                                                     │
│   ════════════════════════════════                                                      │
│                                                                                         │
│   • No discretionary releases                                                          │
│   • No manual override capability                                                      │
│   • Enforced by protocol/smart contract                                                │
│   • Transparent and on-chain verifiable                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Why This Model Is Sustainable

### Long-Term Economic Sustainability

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SUSTAINABILITY FACTORS                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ PREDICTABLE SUPPLY                                                          │  │
│   │                                                                                 │  │
│   │     • Total supply known: 31,536,000 MBO                                       │  │
│   │     • Emission schedule deterministic                                          │  │
│   │     • No surprises or arbitrary changes                                        │  │
│   │     • Enables long-term economic planning                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ STRONG DEFLATIONARY MECHANICS                                               │  │
│   │                                                                                 │  │
│   │     • Base fees burned every transaction                                       │  │
│   │     • Slashing penalties burned                                                │  │
│   │     • High usage = more burns = less supply                                    │  │
│   │     • Network success directly benefits holders                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ GROWING DEMAND FOR COMPUTE                                                  │  │
│   │                                                                                 │  │
│   │     • AI/ML compute demand growing 10x every 2 years                           │  │
│   │     • Mbongo captures this demand on-chain                                     │  │
│   │     • More compute = more MBO needed                                           │  │
│   │     • Growing demand + fixed supply = value appreciation                       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ HETEROGENEOUS COMPUTE MARKET (PoUW)                                         │  │
│   │                                                                                 │  │
│   │     PoUW is designed as a heterogeneous compute market. Any hardware class     │  │
│   │     (GPU, TPU, CPU, FPGA, ASIC, or future accelerators) can participate,       │  │
│   │     provided it runs deterministic jobs and submits verifiable proofs.         │  │
│   │                                                                                 │  │
│   │     This ensures maximum supply-side participation and hardware diversity,     │  │
│   │     while maintaining strict verification and economic guarantees.             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ HYBRID PoS–PoUW STRENGTHENS SECURITY                                        │  │
│   │                                                                                 │  │
│   │     • Two independent security layers                                          │  │
│   │     • Attack requires controlling BOTH stake AND compute                       │  │
│   │     • Economically rational to participate honestly                            │  │
│   │     • Slashing makes attacks costly                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ NO HIDDEN MINTING OR DISCRETIONARY OVERRIDES                                │  │
│   │                                                                                 │  │
│   │     • No admin keys can mint MBO                                               │  │
│   │     • Governance cannot increase supply                                        │  │
│   │     • No Foundation emergency powers over economics                            │  │
│   │     • Code is law—no exceptions                                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ ALL ECONOMIC ACTIONS TRANSPARENT & ON-CHAIN                                 │  │
│   │                                                                                 │  │
│   │     • Every reward distributed on-chain                                        │  │
│   │     • Every fee burned on-chain                                                │  │
│   │     • Every unlock verifiable on-chain                                         │  │
│   │     • Full transparency for auditors and researchers                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### Sustainability Summary

| Factor | Mechanism | Benefit |
|--------|-----------|---------|
| **Predictable Supply** | Fixed cap, known schedule | Long-term planning |
| **Deflationary** | Fee burning, slashing | Increasing scarcity |
| **Growing Demand** | AI compute marketplace | Utility-driven value |
| **Hybrid Security** | PoS + PoUW | Attack resistance |
| **No Hidden Minting** | Protocol-enforced limits | Trust and transparency |
| **On-Chain Transparency** | All actions verifiable | Audit-friendly |

---

## 8. References

### Detailed Documentation

For comprehensive information on each topic, refer to:

| Document | Description |
|----------|-------------|
| **[token_intro.md](./token_intro.md)** | Introduction to MBO token |
| **[monetary_policy.md](./monetary_policy.md)** | Fixed supply, halving schedule, emission model |
| **[reward_mechanics.md](./reward_mechanics.md)** | PoS/PoUW reward calculations, fee distribution |
| **[token_distribution.md](./token_distribution.md)** | Allocation breakdown, vesting schedules |
| **[vesting_model.md](./vesting_model.md)** | Unlock mechanics, cliff periods |
| **[incentive_design.md](./incentive_design.md)** | Participant incentives, slashing model |
| **[compute_value.md](./compute_value.md)** | Heterogeneous compute economics, PoUW value |
| **[utility_value.md](./utility_value.md)** | MBO token utility across ecosystem |
| **[economic_security.md](./economic_security.md)** | Security model, attack economics |
| **[supply_schedule.md](./supply_schedule.md)** | 50-year emission projection |
| **[staking_model.md](./staking_model.md)** | Validator/delegator mechanics |
| **[fee_model.md](./fee_model.md)** | Gas structure, burn mechanics |
| **[governance_model.md](./governance_model.md)** | Governance scope and limits |

### Canonical Source of Truth

All economic parameters are defined in:

> **[spec/tokenomics.md](../spec/tokenomics.md)** — Authoritative tokenomics specification

---

## Quick Reference Card

```
╔═════════════════════════════════════════════════════════════════════════════════════════╗
║                         MBONGO ECONOMICS — QUICK REFERENCE                              ║
╠═════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                         ║
║   SUPPLY                           REWARDS                                              ║
║   ──────                           ───────                                              ║
║   Total:     31,536,000 MBO        Block:      0.1 MBO (Year 1-5)                       ║
║   Inflation: 0%                    PoS:        50% (0.05 MBO)                           ║
║   Burns:     Base fees + slashing  PoUW:       50% (0.05 MBO)                           ║
║                                    Halving:    Every 5 years                            ║
║                                                                                         ║
║   STAKING                          SLASHING                                             ║
║   ───────                          ────────                                             ║
║   Validator: 50,000 MBO min        Double-sign: 5% stake                                ║
║   Delegator: 100 MBO min           Downtime:    0.5% stake                              ║
║   Unbonding: 21 days               Invalid:     1,000 MBO                               ║
║                                    Destination: BURNED                                  ║
║                                                                                         ║
║   CONSENSUS                        VALUE DRIVERS                                        ║
║   ─────────                        ─────────────                                        ║
║   Block time: 1 second             Fixed supply + burns                                 ║
║   Blocks/yr:  31,536,000           Growing AI demand                                    ║
║   Security:   PoS + PoUW           Utility (gas, stake, compute)                        ║
║   Finality:   Economic             Transparent economics                                ║
║                                                                                         ║
╚═════════════════════════════════════════════════════════════════════════════════════════╝
```

---

*This document provides a high-level summary of Mbongo Chain economics. For detailed specifications, refer to the linked documentation.*

