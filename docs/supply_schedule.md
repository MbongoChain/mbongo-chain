<!-- Verified against tokenomics.md -->
# Mbongo Chain — Supply Schedule

> **Document Type:** Supply & Emission Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Fixed Supply Summary](#2-fixed-supply-summary)
3. [Emission Schedule Overview](#3-emission-schedule-overview)
4. [Detailed Halving Table (50-Year Projection)](#4-detailed-halving-table-50-year-projection)
5. [Reward Split Alignment](#5-reward-split-alignment)
6. [Sustainability Notes](#6-sustainability-notes)
7. [Full Emission Curve](#7-full-emission-curve)
8. [Economic Safety Guarantees](#8-economic-safety-guarantees)

---

## 1. Purpose of This Document

This document defines the complete supply schedule for Mbongo (MBO) tokens. It serves as the authoritative reference for understanding how MBO enters circulation over time.

### 1.1 Supply Philosophy

Mbongo Chain adopts a **sound money** approach to token economics:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SUPPLY PHILOSOPHY                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FIXED CAP                                                                             │
│   ─────────                                                                             │
│   The total supply of MBO is mathematically capped at 31,536,000 tokens.               │
│   This number can never increase. No entity—including governance, the                  │
│   Foundation, or protocol upgrades—can create additional MBO.                          │
│                                                                                         │
│   PREDICTABLE ISSUANCE                                                                  │
│   ────────────────────                                                                  │
│   Every MBO that will ever exist is created according to a deterministic               │
│   schedule. Anyone can calculate the exact supply at any future block.                 │
│   No surprises, no emergency minting, no hidden inflation.                             │
│                                                                                         │
│   HALVING SCHEDULE                                                                      │
│   ────────────────                                                                      │
│   Block rewards decrease by 50% every 5 years. This creates:                           │
│   • Predictable scarcity over time                                                     │
│   • Early participant rewards for bootstrapping risk                                   │
│   • Multi-decade emission runway                                                       │
│   • Transition to fee-based security model                                             │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Design Rationale

| Principle | Implementation | Benefit |
|-----------|---------------|---------|
| **Scarcity** | Fixed 31.5M cap | Value preservation |
| **Predictability** | Deterministic formula | Economic planning |
| **Fairness** | Public schedule | No insider advantages |
| **Sustainability** | Gradual reduction | Long-term viability |

---

## 2. Fixed Supply Summary

```
╔═════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                         ║
║                            FIXED SUPPLY: 31,536,000 MBO                                 ║
║                                                                                         ║
╠═════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                         ║
║   ┌───────────────────────────────────────────────────────────────────────────────┐    ║
║   │                                                                               │    ║
║   │   ✓ TOTAL SUPPLY: 31,536,000 MBO                                             │    ║
║   │     • Equals seconds in a year (365.25 × 24 × 60 × 60)                       │    ║
║   │     • Symbolic: "One MBO per second of opportunity"                          │    ║
║   │     • Immutable at protocol level                                            │    ║
║   │                                                                               │    ║
║   │   ✓ NO INFLATION                                                             │    ║
║   │     • Zero new tokens beyond emission schedule                               │    ║
║   │     • No emergency minting mechanism                                         │    ║
║   │     • No "reserve" that can be unlocked                                      │    ║
║   │                                                                               │    ║
║   │   ✓ NO MINTING                                                               │    ║
║   │     • No admin keys can create tokens                                        │    ║
║   │     • No smart contract can mint                                             │    ║
║   │     • Consensus rejects invalid supply                                       │    ║
║   │                                                                               │    ║
║   │   ✓ NO DISCRETIONARY EXPANSION                                               │    ║
║   │     • Foundation cannot increase supply                                      │    ║
║   │     • No "treasury mints" or "ecosystem mints"                               │    ║
║   │     • All tokens come from block rewards only                                │    ║
║   │                                                                               │    ║
║   │   ✓ GOVERNANCE CANNOT MODIFY SUPPLY                                          │    ║
║   │     • Supply cap outside governance scope                                    │    ║
║   │     • Even 100% vote cannot create MBO                                       │    ║
║   │     • Constitutional constraint                                              │    ║
║   │                                                                               │    ║
║   └───────────────────────────────────────────────────────────────────────────────┘    ║
║                                                                                         ║
╚═════════════════════════════════════════════════════════════════════════════════════════╝
```

### 2.1 Supply Origin

All 31,536,000 MBO are created exclusively through block rewards. There is no:
- Pre-mine
- ICO allocation
- Airdrop from thin air
- Treasury mint

Every token in existence can be traced back to a specific block reward.

### 2.2 Supply Verification

Anyone can verify the total supply at any block height:

```
total_supply(height) = Σ block_reward(h) for h = 0 to height

Where:
  block_reward(h) = 0.1 × (0.5 ^ floor(h / 157,680,000))
```

---

## 3. Emission Schedule Overview

### 3.1 Core Parameters

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         EMISSION PARAMETERS                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   BLOCK TIME                                                                            │
│   ──────────                                                                            │
│   1 second per block                                                                   │
│                                                                                         │
│   BLOCKS PER YEAR                                                                       │
│   ────────────────                                                                      │
│   31,536,000 blocks (365.25 days × 24 hours × 60 min × 60 sec)                         │
│                                                                                         │
│   INITIAL BLOCK REWARD (Year 1–5)                                                       │
│   ───────────────────────────────                                                       │
│   0.1 MBO per block                                                                    │
│                                                                                         │
│   HALVING INTERVAL                                                                      │
│   ────────────────                                                                      │
│   Every 5 years (157,680,000 blocks)                                                   │
│                                                                                         │
│   HALVING FORMULA                                                                       │
│   ───────────────                                                                       │
│   block_reward(height) = INITIAL_REWARD × (0.5 ^ (height / HALVING_INTERVAL))         │
│                                                                                         │
│   Where:                                                                                │
│   • INITIAL_REWARD = 0.1 MBO                                                           │
│   • HALVING_INTERVAL = 157,680,000 blocks                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 First 25 Years Emission Table

```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                        EMISSION SCHEDULE (YEAR 1–25)                                                │
├──────────┬─────────────────────┬───────────────┬──────────────────┬───────────────────┬─────────────────────────────┤
│  Period  │       Blocks        │ Block Reward  │ Annual Emission  │ Cumulative Issued │     Remaining Supply        │
├──────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────┤
│          │                     │               │                  │                   │                             │
│  Year 1  │      31,536,000     │   0.1 MBO     │   3,153,600 MBO  │    3,153,600 MBO  │        28,382,400 MBO       │
│  Year 2  │      63,072,000     │   0.1 MBO     │   3,153,600 MBO  │    6,307,200 MBO  │        25,228,800 MBO       │
│  Year 3  │      94,608,000     │   0.1 MBO     │   3,153,600 MBO  │    9,460,800 MBO  │        22,075,200 MBO       │
│  Year 4  │     126,144,000     │   0.1 MBO     │   3,153,600 MBO  │   12,614,400 MBO  │        18,921,600 MBO       │
│  Year 5  │     157,680,000     │   0.1 MBO     │   3,153,600 MBO  │   15,768,000 MBO  │        15,768,000 MBO       │
│          │                     │               │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │  HALVING #1   │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │               │                  │                   │                             │
│  Year 6  │     189,216,000     │   0.05 MBO    │   1,576,800 MBO  │   17,344,800 MBO  │        14,191,200 MBO       │
│  Year 7  │     220,752,000     │   0.05 MBO    │   1,576,800 MBO  │   18,921,600 MBO  │        12,614,400 MBO       │
│  Year 8  │     252,288,000     │   0.05 MBO    │   1,576,800 MBO  │   20,498,400 MBO  │        11,037,600 MBO       │
│  Year 9  │     283,824,000     │   0.05 MBO    │   1,576,800 MBO  │   22,075,200 MBO  │         9,460,800 MBO       │
│  Year 10 │     315,360,000     │   0.05 MBO    │   1,576,800 MBO  │   23,652,000 MBO  │         7,884,000 MBO       │
│          │                     │               │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │  HALVING #2   │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │               │                  │                   │                             │
│  Year 11 │     346,896,000     │   0.025 MBO   │     788,400 MBO  │   24,440,400 MBO  │         7,095,600 MBO       │
│  Year 12 │     378,432,000     │   0.025 MBO   │     788,400 MBO  │   25,228,800 MBO  │         6,307,200 MBO       │
│  Year 13 │     409,968,000     │   0.025 MBO   │     788,400 MBO  │   26,017,200 MBO  │         5,518,800 MBO       │
│  Year 14 │     441,504,000     │   0.025 MBO   │     788,400 MBO  │   26,805,600 MBO  │         4,730,400 MBO       │
│  Year 15 │     473,040,000     │   0.025 MBO   │     788,400 MBO  │   27,594,000 MBO  │         3,942,000 MBO       │
│          │                     │               │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │  HALVING #3   │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │               │                  │                   │                             │
│  Year 16 │     504,576,000     │   0.0125 MBO  │     394,200 MBO  │   27,988,200 MBO  │         3,547,800 MBO       │
│  Year 17 │     536,112,000     │   0.0125 MBO  │     394,200 MBO  │   28,382,400 MBO  │         3,153,600 MBO       │
│  Year 18 │     567,648,000     │   0.0125 MBO  │     394,200 MBO  │   28,776,600 MBO  │         2,759,400 MBO       │
│  Year 19 │     599,184,000     │   0.0125 MBO  │     394,200 MBO  │   29,170,800 MBO  │         2,365,200 MBO       │
│  Year 20 │     630,720,000     │   0.0125 MBO  │     394,200 MBO  │   29,565,000 MBO  │         1,971,000 MBO       │
│          │                     │               │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │  HALVING #4   │                  │                   │                             │
│  ────────┼─────────────────────┼───────────────┼──────────────────┼───────────────────┼─────────────────────────────│
│          │                     │               │                  │                   │                             │
│  Year 21 │     662,256,000     │  0.00625 MBO  │     197,100 MBO  │   29,762,100 MBO  │         1,773,900 MBO       │
│  Year 22 │     693,792,000     │  0.00625 MBO  │     197,100 MBO  │   29,959,200 MBO  │         1,576,800 MBO       │
│  Year 23 │     725,328,000     │  0.00625 MBO  │     197,100 MBO  │   30,156,300 MBO  │         1,379,700 MBO       │
│  Year 24 │     756,864,000     │  0.00625 MBO  │     197,100 MBO  │   30,353,400 MBO  │         1,182,600 MBO       │
│  Year 25 │     788,400,000     │  0.00625 MBO  │     197,100 MBO  │   30,550,500 MBO  │           985,500 MBO       │
│          │                     │               │                  │                   │                             │
└──────────┴─────────────────────┴───────────────┴──────────────────┴───────────────────┴─────────────────────────────┘
```

### 3.3 Emission Milestones

| Milestone | Year | Cumulative Supply | % of Total |
|-----------|------|-------------------|------------|
| **First Halving** | 5 | 15,768,000 MBO | 50.0% |
| **Second Halving** | 10 | 23,652,000 MBO | 75.0% |
| **Third Halving** | 15 | 27,594,000 MBO | 87.5% |
| **Fourth Halving** | 20 | 29,565,000 MBO | 93.75% |
| **Fifth Halving** | 25 | 30,550,500 MBO | 96.875% |
| **~99% Issued** | ~30 | ~31,200,000 MBO | ~98.9% |
| **Approaching Cap** | 50+ | ~31,536,000 MBO | ~100% |

---

## 4. Detailed Halving Table (50-Year Projection)

### 4.1 Complete 50-Year Schedule

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                              50-YEAR EMISSION PROJECTION                                                       │
├─────────────┬──────────────────┬────────────────────┬─────────────────────┬────────────────────┬──────────────────────────────┤
│   Period    │   Block Range    │   Block Reward     │  Period Emission    │ Cumulative Issued  │      % of Total Supply       │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │                  │                    │                     │                    │                              │
│  YEARS 1–5  │        0 –       │                    │                     │                    │                              │
│             │  157,679,999     │    0.1 MBO         │   15,768,000 MBO    │   15,768,000 MBO   │           50.00%             │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │  157,680,000 –   │                    │                     │                    │                              │
│  YEARS 6–10 │  315,359,999     │    0.05 MBO        │    7,884,000 MBO    │   23,652,000 MBO   │           75.00%             │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │  315,360,000 –   │                    │                     │                    │                              │
│ YEARS 11–15 │  473,039,999     │    0.025 MBO       │    3,942,000 MBO    │   27,594,000 MBO   │           87.50%             │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │  473,040,000 –   │                    │                     │                    │                              │
│ YEARS 16–20 │  630,719,999     │    0.0125 MBO      │    1,971,000 MBO    │   29,565,000 MBO   │           93.75%             │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │  630,720,000 –   │                    │                     │                    │                              │
│ YEARS 21–25 │  788,399,999     │    0.00625 MBO     │      985,500 MBO    │   30,550,500 MBO   │           96.875%            │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │  788,400,000 –   │                    │                     │                    │                              │
│ YEARS 26–30 │  946,079,999     │    0.003125 MBO    │      492,750 MBO    │   31,043,250 MBO   │           98.4375%           │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │  946,080,000 –   │                    │                     │                    │                              │
│ YEARS 31–35 │ 1,103,759,999    │    0.0015625 MBO   │      246,375 MBO    │   31,289,625 MBO   │           99.21875%          │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │ 1,103,760,000 –  │                    │                     │                    │                              │
│ YEARS 36–40 │ 1,261,439,999    │   0.00078125 MBO   │      123,187.5 MBO  │  31,412,812.5 MBO  │          99.609375%          │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │ 1,261,440,000 –  │                    │                     │                    │                              │
│ YEARS 41–45 │ 1,419,119,999    │  0.000390625 MBO   │     61,593.75 MBO   │ 31,474,406.25 MBO  │         99.8046875%          │
│             │                  │                    │                     │                    │                              │
├─────────────┼──────────────────┼────────────────────┼─────────────────────┼────────────────────┼──────────────────────────────┤
│             │ 1,419,120,000 –  │                    │                     │                    │                              │
│ YEARS 46–50 │ 1,576,799,999    │  0.000195313 MBO   │    30,796.875 MBO   │ 31,505,203.125 MBO │        99.90234375%          │
│             │                  │                    │                     │                    │                              │
└─────────────┴──────────────────┴────────────────────┴─────────────────────┴────────────────────┴──────────────────────────────┘
```

### 4.2 Halving Summary

| Halving # | Year | Block Height | Block Reward | Cumulative % |
|-----------|------|--------------|--------------|--------------|
| Genesis | 0 | 0 | 0.1 MBO | 0% |
| **1st** | 5 | 157,680,000 | 0.05 MBO | 50% |
| **2nd** | 10 | 315,360,000 | 0.025 MBO | 75% |
| **3rd** | 15 | 473,040,000 | 0.0125 MBO | 87.5% |
| **4th** | 20 | 630,720,000 | 0.00625 MBO | 93.75% |
| **5th** | 25 | 788,400,000 | 0.003125 MBO | 96.875% |
| **6th** | 30 | 946,080,000 | 0.0015625 MBO | 98.4375% |
| **7th** | 35 | 1,103,760,000 | 0.00078125 MBO | 99.21875% |
| **8th** | 40 | 1,261,440,000 | 0.000390625 MBO | 99.609375% |
| **9th** | 45 | 1,419,120,000 | 0.000195313 MBO | 99.8046875% |
| **10th** | 50 | 1,576,800,000 | 0.000097656 MBO | 99.90234375% |

### 4.3 Asymptotic Approach to Cap

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SUPPLY ASYMPTOTIC BEHAVIOR                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   The supply approaches 31,536,000 MBO but never exceeds it.                           │
│                                                                                         │
│   Mathematical property:                                                                │
│   ───────────────────────                                                               │
│   Σ (0.1 × 0.5^n × 157,680,000) for n = 0 to ∞                                         │
│   = 0.1 × 157,680,000 × Σ (0.5^n)                                                      │
│   = 0.1 × 157,680,000 × 2                                                              │
│   = 31,536,000 MBO                                                                     │
│                                                                                         │
│   This geometric series converges exactly to the cap.                                  │
│                                                                                         │
│   Practical implication:                                                                │
│   ──────────────────────                                                                │
│   • By year 50: 99.9% issued                                                           │
│   • By year 100: 99.9999% issued                                                       │
│   • Rewards become negligible but never zero                                           │
│   • Fee revenue replaces block rewards as primary incentive                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Reward Split Alignment

### 5.1 Block Reward Distribution

Every block reward is split equally between Proof-of-Stake and Proof-of-Useful-Work participants:

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         REWARD SPLIT: 50% PoS / 50% PoUW                                │
└─────────────────────────────────────────────────────────────────────────────────────────┘

                              ┌─────────────────────────┐
                              │                         │
                              │      BLOCK REWARD       │
                              │                         │
                              │   0.1 MBO (Year 1-5)    │
                              │                         │
                              └────────────┬────────────┘
                                           │
                    ┌──────────────────────┴──────────────────────┐
                    │                                             │
                    ▼                                             ▼
       ┌────────────────────────────┐            ┌────────────────────────────┐
       │                            │            │                            │
       │     PoS VALIDATORS         │            │     PoUW COMPUTE           │
       │       & DELEGATORS         │            │       PROVIDERS            │
       │                            │            │                            │
       │         50%                │            │         50%                │
       │                            │            │                            │
       │   ┌────────────────────┐   │            │   ┌────────────────────┐   │
       │   │                    │   │            │   │                    │   │
       │   │  0.05 MBO/block    │   │            │   │  0.05 MBO/block    │   │
       │   │                    │   │            │   │                    │   │
       │   └────────────────────┘   │            │   └────────────────────┘   │
       │                            │            │                            │
       │   Distribution:            │            │   Distribution:            │
       │   • Block proposer         │            │   • Work units             │
       │   • Attesters              │            │   • Quality score          │
       │   • Delegators (indirect)  │            │   • Verification rate      │
       │                            │            │                            │
       └────────────────────────────┘            └────────────────────────────┘
```

### 5.2 Annual Reward Allocation (Year 1)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ANNUAL REWARD ALLOCATION (YEAR 1)                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Total Annual Emission: 3,153,600 MBO                                                 │
│                                                                                         │
│   ┌───────────────────────────────────────────────────────────────────────────────┐    │
│   │                                                                               │    │
│   │                     3,153,600 MBO / year                                      │    │
│   │   ╔═══════════════════════════════════════════════════════════════════════╗   │    │
│   │   ║                                                                       ║   │    │
│   │   ║    PoS (50%)              │           PoUW (50%)                      ║   │    │
│   │   ║    1,576,800 MBO          │           1,576,800 MBO                   ║   │    │
│   │   ║                           │                                           ║   │    │
│   │   ╚═══════════════════════════════════════════════════════════════════════╝   │    │
│   │                                                                               │    │
│   └───────────────────────────────────────────────────────────────────────────────┘    │
│                                                                                         │
│   Per-Block Breakdown:                                                                  │
│   ────────────────────                                                                  │
│   Total:     0.1 MBO                                                                   │
│   PoS:       0.05 MBO (validators + delegators)                                        │
│   PoUW:      0.05 MBO (compute providers)                                              │
│                                                                                         │
│   Daily Breakdown:                                                                      │
│   ────────────────                                                                      │
│   Blocks/day:  86,400                                                                  │
│   Total:       8,640 MBO/day                                                           │
│   PoS:         4,320 MBO/day                                                           │
│   PoUW:        4,320 MBO/day                                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Split Rationale

| Aspect | Benefit |
|--------|---------|
| **Security Balance** | Neither stake nor compute alone controls rewards |
| **Decentralization** | Multiple paths to participation |
| **Attack Resistance** | Must control both stake AND compute to dominate |
| **Innovation Incentive** | Compute providers rewarded for useful work |
| **Capital Efficiency** | Stakers don't need hardware, providers don't need capital |

---

## 6. Sustainability Notes

### 6.1 Why 5-Year Halvings Maintain Security

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         HALVING & SECURITY                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   GRADUAL TRANSITION                                                                    │
│   ──────────────────                                                                    │
│   5-year intervals provide ample time for the network to grow transaction              │
│   volume. As block rewards decrease, fee revenue increases to compensate.              │
│                                                                                         │
│   SECURITY BUDGET EVOLUTION                                                             │
│   ─────────────────────────                                                             │
│                                                                                         │
│   Year 1-5:    Security = High rewards + Low fees                                      │
│   Year 6-10:   Security = Medium rewards + Growing fees                                │
│   Year 11-15:  Security = Low rewards + Significant fees                               │
│   Year 16+:    Security = Minimal rewards + Fee-dominant                               │
│                                                                                         │
│   WHY THIS WORKS                                                                        │
│   ──────────────                                                                        │
│   • Network utility grows over time                                                    │
│   • More users = more transactions = more fees                                         │
│   • Fee burning creates deflationary pressure                                          │
│   • Token appreciation compensates for lower nominal rewards                           │
│   • Validators optimize operations over time                                           │
│                                                                                         │
│   HISTORICAL PRECEDENT                                                                  │
│   ────────────────────                                                                  │
│   Bitcoin has maintained security through 4 halvings. Mbongo's longer                  │
│   intervals and dual reward mechanism (PoS + PoUW) provide even more                   │
│   stability during transitions.                                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Why Fixed Cap Prevents Dilution

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FIXED CAP & DILUTION PROTECTION                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ZERO INFLATION GUARANTEE                                                              │
│   ────────────────────────                                                              │
│   • Total supply NEVER exceeds 31,536,000 MBO                                          │
│   • No mechanism exists to create additional tokens                                    │
│   • Existing holders cannot be diluted by future minting                               │
│                                                                                         │
│   COMPARISON TO INFLATIONARY SYSTEMS                                                    │
│   ──────────────────────────────────                                                    │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   System Type        │  Your Share Over Time  │  Purchasing Power               │  │
│   ├──────────────────────┼────────────────────────┼─────────────────────────────────┤  │
│   │   Inflationary (5%)  │  Decreases yearly      │  Erodes continuously            │  │
│   │   Fixed Supply       │  Stays constant        │  Preserved or grows             │  │
│   │   Deflationary       │  Increases (via burn)  │  Grows with network usage       │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   MBO IS DEFLATIONARY                                                                   │
│   ───────────────────                                                                   │
│   With fee burning, circulating supply can actually DECREASE over time,                │
│   making each remaining MBO more valuable.                                             │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Why Deterministic Emission Matters

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DETERMINISTIC EMISSION BENEFITS                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   FOR INVESTORS                                                                         │
│   ─────────────                                                                         │
│   • Calculate exact inflation rate at any future date                                  │
│   • No risk of surprise dilution events                                                │
│   • Model token economics with certainty                                               │
│   • Plan long-term positions based on known schedule                                   │
│                                                                                         │
│   FOR BUILDERS                                                                          │
│   ────────────                                                                          │
│   • Design applications knowing future supply                                          │
│   • Price services based on predictable economics                                      │
│   • Build sustainable business models                                                  │
│   • Plan around halving events                                                         │
│                                                                                         │
│   FOR COMPUTE PROVIDERS                                                                 │
│   ─────────────────────                                                                 │
│   • Calculate ROI on hardware investments                                              │
│   • Plan capacity based on reward projections                                          │
│   • Understand long-term revenue potential                                             │
│   • Factor halvings into business planning                                             │
│                                                                                         │
│   FOR VALIDATORS                                                                        │
│   ──────────────                                                                        │
│   • Know exactly how rewards change over time                                          │
│   • Plan infrastructure investments                                                    │
│   • Set commission rates appropriately                                                 │
│   • Communicate clearly with delegators                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.4 PoS + PoUW Viability as Rewards Decline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         LONG-TERM PoS + PoUW VIABILITY                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VALIDATORS (PoS) — Revenue Sources Over Time                                         │
│   ─────────────────────────────────────────────                                         │
│                                                                                         │
│   Year 1-10:   Block Rewards ████████████████████████████░░░░░░ 70%                    │
│                Tx Fees       ██████████░░░░░░░░░░░░░░░░░░░░░░░░ 25%                    │
│                MEV (future)  ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  5%                    │
│                                                                                         │
│   Year 20+:    Block Rewards ██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 15%                    │
│                Tx Fees       ████████████████████████████░░░░░░ 70%                    │
│                MEV (future)  ██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 15%                    │
│                                                                                         │
│   COMPUTE PROVIDERS (PoUW) — Revenue Sources Over Time                                 │
│   ────────────────────────────────────────────────────                                  │
│                                                                                         │
│   Year 1-10:   Block Rewards ██████████████████████████░░░░░░░░ 65%                    │
│                Compute Fees  ██████████████░░░░░░░░░░░░░░░░░░░░ 35%                    │
│                                                                                         │
│   Year 20+:    Block Rewards ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 10%                    │
│                Compute Fees  ████████████████████████████████░░ 80%                    │
│                Premium Tasks ████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 10%                    │
│                                                                                         │
│   WHY THIS REMAINS VIABLE                                                               │
│   ───────────────────────                                                               │
│   • Compute demand grows exponentially (AI, ML, rendering)                             │
│   • Network effects attract more users and transactions                                │
│   • Premium compute markets develop for specialized tasks                              │
│   • Token appreciation compensates for nominal reward decrease                         │
│   • Operational efficiency improves over time                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 7. Full Emission Curve

### 7.1 50-Year Supply Curve

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         50-YEAR SUPPLY EMISSION CURVE                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Supply                                                                                │
│   (Million                                                                              │
│    MBO)    31.5M ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─●═════ CAP        │
│      │                                                          ●═══●                   │
│   30 ┤                                                     ●════●                       │
│      │                                                ●════●                            │
│      │                                           ●════●                                 │
│   25 ┤                                      ●════●                                      │
│      │                                 ●════●                                           │
│      │                            ●════●                                                │
│   20 ┤                       ●════●                                                     │
│      │                  ●════●                                                          │
│      │             ●════●                                                               │
│   15 ┤        ●════●                                                                    │
│      │   ●════●                                                                         │
│      │  ●●                                                                              │
│   10 ┤ ●                                                                                │
│      │●                                                                                 │
│      │                                                                                  │
│    5 ┤                                                                                  │
│      │                                                                                  │
│      │                                                                                  │
│    0 └──┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────┬─────▶ Years            │
│         5    10    15    20    25    30    35    40    45    50                         │
│         │     │     │     │     │     │     │     │     │     │                         │
│        50%  75%  87.5% 93.8% 96.9% 98.4% 99.2% 99.6% 99.8% 99.9%                        │
│                                                                                         │
│   Key:                                                                                  │
│   ●════●  Supply growth during period                                                  │
│   ═════   Cap line (31,536,000 MBO)                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Block Reward Step Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         BLOCK REWARD HALVING STEPS                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Block                                                                                 │
│   Reward                                                                                │
│   (MBO)                                                                                 │
│                                                                                         │
│   0.100 ┤████████████████████████████████▌                                             │
│         │                                │                                              │
│   0.050 ┤                                ▼████████████████████████████▌                │
│         │                                                             │                 │
│   0.025 ┤                                                             ▼█████████████▌  │
│         │                                                                           │   │
│   0.0125┤                                                                           ▼██▌│
│         │                                                                              ││
│   0.006 ┤                                                                              ▼│
│         │                                                                               │
│         └─────────────┬─────────────┬─────────────┬─────────────┬─────────────┬────▶   │
│                       5            10            15            20            25   Years │
│                       │             │             │             │             │         │
│                   Halving 1     Halving 2     Halving 3     Halving 4     Halving 5    │
│                                                                                         │
│                                                                                         │
│   Each halving:                                                                         │
│   ──────────────                                                                        │
│   • Occurs at block height = n × 157,680,000                                           │
│   • Reduces reward by exactly 50%                                                      │
│   • Is deterministic and predictable                                                   │
│   • Cannot be delayed or accelerated                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 Annual Emission Decline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ANNUAL EMISSION BY PERIOD                                       │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Annual                                                                                │
│   Emission                                                                              │
│   (MBO)                                                                                 │
│                                                                                         │
│   3,153,600 ┤████████████████████████████████████████████████████████████████████████  │
│             │███████████████████████████████████████████████████████████████████████   │
│             │██████████████████████████████████████████████████████████████████████    │
│             │                                                                          │
│   1,576,800 ┤                                 ████████████████████████████████████████ │
│             │                                 ███████████████████████████████████████  │
│             │                                 ██████████████████████████████████████   │
│             │                                                                          │
│     788,400 ┤                                                      ██████████████████  │
│             │                                                      █████████████████   │
│             │                                                                          │
│     394,200 ┤                                                                  ███████ │
│             │                                                                          │
│     197,100 ┤                                                                      ████│
│             │                                                                          │
│           0 └──────────────────┬────────────────────┬────────────────────┬─────────▶   │
│                             Year 5              Year 10              Year 15     Year 25│
│                                                                                         │
│   Emissions per 5-year period:                                                         │
│   ───────────────────────────                                                           │
│   Years 1-5:   15,768,000 MBO (50% of total)                                           │
│   Years 6-10:   7,884,000 MBO (25% of total)                                           │
│   Years 11-15:  3,942,000 MBO (12.5% of total)                                         │
│   Years 16-20:  1,971,000 MBO (6.25% of total)                                         │
│   Years 21-25:    985,500 MBO (3.125% of total)                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. Economic Safety Guarantees

### 8.1 Supply Integrity Guarantees

```
╔═════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                         ║
║                           ECONOMIC SAFETY GUARANTEES                                    ║
║                                                                                         ║
╠═════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   ✓ NO SUPPLY MANIPULATION                                                      │  ║
║   │   ────────────────────────                                                      │  ║
║   │                                                                                 │  ║
║   │   • Total supply hardcoded: 31,536,000 MBO                                     │  ║
║   │   • No admin function to create tokens                                         │  ║
║   │   • No smart contract can mint MBO                                             │  ║
║   │   • Consensus layer rejects invalid supply                                     │  ║
║   │   • Any node can verify total supply independently                             │  ║
║   │                                                                                 │  ║
║   │   ENFORCEMENT: Protocol consensus rules                                         │  ║
║   │   VERIFICATION: total_supply ≤ 31,536,000                                       │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   ✓ NO UNLIMITED EMISSIONS                                                      │  ║
║   │   ─────────────────────────                                                     │  ║
║   │                                                                                 │  ║
║   │   • Block reward formula is deterministic                                      │  ║
║   │   • Halving schedule is immutable                                              │  ║
║   │   • Emissions decrease geometrically                                           │  ║
║   │   • Cap is mathematically guaranteed                                           │  ║
║   │   • No mechanism for "extra" rewards                                           │  ║
║   │                                                                                 │  ║
║   │   ENFORCEMENT: Hardcoded emission formula                                       │  ║
║   │   VERIFICATION: block_reward(h) = 0.1 × 0.5^(h/157680000)                       │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   ✓ PREDICTABLE HALVING LOCKED IN PROTOCOL                                      │  ║
║   │   ────────────────────────────────────────                                      │  ║
║   │                                                                                 │  ║
║   │   • Halving occurs exactly every 157,680,000 blocks                            │  ║
║   │   • No entity can delay or accelerate halving                                  │  ║
║   │   • Block height determines reward automatically                               │  ║
║   │   • Governance cannot modify halving schedule                                  │  ║
║   │   • Foundation has no control over halving                                     │  ║
║   │                                                                                 │  ║
║   │   ENFORCEMENT: Consensus block validation                                       │  ║
║   │   VERIFICATION: Check reward matches height formula                             │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
╚═════════════════════════════════════════════════════════════════════════════════════════╝
```

### 8.2 Guarantee Verification Methods

| Guarantee | Verification Method | Who Can Verify |
|-----------|---------------------|----------------|
| **Supply Cap** | Sum all block rewards ≤ 31,536,000 | Any full node |
| **Halving Schedule** | Check block height vs reward | Any full node |
| **No Minting** | Verify no mint transactions | Any full node |
| **Deterministic Emission** | Recalculate from genesis | Any full node |
| **No Governance Override** | Review consensus code | Any developer |

### 8.3 Attack Resistance

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ATTACK RESISTANCE                                               │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   INFLATION ATTACK                                                                      │
│   ────────────────                                                                      │
│   Threat:     Create tokens beyond schedule                                            │
│   Protection: Hardcoded cap in consensus                                               │
│   Result:     Invalid blocks rejected by all nodes                                     │
│                                                                                         │
│   HALVING DELAY ATTACK                                                                  │
│   ────────────────────                                                                  │
│   Threat:     Postpone halving to extend high rewards                                  │
│   Protection: Height-based calculation, not time-based                                 │
│   Result:     Cannot manipulate block heights                                          │
│                                                                                         │
│   GOVERNANCE CAPTURE                                                                    │
│   ──────────────────                                                                    │
│   Threat:     Vote to increase supply                                                  │
│   Protection: Supply outside governance scope                                          │
│   Result:     No proposal type exists for minting                                      │
│                                                                                         │
│   HIDDEN INFLATION                                                                      │
│   ────────────────                                                                      │
│   Threat:     Secretly create tokens off-schedule                                      │
│   Protection: All issuance in block rewards only                                       │
│   Result:     Full audit trail on-chain                                                │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Appendix: Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SUPPLY SCHEDULE QUICK REFERENCE                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CANONICAL VALUES                                                                      │
│   ────────────────                                                                      │
│   Total Supply:         31,536,000 MBO (fixed, immutable)                              │
│   Block Time:           1 second                                                       │
│   Blocks per Year:      31,536,000                                                     │
│   Initial Block Reward: 0.1 MBO                                                        │
│   Halving Interval:     5 years (157,680,000 blocks)                                   │
│   PoS/PoUW Split:       50% / 50%                                                      │
│                                                                                         │
│   KEY MILESTONES                                                                        │
│   ──────────────                                                                        │
│   Year 5:   50% issued (15,768,000 MBO)                                                │
│   Year 10:  75% issued (23,652,000 MBO)                                                │
│   Year 20:  93.75% issued (29,565,000 MBO)                                             │
│   Year 50:  99.9% issued (~31,505,000 MBO)                                             │
│                                                                                         │
│   HALVING REWARDS                                                                       │
│   ────────────────                                                                      │
│   Year 1-5:    0.1 MBO/block                                                           │
│   Year 6-10:   0.05 MBO/block                                                          │
│   Year 11-15:  0.025 MBO/block                                                         │
│   Year 16-20:  0.0125 MBO/block                                                        │
│   Year 21-25:  0.00625 MBO/block                                                       │
│                                                                                         │
│   SAFETY GUARANTEES                                                                     │
│   ─────────────────                                                                     │
│   ✓ No supply manipulation                                                             │
│   ✓ No unlimited emissions                                                             │
│   ✓ Predictable halving locked in protocol                                             │
│   ✓ Governance cannot modify supply                                                    │
│   ✓ All issuance verifiable on-chain                                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [token_distribution.md](./token_distribution.md) | Allocation breakdown |
| [monetary_policy.md](./monetary_policy.md) | Monetary policy details |
| [incentive_design.md](./incentive_design.md) | Incentive mechanisms |
| [vesting_model.md](./vesting_model.md) | Token unlock schedules |

---

*This document defines the official supply schedule for Mbongo Chain. All emission parameters are enforced by consensus rules and cannot be modified.*

