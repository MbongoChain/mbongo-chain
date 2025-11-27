<!-- Verified against tokenomics.md -->
# MBO Monetary Policy

> **Document Type:** Monetary Policy Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## 1. Overview

This document defines the monetary policy governing MBO, the native token of Mbongo Chain. The policy is designed to provide predictable economics, long-term sustainability, and resistance to inflation.

---

## 2. Fixed Supply

MBO has an **absolute fixed supply** that will never be increased:

```
┌─────────────────────────────────────────────────────────────┐
│                    SUPPLY PARAMETERS                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Maximum Supply:         31,536,000 MBO                   │
│   Genesis Supply:         0 MBO (all minted via rewards)   │
│   Inflation Rate:         0% (permanently)                 │
│   Supply Cap:             Immutable (protocol-enforced)    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**No inflation — ever.** The 31,536,000 MBO cap is enforced at the protocol level and cannot be modified without a hard fork with unanimous network consensus.

---

## 3. Halving Schedule

Block rewards are reduced by **50% every 5 years** (approximately every 157,680,000 blocks at 1-second block time).

### Halving Rules

- Halving applies to **both PoS and PoUW reward budgets**
- The 50/50 split between PoS and PoUW is maintained after each halving
- Halving occurs automatically at predetermined block heights
- No manual intervention or governance vote required

```
┌─────────────────────────────────────────────────────────────┐
│                   HALVING MECHANISM                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Halving Interval:       5 years                          │
│   Reward Reduction:       50% per halving                  │
│   First Halving:          Year 5 (block ~157,680,000)      │
│   PoS/PoUW Split:         Maintained at 50/50              │
│                                                             │
│   Post-Halving Rewards:                                    │
│                                                             │
│     Year 0-5:    100% of base reward                       │
│     Year 5-10:    50% of base reward                       │
│     Year 10-15:   25% of base reward                       │
│     Year 15-20:   12.5% of base reward                     │
│     Year 20-25:    6.25% of base reward                    │
│     Year 25-30:    3.125% of base reward                   │
│     ...            Continues indefinitely                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Reward Definitions

### Block Reward

The **block reward** is the total MBO issued when a new block is finalized. It is distributed to the block proposer (validator) and compute providers who contributed PoUW receipts to that block.

```
Block Reward = PoS Reward + PoUW Reward
```

### Epoch Reward

An **epoch** consists of a fixed number of blocks (e.g., 1,000 blocks). The epoch reward is the sum of all block rewards within one epoch. Epoch boundaries are used for:

- Validator set rotation
- PoUW score recalculation
- Reward distribution batching

### Validator Reward (PoS)

The **validator reward** is the portion of the block reward allocated to Proof-of-Stake participants:

- **50%** of each block reward goes to PoS
- Distributed to the block proposer
- Proposer shares with delegators according to commission rate

### Compute Reward (PoUW)

The **compute reward** is the portion of the block reward allocated to Proof-of-Useful-Work contributors:

- **50%** of each block reward goes to PoUW
- Distributed proportionally to verified compute receipts in the block
- GPU providers receive rewards based on task completion and score

```
┌─────────────────────────────────────────────────────────────┐
│                  REWARD DISTRIBUTION                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌───────────────────────────────────────────────────┐    │
│   │              BLOCK REWARD (100%)                  │    │
│   └───────────────────────┬───────────────────────────┘    │
│                           │                                 │
│            ┌──────────────┴──────────────┐                 │
│            │                             │                 │
│            ▼                             ▼                 │
│   ┌─────────────────┐           ┌─────────────────┐        │
│   │   PoS (50%)     │           │   PoUW (50%)    │        │
│   │                 │           │                 │        │
│   │  → Proposer     │           │  → GPU Provider │        │
│   │  → Delegators   │           │  → Task Score   │        │
│   └─────────────────┘           └─────────────────┘        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Emission Schedule

### 10-Year Emission Curve

The following table shows the projected reward schedule over the first 10 years:

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                           MBO EMISSION SCHEDULE (YEARS 0-10)                             │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│   Years  │ Block Reward │ Annual Emission  │ Cumulative Issued │ Remaining to Issue    │
│   ───────┼──────────────┼──────────────────┼───────────────────┼───────────────────────│
│    0-5   │   0.1 MBO    │    3,153,600 MBO │    15,768,000 MBO │    15,768,000 MBO     │
│    5-10  │  0.05 MBO    │    1,576,800 MBO │    23,652,000 MBO │     7,884,000 MBO     │
│   10-15  │ 0.025 MBO    │      788,400 MBO │    27,594,000 MBO │     3,942,000 MBO     │
│   15-20  │0.0125 MBO    │      394,200 MBO │    29,565,000 MBO │     1,971,000 MBO     │
│   20-25  │0.00625 MBO   │      197,100 MBO │    30,550,500 MBO │       985,500 MBO     │
│   ───────┼──────────────┼──────────────────┼───────────────────┼───────────────────────│
│    25+   │ Continues    │   Decreasing     │   Asymptotic      │   Approaching 0       │
│                                                                                          │
│   Note: ~95% emitted by year ~20, ~99.9% by year ~50                                    │
│                                                                                          │
│   Block time: 1 second                                                                  │
│   Blocks per year: ~31,536,000                                                          │
│   First halving: Year 5                                                                 │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Post-Cap Reward Model

Once the 31,536,000 MBO supply cap is reached:

- New block rewards cease from issuance
- Validator and compute rewards transition to **transaction fee distribution**
- Network security is maintained through fee-based incentives
- The economic model becomes fully sustainable without new issuance

---

## 6. Security Through Scarcity

### Decreasing Rewards, Sustained Security

As block rewards decrease through halving, network security is maintained by:

| Mechanism | Description |
|-----------|-------------|
| **Increasing Token Value** | Reduced supply pressure may increase MBO value, maintaining reward purchasing power |
| **Transaction Fees** | Growing network usage generates fee revenue for validators |
| **Staking Lock-ups** | Staked MBO is removed from circulation, reducing sell pressure |
| **PoUW Utility** | Compute demand creates sustained reward opportunities |

### Long-Term Sustainability

```
┌─────────────────────────────────────────────────────────────┐
│              SUSTAINABILITY MODEL                           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Phase 1 (Years 0-5):                                     │
│   → Block rewards dominate validator income                │
│   → Network bootstraps with strong incentives              │
│                                                             │
│   Phase 2 (Years 5-15):                                    │
│   → Block rewards decrease via halving                     │
│   → Transaction fees become increasingly significant       │
│                                                             │
│   Phase 3 (Years 15+):                                     │
│   → Transaction fees become primary reward source          │
│   → Block rewards approach minimal levels                  │
│   → Network operates on sustainable fee model              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. Economic Benefits

### Predictable Economics

- **Known supply:** All participants can calculate future token issuance
- **Deterministic halving:** Reward reductions occur on schedule
- **No surprises:** No governance votes can increase supply

### Long-Term Network Sustainability

- **Gradual transition:** From block rewards to fee-based model
- **Aligned incentives:** Validators benefit from network growth
- **Compute economy:** PoUW creates real utility demand for MBO

### Anti-Inflationary Design

- **Fixed cap:** 31,536,000 MBO is the permanent maximum
- **Decreasing issuance:** Halving reduces new supply over time
- **Deflationary pressure:** Fee burning mechanisms may be introduced

```
┌─────────────────────────────────────────────────────────────┐
│              ANTI-INFLATION GUARANTEES                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ✓ Supply cap enforced at protocol level                  │
│   ✓ No minting function for additional tokens              │
│   ✓ Halving schedule immutable                             │
│   ✓ Governance cannot increase supply                      │
│   ✓ All issuance is transparent and auditable              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. Stakeholder Considerations

### For Investors

MBO's monetary policy provides:

- **Supply certainty:** The 31,536,000 cap is permanent and verifiable
- **Predictable dilution:** Future issuance is calculable from the halving schedule
- **Sound money properties:** Fixed supply with decreasing emission mirrors Bitcoin's model
- **No governance risk:** Supply cannot be increased by protocol changes

### For Validators & Compute Providers

Understanding the reward schedule enables:

- **Revenue planning:** Calculate expected rewards based on stake and compute contribution
- **Long-term strategy:** Prepare for halving events and fee-based transition
- **Commission setting:** Set sustainable rates considering reward reductions
- **Hardware decisions:** Plan GPU investments with decreasing but stable PoUW rewards

### For Developers

The monetary policy affects application design:

- **Fee estimation:** Transaction costs depend on network congestion, not inflation
- **Token integration:** MBO maintains value stability through fixed supply
- **Economic modeling:** Build applications with predictable token economics
- **Compute pricing:** Price GPU tasks knowing reward structure is stable

---

## 9. Related Documentation

| Document | Description |
|----------|-------------|
| `token_intro.md` | MBO token introduction |
| `tokenomics_full.md` | Complete tokenomics specification |
| `staking_economics.md` | Validator staking economics |
| `pouw_rewards.md` | PoUW reward mechanics |

---

*This document defines the immutable monetary policy of MBO. The fixed supply and halving schedule are protocol-enforced and cannot be changed without network-wide consensus.*

