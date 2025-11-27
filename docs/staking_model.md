<!-- Verified against tokenomics.md -->
# Mbongo Chain — Staking Model

> **Document Type:** Staking Specification  
> **Last Updated:** November 2025  
> **Status:** Official Reference

---

## Table of Contents

1. [Purpose of This Document](#1-purpose-of-this-document)
2. [Role Definitions](#2-role-definitions)
3. [Staking Requirements](#3-staking-requirements)
4. [Validator Lifecycle](#4-validator-lifecycle)
5. [Reward Model (PoS Portion)](#5-reward-model-pos-portion)
6. [Unbonding & Withdrawal Rules](#6-unbonding--withdrawal-rules)
7. [Slashing Model](#7-slashing-model)
8. [Security Invariants](#8-security-invariants)
9. [Future Extensions](#9-future-extensions)

---

## 1. Purpose of This Document

This document defines the staking model for Mbongo Chain's Proof-of-Stake (PoS) consensus mechanism. Staking is fundamental to network security, validator selection, and economic alignment.

### 1.1 Role of Staking in Mbongo Chain

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         STAKING IN MBONGO CHAIN                                         │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SECURITY BACKBONE FOR PoS                                                             │
│   ─────────────────────────                                                             │
│   Staking provides the economic security layer for the PoS half of Mbongo's            │
│   hybrid consensus. Validators lock MBO tokens as collateral, creating                 │
│   economic skin-in-the-game that makes attacks prohibitively expensive.                │
│                                                                                         │
│   In Mbongo's 50/50 PoS + PoUW model:                                                  │
│   • PoS secures block production and finality                                          │
│   • PoUW secures compute verification and useful work                                  │
│   • Together, they provide defense-in-depth security                                   │
│                                                                                         │
│                                                                                         │
│   VALIDATOR SELECTION                                                                   │
│   ───────────────────                                                                   │
│   Stake weight determines:                                                             │
│   • Probability of being selected as block proposer                                    │
│   • Voting power in consensus                                                          │
│   • Share of rewards                                                                   │
│                                                                                         │
│   Higher stake = higher responsibility = higher rewards (and higher risk)              │
│                                                                                         │
│                                                                                         │
│   ECONOMIC INCENTIVES                                                                   │
│   ────────────────────                                                                  │
│   Staking aligns validator incentives with network health:                             │
│   • Honest behavior → rewards                                                          │
│   • Malicious behavior → slashing                                                      │
│   • Network success → token appreciation                                               │
│                                                                                         │
│   This creates a positive-sum game where validators profit by securing the network.    │
│                                                                                         │
│                                                                                         │
│   INTEGRITY GUARANTEES                                                                  │
│   ─────────────────────                                                                 │
│   Staking provides:                                                                    │
│   • Sybil resistance (stake required to participate)                                   │
│   • Economic finality (attack cost > potential gain)                                   │
│   • Accountability (slashing for misbehavior)                                          │
│   • Decentralization (many validators can participate)                                 │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Document Scope

| Topic | Coverage |
|-------|----------|
| **Validator Operations** | Requirements, lifecycle, responsibilities |
| **Delegator Participation** | How to delegate, rewards, risks |
| **Reward Distribution** | PoS reward allocation formula |
| **Slashing** | Conditions, penalties, enforcement |
| **Unbonding** | Withdrawal process and timeline |
| **Security** | Invariants and guarantees |

---

## 2. Role Definitions

### 2.1 Validators

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATORS                                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEFINITION                                                                            │
│   ──────────                                                                            │
│   Validators are network participants who stake MBO tokens and run validator           │
│   node software to propose blocks, validate transactions, and participate              │
│   in consensus.                                                                        │
│                                                                                         │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   BLOCK PRODUCTION                                                              │  │
│   │   • Propose new blocks when selected                                            │  │
│   │   • Include valid transactions from mempool                                     │  │
│   │   • Attach PoUW compute receipts                                                │  │
│   │   • Sign blocks with validator key                                              │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   CONSENSUS PARTICIPATION                                                       │  │
│   │   • Vote on proposed blocks (PREVOTE, PRECOMMIT)                                │  │
│   │   • Participate in finality rounds                                              │  │
│   │   • Maintain network connectivity                                               │  │
│   │   • Respond to consensus messages promptly                                      │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   INFRASTRUCTURE MAINTENANCE                                                    │  │
│   │   • Run 24/7 node infrastructure                                                │  │
│   │   • Maintain high uptime (>99%)                                                 │  │
│   │   • Keep software updated                                                       │  │
│   │   • Secure validator keys                                                       │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   DELEGATOR MANAGEMENT                                                          │  │
│   │   • Set commission rate                                                         │  │
│   │   • Communicate with delegators                                                 │  │
│   │   • Distribute rewards fairly                                                   │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   REQUIREMENTS                                                                          │
│   ────────────                                                                          │
│   • Minimum stake: 50,000 MBO                                                          │
│   • Technical infrastructure (see hardware requirements)                               │
│   • Network connectivity (stable, low-latency)                                         │
│   • Operational expertise                                                              │
│   • Security best practices                                                            │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Delegators

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DELEGATORS                                                      │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DEFINITION                                                                            │
│   ──────────                                                                            │
│   Delegators are MBO token holders who stake their tokens with validators              │
│   without running infrastructure themselves. They contribute to network                │
│   security while earning passive rewards.                                              │
│                                                                                         │
│                                                                                         │
│   RESPONSIBILITIES                                                                      │
│   ────────────────                                                                      │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   VALIDATOR SELECTION                                                           │  │
│   │   • Research validator performance and reputation                               │  │
│   │   • Choose reliable validators to delegate to                                   │  │
│   │   • Diversify across multiple validators (recommended)                          │  │
│   │   • Monitor validator status                                                    │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   STAKE MANAGEMENT                                                              │  │
│   │   • Deposit MBO to delegation contract                                          │  │
│   │   • Re-delegate if validator underperforms                                      │  │
│   │   • Initiate unbonding when needed                                              │  │
│   │   • Claim accumulated rewards                                                   │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   GOVERNANCE PARTICIPATION (Optional)                                           │  │
│   │   • Vote on governance proposals                                                │  │
│   │   • Override validator vote if desired                                          │  │
│   │   • Participate in community discussions                                        │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│                                                                                         │
│   REWARD PARTICIPATION                                                                  │
│   ────────────────────                                                                  │
│   • Earn share of validator's PoS rewards                                              │
│   • Rewards proportional to delegated amount                                           │
│   • Subject to validator's commission rate                                             │
│   • Rewards accumulate per block                                                       │
│                                                                                         │
│                                                                                         │
│   REQUIREMENTS                                                                          │
│   ────────────                                                                          │
│   • Minimum delegation: 100 MBO                                                        │
│   • MBO tokens to delegate                                                             │
│   • Wallet for managing delegation                                                     │
│                                                                                         │
│                                                                                         │
│   RISKS                                                                                 │
│   ─────                                                                                 │
│   • Slashing: If validator misbehaves, delegated stake is slashed                      │
│   • Opportunity cost: Tokens locked during delegation                                  │
│   • Unbonding period: 21 days to withdraw                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Governance Note

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         GOVERNANCE & LONG-TERM STABILITY                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VALIDATOR GOVERNANCE POWER                                                            │
│   ──────────────────────────                                                            │
│   Validators participate in governance with stake-weighted voting power.               │
│   Their responsibilities include:                                                      │
│   • Voting on protocol upgrades                                                        │
│   • Reviewing Tier 1 (critical) proposals                                              │
│   • Signaling upgrade readiness                                                        │
│                                                                                         │
│   DELEGATOR GOVERNANCE RIGHTS                                                           │
│   ───────────────────────────                                                           │
│   Delegators can:                                                                      │
│   • Vote independently (overriding validator)                                          │
│   • Abstain (inherit validator's vote)                                                 │
│   • Participate in all governance tiers                                                │
│                                                                                         │
│   LONG-TERM STABILITY                                                                   │
│   ───────────────────                                                                   │
│   The staking model contributes to stability through:                                  │
│   • Economic alignment (stake at risk)                                                 │
│   • Decentralization (many validators)                                                 │
│   • Predictable economics (fixed supply, known rewards)                                │
│   • Clear rules (deterministic slashing)                                               │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Staking Requirements

### 3.1 Canonical Staking Parameters

```
╔═════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                         ║
║                           STAKING REQUIREMENTS                                          ║
║                                                                                         ║
╠═════════════════════════════════════════════════════════════════════════════════════════╣
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   MINIMUM VALIDATOR STAKE                                                       │  ║
║   │   ═══════════════════════                                                       │  ║
║   │                                                                                 │  ║
║   │                        50,000 MBO                                               │  ║
║   │                                                                                 │  ║
║   │   • Required to register as a validator                                        │  ║
║   │   • Must be maintained at all times                                            │  ║
║   │   • Falling below triggers forced exit                                         │  ║
║   │   • Self-stake counts toward this minimum                                      │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   MINIMUM DELEGATOR STAKE                                                       │  ║
║   │   ═══════════════════════                                                       │  ║
║   │                                                                                 │  ║
║   │                          100 MBO                                                │  ║
║   │                                                                                 │  ║
║   │   • Minimum per delegation transaction                                         │  ║
║   │   • Can delegate to multiple validators                                        │  ║
║   │   • Each delegation must meet minimum                                          │  ║
║   │   • Prevents dust delegations                                                  │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   NO DYNAMIC/ADJUSTABLE THRESHOLDS                                              │  ║
║   │   ════════════════════════════════                                              │  ║
║   │                                                                                 │  ║
║   │   • Thresholds are fixed in protocol                                           │  ║
║   │   • Cannot be changed by governance                                            │  ║
║   │   • No automatic adjustment based on market conditions                         │  ║
║   │   • Provides predictable entry requirements                                    │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
║   ┌─────────────────────────────────────────────────────────────────────────────────┐  ║
║   │                                                                                 │  ║
║   │   STAKE CANNOT BE REHYPOTHECATED                                                │  ║
║   │   ══════════════════════════════                                                │  ║
║   │                                                                                 │  ║
║   │   • Staked MBO is locked in protocol                                           │  ║
║   │   • Cannot be used as collateral elsewhere                                     │  ║
║   │   • Cannot be lent or borrowed against                                         │  ║
║   │   • No liquid staking derivatives at protocol level                            │  ║
║   │   • Ensures security model integrity                                           │  ║
║   │                                                                                 │  ║
║   └─────────────────────────────────────────────────────────────────────────────────┘  ║
║                                                                                         ║
╚═════════════════════════════════════════════════════════════════════════════════════════╝
```

### 3.2 Requirements Summary Table

| Parameter | Value | Modifiable | Notes |
|-----------|-------|------------|-------|
| **Validator Minimum** | 50,000 MBO | No | Fixed in protocol |
| **Delegator Minimum** | 100 MBO | No | Per delegation |
| **Maximum Validators** | No hard cap | N/A | Network scales |
| **Maximum Delegation** | No limit | N/A | Per validator |
| **Commission Range** | 0% - 100% | Validator-set | Capped at 100% |
| **Unbonding Period** | 21 days | No | Fixed in protocol |
| **Rehypothecation** | Prohibited | No | Security requirement |

### 3.3 Hardware Requirements (Validators)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATOR HARDWARE REQUIREMENTS                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   MINIMUM SPECIFICATIONS                                                                │
│   ──────────────────────                                                                │
│   • CPU: 8 cores / 16 threads (modern x86_64)                                          │
│   • RAM: 32 GB                                                                         │
│   • Storage: 1 TB NVMe SSD                                                             │
│   • Network: 1 Gbps dedicated                                                          │
│   • Uptime: 99%+ availability                                                          │
│                                                                                         │
│   RECOMMENDED SPECIFICATIONS                                                            │
│   ──────────────────────────                                                            │
│   • CPU: 16 cores / 32 threads                                                         │
│   • RAM: 64 GB                                                                         │
│   • Storage: 2 TB NVMe SSD (RAID recommended)                                          │
│   • Network: 10 Gbps dedicated                                                         │
│   • Uptime: 99.9%+ availability                                                        │
│   • Redundant infrastructure                                                           │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Validator Lifecycle

### 4.1 Six-Step Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATOR LIFECYCLE                                             │
└─────────────────────────────────────────────────────────────────────────────────────────┘

  ════════════════════════════════════════════════════════════════════════════════════════
  STEP 1: REGISTER
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   REGISTRATION PROCESS                                                               │
  │   ────────────────────                                                               │
  │                                                                                      │
  │   1. Generate validator keypair (Ed25519)                                            │
  │   2. Set up node infrastructure                                                      │
  │   3. Submit registration transaction:                                                │
  │      • Validator public key                                                          │
  │      • Node endpoint (IP/DNS)                                                        │
  │      • Commission rate                                                               │
  │      • Validator metadata (name, website, etc.)                                      │
  │   4. Wait for registration confirmation                                              │
  │                                                                                      │
  │   Status: REGISTERED (not yet active)                                                │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STEP 2: STAKE
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   STAKING PROCESS                                                                    │
  │   ───────────────                                                                    │
  │                                                                                      │
  │   1. Transfer ≥50,000 MBO to staking contract                                        │
  │   2. Bond stake to validator identity                                                │
  │   3. Stake becomes active after bonding period (1 epoch)                             │
  │   4. Validator enters active set                                                     │
  │                                                                                      │
  │   ┌────────────────────────────────────────────────────────────────────────────────┐ │
  │   │   Self-Stake Transaction                                                       │ │
  │   │                                                                                │ │
  │   │   From:    validator_wallet                                                    │ │
  │   │   To:      staking_contract                                                    │ │
  │   │   Amount:  50,000 MBO (minimum)                                                │ │
  │   │   Type:    STAKE_BOND                                                          │ │
  │   └────────────────────────────────────────────────────────────────────────────────┘ │
  │                                                                                      │
  │   Status: BONDING → ACTIVE                                                           │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STEP 3: PARTICIPATE (Propose/Validate)
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   ACTIVE PARTICIPATION                                                               │
  │   ────────────────────                                                               │
  │                                                                                      │
  │   Block Proposal (when selected):                                                    │
  │   ┌────────────────────────────────────────────────────────────────────────────────┐ │
  │   │   1. Collect pending transactions from mempool                                 │ │
  │   │   2. Verify PoUW compute receipts                                              │ │
  │   │   3. Build block with transactions + receipts                                  │ │
  │   │   4. Sign block with validator key                                             │ │
  │   │   5. Broadcast proposal to network                                             │ │
  │   └────────────────────────────────────────────────────────────────────────────────┘ │
  │                                                                                      │
  │   Block Validation (always):                                                         │
  │   ┌────────────────────────────────────────────────────────────────────────────────┐ │
  │   │   1. Receive block proposals                                                   │ │
  │   │   2. Validate transactions and receipts                                        │ │
  │   │   3. Vote PREVOTE if valid                                                     │ │
  │   │   4. Vote PRECOMMIT if 2/3+ PREVOTE                                            │ │
  │   │   5. Finalize block if 2/3+ PRECOMMIT                                          │ │
  │   └────────────────────────────────────────────────────────────────────────────────┘ │
  │                                                                                      │
  │   Status: ACTIVE                                                                     │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STEP 4: GET REWARDED
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌──────────────────────────────────────────────────────────────────────────────────────┐
  │                                                                                      │
  │   REWARD ACCUMULATION                                                                │
  │   ───────────────────                                                                │
  │                                                                                      │
  │   Per-Block Rewards:                                                                 │
  │   • Proposer bonus (if selected and produced valid block)                            │
  │   • Attestation rewards (for voting correctly)                                       │
  │   • Share of PoS pool proportional to stake                                          │
  │                                                                                      │
  │   ┌────────────────────────────────────────────────────────────────────────────────┐ │
  │   │   Reward Calculation (per block):                                              │ │
  │   │                                                                                │ │
  │   │   validator_reward = PoS_Pool × (stake / total_stake) × performance_score     │ │
  │   │                                                                                │ │
  │   │   Where:                                                                       │ │
  │   │   • PoS_Pool = 0.05 MBO (50% of 0.1 MBO block reward)                          │ │
  │   │   • performance_score = uptime × correctness                                   │ │
  │   └────────────────────────────────────────────────────────────────────────────────┘ │
  │                                                                                      │
  │   Reward Distribution:                                                               │
  │   • Rewards accumulate in validator's reward balance                                 │
  │   • Can be claimed at any time                                                       │
  │   • Delegator rewards distributed automatically                                      │
  │                                                                                      │
  │   Status: ACTIVE (earning)                                                           │
  │                                                                                      │
  └──────────────────────────────────────────────────────────────────────────────────────┘
                                         │
                         ┌───────────────┴───────────────┐
                         │                               │
                         ▼                               ▼
  ════════════════════════════════════════════════════════════════════════════════════════
  STEP 5: GET SLASHED (If Malicious)              STEP 6: EXIT/UNBOND (Voluntary)
  ════════════════════════════════════════════════════════════════════════════════════════

  ┌─────────────────────────────────────┐    ┌─────────────────────────────────────┐
  │                                     │    │                                     │
  │   SLASHING (Involuntary)            │    │   VOLUNTARY EXIT                    │
  │   ──────────────────────            │    │   ──────────────                    │
  │                                     │    │                                     │
  │   Triggers:                         │    │   Process:                          │
  │   • Double signing                  │    │   1. Submit unbond transaction      │
  │   • Extended downtime               │    │   2. Enter UNBONDING status         │
  │   • Invalid block production        │    │   3. Wait 21-day unbonding period   │
  │                                     │    │   4. Stake becomes withdrawable     │
  │   Consequences:                     │    │   5. Withdraw to wallet             │
  │   • Stake percentage burned         │    │                                     │
  │   • Validator jailed                │    │   During unbonding:                 │
  │   • Reputation damaged              │    │   • No rewards earned               │
  │   • May be permanently banned       │    │   • Still subject to slashing       │
  │                                     │    │   • Cannot cancel unbonding         │
  │   Status: SLASHED → JAILED          │    │                                     │
  │                                     │    │   Status: UNBONDING → EXITED        │
  │                                     │    │                                     │
  └─────────────────────────────────────┘    └─────────────────────────────────────┘
```

### 4.2 Lifecycle State Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         VALIDATOR STATE TRANSITIONS                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│                                                                                         │
│   ┌──────────────┐         ┌──────────────┐         ┌──────────────┐                   │
│   │              │  stake  │              │  epoch  │              │                   │
│   │  REGISTERED  │────────▶│   BONDING    │────────▶│    ACTIVE    │◀──┐              │
│   │              │         │              │         │              │   │              │
│   └──────────────┘         └──────────────┘         └──────┬───────┘   │              │
│                                                            │           │              │
│                                                            │           │              │
│                            ┌───────────────────────────────┼───────────┘              │
│                            │                               │                          │
│                            │  unjail                       │  unbond                  │
│                            │  (if allowed)                 │                          │
│                            │                               ▼                          │
│                   ┌────────┴─────┐                ┌──────────────┐                    │
│                   │              │                │              │                    │
│                   │    JAILED    │                │  UNBONDING   │                    │
│                   │              │                │  (21 days)   │                    │
│                   └──────────────┘                └──────┬───────┘                    │
│                            ▲                             │                            │
│                            │  slash                      │  complete                  │
│                            │                             ▼                            │
│                            │                    ┌──────────────┐                      │
│                            │                    │              │                      │
│                            └────────────────────│    EXITED    │                      │
│                              (severe slash)     │              │                      │
│                                                 └──────────────┘                      │
│                                                                                         │
│   Legend:                                                                               │
│   ────────                                                                              │
│   REGISTERED: Validator identity created, no stake                                     │
│   BONDING:    Stake deposited, waiting for activation                                  │
│   ACTIVE:     Participating in consensus, earning rewards                              │
│   JAILED:     Slashed, temporarily or permanently excluded                             │
│   UNBONDING:  Voluntary exit initiated, 21-day wait                                    │
│   EXITED:     Stake withdrawn, no longer a validator                                   │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Reward Model (PoS Portion)

### 5.1 PoS Reward Allocation

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         PoS REWARD MODEL                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   BLOCK REWARD SPLIT                                                                    │
│   ══════════════════                                                                    │
│                                                                                         │
│   Total Block Reward: 0.1 MBO (Year 1-5)                                               │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ┌─────────────────────────┐    ┌─────────────────────────┐                   │  │
│   │   │                         │    │                         │                   │  │
│   │   │       PoS POOL          │    │      PoUW POOL          │                   │  │
│   │   │         50%             │    │         50%             │                   │  │
│   │   │                         │    │                         │                   │  │
│   │   │      0.05 MBO           │    │      0.05 MBO           │                   │  │
│   │   │                         │    │                         │                   │  │
│   │   └───────────┬─────────────┘    └─────────────────────────┘                   │  │
│   │               │                                                                 │  │
│   │               │                                                                 │  │
│   │               ▼                                                                 │  │
│   │   ┌─────────────────────────────────────────────────────────┐                  │  │
│   │   │              PoS INTERNAL DISTRIBUTION                  │                  │  │
│   │   ├─────────────────────────────────────────────────────────┤                  │  │
│   │   │                                                         │                  │  │
│   │   │   ┌───────────────────┐    ┌───────────────────┐       │                  │  │
│   │   │   │                   │    │                   │       │                  │  │
│   │   │   │   VALIDATORS      │    │   DELEGATORS      │       │                  │  │
│   │   │   │      80%          │    │      20%          │       │                  │  │
│   │   │   │                   │    │                   │       │                  │  │
│   │   │   │   0.04 MBO        │    │   0.01 MBO        │       │                  │  │
│   │   │   │                   │    │                   │       │                  │  │
│   │   │   └───────────────────┘    └───────────────────┘       │                  │  │
│   │   │                                                         │                  │  │
│   │   └─────────────────────────────────────────────────────────┘                  │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Reward Distribution Formula

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         REWARD FORMULAS                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   VALIDATOR REWARD (per block)                                                          │
│   ────────────────────────────                                                          │
│                                                                                         │
│   validator_reward(v) = PoS_Validator_Pool × stake_weight(v) × performance(v)          │
│                                                                                         │
│   Where:                                                                                │
│   • PoS_Validator_Pool = Block_Reward × 0.50 × 0.80 = 0.04 MBO                         │
│   • stake_weight(v) = (self_stake(v) + delegations(v)) / total_staked                  │
│   • performance(v) = uptime_score × correctness_score                                  │
│                                                                                         │
│                                                                                         │
│   DELEGATOR REWARD (per block)                                                          │
│   ────────────────────────────                                                          │
│                                                                                         │
│   delegator_reward(d) = validator_delegator_pool(v) × delegation_weight(d)             │
│                         × (1 - commission_rate(v))                                     │
│                                                                                         │
│   Where:                                                                                │
│   • validator_delegator_pool(v) = PoS_Delegator_Pool × stake_weight(v)                 │
│   • PoS_Delegator_Pool = Block_Reward × 0.50 × 0.20 = 0.01 MBO                         │
│   • delegation_weight(d) = delegation(d) / total_delegations(v)                        │
│   • commission_rate(v) = validator-set rate (0-100%)                                   │
│                                                                                         │
│                                                                                         │
│   FULLY DETERMINISTIC                                                                   │
│   ───────────────────                                                                   │
│   • All calculations on-chain                                                          │
│   • No off-chain distribution                                                          │
│   • Rewards verifiable by anyone                                                       │
│   • Same result on every node                                                          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 5.3 Reward Table

| Component | Percentage | Amount (Year 1) | Recipient | Distribution |
|-----------|------------|-----------------|-----------|--------------|
| **Total Block Reward** | 100% | 0.1 MBO | - | Per block |
| **PoS Pool** | 50% | 0.05 MBO | Validators + Delegators | Stake-weighted |
| **→ Validators** | 80% of PoS | 0.04 MBO | Active validators | Performance-weighted |
| **→ Delegators** | 20% of PoS | 0.01 MBO | All delegators | Delegation-weighted |
| **PoUW Pool** | 50% | 0.05 MBO | Compute providers | Work-weighted |

### 5.4 Annual Reward Projections (Year 1)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         ANNUAL PoS REWARDS (YEAR 1)                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Blocks per year: 31,536,000                                                          │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   Category              │  Per Block  │  Daily        │  Annual                 │  │
│   ├─────────────────────────┼─────────────┼───────────────┼─────────────────────────┤  │
│   │   Total PoS Pool        │  0.05 MBO   │  4,320 MBO    │  1,576,800 MBO          │  │
│   │   Validator Share       │  0.04 MBO   │  3,456 MBO    │  1,261,440 MBO          │  │
│   │   Delegator Share       │  0.01 MBO   │    864 MBO    │    315,360 MBO          │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   Example: Validator with 5% of total stake                                            │
│   ──────────────────────────────────────────                                            │
│   • Annual validator rewards: 1,261,440 × 0.05 = 63,072 MBO                            │
│   • Plus commission on delegator rewards                                               │
│   • Plus proposer bonuses                                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Unbonding & Withdrawal Rules

### 6.1 Unbonding Process

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         UNBONDING RULES                                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   UNBONDING PERIOD: 21 DAYS                                                             │
│   ═════════════════════════                                                             │
│                                                                                         │
│   The unbonding period is a mandatory waiting time between initiating                  │
│   withdrawal and receiving funds. This protects the network by:                        │
│   • Preventing rapid stake removal during attacks                                      │
│   • Allowing time to detect and slash misbehavior                                      │
│   • Maintaining security budget stability                                              │
│                                                                                         │
│                                                                                         │
│   KEY RULES                                                                             │
│   ─────────                                                                             │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ DURING UNBONDING: NO REWARDS                                               │  │
│   │   ──────────────────────────────                                                │  │
│   │   • Stake no longer earns rewards                                              │  │
│   │   • Not counted in active validator set                                        │  │
│   │   • Does not participate in consensus                                          │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ NO FAST WITHDRAWALS                                                        │  │
│   │   ─────────────────────                                                         │  │
│   │   • No mechanism to skip unbonding period                                      │  │
│   │   • No "emergency withdrawal" option                                           │  │
│   │   • No governance override                                                     │  │
│   │   • 21 days is absolute minimum                                                │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ WITHDRAWALS ONLY AFTER COMPLETION                                          │  │
│   │   ───────────────────────────────────                                           │  │
│   │   • Stake locked until unbonding completes                                     │  │
│   │   • Withdrawal transaction only valid after 21 days                            │  │
│   │   • Partial unbonding allowed (multiple queues)                                │  │
│   │   • Each unbonding request has its own timer                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   ✓ STILL SUBJECT TO SLASHING                                                  │  │
│   │   ───────────────────────────                                                   │  │
│   │   • Unbonding stake can still be slashed                                       │  │
│   │   • Evidence from before unbonding applies                                     │  │
│   │   • Protects against "slash and run" attacks                                   │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.2 Unbonding Timeline

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         UNBONDING TIMELINE                                              │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   Day 0              Day 7              Day 14             Day 21                       │
│     │                  │                  │                  │                          │
│     ▼                  ▼                  ▼                  ▼                          │
│   ┌─────┬─────────────────────────────────────────────────────┬─────┐                  │
│   │ TX  │░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│ WD  │                  │
│   └─────┴─────────────────────────────────────────────────────┴─────┘                  │
│     │                                                           │                       │
│     │                    UNBONDING PERIOD                       │                       │
│     │                       (21 DAYS)                           │                       │
│     │                                                           │                       │
│   Unbond                                                     Withdraw                   │
│   Request                                                    Available                  │
│   Submitted                                                                             │
│                                                                                         │
│                                                                                         │
│   TIMELINE DETAILS                                                                      │
│   ────────────────                                                                      │
│                                                                                         │
│   ┌──────────────────────────────────────────────────────────────────────────────────┐ │
│   │   Phase           │  Duration  │  Status           │  Actions                    │ │
│   ├───────────────────┼────────────┼───────────────────┼─────────────────────────────┤ │
│   │   Active Staking  │  Ongoing   │  ACTIVE           │  Earning rewards            │ │
│   │   Unbond Request  │  Instant   │  TX submitted     │  Stake enters unbonding     │ │
│   │   Unbonding       │  21 days   │  UNBONDING        │  No rewards, still slashable│ │
│   │   Completion      │  Instant   │  UNBONDED         │  Stake unlocked             │ │
│   │   Withdrawal      │  Instant   │  WITHDRAWN        │  MBO in wallet              │ │
│   └──────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                         │
│                                                                                         │
│   MULTIPLE UNBONDING QUEUES                                                             │
│   ─────────────────────────                                                             │
│                                                                                         │
│   Validators/delegators can have multiple unbonding requests:                          │
│                                                                                         │
│   Request 1: 10,000 MBO   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░  Day 15/21                 │
│   Request 2: 5,000 MBO    ▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░  Day 7/21                  │
│   Request 3: 3,000 MBO    ▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  Day 2/21                  │
│                                                                                         │
│   Each completes independently.                                                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Delegator Unbonding

| Action | Duration | Rewards | Slashing Risk |
|--------|----------|---------|---------------|
| **Request Unbond** | Instant | Stops immediately | Yes |
| **Unbonding Period** | 21 days | None | Yes (if validator slashed) |
| **Completion** | Instant | N/A | No |
| **Re-delegation** | Instant | Continues | Different validator |

---

## 7. Slashing Model

### 7.1 Slashing Overview

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING MODEL                                                  │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PURPOSE                                                                               │
│   ───────                                                                               │
│   Slashing enforces honest behavior by destroying stake of misbehaving validators.     │
│   It creates economic consequences that make attacks irrational.                       │
│                                                                                         │
│   KEY PRINCIPLES                                                                        │
│   ──────────────                                                                        │
│   • All slashing is permanent (cannot be reversed)                                     │
│   • All slashing is on-chain (transparent and verifiable)                              │
│   • Slashed MBO is BURNED (not redistributed)                                          │
│   • Delegators share in slashing (risk of delegation)                                  │
│   • Evidence-based (cryptographic proof required)                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.2 Slashing Conditions

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING CONDITIONS                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   DOUBLE-SIGN SLASHING                                                          │  │
│   │   ════════════════════                                                          │  │
│   │                                                                                 │  │
│   │   Penalty: -5% of total stake                                                  │  │
│   │                                                                                 │  │
│   │   Trigger:                                                                      │  │
│   │   • Signing two different blocks at the same height                            │  │
│   │   • Signing conflicting PREVOTE or PRECOMMIT messages                          │  │
│   │   • Any equivocation attack                                                    │  │
│   │                                                                                 │  │
│   │   Evidence:                                                                     │  │
│   │   • Two valid signatures from same validator                                   │  │
│   │   • On conflicting data at same height/round                                   │  │
│   │   • Submitted by any network participant                                       │  │
│   │                                                                                 │  │
│   │   Additional Consequences:                                                      │  │
│   │   • Validator jailed for 30 days                                               │  │
│   │   • Cannot unjail during this period                                           │  │
│   │   • Reputation permanently marked                                              │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   DOWNTIME SLASHING                                                             │  │
│   │   ═════════════════                                                             │  │
│   │                                                                                 │  │
│   │   Penalty: -0.5% of total stake                                                │  │
│   │                                                                                 │  │
│   │   Trigger:                                                                      │  │
│   │   • Missing >500 consecutive blocks (~8 hours)                                 │  │
│   │   • Failing to sign attestations                                               │  │
│   │   • Node offline or unreachable                                                │  │
│   │                                                                                 │  │
│   │   Evidence:                                                                     │  │
│   │   • Absence of validator signatures in block headers                           │  │
│   │   • Automatic detection by protocol                                            │  │
│   │                                                                                 │  │
│   │   Additional Consequences:                                                      │  │
│   │   • Validator jailed for 1 hour                                                │  │
│   │   • Can unjail after jail period                                               │  │
│   │   • Repeated offenses increase penalties                                       │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.3 Severity Scaling Rules

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SEVERITY SCALING                                                │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ESCALATION MODEL                                                                      │
│   ────────────────                                                                      │
│   Repeated offenses result in escalating penalties:                                    │
│                                                                                         │
│   Downtime Slashing:                                                                   │
│   • 1st offense:  0.5% slash,  1 hour jail                                             │
│   • 2nd offense:  1.0% slash,  6 hours jail                                            │
│   • 3rd offense:  2.0% slash, 24 hours jail                                            │
│   • 4th offense:  5.0% slash,  7 days jail                                             │
│   • 5th offense: 10.0% slash, permanent ban consideration                              │
│                                                                                         │
│   Double-Sign Slashing:                                                                │
│   • 1st offense:  5.0% slash, 30 days jail                                             │
│   • 2nd offense: 33.0% slash, permanent ban                                            │
│                                                                                         │
│   Offense history tracked per validator, does not reset.                               │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.4 Severity Ladder (ASCII)

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SLASHING SEVERITY LADDER                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   SEVERITY                                                                              │
│      ▲                                                                                  │
│      │                                                                                  │
│      │  ┌─────────────────────────────────────────────────────────────────────────┐    │
│      │  │                                                                         │    │
│      │  │   LEVEL 5: PERMANENT BAN                                                │    │
│      │  │   ──────────────────────                                                │    │
│      │  │   • Coordinated attack / Repeated double-signing                        │    │
│      │  │   • Penalty: 33%+ slash, permanent exclusion                            │    │
│      │  │   • Slashed MBO: BURNED                                                 │    │
│      │  │                                                                         │    │
│      │  └─────────────────────────────────────────────────────────────────────────┘    │
│      │                                                                                  │
│      │  ┌─────────────────────────────────────────────────────────────────────────┐    │
│      │  │                                                                         │    │
│      │  │   LEVEL 4: SEVERE SLASH                                                 │    │
│      │  │   ─────────────────────                                                 │    │
│      │  │   • Double-signing (equivocation)                                       │    │
│      │  │   • Penalty: 5% slash, 30-day jail                                      │    │
│      │  │   • Slashed MBO: BURNED                                                 │    │
│      │  │                                                                         │    │
│      │  └─────────────────────────────────────────────────────────────────────────┘    │
│      │                                                                                  │
│      │  ┌─────────────────────────────────────────────────────────────────────────┐    │
│      │  │                                                                         │    │
│      │  │   LEVEL 3: MODERATE SLASH                                               │    │
│      │  │   ───────────────────────                                               │    │
│      │  │   • Repeated downtime (3rd+ offense)                                    │    │
│      │  │   • Penalty: 2-5% slash, extended jail                                  │    │
│      │  │   • Slashed MBO: BURNED                                                 │    │
│      │  │                                                                         │    │
│      │  └─────────────────────────────────────────────────────────────────────────┘    │
│      │                                                                                  │
│      │  ┌─────────────────────────────────────────────────────────────────────────┐    │
│      │  │                                                                         │    │
│      │  │   LEVEL 2: MINOR SLASH                                                  │    │
│      │  │   ────────────────────                                                  │    │
│      │  │   • Extended downtime (>8 hours)                                        │    │
│      │  │   • Penalty: 0.5-1% slash, short jail                                   │    │
│      │  │   • Slashed MBO: BURNED                                                 │    │
│      │  │                                                                         │    │
│      │  └─────────────────────────────────────────────────────────────────────────┘    │
│      │                                                                                  │
│      │  ┌─────────────────────────────────────────────────────────────────────────┐    │
│      │  │                                                                         │    │
│      │  │   LEVEL 1: WARNING                                                      │    │
│      │  │   ────────────────                                                      │    │
│      │  │   • Brief downtime (<500 blocks)                                        │    │
│      │  │   • Penalty: Reward withholding only                                    │    │
│      │  │   • No stake slashed                                                    │    │
│      │  │                                                                         │    │
│      ▼  └─────────────────────────────────────────────────────────────────────────┘    │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 7.5 Slashing Summary Table

| Offense | Penalty | Jail Duration | Burn Destination | Escalation |
|---------|---------|---------------|------------------|------------|
| **Double-Sign (1st)** | -5% stake | 30 days | Burned | → 33% + ban |
| **Double-Sign (2nd)** | -33% stake | Permanent | Burned | N/A |
| **Downtime (1st)** | -0.5% stake | 1 hour | Burned | → 1% |
| **Downtime (2nd)** | -1.0% stake | 6 hours | Burned | → 2% |
| **Downtime (3rd)** | -2.0% stake | 24 hours | Burned | → 5% |
| **Downtime (4th+)** | -5%+ stake | 7+ days | Burned | → ban |

---

## 8. Security Invariants

### 8.1 Honest-Majority Requirement

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         SECURITY INVARIANTS                                             │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   HONEST-MAJORITY REQUIREMENT                                                           │
│   ═══════════════════════════                                                           │
│                                                                                         │
│   The PoS consensus assumes that >2/3 of staked tokens are controlled by               │
│   honest validators.                                                                   │
│                                                                                         │
│   Safety Threshold: 2/3+ honest stake                                                  │
│   ──────────────────────────────────                                                    │
│   • With 2/3+ honest: Consensus is safe and live                                       │
│   • With 1/3-2/3 honest: Safety guaranteed, liveness may degrade                       │
│   • With <1/3 honest: Safety cannot be guaranteed                                      │
│                                                                                         │
│   Why 2/3?                                                                              │
│   ────────                                                                              │
│   • BFT consensus requires 2f+1 honest nodes out of 3f+1 total                        │
│   • f = maximum Byzantine (malicious) nodes tolerated                                  │
│   • For f failures, need 3f+1 total → 2/3+ must be honest                             │
│                                                                                         │
│   Economic Interpretation:                                                              │
│   ────────────────────────                                                              │
│   An attacker needs >1/3 of total stake to disrupt consensus.                          │
│   At current prices, this represents significant economic cost.                        │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.2 Minimum Economic Thresholds

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         MINIMUM ECONOMIC THRESHOLDS                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ATTACK COST ANALYSIS                                                                  │
│   ────────────────────                                                                  │
│                                                                                         │
│   To attack Mbongo Chain consensus, an attacker would need:                            │
│                                                                                         │
│   1. ACQUIRE 1/3+ OF STAKED TOKENS                                                     │
│      • Must purchase or accumulate significant MBO                                     │
│      • Market impact would drive price up                                              │
│      • Cost increases as attack progresses                                             │
│                                                                                         │
│   2. STAKE THE TOKENS                                                                  │
│      • 50,000 MBO minimum per validator                                                │
│      • Or delegate to controlled validators                                            │
│      • Tokens locked during attack                                                     │
│                                                                                         │
│   3. RISK SLASHING                                                                     │
│      • Any detectable misbehavior → 5%+ slash                                          │
│      • Attack evidence is permanent                                                    │
│      • Cannot withdraw during unbonding                                                │
│                                                                                         │
│   MINIMUM SECURITY BUDGET                                                               │
│   ───────────────────────                                                               │
│   The network maintains minimum security when:                                         │
│   • Total staked value >> potential attack gains                                       │
│   • Slashing penalties exceed attack profits                                           │
│   • Token appreciation aligns validator interests                                      │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.3 Why Deterministic Slashing Ensures Safety

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DETERMINISTIC SLASHING                                          │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   PROPERTIES                                                                            │
│   ──────────                                                                            │
│                                                                                         │
│   1. EVIDENCE-BASED                                                                     │
│      • Slashing requires cryptographic proof                                           │
│      • Anyone can verify evidence validity                                             │
│      • False evidence is rejected                                                      │
│      • No subjective judgment required                                                 │
│                                                                                         │
│   2. AUTOMATIC EXECUTION                                                                │
│      • Valid evidence triggers immediate slashing                                      │
│      • No human decision in the loop                                                   │
│      • Cannot be blocked or delayed                                                    │
│      • Same result on every node                                                       │
│                                                                                         │
│   3. PREDICTABLE PENALTIES                                                              │
│      • Known penalty for each offense type                                             │
│      • Validators can calculate risk                                                   │
│      • No arbitrary punishment                                                         │
│      • Transparent enforcement                                                         │
│                                                                                         │
│   4. PERMANENT RECORD                                                                   │
│      • All slashing events on-chain                                                    │
│      • Cannot be erased or hidden                                                      │
│      • Full audit trail                                                                │
│      • Reputation is public                                                            │
│                                                                                         │
│   WHY THIS ENSURES SAFETY                                                               │
│   ───────────────────────                                                               │
│   • Attackers KNOW they will be caught and punished                                    │
│   • Punishment is certain, not probabilistic                                           │
│   • Cannot bribe or negotiate out of slashing                                          │
│   • Economic rationality prevents attacks                                              │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 8.4 Why Delegators Must Choose Carefully

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         DELEGATOR RESPONSIBILITY                                        │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   DELEGATORS SHARE SLASHING RISK                                                        │
│   ══════════════════════════════                                                        │
│                                                                                         │
│   When a validator is slashed, delegators lose proportionally:                         │
│                                                                                         │
│   Example:                                                                              │
│   ────────                                                                              │
│   • Validator has 100,000 MBO total stake                                              │
│   • Self-stake: 50,000 MBO                                                             │
│   • Delegated: 50,000 MBO (from 10 delegators)                                         │
│   • Validator double-signs → 5% slash                                                  │
│                                                                                         │
│   Slashing Distribution:                                                                │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │   Participant      │  Original Stake  │  Slashed (5%)  │  Remaining           │  │
│   ├────────────────────┼──────────────────┼────────────────┼──────────────────────┤  │
│   │   Validator        │   50,000 MBO     │   2,500 MBO    │   47,500 MBO         │  │
│   │   Delegator A      │   10,000 MBO     │     500 MBO    │    9,500 MBO         │  │
│   │   Delegator B      │    5,000 MBO     │     250 MBO    │    4,750 MBO         │  │
│   │   ...              │   ...            │   ...          │   ...                │  │
│   │   Total            │  100,000 MBO     │   5,000 MBO    │   95,000 MBO         │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   DELEGATOR DUE DILIGENCE                                                               │
│   ───────────────────────                                                               │
│   Before delegating, check:                                                            │
│   • Validator uptime history                                                           │
│   • Previous slashing events                                                           │
│   • Commission rate                                                                    │
│   • Self-stake ratio (skin in game)                                                    │
│   • Community reputation                                                               │
│   • Infrastructure quality                                                             │
│                                                                                         │
│   DIVERSIFICATION                                                                       │
│   ───────────────                                                                       │
│   Recommended: Delegate across 3-5 validators to reduce single-validator risk          │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Future Extensions

### 9.1 Planned Enhancements

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         FUTURE EXTENSIONS [PLACEHOLDER]                                 │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   WEIGHTED STAKE WINDOWS                                                        │  │
│   │   ══════════════════════                                                        │  │
│   │                                                                                 │  │
│   │   Current: Stake weight is immediate (snapshot-based)                          │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Time-weighted stake (longer stake = higher weight)                         │  │
│   │   • Reduces short-term stake manipulation                                      │  │
│   │   • Rewards long-term commitment                                               │  │
│   │   • Smooths validator set changes                                              │  │
│   │                                                                                 │  │
│   │   Timeline: Research phase, Year 2-3                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   MULTI-SHARD VALIDATOR SET                                                     │  │
│   │   ═════════════════════════                                                     │  │
│   │                                                                                 │  │
│   │   Current: Single validator set for entire chain                               │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Validators assigned to specific shards                                     │  │
│   │   • Rotation between shards                                                    │  │
│   │   • Cross-shard finality coordination                                          │  │
│   │   • Scales validator capacity with network growth                              │  │
│   │                                                                                 │  │
│   │   Timeline: Research phase, Year 3-4                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
│   ┌─────────────────────────────────────────────────────────────────────────────────┐  │
│   │                                                                                 │  │
│   │   LIGHT CLIENT STAKING                                                          │  │
│   │   ════════════════════                                                          │  │
│   │                                                                                 │  │
│   │   Current: Full node required for validation                                   │  │
│   │                                                                                 │  │
│   │   Future Enhancement:                                                           │  │
│   │   • Delegators can verify via light clients                                    │  │
│   │   • Reduced hardware requirements                                              │  │
│   │   • Mobile delegation support                                                  │  │
│   │   • ZK-proof based stake verification                                          │  │
│   │                                                                                 │  │
│   │   Timeline: Research phase, Year 2-3                                           │  │
│   │                                                                                 │  │
│   └─────────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Extension Roadmap

| Extension | Status | Target | Dependencies |
|-----------|--------|--------|--------------|
| **Weighted Stake Windows** | Research | Year 2-3 | Consensus upgrade |
| **Multi-Shard Validators** | Research | Year 3-4 | Sharding implementation |
| **Light Client Staking** | Research | Year 2-3 | Light client protocol |
| **Liquid Staking (L2)** | Community | TBD | External development |
| **Cross-Chain Staking** | Exploration | Year 4+ | Bridge infrastructure |

---

## Appendix: Quick Reference

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                         STAKING MODEL QUICK REFERENCE                                   │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│   STAKING REQUIREMENTS                                                                  │
│   ────────────────────                                                                  │
│   Validator Minimum:    50,000 MBO                                                     │
│   Delegator Minimum:    100 MBO                                                        │
│   Unbonding Period:     21 days                                                        │
│   Rehypothecation:      Prohibited                                                     │
│                                                                                         │
│   REWARD DISTRIBUTION                                                                   │
│   ────────────────────                                                                  │
│   Block Reward:         0.1 MBO (Year 1-5)                                             │
│   PoS Pool:             50% (0.05 MBO)                                                 │
│   → Validators:         80% of PoS (0.04 MBO)                                          │
│   → Delegators:         20% of PoS (0.01 MBO)                                          │
│                                                                                         │
│   SLASHING PENALTIES                                                                    │
│   ──────────────────                                                                    │
│   Double-Sign:          -5% stake, 30-day jail                                         │
│   Downtime:             -0.5% stake, 1-hour jail                                       │
│   Slashed MBO:          BURNED (not redistributed)                                     │
│                                                                                         │
│   VALIDATOR LIFECYCLE                                                                   │
│   ────────────────────                                                                  │
│   1. Register  →  2. Stake  →  3. Participate  →  4. Earn                              │
│                        ↓              ↓                                                │
│                   5. Slash (if bad)   6. Exit/Unbond                                   │
│                                                                                         │
│   SECURITY INVARIANTS                                                                   │
│   ───────────────────                                                                   │
│   • 2/3+ honest stake required                                                         │
│   • Deterministic slashing                                                             │
│   • Evidence-based enforcement                                                         │
│   • Delegators share slashing risk                                                     │
│                                                                                         │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Related Documentation

| Document | Description |
|----------|-------------|
| [tokenomics.md](../spec/tokenomics.md) | Canonical economic specification |
| [incentive_design.md](./incentive_design.md) | Incentive mechanisms |
| [reward_mechanics.md](./reward_mechanics.md) | Detailed reward calculations |
| [consensus_master_overview.md](./consensus_master_overview.md) | Consensus specification |
| [governance_model.md](./governance_model.md) | Governance rules |

---

*This document defines the official staking model for Mbongo Chain. All staking parameters are enforced by consensus rules.*

