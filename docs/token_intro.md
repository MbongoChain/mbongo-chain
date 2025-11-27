<!-- Verified against tokenomics.md -->
# Mbongo (MBO) — Token Introduction

> **Document Type:** Token Overview  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## 1. Overview

**MBO** is the native token of Mbongo Chain, a compute-first blockchain designed for global GPU coordination and high-throughput state execution.

MBO serves as the fundamental unit of value within the network, enabling consensus participation, compute marketplace transactions, and network fee payments.

---

## 2. Purpose of MBO

MBO fulfills four core functions within the Mbongo Chain ecosystem:

| Function | Description |
|----------|-------------|
| **Staking** | Validators stake MBO to participate in Proof-of-Stake consensus |
| **Compute Incentives** | GPU providers earn MBO rewards for Proof-of-Useful-Work contributions |
| **Transaction Fees** | All network operations require MBO for gas and fee payments |
| **Governance** | MBO holders participate in protocol governance decisions |

---

## 3. Key Properties

### Fixed Supply

MBO has a **fixed maximum supply** with no inflation mechanism:

```
┌─────────────────────────────────────────────────────────────┐
│                    MBO SUPPLY                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Maximum Supply:     31,536,000 MBO                       │
│   Inflation:          None (fixed cap)                     │
│   Smallest Unit:      1 nMBO (10⁻⁹ MBO)                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Halving Schedule

Block rewards are reduced by 50% every 5 years to ensure long-term sustainability:

```
┌─────────────────────────────────────────────────────────────┐
│                 HALVING SCHEDULE                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Year 0-5:       Base reward                              │
│   Year 5-10:      50% of base reward                       │
│   Year 10-15:     25% of base reward                       │
│   Year 15-20:     12.5% of base reward                     │
│   ...             Continues halving every 5 years          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Role in Consensus

MBO is integral to the hybrid PoS + PoUW consensus mechanism:

- **Proof-of-Stake (PoS):** Validators must stake MBO to participate in block production and earn staking rewards
- **Proof-of-Useful-Work (PoUW):** GPU compute providers earn MBO for verified compute contributions

---

## 4. Supply Breakdown

```
┌─────────────────────────────────────────────────────────────┐
│              HIGH-LEVEL SUPPLY ALLOCATION                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Category                        │  Allocation            │
│   ────────────────────────────────┼────────────────────────│
│   Block Rewards (PoS + PoUW)      │  Majority of supply    │
│   Ecosystem Development           │  Reserved              │
│   Team & Contributors             │  Vested allocation     │
│   Foundation Reserve              │  Long-term fund        │
│                                                             │
│   Note: Detailed breakdown in tokenomics_full.md           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Reward Distribution

Block rewards are split between PoS validators and PoUW compute providers:

```
┌─────────────────────────────────────────────────────────────┐
│              REWARD SPLIT (PER BLOCK)                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Recipient                       │  Share                 │
│   ────────────────────────────────┼────────────────────────│
│   PoS Validators                  │  50%                   │
│   PoUW Compute Providers          │  50%                   │
│                                                             │
│   Total                           │  100%                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

This split aligns incentives between network security (PoS) and useful computation (PoUW).

---

## 6. For Developers

MBO is the native asset required for all on-chain operations. When building on Mbongo Chain:

- **Gas fees** are paid in MBO for transaction execution
- **Staking contracts** accept MBO for validator delegation
- **Compute tasks** are priced and settled in MBO
- **Smart contracts** (future) will use MBO for deployment and execution costs

All SDK methods and RPC endpoints denominate values in MBO or its smallest unit (nMBO).

---

## 7. For Participants

MBO represents ownership in the Mbongo Chain network. Token holders can:

- **Stake** MBO to earn validator rewards and support network security
- **Delegate** to validators without running infrastructure
- **Provide compute** via GPU nodes to earn PoUW rewards
- **Participate in governance** as the protocol evolves

The fixed supply and halving schedule create predictable token economics over time.

---

## 8. Related Documentation

| Document | Description |
|----------|-------------|
| `tokenomics_full.md` | Complete tokenomics specification |
| `staking_guide.md` | Validator staking instructions |
| `compute_provider_guide.md` | GPU provider setup |
| `governance.md` | Governance participation |

---

*This document provides an introduction to MBO. For detailed economic specifications, see the full tokenomics documentation.*

