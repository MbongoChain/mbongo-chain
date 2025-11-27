<!-- Verified against tokenomics.md -->
# Mbongo Chain — Utility & Value

> **Document Type:** Token Utility Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Core Utilities of MBO](#2-core-utilities-of-mbo)
3. [Marketplace Utility](#3-marketplace-utility)
4. [Ecosystem Incentives](#4-ecosystem-incentives)
5. [Why MBO Has Intrinsic Value](#5-why-mbo-has-intrinsic-value)
6. [Summary](#6-summary)

---

## 1. Purpose of This Document

This document formalizes the **economic and functional utility** of the MBO token across the entire Mbongo Chain ecosystem. It provides a comprehensive reference for understanding how MBO is used, why it is required, and what drives its value.

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBO TOKEN UTILITY OVERVIEW                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MBO is not merely a speculative asset—it is the fundamental economic unit            │
│   that powers every operation on Mbongo Chain.                                         │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   UTILITY CATEGORIES                                                            │  │
│   │   ══════════════════                                                            │  │
│   │                                                                                 │  │
│   │   1. TRANSACTIONAL     → Gas fees for all on-chain operations                  │  │
│   │   2. SECURITY          → Staking for PoS consensus                             │  │
│   │   3. COMPUTE           → Payments for PoUW GPU workloads                       │  │
│   │   4. GOVERNANCE        → Voting power for protocol decisions                   │  │
│   │   5. MARKETPLACE       → Medium of exchange in compute markets                 │  │
│   │   6. ECOSYSTEM         → Grants, incentives, and development                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   Every participant in the Mbongo ecosystem—validators, compute providers,             │
│   developers, and users—requires MBO to participate.                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.1 Document Scope

| Aspect | Coverage |
|--------|----------|
| **Core Utilities** | Gas, staking, compute, governance, security |
| **Marketplace** | Compute jobs, storage, bridges, smart contracts |
| **Ecosystem** | Grants, developer incentives, community rewards |
| **Value Drivers** | Scarcity, demand, deflationary mechanics |

---

## 2. Core Utilities of MBO

### 2.a Gas Fees

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GAS FEES                                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MBO IS THE NATIVE GAS TOKEN                                                           │
│   ═══════════════════════════                                                           │
│                                                                                         │
│   Every operation on Mbongo Chain requires MBO to pay for:                             │
│   • Transaction execution                                                              │
│   • State storage                                                                      │
│   • Compute verification                                                               │
│   • Network bandwidth                                                                  │
│                                                                                         │
│   Without MBO, no action can be performed on-chain.                                    │
│                                                                                         │
│                                                                                         │
│   FEE STRUCTURE                                                                         │
│   ═════════════                                                                         │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   BASE FEE: 100% BURNED                                                         │  │
│   │   ─────────────────────                                                         │  │
│   │                                                                                 │  │
│   │   • Calculated based on gas used                                               │  │
│   │   • Sent to null address (0x0000...0000)                                       │  │
│   │   • Permanently removed from circulation                                       │  │
│   │   • Creates deflationary pressure                                              │  │
│   │                                                                                 │  │
│   │   base_fee_burned = gas_used × base_fee_per_gas                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   PRIORITY FEE: ROUTED BY ACTOR TYPE                                            │  │
│   │   ──────────────────────────────────                                            │  │
│   │                                                                                 │  │
│   │   Standard Transactions:                                                        │  │
│   │   → Priority fee paid to VALIDATOR (block proposer)                            │  │
│   │                                                                                 │  │
│   │   Compute Transactions (PoUW):                                                  │  │
│   │   → Priority fee paid to GPU PROVIDER                                          │  │
│   │                                                                                 │  │
│   │   Oracle Messages:                                                              │  │
│   │   → Priority fee paid to ATTESTERS                                             │  │
│   │                                                                                 │  │
│   │   priority_payment = gas_used × priority_fee_per_gas                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   GAS UTILITY SUMMARY                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   • MBO is REQUIRED for every transaction                                              │
│   • Usage directly reduces circulating supply (burns)                                  │
│   • Higher network usage → more MBO burned → increased scarcity                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.b Staking (PoS)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         STAKING (PoS)                                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VALIDATORS STAKE MBO TO SECURE THE NETWORK                                            │
│   ══════════════════════════════════════════                                            │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   VALIDATOR STAKING                                                             │  │
│   │   ─────────────────                                                             │  │
│   │                                                                                 │  │
│   │   Requirement:                                                                  │  │
│   │   • Minimum stake: 50,000 MBO                                                  │  │
│   │   • Locked in staking contract                                                 │  │
│   │   • Subject to slashing for misbehavior                                        │  │
│   │                                                                                 │  │
│   │   Purpose:                                                                      │  │
│   │   • Economic security collateral                                               │  │
│   │   • Sybil resistance                                                           │  │
│   │   • Alignment with network success                                             │  │
│   │                                                                                 │  │
│   │   Rewards:                                                                      │  │
│   │   • 50% of block rewards allocated to PoS pool                                 │  │
│   │   • 80% of PoS pool to validators                                              │  │
│   │   • Proposer bonuses for block production                                      │  │
│   │   • Priority fees from transactions                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   DELEGATORS STAKE MBO TO EARN BLOCK REWARDS                                            │
│   ════════════════════════════════════════════                                          │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   DELEGATOR STAKING                                                             │  │
│   │   ─────────────────                                                             │  │
│   │                                                                                 │  │
│   │   Requirement:                                                                  │  │
│   │   • Minimum delegation: 100 MBO                                                │  │
│   │   • Choose validator to delegate to                                            │  │
│   │   • Share slashing risk with validator                                         │  │
│   │                                                                                 │  │
│   │   Purpose:                                                                      │  │
│   │   • Passive participation in security                                          │  │
│   │   • Decentralize stake distribution                                            │  │
│   │   • Support validator infrastructure                                           │  │
│   │                                                                                 │  │
│   │   Rewards:                                                                      │  │
│   │   • 20% of PoS pool to delegators                                              │  │
│   │   • Proportional to delegated amount                                           │  │
│   │   • Minus validator commission                                                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   WEIGHTED STAKE AFFECTS CONSENSUS                                                      │
│   ════════════════════════════════                                                      │
│                                                                                         │
│   Stake weight determines:                                                             │
│   • Probability of being selected as block proposer                                    │
│   • Voting power in consensus rounds                                                   │
│   • Share of rewards                                                                   │
│                                                                                         │
│   Formula:                                                                              │
│   proposer_probability(v) = stake(v) / total_stake                                     │
│                                                                                         │
│   Higher stake → more responsibility → more rewards (and risk)                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.c PoUW Compute Payments

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoUW COMPUTE PAYMENTS                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   GPU PROVIDERS RECEIVE MBO FOR VALIDATED COMPUTE                                       │
│   ═══════════════════════════════════════════════                                       │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   COMPUTE REWARD MECHANISM                                                      │  │
│   │   ────────────────────────                                                      │  │
│   │                                                                                 │  │
│   │   1. GPU provider executes assigned compute task                               │  │
│   │   2. Provider generates compute receipt with result hash                       │  │
│   │   3. Attesters verify and sign receipt                                         │  │
│   │   4. Receipt included in block                                                 │  │
│   │   5. Provider receives MBO reward                                              │  │
│   │                                                                                 │  │
│   │   Reward Source:                                                                │  │
│   │   • 50% of block rewards allocated to PoUW pool                                │  │
│   │   • Distributed proportionally to verified work units                          │  │
│   │   • Plus compute fees from task requesters                                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   HYBRID 50/50 REWARD DISTRIBUTION                                                      │
│   ════════════════════════════════                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │                      BLOCK REWARD: 0.1 MBO (Year 1-5)                           │  │
│   │                                                                                 │  │
│   │   ┌─────────────────────────┐    ┌─────────────────────────┐                   │  │
│   │   │                         │    │                         │                   │  │
│   │   │     PoS POOL (50%)      │    │    PoUW POOL (50%)      │                   │  │
│   │   │       0.05 MBO          │    │       0.05 MBO          │                   │  │
│   │   │                         │    │                         │                   │  │
│   │   │   → Validators (80%)    │    │   → GPU Providers       │                   │  │
│   │   │   → Delegators (20%)    │    │   (proportional to work)│                   │  │
│   │   │                         │    │                         │                   │  │
│   │   └─────────────────────────┘    └─────────────────────────┘                   │  │
│   │                                                                                 │  │
│   │   This 50/50 split ensures both security AND compute are incentivized.         │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   DETERMINISTIC COMPUTE PRICING MODEL                                                   │
│   ═══════════════════════════════════                                                   │
│                                                                                         │
│   Compute costs are calculated deterministically:                                      │
│                                                                                         │
│   compute_fee = base_compute_cost + (work_units × unit_price)                          │
│                                                                                         │
│   Where:                                                                                │
│   • base_compute_cost: Fixed overhead per task                                         │
│   • work_units: Measured GPU cycles consumed                                           │
│   • unit_price: Protocol-defined (AIDA-regulated)                                      │
│                                                                                         │
│   All pricing is:                                                                       │
│   • Integer-based (no floating-point)                                                  │
│   • Verifiable on-chain                                                                │
│   • Same on all nodes                                                                  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.d Governance

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE                                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   GOVERNANCE USES STAKED MBO                                                            │
│   ══════════════════════════                                                            │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   VOTING POWER                                                                  │  │
│   │   ────────────                                                                  │  │
│   │                                                                                 │  │
│   │   Validators & Delegators:                                                      │  │
│   │   • Voting weight = staked MBO at snapshot                                     │  │
│   │   • Eligible for all proposal tiers                                            │  │
│   │   • Delegators can override validator vote                                     │  │
│   │                                                                                 │  │
│   │   Compute Providers:                                                            │  │
│   │   • Voting weight = reputation score (non-financial)                           │  │
│   │   • Eligible for all proposal tiers                                            │  │
│   │   • Prevents purchased governance control                                      │  │
│   │                                                                                 │  │
│   │   Token Holders (non-stakers):                                                  │  │
│   │   • 1 MBO = 1 vote (Tier 3 proposals only)                                     │  │
│   │   • Limited to community/social proposals                                      │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   NO NEW TOKENS MINTED THROUGH GOVERNANCE                                               │
│   ═══════════════════════════════════════                                               │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   GOVERNANCE CANNOT:                                                            │  │
│   │   ─────────────────                                                             │  │
│   │                                                                                 │  │
│   │   ✗ Create new MBO tokens                                                      │  │
│   │   ✗ Increase total supply beyond 31,536,000                                    │  │
│   │   ✗ Modify the halving schedule                                                │  │
│   │   ✗ Override vesting schedules                                                 │  │
│   │   ✗ Confiscate user funds                                                      │  │
│   │   ✗ Change historical ledger state                                             │  │
│   │                                                                                 │  │
│   │   These are CONSTITUTIONAL constraints outside governance scope.               │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ONLY PROTOCOL-CONFIG UPDATES ALLOWED                                                  │
│   ════════════════════════════════════                                                  │
│                                                                                         │
│   Governance CAN modify:                                                               │
│   • Gas pricing parameters (within bounds)                                             │
│   • Compute weight adjustments                                                         │
│   • Fee distribution ratios (within bounds)                                            │
│   • Network parameters                                                                 │
│   • Runtime module additions                                                           │
│   • Grant allocations from ecosystem fund                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.e Security Incentives

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY INCENTIVES                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SLASHING PENALTIES DENOMINATED IN MBO                                                 │
│   ═════════════════════════════════════                                                 │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   SLASHING SCHEDULE                                                             │  │
│   │   ─────────────────                                                             │  │
│   │                                                                                 │  │
│   │   Offense                   │ Penalty           │ Destination                   │  │
│   │   ──────────────────────────┼───────────────────┼───────────────────────────────│  │
│   │   Double-signing            │ 5% of stake       │ BURNED                        │  │
│   │   Extended downtime         │ 0.5% of stake     │ BURNED                        │  │
│   │   Invalid compute receipt   │ 1,000 MBO fixed   │ BURNED                        │  │
│   │   Repeated offenses         │ Escalating        │ BURNED                        │  │
│   │                                                                                 │  │
│   │   All slashing:                                                                 │  │
│   │   • Evidence-based and deterministic                                           │  │
│   │   • Automatic execution (no human judgment)                                    │  │
│   │   • Irreversible once executed                                                 │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   BURNED SLASHES REDUCE SUPPLY, INCREASING SCARCITY                                     │
│   ═════════════════════════════════════════════════                                     │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   WHY BURN INSTEAD OF REDISTRIBUTE?                                             │  │
│   │   ─────────────────────────────────                                             │  │
│   │                                                                                 │  │
│   │   1. Prevents perverse incentives                                              │  │
│   │      • No profit motive for false accusations                                  │  │
│   │      • No collusion to slash and share proceeds                                │  │
│   │                                                                                 │  │
│   │   2. Benefits all holders equally                                              │  │
│   │      • Reduced supply → increased scarcity                                     │  │
│   │      • Every MBO holder benefits proportionally                                │  │
│   │                                                                                 │  │
│   │   3. Simplifies the protocol                                                   │  │
│   │      • No complex redistribution logic                                         │  │
│   │      • Deterministic outcome                                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ANTI-SPAM FEES AND ATTACK COSTS IN MBO                                                │
│   ══════════════════════════════════════                                                │
│                                                                                         │
│   Economic attack prevention:                                                          │
│                                                                                         │
│   • Minimum gas price: Prevents free spam                                              │
│   • Exponential spam costs: Repeated actions become expensive                          │
│   • Storage rent: Long-term state occupation has ongoing cost                          │
│   • Attack cost: Controlling consensus requires massive MBO stake                      │
│                                                                                         │
│   All security mechanisms are denominated and enforced in MBO.                         │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.f Core Utilities Summary Table

| Utility | MBO Requirement | Flow | Economic Effect |
|---------|-----------------|------|-----------------|
| **Gas Fees** | Required for all transactions | Base → Burned, Priority → Recipient | Deflationary |
| **Validator Staking** | Min 50,000 MBO | Locked → Earns rewards | Security + Rewards |
| **Delegator Staking** | Min 100 MBO | Locked → Earns rewards | Passive income |
| **Compute Payments** | Per work unit | User → Provider | Compute incentive |
| **Governance** | Staked MBO | Voting weight | Protocol decisions |
| **Slashing** | Stake at risk | Slashed → Burned | Security enforcement |

---

## 3. Marketplace Utility

### 3.1 Compute Marketplace

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE MARKETPLACE                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CROSS-CHAIN COMPUTE JOBS                                                              │
│   ════════════════════════                                                              │
│                                                                                         │
│   MBO serves as the native currency for the decentralized compute marketplace:         │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   JOB SUBMISSION FLOW                                                           │  │
│   │   ───────────────────                                                           │  │
│   │                                                                                 │  │
│   │   1. User submits compute job with MBO payment                                 │  │
│   │   2. Job enters task queue                                                     │  │
│   │   3. GPU provider assigned via matching algorithm                              │  │
│   │   4. Provider executes task                                                    │  │
│   │   5. Result verified via receipt                                               │  │
│   │   6. MBO released to provider                                                  │  │
│   │                                                                                 │  │
│   │   Payment Structure:                                                            │  │
│   │   • Compute fee: Paid to GPU provider                                          │  │
│   │   • Base gas: Burned                                                           │  │
│   │   • Priority fee: To provider (compute tx) or validator                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   SUPPORTED WORKLOADS                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   • AI/ML inference (image, text, audio)                                               │
│   • Training job chunks                                                                │
│   • Rendering and encoding                                                             │
│   • ZK proof generation                                                                │
│   • Scientific computation                                                             │
│   • Batch data processing                                                              │
│                                                                                         │
│   All paid in MBO with deterministic pricing.                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Storage & Bandwidth Markets (Future)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         STORAGE & BANDWIDTH MARKETS [FUTURE]                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PLANNED EXTENSIONS                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   Storage Market:                                                                       │
│   • Pay MBO for persistent data storage                                                │
│   • Storage providers stake MBO as collateral                                          │
│   • Rent model with ongoing MBO payments                                               │
│   • Retrieval fees in MBO                                                              │
│                                                                                         │
│   Bandwidth Market:                                                                     │
│   • Pay MBO for data relay services                                                    │
│   • CDN-like distribution incentives                                                   │
│   • Priority bandwidth allocation                                                      │
│                                                                                         │
│   Status: Placeholder for future development                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.3 Smart Contract Execution Pricing

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SMART CONTRACT PRICING [FUTURE]                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   EXECUTION COST MODEL                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   When smart contracts are enabled, MBO will be required for:                          │
│                                                                                         │
│   • Contract deployment gas                                                            │
│   • Function call execution                                                            │
│   • State storage operations                                                           │
│   • Event logging                                                                      │
│   • Cross-contract calls                                                               │
│                                                                                         │
│   Pricing Formula:                                                                      │
│   contract_cost = deployment_gas + execution_gas + storage_gas + log_gas               │
│                                                                                         │
│   All denominated in MBO with deterministic gas schedule.                              │
│                                                                                         │
│   Status: Planned for future VM integration                                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.4 Interoperability Bridges Cost Model

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         BRIDGE COST MODEL [FUTURE]                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CROSS-CHAIN OPERATIONS                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   Bridge operations will require MBO for:                                              │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   OUTBOUND (Mbongo → Other Chain)                                               │  │
│   │   ─────────────────────────────────                                             │  │
│   │   • Lock MBO in bridge contract                                                │  │
│   │   • Pay bridge fee (MBO)                                                       │  │
│   │   • Gas for proof generation                                                   │  │
│   │                                                                                 │  │
│   │   INBOUND (Other Chain → Mbongo)                                               │  │
│   │   ─────────────────────────────────                                             │  │
│   │   • Verification gas (MBO)                                                     │  │
│   │   • Mint/unlock fee (MBO)                                                      │  │
│   │   • State update gas (MBO)                                                     │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   Bridge operators stake MBO as security collateral.                                   │
│                                                                                         │
│   Status: Planned for cross-chain integration                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.5 Marketplace Utility Summary

| Market | MBO Use | Status |
|--------|---------|--------|
| **Compute Marketplace** | Task payments, provider rewards | Active |
| **Storage Market** | Storage fees, provider collateral | Future |
| **Bandwidth Market** | Relay fees, CDN incentives | Future |
| **Smart Contracts** | Execution gas, deployment | Future |
| **Cross-Chain Bridges** | Bridge fees, operator collateral | Future |

---

## 4. Ecosystem Incentives

### 4.1 Grants Funded via Locked MBO Treasury

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ECOSYSTEM GRANTS                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   TREASURY ALLOCATION                                                                   │
│   ═══════════════════                                                                   │
│                                                                                         │
│   From the total supply of 31,536,000 MBO:                                             │
│                                                                                         │
│   • 15% (4,730,400 MBO) allocated to Ecosystem Grants & Developers                     │
│   • Locked in treasury with milestone-based unlocks                                    │
│   • Governed by multi-sig with community oversight                                     │
│                                                                                         │
│                                                                                         │
│   GRANT CATEGORIES                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   INFRASTRUCTURE GRANTS                                                         │  │
│   │   ─────────────────────                                                         │  │
│   │   • Core protocol development                                                  │  │
│   │   • Node software improvements                                                 │  │
│   │   • Security audits                                                            │  │
│   │   • Performance optimization                                                   │  │
│   │                                                                                 │  │
│   │   APPLICATION GRANTS                                                            │  │
│   │   ──────────────────                                                            │  │
│   │   • DeFi protocols                                                             │  │
│   │   • AI/ML applications                                                         │  │
│   │   • Developer tools                                                            │  │
│   │   • User interfaces                                                            │  │
│   │                                                                                 │  │
│   │   RESEARCH GRANTS                                                               │  │
│   │   ───────────────                                                               │  │
│   │   • Cryptography research                                                      │  │
│   │   • Consensus improvements                                                     │  │
│   │   • Scalability studies                                                        │  │
│   │   • Economic modeling                                                          │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   UNLOCK MECHANISM                                                                      │
│   ════════════════                                                                      │
│                                                                                         │
│   • Milestone-based: Funds released upon verified deliverables                         │
│   • Multi-sig approval required                                                        │
│   • No discretionary unlocks                                                           │
│   • Transparent on-chain record                                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Long-Term Ecosystem Development

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         LONG-TERM DEVELOPMENT                                           │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEVELOPMENT TIMELINE                                                                  │
│   ════════════════════                                                                  │
│                                                                                         │
│   Year 1-2: Foundation                                                                 │
│   • Core protocol stabilization                                                        │
│   • Initial dApp ecosystem                                                             │
│   • Developer onboarding                                                               │
│   • Security hardening                                                                 │
│                                                                                         │
│   Year 3-5: Growth                                                                     │
│   • Smart contract layer                                                               │
│   • Cross-chain bridges                                                                │
│   • Advanced compute features                                                          │
│   • Enterprise adoption                                                                │
│                                                                                         │
│   Year 5-10: Maturity                                                                  │
│   • Full decentralization                                                              │
│   • Global compute marketplace                                                         │
│   • Sustainable fee-based economy                                                      │
│   • Protocol self-sufficiency                                                          │
│                                                                                         │
│                                                                                         │
│   FUNDING SUSTAINABILITY                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   • Grant funds are from initial allocation (no new minting)                           │
│   • Long-term: ecosystem sustains through fees and compute revenue                     │
│   • Treasury DAO (future) manages allocations                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Rewards for Community Contributors

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         COMMUNITY INCENTIVES                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ALLOCATION                                                                            │
│   ══════════                                                                            │
│                                                                                         │
│   • 10% (3,153,600 MBO) allocated to Community & Incentives                            │
│   • Streamed per epoch (no cliff)                                                      │
│   • Used for ongoing community programs                                                │
│                                                                                         │
│                                                                                         │
│   INCENTIVE PROGRAMS                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   BUG BOUNTIES                                                                  │  │
│   │   ────────────                                                                  │  │
│   │   • Critical bugs: Up to 100,000 MBO                                           │  │
│   │   • High severity: Up to 50,000 MBO                                            │  │
│   │   • Medium severity: Up to 10,000 MBO                                          │  │
│   │   • Low severity: Up to 1,000 MBO                                              │  │
│   │                                                                                 │  │
│   │   AMBASSADOR PROGRAM                                                            │  │
│   │   ──────────────────                                                            │  │
│   │   • Regional community building                                                │  │
│   │   • Education and onboarding                                                   │  │
│   │   • Event organization                                                         │  │
│   │   • Monthly MBO stipends                                                       │  │
│   │                                                                                 │  │
│   │   CONTENT CREATION                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • Technical tutorials                                                        │  │
│   │   • Documentation contributions                                                │  │
│   │   • Video content                                                              │  │
│   │   • Bounty-based rewards                                                       │  │
│   │                                                                                 │  │
│   │   GOVERNANCE PARTICIPATION                                                      │  │
│   │   ────────────────────────                                                      │  │
│   │   • Proposal creation rewards                                                  │  │
│   │   • Voting incentives (future)                                                 │  │
│   │   • Committee participation                                                    │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.4 Developer Incentives Pool

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DEVELOPER INCENTIVES                                            │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEVELOPER-FOCUSED MBO                                                                 │
│   ═════════════════════                                                                 │
│                                                                                         │
│   Part of the Ecosystem allocation is specifically for developers:                     │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   HACKATHON PRIZES                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • Quarterly hackathons with MBO prize pools                                  │  │
│   │   • 1st place: 50,000 MBO                                                      │  │
│   │   • 2nd place: 25,000 MBO                                                      │  │
│   │   • 3rd place: 10,000 MBO                                                      │  │
│   │   • Honorable mentions: 1,000 MBO each                                         │  │
│   │                                                                                 │  │
│   │   DEVELOPER GRANTS                                                              │  │
│   │   ────────────────                                                              │  │
│   │   • Small grants: 5,000 - 25,000 MBO                                           │  │
│   │   • Medium grants: 25,000 - 100,000 MBO                                        │  │
│   │   • Large grants: 100,000 - 500,000 MBO                                        │  │
│   │   • Milestone-based release                                                    │  │
│   │                                                                                 │  │
│   │   RETROACTIVE FUNDING                                                           │  │
│   │   ───────────────────                                                           │  │
│   │   • Rewards for valuable contributions already made                            │  │
│   │   • Community nominations                                                      │  │
│   │   • Quarterly allocation                                                       │  │
│   │                                                                                 │  │
│   │   SDK & TOOLING BOUNTIES                                                        │  │
│   │   ──────────────────────                                                        │  │
│   │   • Language bindings                                                          │  │
│   │   • Developer tools                                                            │  │
│   │   • Testing frameworks                                                         │  │
│   │   • Documentation                                                              │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 4.5 Ecosystem Allocation Summary

| Category | Allocation | MBO Amount | Unlock |
|----------|------------|------------|--------|
| **Ecosystem Grants** | 15% | 4,730,400 MBO | Milestone-based |
| **Community Incentives** | 10% | 3,153,600 MBO | Per-epoch stream |
| **Foundation Operations** | 10% | 3,153,600 MBO | 4-year vesting |
| **Early Contributors** | 5% | 1,576,800 MBO | 4-year, 1-year cliff |

---

## 5. Why MBO Has Intrinsic Value

### 5.1 Value Drivers

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         INTRINSIC VALUE DRIVERS                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   1. FIXED SUPPLY                                                               │  │
│   │   ═══════════════                                                               │  │
│   │                                                                                 │  │
│   │   Total Supply: 31,536,000 MBO                                                 │  │
│   │                                                                                 │  │
│   │   • Hardcoded maximum that can NEVER increase                                  │  │
│   │   • No governance, Foundation, or emergency mechanism can mint more            │  │
│   │   • Absolute mathematical scarcity                                             │  │
│   │                                                                                 │  │
│   │   Value Impact:                                                                 │  │
│   │   → Existing holders can never be diluted                                      │  │
│   │   → Scarcity creates baseline value floor                                      │  │
│   │   → Predictable long-term economics                                            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   2. DEFLATIONARY BASE-FEE MECHANISM                                            │  │
│   │   ══════════════════════════════════                                            │  │
│   │                                                                                 │  │
│   │   Every transaction burns MBO:                                                 │  │
│   │   • Base fee sent to null address                                              │  │
│   │   • Permanently removed from circulation                                       │  │
│   │   • Slashed stake also burned                                                  │  │
│   │                                                                                 │  │
│   │   Value Impact:                                                                 │  │
│   │   → Circulating supply decreases over time                                     │  │
│   │   → High usage = more burns = more scarcity                                    │  │
│   │   → Network success directly increases token value                             │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   3. INCREASING DEMAND FROM COMPUTE WORKLOADS                                   │  │
│   │   ═══════════════════════════════════════════                                   │  │
│   │                                                                                 │  │
│   │   Compute demand is growing exponentially:                                     │  │
│   │   • AI/ML inference and training                                               │  │
│   │   • Scientific computation                                                     │  │
│   │   • Rendering and media processing                                             │  │
│   │   • ZK proof generation                                                        │  │
│   │                                                                                 │  │
│   │   Value Impact:                                                                 │  │
│   │   → More compute jobs = more MBO needed                                        │  │
│   │   → GPU providers need MBO for collateral                                      │  │
│   │   → Growing demand with fixed/decreasing supply                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   4. SECURITY BASED ON ECONOMIC GUARANTEES                                      │  │
│   │   ════════════════════════════════════════                                      │  │
│   │                                                                                 │  │
│   │   MBO provides security through:                                               │  │
│   │   • Stake requirements (50,000 MBO minimum)                                    │  │
│   │   • Slashing penalties (5% for misbehavior)                                    │  │
│   │   • Attack costs (millions of MBO needed)                                      │  │
│   │                                                                                 │  │
│   │   Value Impact:                                                                 │  │
│   │   → Security guarantees require holding MBO                                    │  │
│   │   → Higher value = higher security (attack cost)                               │  │
│   │   → Economic finality provides trust                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   5. LONG-TERM ALIGNMENT OF ALL PARTICIPANTS                                    │  │
│   │   ══════════════════════════════════════════                                    │  │
│   │                                                                                 │  │
│   │   All ecosystem participants benefit from MBO value:                           │  │
│   │                                                                                 │  │
│   │   Validators:                                                                   │  │
│   │   • Stake MBO for security                                                     │  │
│   │   • Earn MBO rewards                                                           │  │
│   │   • Value appreciation benefits holdings                                       │  │
│   │                                                                                 │  │
│   │   GPU Providers:                                                                │  │
│   │   • Earn MBO for compute                                                       │  │
│   │   • Higher MBO value = better ROI                                              │  │
│   │   • Long-term compute contracts in MBO                                         │  │
│   │                                                                                 │  │
│   │   Developers:                                                                   │  │
│   │   • Receive grants in MBO                                                      │  │
│   │   • Build apps that use MBO                                                    │  │
│   │   • Ecosystem growth benefits all                                              │  │
│   │                                                                                 │  │
│   │   Users:                                                                        │  │
│   │   • Pay fees in MBO                                                            │  │
│   │   • Access compute resources                                                   │  │
│   │   • Benefit from secure network                                                │  │
│   │                                                                                 │  │
│   │   Value Impact:                                                                 │  │
│   │   → Aligned incentives create positive-sum game                                │  │
│   │   → Everyone benefits from network success                                     │  │
│   │   → Long-term thinking over short-term extraction                              │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Value Equation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBO VALUE EQUATION                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                                                                                         │
│   MBO_Value = f(Scarcity, Utility, Demand, Security)                                   │
│                                                                                         │
│                                                                                         │
│   WHERE:                                                                                │
│   ══════                                                                                │
│                                                                                         │
│   Scarcity:                                                                             │
│   • Fixed supply (31,536,000 MBO)                                                      │
│   • Decreasing via burns                                                               │
│   • No inflation                                                                       │
│                                                                                         │
│   Utility:                                                                              │
│   • Required for ALL on-chain actions                                                  │
│   • Gas, staking, compute, governance                                                  │
│   • Growing marketplace use cases                                                      │
│                                                                                         │
│   Demand:                                                                               │
│   • Network usage creates demand                                                       │
│   • Compute workloads require MBO                                                      │
│   • Staking locks supply                                                               │
│                                                                                         │
│   Security:                                                                             │
│   • Attack resistance increases trust                                                  │
│   • Economic finality provides guarantees                                              │
│   • Higher value = more secure network                                                 │
│                                                                                         │
│                                                                                         │
│   POSITIVE FEEDBACK LOOP                                                                │
│   ══════════════════════                                                                │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   Network Growth                                                                │  │
│   │        │                                                                        │  │
│   │        ▼                                                                        │  │
│   │   More Usage ──────────────────────────────────────────┐                       │  │
│   │        │                                                │                       │  │
│   │        ▼                                                │                       │  │
│   │   More Fees Burned                                      │                       │  │
│   │        │                                                │                       │  │
│   │        ▼                                                │                       │  │
│   │   Lower Supply                                          │                       │  │
│   │        │                                                │                       │  │
│   │        ▼                                                │                       │  │
│   │   Higher Scarcity ─────────────────▶ Higher Value      │                       │  │
│   │                                            │            │                       │  │
│   │                                            ▼            │                       │  │
│   │                                   More Attractive ──────┘                       │  │
│   │                                            │                                    │  │
│   │                                            ▼                                    │  │
│   │                                   More Participants                             │  │
│   │                                            │                                    │  │
│   │                                            └────────▶ Network Growth            │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Comparison to Other Assets

| Property | MBO | Inflationary Tokens | Fiat Currency |
|----------|-----|---------------------|---------------|
| **Supply Cap** | 31,536,000 (fixed) | Unlimited | Unlimited |
| **Inflation** | 0% (deflationary) | Varies (1-10%+) | ~2-10%/year |
| **Utility** | Required for all ops | Often optional | Medium of exchange |
| **Scarcity** | Mathematically guaranteed | Not guaranteed | Not guaranteed |
| **Value Backing** | Network utility + security | Varies | Government decree |

---

## 6. Summary

### 6.1 Key Points

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBO UTILITY & VALUE SUMMARY                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ MBO IS REQUIRED FOR EVERY ACTION ON-CHAIN                                  │  │
│   │                                                                                 │  │
│   │     • No transaction without MBO gas                                           │  │
│   │     • No staking without MBO stake                                             │  │
│   │     • No compute without MBO payment                                           │  │
│   │     • No governance without staked MBO                                         │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ NO INFLATION ENSURES STRONG VALUE RETENTION                                │  │
│   │                                                                                 │  │
│   │     • Total supply capped at 31,536,000 MBO                                    │  │
│   │     • No mechanism to create new tokens                                        │  │
│   │     • Existing holders never diluted                                           │  │
│   │     • Predictable long-term economics                                          │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ BOTH PoS AND PoUW DEPEND EXPLICITLY ON MBO                                 │  │
│   │                                                                                 │  │
│   │     • PoS: Validators stake MBO for security (50% of rewards)                  │  │
│   │     • PoUW: Providers earn MBO for compute (50% of rewards)                    │  │
│   │     • Hybrid model requires MBO for both security layers                       │  │
│   │     • 50/50 split ensures balanced incentives                                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ FEES BURNED REDUCE CIRCULATING SUPPLY                                      │  │
│   │                                                                                 │  │
│   │     • 100% of base fees permanently destroyed                                  │  │
│   │     • Slashed stake also burned                                                │  │
│   │     • High network usage = more burns                                          │  │
│   │     • Long-term deflationary trajectory                                        │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ COMPUTE DEMAND INCREASES UTILITY                                           │  │
│   │                                                                                 │  │
│   │     • AI/ML workloads growing exponentially                                    │  │
│   │     • GPU providers need MBO for operations                                    │  │
│   │     • Compute marketplace paid in MBO                                          │  │
│   │     • Network becomes more valuable with more compute                          │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ GOVERNANCE RELIES EXCLUSIVELY ON STAKED MBO                                │  │
│   │                                                                                 │  │
│   │     • Voting power from staked tokens                                          │  │
│   │     • No new tokens minted through governance                                  │  │
│   │     • Long-term stake = long-term governance influence                         │  │
│   │     • Aligns governance with network success                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MBO UTILITY QUICK REFERENCE                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   CORE UTILITIES                                                                        │
│   ──────────────                                                                        │
│   Gas Fees:        Required for all transactions (base burned, priority routed)        │
│   Staking:         50,000 MBO min (validators), 100 MBO min (delegators)               │
│   Compute:         Payment for GPU workloads (50% of block rewards)                    │
│   Governance:      Voting power from staked MBO                                        │
│   Security:        Slashing penalties denominated in MBO (burned)                      │
│                                                                                         │
│   VALUE DRIVERS                                                                         │
│   ─────────────                                                                         │
│   Supply:          31,536,000 MBO (fixed forever)                                      │
│   Inflation:       0% (deflationary via burns)                                         │
│   Demand:          Growing (compute, staking, fees)                                    │
│   Security:        Economic guarantees require MBO                                     │
│   Alignment:       All participants benefit from MBO value                             │
│                                                                                         │
│   ECOSYSTEM                                                                             │
│   ─────────                                                                             │
│   Grants:          15% allocation (milestone-based)                                    │
│   Community:       10% allocation (per-epoch stream)                                   │
│   Developer:       Hackathons, bounties, retroactive funding                           │
│                                                                                         │
│   MARKETPLACE                                                                           │
│   ───────────                                                                           │
│   Compute:         Active (AI/ML, rendering, ZK)                                       │
│   Storage:         Future extension                                                    │
│   Bridges:         Future extension                                                    │
│   Smart Contracts: Future extension                                                    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [incentive_design.md](./incentive_design.md) | Incentive mechanisms |
| [fee_model.md](./fee_model.md) | Fee structure |
| [staking_model.md](./staking_model.md) | Staking specification |
| [governance_model.md](./governance_model.md) | Governance rules |
| [economic_security.md](./economic_security.md) | Security model |

---

*This document formalizes the utility and value proposition of MBO. All utilities are enforced by protocol rules and verifiable on-chain.*

